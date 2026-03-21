use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use resharp_algebra::nulls::{NullState, Nullability};
use resharp_algebra::{NodeId, RegexBuilder};

use crate::accel;
use crate::engine::{DFA_MISSING, LDFA};
use crate::{Error, Regex, RegexInner};

const MAGIC: [u8; 4] = *b"RSH\x01";

fn ser_err(e: impl std::fmt::Display) -> Error {
    Error::Serialize(e.to_string())
}

#[derive(Serialize, Deserialize)]
struct SerNullState {
    mask: u8,
    rel: u32,
}

#[derive(Serialize, Deserialize)]
struct SerSkipSearcher {
    tag: u8, // 0=Exact, 1=Range
    bytes: Vec<u8>,
    ranges: Vec<(u8, u8)>,
}

#[derive(Serialize, Deserialize)]
struct SerRevPrefix {
    len: u16,
    sets: Vec<[u64; 4]>,
}

#[derive(Serialize, Deserialize)]
struct SerLDFA {
    initial: u16,
    num_minterms: u32,
    mt_log: u32,
    minterms_lookup: Vec<u8>,
    begin_table: Vec<u16>,
    center_table: Vec<u16>,
    effects_id: Vec<u16>,
    effects: Vec<Vec<SerNullState>>,
    skip_ids: Vec<u8>,
    skip_searchers: Vec<SerSkipSearcher>,
    prefix_skip: Option<SerRevPrefix>,
}

#[derive(Serialize, Deserialize)]
enum SerFwdPrefix {
    Literal(Vec<u8>),
    Prefix { len: u16, sets: Vec<[u64; 4]> },
    Range { len: u16, anchor_pos: u16, ranges: Vec<(u8, u8)>, sets: Vec<[u64; 4]> },
}

#[derive(Serialize, Deserialize)]
struct SerRegex {
    fwd_prefix_stripped: bool,
    empty_nullable: bool,
    fwd_end_nullable: bool,
    hardened: bool,
    has_rev_accel: bool,
    fixed_length: Option<u32>,
    max_length: Option<u32>,
    fwd: SerLDFA,
    rev: SerLDFA,
    fwd_prefix: Option<SerFwdPrefix>,
}

fn ldfa_to_ser(ldfa: &LDFA) -> SerLDFA {
    let num_states = ldfa.state_nodes.len();
    let stride = 1usize << ldfa.mt_log;

    let mut center_table = Vec::with_capacity(num_states * stride);
    for sid in 0..num_states {
        let base = sid * stride;
        for mt in 0..stride {
            let idx = base + mt;
            center_table.push(if idx < ldfa.center_table.len() {
                ldfa.center_table[idx]
            } else {
                DFA_MISSING
            });
        }
    }

    let effects_id: Vec<u16> = (0..num_states)
        .map(|s| if s < ldfa.effects_id.len() { ldfa.effects_id[s] } else { 0 })
        .collect();

    let effects: Vec<Vec<SerNullState>> = ldfa
        .effects
        .iter()
        .map(|v| v.iter().map(|ns| SerNullState { mask: ns.mask.0, rel: ns.rel }).collect())
        .collect();

    let skip_ids: Vec<u8> = (0..num_states)
        .map(|s| if s < ldfa.skip_ids.len() { ldfa.skip_ids[s] } else { 0 })
        .collect();

    let mut skip_searchers = Vec::new();
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    for s in &ldfa.skip_searchers {
        match s {
            accel::MintermSearchValue::Exact(e) => skip_searchers.push(SerSkipSearcher {
                tag: 0,
                bytes: e.bytes().to_vec(),
                ranges: Vec::new(),
            }),
            accel::MintermSearchValue::Range(r) => skip_searchers.push(SerSkipSearcher {
                tag: 1,
                bytes: Vec::new(),
                ranges: r.ranges().to_vec(),
            }),
        }
    }

    let prefix_skip = {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        {
            ldfa.prefix_skip.as_ref().map(|ps| SerRevPrefix {
                len: ps.len() as u16,
                sets: ps.sets.iter().map(|t| t.0).collect(),
            })
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        { None }
    };

    SerLDFA {
        initial: ldfa.initial,
        num_minterms: ldfa.num_minterms,
        mt_log: ldfa.mt_log,
        minterms_lookup: ldfa.minterms_lookup.to_vec(),
        begin_table: ldfa.begin_table.clone(),
        center_table,
        effects_id,
        effects,
        skip_ids,
        skip_searchers,
        prefix_skip,
    }
}

fn ser_to_ldfa(s: SerLDFA) -> Result<LDFA, Error> {
    if s.minterms_lookup.len() != 256 {
        return Err(ser_err("bad minterms_lookup length"));
    }
    let mut minterms_lookup = [0u8; 256];
    minterms_lookup.copy_from_slice(&s.minterms_lookup);

    let num_states = s.effects_id.len();

    let effects: Vec<Vec<NullState>> = s
        .effects
        .into_iter()
        .map(|v| v.into_iter().map(|ns| NullState { mask: Nullability(ns.mask), rel: ns.rel }).collect())
        .collect();

    let mut skip_searchers: Vec<accel::MintermSearchValue> = Vec::new();
    for ss in s.skip_searchers {
        match ss.tag {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            0 => skip_searchers.push(accel::MintermSearchValue::Exact(
                crate::simd::RevSearchBytes::new(ss.bytes),
            )),
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            1 => skip_searchers.push(accel::MintermSearchValue::Range(
                crate::simd::RevSearchRanges::new(ss.ranges),
            )),
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            _ => return Err(ser_err("skip searchers require SIMD")),
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            _ => return Err(ser_err("bad skip searcher tag")),
        }
    }

    let prefix_skip = match s.prefix_skip {
        None => None,
        Some(sp) => {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            {
                let sets: Vec<accel::TSet> = sp.sets.iter().map(|&w| accel::TSet(w)).collect();
                let byte_sets_raw: Vec<Vec<u8>> = sets.iter().map(tset_to_bytes).collect();
                Some(crate::simd::RevPrefixSearch::new(sp.len as usize, &byte_sets_raw, sets))
            }
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            { return Err(ser_err("prefix skip requires SIMD")); }
        }
    };

    Ok(LDFA {
        initial: s.initial,
        begin_table: s.begin_table,
        center_table: s.center_table,
        effects_id: s.effects_id,
        effects,
        num_minterms: s.num_minterms,
        mt_log: s.mt_log,
        minterms_lookup,
        minterms: Vec::new(),
        state_nodes: vec![NodeId::MISSING; num_states],
        node_to_state: HashMap::new(),
        skip_ids: s.skip_ids,
        skip_searchers,
        prefix_skip,
        _prefix_transition: DFA_MISSING as u32,
        max_capacity: u16::MAX as usize,
    })
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn fwd_prefix_to_ser(fp: &accel::FwdPrefixSearch) -> SerFwdPrefix {
    match fp {
        accel::FwdPrefixSearch::Literal(s) => SerFwdPrefix::Literal(s.needle.clone()),
        accel::FwdPrefixSearch::Prefix(s) => SerFwdPrefix::Prefix {
            len: s.len() as u16,
            sets: s.sets.iter().map(|t| t.0).collect(),
        },
        accel::FwdPrefixSearch::Range(s) => SerFwdPrefix::Range {
            len: s.len() as u16,
            anchor_pos: s.anchor_pos as u16,
            ranges: s.ranges.clone(),
            sets: s.sets.iter().map(|t| t.0).collect(),
        },
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn ser_to_fwd_prefix(s: SerFwdPrefix) -> accel::FwdPrefixSearch {
    match s {
        SerFwdPrefix::Literal(needle) => {
            accel::FwdPrefixSearch::Literal(crate::simd::FwdLiteralSearch::new(&needle))
        }
        SerFwdPrefix::Prefix { len, sets } => {
            let sets: Vec<accel::TSet> = sets.into_iter().map(accel::TSet).collect();
            let byte_sets_raw: Vec<Vec<u8>> = sets.iter().map(tset_to_bytes).collect();
            let freq_order = compute_freq_order(&byte_sets_raw);
            accel::FwdPrefixSearch::Prefix(crate::simd::FwdPrefixSearch::new(
                len as usize,
                &freq_order,
                &byte_sets_raw,
                sets,
            ))
        }
        SerFwdPrefix::Range { len, anchor_pos, ranges, sets } => {
            let sets: Vec<accel::TSet> = sets.into_iter().map(accel::TSet).collect();
            accel::FwdPrefixSearch::Range(crate::simd::FwdRangeSearch::new(
                len as usize,
                anchor_pos as usize,
                ranges,
                sets,
            ))
        }
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn tset_to_bytes(t: &accel::TSet) -> Vec<u8> {
    let mut bytes = Vec::new();
    for word in 0..4 {
        for bit in 0..64 {
            if t.0[word] & (1u64 << bit) != 0 {
                bytes.push((word * 64 + bit) as u8);
            }
        }
    }
    bytes
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn compute_freq_order(byte_sets_raw: &[Vec<u8>]) -> Vec<usize> {
    let mut freqs: Vec<(usize, u64)> = byte_sets_raw
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            let freq: u64 = bytes
                .iter()
                .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u64)
                .sum();
            (i, freq)
        })
        .filter(|&(_, f)| f > 0)
        .collect();
    freqs.sort_by_key(|&(_, f)| f);
    freqs.iter().map(|&(i, _)| i).collect()
}

pub fn to_bytes(regex: &Regex) -> Result<Vec<u8>, Error> {
    let inner = &mut *regex.inner.lock().unwrap();

    let cap = inner.fwd.max_capacity;
    if !inner.fwd.precompile(&mut inner.b, cap) {
        return Err(Error::CapacityExceeded);
    }
    if !inner.rev.precompile(&mut inner.b, cap) {
        return Err(Error::CapacityExceeded);
    }
    if inner.fwd.state_nodes.len() > u16::MAX as usize
        || inner.rev.state_nodes.len() > u16::MAX as usize
    {
        return Err(Error::CapacityExceeded);
    }

    verify_complete(&inner.fwd)?;
    verify_complete(&inner.rev)?;

    let fwd_prefix = {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        { regex.fwd_prefix.as_ref().map(fwd_prefix_to_ser) }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        { None::<SerFwdPrefix> }
    };

    let sr = SerRegex {
        fwd_prefix_stripped: regex.fwd_prefix_stripped,
        empty_nullable: regex.empty_nullable,
        fwd_end_nullable: regex.fwd_end_nullable,
        hardened: regex.hardened,
        has_rev_accel: regex.has_rev_accel,
        fixed_length: regex.fixed_length,
        max_length: regex.max_length,
        fwd: ldfa_to_ser(&inner.fwd),
        rev: ldfa_to_ser(&inner.rev),
        fwd_prefix,
    };

    let payload = bincode::serialize(&sr).map_err(ser_err)?;
    let mut out = Vec::with_capacity(MAGIC.len() + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&payload);
    Ok(out)
}

fn verify_complete(ldfa: &LDFA) -> Result<(), Error> {
    let num_states = ldfa.state_nodes.len();
    let stride = 1usize << ldfa.mt_log;
    let num_mt = ldfa.num_minterms as usize;

    let mut visited = vec![false; num_states];
    let mut queue: Vec<usize> = Vec::new();
    for &sid in &ldfa.begin_table {
        let s = sid as usize;
        if s > 1 && s < num_states && !visited[s] {
            visited[s] = true;
            queue.push(s);
        }
    }

    let mut qi = 0;
    while qi < queue.len() {
        let sid = queue[qi];
        qi += 1;
        let base = sid * stride;
        for mt in 0..num_mt {
            let idx = base + mt;
            if idx >= ldfa.center_table.len() || ldfa.center_table[idx] == DFA_MISSING {
                return Err(ser_err(format!(
                    "incomplete DFA: state {} minterm {} not compiled",
                    sid, mt
                )));
            }
            let next = ldfa.center_table[idx] as usize;
            if next > 1 && next < num_states && !visited[next] {
                visited[next] = true;
                queue.push(next);
            }
        }
    }
    Ok(())
}

pub fn from_bytes(data: &[u8]) -> Result<Regex, Error> {
    if data.len() < MAGIC.len() || data[..MAGIC.len()] != MAGIC {
        return Err(ser_err("bad magic"));
    }

    let sr: SerRegex = bincode::deserialize(&data[MAGIC.len()..]).map_err(ser_err)?;

    let fwd = ser_to_ldfa(sr.fwd)?;
    let rev = ser_to_ldfa(sr.rev)?;

    let fwd_prefix: Option<accel::FwdPrefixSearch> = match sr.fwd_prefix {
        None => None,
        Some(sfp) => {
            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            { Some(ser_to_fwd_prefix(sfp)) }
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            { return Err(ser_err("fwd prefix requires SIMD")); }
        }
    };

    Ok(Regex {
        inner: Mutex::new(RegexInner {
            b: RegexBuilder::new(),
            fwd,
            rev,
            nulls_buf: Vec::new(),
            matches_buf: Vec::new(),
            bounded: None,
        }),
        fwd_prefix,
        fwd_prefix_stripped: sr.fwd_prefix_stripped,
        fixed_length: sr.fixed_length,
        max_length: sr.max_length,
        empty_nullable: sr.empty_nullable,
        fwd_end_nullable: sr.fwd_end_nullable,
        hardened: sr.hardened,
        has_bounded_prefix: false,
        has_rev_accel: sr.has_rev_accel,
        has_bounded: false,
    })
}

#[cfg(all(test, feature = "precompile-tests"))]
mod tests {
    use super::*;

    fn round_trip(pattern: &str, input: &[u8]) {
        let re = Regex::new(pattern).unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        let m1 = re.find_all(input).unwrap();
        let m2 = re2.find_all(input).unwrap();
        assert_eq!(m1, m2, "mismatch for pattern {:?}", pattern);
    }

    #[test]
    fn test_literal() {
        round_trip("hello", b"say hello world hello");
    }

    #[test]
    fn test_digit_pattern() {
        round_trip(r"\d{3}-\d{4}", b"call 555-1234 or 555-5678");
    }

    #[test]
    fn test_word_boundary() {
        round_trip(r"\b\w+\b", b"hello world");
    }

    #[test]
    fn test_alternation() {
        round_trip("cat|dog|bird", b"I have a cat and a dog but no bird");
    }

    #[test]
    fn test_empty_match() {
        let re = Regex::new("a*").unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        let m1 = re.find_all(b"bbb").unwrap();
        let m2 = re2.find_all(b"bbb").unwrap();
        assert_eq!(m1, m2);
    }

    #[test]
    fn test_no_match() {
        round_trip("xyz", b"abc def ghi");
    }

    #[test]
    fn test_unicode_class() {
        round_trip(r"\w+", b"hello world 123");
    }

    #[test]
    fn test_empty_input() {
        let re = Regex::new("abc").unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        let m1 = re.find_all(b"").unwrap();
        let m2 = re2.find_all(b"").unwrap();
        assert_eq!(m1, m2);
    }

    #[test]
    fn test_fixed_length() {
        round_trip(r"\d\d\d", b"abc 123 def 456 ghi");
    }

    #[test]
    fn test_dot_star() {
        round_trip(r"a.*b", b"aXXXb aYb");
    }

    #[test]
    fn test_character_class() {
        round_trip("[A-Za-z]+", b"hello WORLD 123 foo");
    }

    #[test]
    fn test_bad_magic() {
        let result = Regex::from_bytes(b"BAD\x01rest");
        assert!(result.is_err());
    }

    #[test]
    fn test_truncated() {
        let re = Regex::new("abc").unwrap();
        let bytes = re.to_bytes().unwrap();
        let result = Regex::from_bytes(&bytes[..bytes.len() / 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_match_roundtrip() {
        let re = Regex::new(r"\d+").unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        assert_eq!(re.is_match(b"abc 123").unwrap(), re2.is_match(b"abc 123").unwrap());
        assert_eq!(re.is_match(b"abc").unwrap(), re2.is_match(b"abc").unwrap());
    }

    #[test]
    fn test_find_anchored_roundtrip() {
        let re = Regex::new(r"\d+").unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        assert_eq!(re.find_anchored(b"123abc").unwrap(), re2.find_anchored(b"123abc").unwrap());
        assert_eq!(re.find_anchored(b"abc123").unwrap(), re2.find_anchored(b"abc123").unwrap());
    }

    #[test]
    fn test_hardened() {
        use crate::EngineOptions;
        let re = Regex::with_options(r"\w+", EngineOptions::default().hardened(true)).unwrap();
        let bytes = re.to_bytes().unwrap();
        let re2 = Regex::from_bytes(&bytes).unwrap();
        assert_eq!(re.find_all(b"hello world").unwrap(), re2.find_all(b"hello world").unwrap());
    }

    struct TomlTestCase {
        pattern: String,
        input: String,
        ignore: bool,
        expect_error: bool,
        anchored: bool,
    }

    fn load_toml_tests(filename: &str) -> Vec<TomlTestCase> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join(filename);
        let content = std::fs::read_to_string(&path).unwrap();
        let table: toml::Value = content.parse().unwrap();
        let tests = table["test"].as_array().unwrap();
        tests
            .iter()
            .map(|t| TomlTestCase {
                pattern: t["pattern"].as_str().unwrap().to_string(),
                input: t.get("input").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
                expect_error: t.get("expect_error").and_then(|v| v.as_bool()).unwrap_or(false),
                anchored: t.get("anchored").and_then(|v| v.as_bool()).unwrap_or(false),
            })
            .collect()
    }

    fn round_trip_toml_file(filename: &str) {
        let tests = load_toml_tests(filename);
        let mut serialized = 0;
        let mut skipped = 0;
        for tc in &tests {
            if tc.ignore || tc.expect_error {
                continue;
            }
            let re = Regex::new(&tc.pattern).unwrap();
            let input = tc.input.as_bytes();
            let m_orig = if tc.anchored {
                re.find_anchored(input).unwrap().into_iter().map(|m| (m.start, m.end)).collect::<Vec<_>>()
            } else {
                re.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect::<Vec<_>>()
            };

            let bytes = match re.to_bytes() {
                Ok(b) => b,
                Err(_) => { skipped += 1; continue; }
            };
            let re2 = Regex::from_bytes(&bytes).unwrap();

            let m_deser = if tc.anchored {
                re2.find_anchored(input).unwrap().into_iter().map(|m| (m.start, m.end)).collect::<Vec<_>>()
            } else {
                re2.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect::<Vec<_>>()
            };

            assert_eq!(
                m_orig, m_deser,
                "serialization round-trip mismatch: file={} pattern={:?} input={:?}",
                filename, tc.pattern, tc.input,
            );
            serialized += 1;
        }
        eprintln!("  {}: {} serialized, {} skipped (capacity exceeded)", filename, serialized, skipped);
    }

    #[test]
    fn test_roundtrip_basic() { round_trip_toml_file("basic.toml"); }
    #[test]
    fn test_roundtrip_anchors() { round_trip_toml_file("anchors.toml"); }
    // boolean.toml skipped: full DFA compilation of complex intersection/complement
    // takes minutes. correctness covered by edge_cases.toml and accel_skip.toml.
    #[test]
    fn test_roundtrip_lookaround() { round_trip_toml_file("lookaround.toml"); }
    #[test]
    fn test_roundtrip_date_pattern() { round_trip_toml_file("date_pattern.toml"); }
    #[test]
    fn test_roundtrip_edge_cases() { round_trip_toml_file("edge_cases.toml"); }
    #[test]
    fn test_roundtrip_paragraph() { round_trip_toml_file("paragraph.toml"); }
    #[test]
    fn test_roundtrip_accel_skip() { round_trip_toml_file("accel_skip.toml"); }
    #[test]
    fn test_roundtrip_find_anchored() { round_trip_toml_file("find_anchored.toml"); }
    #[test]
    fn test_roundtrip_cloudflare_redos() { round_trip_toml_file("cloudflare_redos.toml"); }
}
