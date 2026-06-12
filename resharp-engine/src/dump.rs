//! binary dump/load for fully precompiled regex.
//! assumes same architecture
//! builder is not needed at all so it may save some memory for large regexes

use std::collections::HashSet;
use std::sync::Mutex;

use resharp_algebra::{NodeId, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::bdfa::BDFA;
use crate::ldfa::{DFA_DEAD, LDFA};
use crate::prefix::PrefixKind;
use crate::stream::{StreamCache, StreamInit};
use crate::{Error, FindAll, Match, Regex, RegexInner};

pub(crate) mod array256 {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(a: &[u8; 256], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(a)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 256], D::Error> {
        let v = <Vec<u8>>::deserialize(d)?;
        if v.len() != 256 {
            return Err(serde::de::Error::custom("array256: wrong length"));
        }
        let mut out = [0u8; 256];
        out.copy_from_slice(&v);
        Ok(out)
    }
}

#[derive(Serialize, Deserialize)]
struct RegexDump {
    fixed_length: Option<u32>,
    empty_nullable: bool,
    always_nullable: bool,
    is_empty_lang: bool,
    initial_nullability: resharp_algebra::nulls::Nullability,
    fwd_end_nullable: bool,
    fwd_begin_anchored: bool,
    rev_end_anchored: bool,
    has_bounded: bool,
    bounded_safe_find_all: bool,
    fwd_lb_body_nullable: bool,
    has_lb: bool,
    has_la: bool,
    find_all: FindAll,
    lb_check_bytes: u8,
    fwd_lb_begin_nullable: bool,
    has_anchors: bool,
    prefix: Option<PrefixKind>,
    fwd: Option<LDFA>,
    rev_ts: Option<LDFA>,
    bounded: Option<BDFA>,
}

fn precompile_ldfa(ldfa: &mut LDFA, b: &mut RegexBuilder) -> Result<(), Error> {
    let mut visited: HashSet<u16> = HashSet::new();
    let mut work: Vec<u16> = Vec::new();
    if ldfa.pruned > DFA_DEAD {
        work.push(ldfa.pruned);
    }
    for &s in &ldfa.begin_table {
        if s > DFA_DEAD {
            work.push(s);
        }
    }
    while let Some(sid) = work.pop() {
        if !visited.insert(sid) {
            continue;
        }
        ldfa.ensure_capacity(sid);
        ldfa.create_state(b, sid)?;
        let stride = 1usize << ldfa.mt_log;
        let base = (sid as usize) * stride;
        for mt in 0..ldfa.minterms.len() {
            let n = ldfa.center_table[base + mt];
            if n > DFA_DEAD && !visited.contains(&n) {
                work.push(n);
            }
        }
    }
    Ok(())
}

fn precompile_bdfa(bdfa: &mut BDFA, b: &mut RegexBuilder) -> Result<(), Error> {
    let n_mt = bdfa.minterms_lookup.iter().copied().max().unwrap_or(0) as usize + 1;
    let mut visited: HashSet<u16> = HashSet::new();
    let mut work: Vec<u16> = vec![bdfa.initial, bdfa.after_prefix];
    while let Some(sid) = work.pop() {
        if !visited.insert(sid) {
            continue;
        }
        for mt in 0..n_mt {
            let entry = bdfa.transition(b, sid, mt)?;
            let next = (entry & 0xFFFF) as u16;
            if next != 0 && !visited.contains(&next) {
                work.push(next);
            }
        }
    }
    Ok(())
}

fn empty_ldfa() -> LDFA {
    LDFA {
        pruned: DFA_DEAD,
        prune_memo: Default::default(),
        begin_table: Vec::new(),
        center_table: Vec::new(),
        effects_id: Vec::new(),
        effects: Vec::new(),
        center_effect_id: Vec::new(),
        mt_log: 0,
        mt_lookup: [0u8; 256],
        minterms: Vec::new(),
        state_nodes: Vec::new(),
        node_to_state: Default::default(),
        skip_ids: Vec::new(),
        skip_searchers: Vec::new(),
        prefix_skip: None,
        max_capacity: 0,
        is_forward: true,
        has_anchors: false,
        initial_nullability: resharp_algebra::nulls::Nullability::NEVER,
    }
}

fn bincode_cfg() -> impl bincode::Options {
    use bincode::Options;
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_little_endian()
}

impl Regex {
    /// fully precompile and serialize the regex, may fail if the regex has unsupported features
    pub fn dump(&self) -> Result<Vec<u8>, Error> {
        use bincode::Options;
        if self.hardened {
            return Err(Error::Serialize("hardened mode not supported".into()));
        }
        let uses_fwd = !self.has_bounded;
        let uses_rev_ts = !self.fwd_begin_anchored
            && !self.has_bounded
            && !matches!(
                &self.prefix,
                Some(PrefixKind::AnchoredFwd(_) | PrefixKind::AnchoredFwdLb(_))
            );

        let inner = &mut *self.inner.lock().unwrap();
        if uses_fwd {
            precompile_ldfa(&mut inner.fwd, &mut inner.b)?;
        }
        if uses_rev_ts {
            precompile_ldfa(&mut inner.rev_ts, &mut inner.b)?;
        }
        if self.has_bounded {
            precompile_bdfa(inner.bounded.as_mut().unwrap(), &mut inner.b)?;
        }

        let dump = RegexDump {
            fixed_length: self.fixed_length,
            empty_nullable: self.empty_nullable,
            always_nullable: self.always_nullable,
            is_empty_lang: self.is_empty_lang,
            initial_nullability: self.initial_nullability,
            fwd_end_nullable: self.fwd_end_nullable,
            rev_end_anchored: self.rev_end_anchored,
            has_bounded: self.has_bounded,
            bounded_safe_find_all: self.bounded_safe_find_all,
            fwd_lb_body_nullable: self.fwd_lb_body_nullable,
            has_lb: self.has_lb,
            has_la: self.has_la,
            lb_check_bytes: self.lb_check_bytes,
            fwd_lb_begin_nullable: self.fwd_lb_begin_nullable,
            has_anchors: self.has_anchors,
            prefix: self.prefix.clone(),
            fwd_begin_anchored: self.fwd_begin_anchored,
            find_all: self.find_all,
            fwd: if uses_fwd {
                Some(std::mem::replace(&mut inner.fwd, empty_ldfa()))
            } else {
                None
            },
            rev_ts: if uses_rev_ts {
                Some(std::mem::replace(&mut inner.rev_ts, empty_ldfa()))
            } else {
                None
            },
            bounded: if self.has_bounded {
                inner.bounded.take()
            } else {
                None
            },
        };

        let out = bincode_cfg()
            .serialize(&dump)
            .map_err(|e| Error::Serialize(format!("bincode: {e}")))?;

        // restore moved-out fields so the source regex stays usable
        if let Some(fwd) = dump.fwd {
            inner.fwd = fwd;
        }
        if let Some(rev_ts) = dump.rev_ts {
            inner.rev_ts = rev_ts;
        }
        if let Some(b) = dump.bounded {
            inner.bounded = Some(b);
        }
        Ok(out)
    }

    /// reconstruct a regex from bytes produced by [`Regex::dump`].
    pub fn load(bytes: &[u8]) -> Result<Regex, Error> {
        use bincode::Options;
        let dump: RegexDump = bincode_cfg()
            .deserialize(bytes)
            .map_err(|e| Error::Serialize(format!("bincode: {e}")))?;

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b: RegexBuilder::new(),
                fwd: dump.fwd.unwrap_or_else(empty_ldfa),
                fwd_ts: empty_ldfa(),
                rev: None,
                rev_ts: dump.rev_ts.unwrap_or_else(empty_ldfa),
                stream: StreamInit {
                    start_node: NodeId::MISSING,
                    seek_fwd: 0,
                    seek_rev: 0,
                },
                nulls: Vec::new(),
                matches: Vec::<Match>::new(),
                bounded: dump.bounded,
                fas: None,
            }),
            prefix: dump.prefix,
            fixed_length: dump.fixed_length,
            empty_nullable: dump.empty_nullable,
            always_nullable: dump.always_nullable,
            is_empty_lang: dump.is_empty_lang,
            fwd_begin_anchored: dump.fwd_begin_anchored,
            find_all: dump.find_all,
            initial_nullability: dump.initial_nullability,
            fwd_end_nullable: dump.fwd_end_nullable,
            hardened: false,
            rev_end_anchored: dump.rev_end_anchored,
            has_bounded: dump.has_bounded,
            bounded_safe_find_all: dump.bounded_safe_find_all,
            fwd_lb_body_nullable: dump.fwd_lb_body_nullable,
            has_lb: dump.has_lb,
            has_la: dump.has_la,
            lb_check_bytes: dump.lb_check_bytes,
            fwd_lb_begin_nullable: dump.fwd_lb_begin_nullable,
            has_anchors: dump.has_anchors,
            stream_cache: StreamCache::default(),
        })
    }
}
