//! binary dump/load for fully precompiled regex.
//! assumes same architecture
//! builder is not needed at all so it may save some memory for large regexes

use std::collections::HashSet;
use std::sync::Mutex;

use resharp_algebra::NodeId;
use resharp_algebra::RegexBuilder;
use serde::{Deserialize, Serialize};

use crate::bdfa::BDFA;
use crate::captures::CaptureDispatch;
use crate::ldfa::{DFA_DEAD, LDFA};
use crate::prefix::PrefixKind;
#[cfg(feature = "stream")]
use crate::stream::{StreamCache, StreamInit};
use crate::{Error, FindAll, Match, Regex, RegexInner, StartPositions};

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
#[allow(missing_docs)]
pub struct RegexDump {
    pub fixed_length: Option<u32>,
    pub empty_nullable: bool,
    pub always_nullable: bool,
    pub star_loop: bool,
    pub is_empty_lang: bool,
    pub initial_nullability: resharp_algebra::nulls::Nullability,
    pub fwd_end_nullable: bool,
    pub rev_end_nullable: bool,
    pub fwd_begin_anchored: bool,
    pub fwd_lb_stripped: bool,
    pub rev_end_anchored: bool,
    pub has_bounded: bool,
    pub bounded_safe_find_all: bool,
    pub fwd_lb_body_nullable: bool,
    pub has_lb: bool,
    pub has_la: bool,
    pub neg_lb: Option<crate::prefix::NegLb>,
    pub find_all: FindAll,
    pub class_plus: Option<[u64; 4]>,
    pub lb_check_bytes: u8,
    pub fwd_lb_begin_nullable: bool,
    pub fwd_lb_begin_len: u8,
    pub fwd_lb_begin_classes: Vec<crate::accel::TSet>,
    pub has_anchors: bool,
    pub prefix: Option<PrefixKind>,
    pub fwd: Option<LDFA>,
    pub lb_verify: Option<LDFA>,
    pub rev_ts: Option<LDFA>,
    pub bounded: Option<BDFA>,
    #[cfg(feature = "convergence_prefix")]
    pub conv_prefix: bool,
    #[cfg(feature = "convergence_prefix")]
    pub conv_b: Option<LDFA>,
    pub group_names: Vec<Option<String>>,
    pub captures_dispatch: CaptureDispatch,
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
        if matches!(self.captures_dispatch, CaptureDispatch::Dfa) {
            return Err(Error::Serialize("capture groups requiring the general tag-tracking DFA are not supported".into()));
        }
        let uses_fwd = !self.has_bounded;
        let uses_lb_verify = matches!(&self.prefix, Some(PrefixKind::AnchoredFwdLb(_)));
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
        if uses_lb_verify {
            precompile_ldfa(inner.lb_verify.as_mut().unwrap(), &mut inner.b)?;
        }
        if uses_rev_ts {
            precompile_ldfa(&mut inner.rev_ts, &mut inner.b)?;
        }
        if self.has_bounded {
            precompile_bdfa(inner.bounded.as_mut().unwrap(), &mut inner.b)?;
        }
        #[cfg(feature = "convergence_prefix")]
        if self.conv_prefix {
            if let Some(cb) = inner.conv_b.as_mut() {
                precompile_ldfa(cb, &mut inner.b)?;
            }
        }

        let dump = RegexDump {
            fixed_length: self.fixed_length,
            empty_nullable: self.empty_nullable,
            always_nullable: self.always_nullable,
            star_loop: self.star_loop,
            is_empty_lang: self.is_empty_lang,
            initial_nullability: self.initial_nullability,
            fwd_end_nullable: self.fwd_end_nullable,
            rev_end_nullable: self.rev_end_nullable,
            rev_end_anchored: self.rev_end_anchored,
            has_bounded: self.has_bounded,
            bounded_safe_find_all: self.bounded_safe_find_all,
            fwd_lb_body_nullable: self.fwd_lb_body_nullable,
            has_lb: self.init_flags.has_lb(),
            has_la: self.init_flags.has_la(),
            neg_lb: self.neg_lb.clone(),
            lb_check_bytes: self.lb_check_bytes,
            fwd_lb_begin_nullable: self.fwd_lb_begin_nullable,
            fwd_lb_begin_len: self.fwd_lb_begin_len,
            fwd_lb_begin_classes: self.fwd_lb_begin_classes.clone(),
            has_anchors: self.init_flags.has_anchors(),
            prefix: self.prefix.clone(),
            fwd_begin_anchored: self.fwd_begin_anchored,
            fwd_lb_stripped: self.fwd_lb_stripped,
            find_all: self.find_all,
            class_plus: self.class_plus,
            fwd: if uses_fwd {
                Some(std::mem::replace(&mut inner.fwd, empty_ldfa()))
            } else {
                None
            },
            lb_verify: if uses_lb_verify {
                inner.lb_verify.take()
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
            #[cfg(feature = "convergence_prefix")]
            conv_prefix: self.conv_prefix,
            #[cfg(feature = "convergence_prefix")]
            conv_b: if self.conv_prefix {
                inner.conv_b.take()
            } else {
                None
            },
            group_names: self.group_names.clone(),
            captures_dispatch: self.captures_dispatch.clone(),
        };

        let out = bincode_cfg()
            .serialize(&dump)
            .map_err(|e| Error::Serialize(format!("bincode: {e}")))?;

        // restore moved-out fields so the source regex stays usable
        if let Some(fwd) = dump.fwd {
            inner.fwd = fwd;
        }
        if let Some(lb_verify) = dump.lb_verify {
            inner.lb_verify = Some(lb_verify);
        }
        if let Some(rev_ts) = dump.rev_ts {
            inner.rev_ts = rev_ts;
        }
        if let Some(b) = dump.bounded {
            inner.bounded = Some(b);
        }
        #[cfg(feature = "convergence_prefix")]
        if let Some(cb) = dump.conv_b {
            inner.conv_b = Some(cb);
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
                #[cfg(feature = "convergence_prefix")]
                conv_b: dump.conv_b,
                #[cfg(feature = "stream")]
                stream: StreamInit {
                    start_node: NodeId::MISSING,
                    seek_fwd: 0,
                    seek_rev: 0,
                },
                nulls: StartPositions::new(),
                matches: Vec::<Match>::new(),
                bounded: dump.bounded,
                fas: None,
                lb_verify: dump.lb_verify,
                capture_root: NodeId::MISSING,
                skeleton: None,
                capture_dfa: crate::pparse::PosixParser::new(0),
            }),
            prefix: dump.prefix,
            fixed_length: dump.fixed_length,
            empty_nullable: dump.empty_nullable,
            always_nullable: dump.always_nullable,
            star_loop: dump.star_loop,
            is_empty_lang: dump.is_empty_lang,
            fwd_begin_anchored: dump.fwd_begin_anchored,
            fwd_lb_stripped: dump.fwd_lb_stripped,
            find_all: dump.find_all,
            class_plus: dump.class_plus,
            initial_nullability: dump.initial_nullability,
            fwd_end_nullable: dump.fwd_end_nullable,
            rev_end_nullable: dump.rev_end_nullable,
            hardened: false,
            rev_end_anchored: dump.rev_end_anchored,
            has_bounded: dump.has_bounded,
            bounded_safe_find_all: dump.bounded_safe_find_all,
            fwd_lb_body_nullable: dump.fwd_lb_body_nullable,
            fwd_lb_begin_len: dump.fwd_lb_begin_len,
            fwd_lb_begin_classes: dump.fwd_lb_begin_classes,
            init_flags: crate::InitialNodeFlags::new(dump.has_anchors, dump.has_lb, dump.has_la),
            #[cfg(feature = "convergence_prefix")]
            conv_prefix: dump.conv_prefix,
            neg_lb: dump.neg_lb,
            lb_check_bytes: dump.lb_check_bytes,
            fwd_lb_begin_nullable: dump.fwd_lb_begin_nullable,
            group_names: dump.group_names,
            captures_dispatch: dump.captures_dispatch,
            #[cfg(feature = "stream")]
            stream_cache: StreamCache::default(),
        })
    }
}
