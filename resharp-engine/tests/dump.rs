#![cfg(feature = "serialize")]

use resharp::Regex;

fn roundtrip(pattern: &str, inputs: &[&str]) {
    let re = Regex::new(pattern).unwrap();
    let bytes = re.dump().unwrap_or_else(|e| panic!("dump {pattern}: {e}"));
    let re2 = Regex::load(&bytes).unwrap_or_else(|e| panic!("load {pattern}: {e}"));
    for s in inputs {
        let a = re.find_all(s.as_bytes()).unwrap();
        let b = re2.find_all(s.as_bytes()).unwrap();
        assert_eq!(a, b, "pattern {pattern:?} input {s:?}");
        assert_eq!(
            re.is_match(s.as_bytes()).unwrap(),
            re2.is_match(s.as_bytes()).unwrap(),
            "is_match pattern {pattern:?} input {s:?}"
        );
    }
}

#[test]
fn missing_contains_dfa_is_rejected() {
    use bincode::Options;
    let re = Regex::new(r"[^:]//+").unwrap();
    assert_eq!(re.find_all_kind_name(), "Dfa");
    let config = bincode::DefaultOptions::new().with_fixint_encoding().with_little_endian();
    let bytes = re.dump().unwrap();
    let mut dump: resharp::dump::RegexDump = config.deserialize(&bytes).unwrap();
    assert!(dump.is_match_dfa.take().is_some());
    let loaded = Regex::load(&config.serialize(&dump).unwrap()).unwrap();
    assert!(matches!(loaded.is_match(b"text // comment"), Err(resharp::Error::InternalError("missing is_match DFA"))));
}

#[test]
fn minimum_match_length_roundtrip() {
    roundtrip(r"[0-9a-f]{64}\.json", &["fff.json", &format!("{}.json", "f".repeat(64))]);
}

#[test]
fn reverse_contains_dfa_roundtrip() {
    roundtrip(r"[^:]//+", &["text // comment", "https://example", "no comment", "//"]);
}

#[test]
fn begin_anchored_arm_split_roundtrip() {
    let re = Regex::with_options(r"[ \t]+$|^[ \t]+", resharp::RegexOptions::default().unicode(resharp::UnicodeMode::Ascii).multiline(false)).unwrap();
    let loaded = Regex::load(&re.dump().unwrap()).unwrap();
    for input in ["", "x", " x", "x ", "x y", "\t", "xx  "] {
        assert_eq!(re.is_match(input.as_bytes()).unwrap(), loaded.is_match(input.as_bytes()).unwrap(), "{input:?}");
    }
}

#[test]
fn forward_is_match_direction_roundtrip() {
    let re = Regex::with_options(r"(className|data-[0-9A-Z_a-z]+|style)[ \t]*=[ \t]*\{[^}]{0,3}\{[^}]{0,3}\}[^}]{0,3}\}", resharp::RegexOptions::default().unicode(resharp::UnicodeMode::Ascii).multiline(false)).unwrap();
    let loaded = Regex::load(&re.dump().unwrap()).unwrap();
    for input in ["", "x", "className={a{b}c}", "style = { {x} y }", "data-id={{}}", "style={a}"] {
        assert_eq!(re.is_match(input.as_bytes()).unwrap(), loaded.is_match(input.as_bytes()).unwrap(), "{input:?}");
    }
}

#[test]
fn word_boundary_literal_contains_roundtrip() {
    let re = Regex::with_options(r"\bxml\b", resharp::RegexOptions::default().unicode(resharp::UnicodeMode::Ascii).multiline(false)).unwrap();
    let loaded = Regex::load(&re.dump().unwrap()).unwrap();
    for input in ["", "none", "xml", "!xml!", "axml", "xml2", "éxmlé"] {
        assert_eq!(re.is_match(input.as_bytes()).unwrap(), loaded.is_match(input.as_bytes()).unwrap(), "{input:?}");
    }
}

#[test]
fn digits() {
    roundtrip(r"\d+", &["abc 123 def 456", "", "no digits", "9"]);
}

#[test]
fn word() {
    roundtrip(r"\w+", &["hello world", "  ", "x"]);
}

#[test]
fn alt() {
    roundtrip(r"cat|dog|bird", &["cat dog bird fish", "catdog", ""]);
}

#[test]
fn lookbehind_inter() {
    roundtrip(r"\d{3}-\d{4}", &["call 555-1234 or 555-5678", "no"]);
}

#[test]
fn begin_anchored() {
    roundtrip(r"\Aabc\d+", &["abc123 xyz", "xyz abc123", "abc", ""]);
}

#[test]
fn anchored_fwd_prefix() {
    roundtrip(
        r"hello+ world",
        &["say hello world", "helloo world", "none"],
    );
}

#[test]
fn ranges_prefix() {
    roundtrip(r"[A-Z]{3}\d+", &["ABC123 XYZ7 hi", "no caps"]);
}

#[test]
fn bdfa_short_alt() {
    // small fixed-max-length pattern: triggers BDFA path in build
    roundtrip(
        r"foo|barz|qux",
        &["foo bar barz qux quxx no", "foobarzqux", "none"],
    );
}

#[test]
fn star_loop_always_nullable() {
    roundtrip(r".*", &["abc", "", "a\nb"]);
    roundtrip(r"(a|b)*", &["aabba c", "", "xyz"]);
}

#[test]
fn captures_fixed_offsets_roundtrip() {
    let re = Regex::new(r"(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})").unwrap();
    assert_eq!(re.captures_kind_name(), "FixedOffsets");
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    assert_eq!(re2.captures_kind_name(), "FixedOffsets");
    assert_eq!(re2.capture_names(), re.capture_names());
    let input = b"logged 2024-01-15 done";
    assert_eq!(
        re2.captures_all(input).unwrap().remove(0).spans(),
        &[Some((7, 17)), Some((7, 11)), Some((12, 14)), Some((15, 17))]
    );
}

#[test]
fn captures_empty_roundtrip() {
    let re = Regex::new(r"\d+").unwrap();
    assert_eq!(re.captures_kind_name(), "Empty");
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    assert_eq!(re2.captures_kind_name(), "Empty");
    assert_eq!(re2.captures_all(b"abc 123").unwrap().remove(0).spans(), &[Some((4, 7))]);
}

#[test]
fn captures_dfa_dispatch_is_rejected_at_dump_time() {
    let re = Regex::new(r"(?P<a>\d{1,3})\.(?P<b>\d{1,3})").unwrap();
    assert_eq!(re.captures_kind_name(), "Dfa");
    assert!(re.dump().is_err());
}

#[test]
fn case_insensitive() {
    let opts = resharp::RegexOptions::default().case_insensitive(true);
    let re = resharp::Regex::with_options(r"hello", opts).unwrap();
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    let i = b"say HELLO and Hello";
    assert_eq!(re.find_all(i).unwrap(), re2.find_all(i).unwrap());
}

#[test]
#[cfg(feature = "regex_set")]
fn dump_rejects_a_regex_set_member() {
    let set = resharp::RegexSet::new(["a", "b"]).unwrap();
    assert!(matches!(set.regex().dump(), Err(resharp::Error::Serialize(_))));
}

#[test]
fn disable_prefixes_dump_matches_prefix_accelerated() {
    let pat = r"age-secret-key-1[0-9a-z]{58}";
    let fast = Regex::new(pat).unwrap();
    assert_eq!(fast.find_all_kind_name(), "FwdPrefix");
    let opts = resharp::RegexOptions { disable_prefixes: true, ..Default::default() };
    let plain = Regex::with_options(pat, opts).unwrap();
    assert!(!plain.has_prefix());
    let back = Regex::load(&plain.dump().unwrap()).unwrap();
    let hay = format!("x age-secret-key-1{} y", "q".repeat(58));
    assert_eq!(
        back.find_all(hay.as_bytes()).unwrap(),
        fast.find_all(hay.as_bytes()).unwrap()
    );
}

#[test]
fn always_nullable_cycle_roundtrip() {
    let inputs = ["abab,x,", "", "a,b,,cc,d", "x y\n,z,", "\"a,b\",c,\"d\"\n"];
    for p in [r"(?:\w+,)*", r"(?:ab)*", r"(?:[A-Za-z]+ )*", r"(?:a|bc)*"] {
        roundtrip(p, &inputs);
    }
}

#[test]
fn dump_of_single_class_stays_small() {
    use resharp::{RegexOptions, UnicodeMode};
    let o = RegexOptions::default().unicode(UnicodeMode::Ascii).multiline(false);
    let re = Regex::with_options("[B-DFGILOS-VZchiqsv-xz]", o).unwrap();
    let t = std::time::Instant::now();
    let d = re.dump().unwrap();
    assert!(d.len() < 100_000, "dump {} bytes in {:?}", d.len(), t.elapsed());
}

#[test]
fn dump_of_single_class_roundtrips() {
    roundtrip("[B-DFGILOS-VZchiqsv-xz]", &["abcdefg BCD xyz", "", "aaa", "zzzBB"]);
}

#[test]
fn dump_is_deterministic_across_builds() {
    use resharp::{RegexOptions, UnicodeMode};
    let o = || RegexOptions::default().unicode(UnicodeMode::Ascii).multiline(false);
    for pat in [
        r"\\([^0-9A-Fa-f]|[0-9A-Fa-f]{1,6})",
        "[$().^-]",
        "[1-9A-HJ-NP-Za-km-z]{87,88}",
    ] {
        let dump = || Regex::with_options(pat, o()).unwrap().dump().unwrap();
        let first = dump();
        for _ in 0..8 {
            assert_eq!(first, dump(), "{pat}");
        }
    }
}

#[test]
fn anchored_is_match() {
    roundtrip(r"^[^\r]{0,10}(?:foo|bar)", &["xx foo", "bar", "aaaaaaaaaaaaaaaaaaaaaaa foo", "", "zzz"]);
}

#[test]
fn is_match_survives_roundtrip() {
    let inputs = ["", "a", "ab", "abc\n", "foo bar", "xx\nab", "b", "aab b", "\n"];
    for p in [
        r"\Aab",
        r"ab\z",
        r"\bfoo\b",
        r"(?<=a)b",
        r"a(?=b)",
        r"(?<!x)a(?!b)",
        r"foo.*bar",
        r"\d+|[a-c]{2,3}",
        r"(?:a|b)+c&~(.*bb.*)",
        r"^ab$",
        r"\Afoo bar",
        r"x*(?:(?!a)\A|c)&(?:|y)",
        r"(?<=\n)&(?:\A(?!a)\A){2}",
    ] {
        roundtrip(p, &inputs);
    }
}
