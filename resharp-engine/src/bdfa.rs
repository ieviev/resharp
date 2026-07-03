use std::collections::HashMap;

use resharp_algebra::nulls::Nullability;
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder};

use crate::minterms::{collect_sets, transition_term, PartitionTree};
use crate::{Error, Match, Regex};

const RARE_BYTE_FREQ_LIMIT: u16 = 25_000;

/// bounded DFA for matching with known max_length eg. abc|def
/// only exists for a slight (20-30%) performance boost on short patterns
/// when two DFAs arent necessary
/// this is basically derivative based Aho-Corasick
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct BDFA {
    #[cfg_attr(feature = "serialize", serde(skip))]
    initial_node: NodeId,
    /// states as Counted node chains.
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub states: Vec<NodeId>,
    #[cfg_attr(feature = "serialize", serde(skip))]
    state_map: HashMap<NodeId, u16>,
    /// packed transition table: entry = (match_rel << 16) | next_state.
    /// 0 = uncached sentinel.
    pub table: Vec<u32>,
    /// match start rel per state: step (0 = no match).
    pub match_rel: Vec<u32>,
    /// match end offset per state: step - best (distance from pos to match end).
    pub match_end_off: Vec<u32>,
    /// log2 of minterm stride.
    pub mt_log: u32,
    #[cfg_attr(feature = "serialize", serde(skip))]
    minterms: Vec<TSetId>,
    /// byte -> minterm index.
    #[cfg_attr(feature = "serialize", serde(with = "crate::dump::array256"))]
    pub minterms_lookup: [u8; 256],
    /// initial state id.
    pub initial: u16,
    /// SIMD prefix search.
    pub prefix: Option<crate::accel::FwdPrefixSearch>,
    /// prefix length in bytes.
    pub prefix_len: usize,
    /// state after transitioning through the prefix.
    pub after_prefix: u16,
}

impl BDFA {
    pub fn new(b: &mut RegexBuilder, pattern_node: NodeId) -> Result<Self, Error> {
        let initial_node = b.mk_ordered(pattern_node, NodeId::MISSING, 0);
        let sets = collect_sets(b, initial_node);
        let minterms = PartitionTree::generate_minterms(sets, b.solver());
        let minterms_lookup = PartitionTree::minterms_lookup(&minterms, b.solver());
        let num_mt = minterms.len();
        let mt_log = num_mt.next_power_of_two().trailing_zeros();
        let stride = 1usize << mt_log;

        let mut dfa = BDFA {
            initial_node,
            states: vec![NodeId::MISSING, NodeId::MISSING],
            state_map: HashMap::new(),
            table: vec![0u32; stride * 2],
            match_rel: vec![0, 0],
            match_end_off: vec![0, 0],
            mt_log,
            minterms,
            minterms_lookup,
            initial: 1,
            prefix: None,
            prefix_len: 0,
            after_prefix: 1,
        };
        dfa.state_map.insert(NodeId::MISSING, 1);
        dfa.build_prefix(b, pattern_node)?;
        Ok(dfa)
    }

    fn build_prefix(&mut self, b: &mut RegexBuilder, pattern_node: NodeId) -> Result<(), Error> {
        if !crate::simd::has_simd() {
            return Ok(());
        }
        let mut prefix_sets = crate::prefix::calc_prefix_sets_inner(b, pattern_node, false)?;
        if prefix_sets.len() > 16 {
            prefix_sets.truncate(16);
        }
        if cfg!(feature = "debug") {
            let byte_counts: Vec<usize> = prefix_sets
                .iter()
                .map(|&s| b.solver_ref().collect_bytes(s).len())
                .collect();
            eprintln!(
                "  [bdfa-build-prefix] linear_sets={} bytes={:?}",
                prefix_sets.len(),
                byte_counts
            );
        }
        if prefix_sets.is_empty() {
            return self.build_prefix_potential(b, pattern_node);
        }

        let byte_sets_raw: Vec<Vec<u8>> = prefix_sets
            .iter()
            .map(|&s| b.solver_ref().collect_bytes(s))
            .collect();

        if byte_sets_raw.len() < 3 && byte_sets_raw.iter().any(|bs| bs.len() > 1) {
            return self.build_prefix_potential(b, pattern_node);
        }

        let search = Self::build_prefix_search(&byte_sets_raw);
        let search = match search {
            Some(s) => s,
            None => return self.build_prefix_potential(b, pattern_node),
        };

        let mut state = self.initial;
        for &set in &prefix_sets {
            let mt_idx = self.minterms.iter().position(|&mt| {
                let mt_set = b.solver_ref().get_set(mt);
                let prefix_set = b.solver_ref().get_set(set);
                Solver::is_sat(&mt_set, &prefix_set)
            });
            match mt_idx {
                Some(idx) => state = (self.transition(b, state, idx)? & 0xFFFF) as u16,
                None => return Ok(()),
            }
        }

        self.prefix = Some(search);
        self.prefix_len = prefix_sets.len();
        self.after_prefix = state;
        Ok(())
    }

    fn build_prefix_potential(
        &mut self,
        b: &mut RegexBuilder,
        pattern_node: NodeId,
    ) -> Result<(), Error> {
        let sets = crate::prefix::calc_potential_start(b, pattern_node, 16, 64, false)?;
        if cfg!(feature = "debug") {
            eprintln!(
                "  [bdfa-prefix-potential] node={:?} sets={}",
                pattern_node,
                sets.len()
            );
        }
        if sets.is_empty() {
            return Ok(());
        }
        let byte_sets_raw: Vec<Vec<u8>> = sets
            .iter()
            .map(|&s| b.solver_ref().collect_bytes(s))
            .collect();
        if cfg!(feature = "debug") {
            for (i, bs) in byte_sets_raw.iter().enumerate() {
                eprintln!("  [bdfa-prefix-potential] pos={} bytes={}", i, bs.len());
            }
        }
        let search = match Self::build_prefix_search(&byte_sets_raw) {
            Some(s) => s,
            None => return Ok(()),
        };
        self.prefix = Some(search);
        self.prefix_len = sets.len();
        Ok(())
    }

    fn build_prefix_search(byte_sets_raw: &[Vec<u8>]) -> Option<crate::accel::FwdPrefixSearch> {
        if byte_sets_raw.iter().all(|bs| bs.len() == 1) {
            let needle: Vec<u8> = byte_sets_raw.iter().map(|bs| bs[0]).collect();
            let lit = crate::simd::FwdLiteralSearch::new(&needle);
            if crate::simd::BYTE_FREQ[lit.rare_byte() as usize] >= RARE_BYTE_FREQ_LIMIT {
                return None;
            }
            return Some(crate::accel::FwdPrefixSearch::Literal(lit));
        }

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
        if freqs.is_empty() {
            return None;
        }
        freqs.sort_by_key(|&(_, f)| f);

        let rarest_idx = freqs[0].0;
        if byte_sets_raw[rarest_idx].len() > 16 {
            return Self::try_build_range_prefix(byte_sets_raw, rarest_idx);
        }

        let freq_order: Vec<usize> = freqs.iter().map(|&(i, _)| i).collect();
        let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
            .iter()
            .map(|bytes| crate::accel::TSet::from_bytes(bytes))
            .collect();

        Some(crate::accel::FwdPrefixSearch::Prefix(
            crate::simd::FwdPrefixSearch::new(
                byte_sets_raw.len(),
                &freq_order,
                byte_sets_raw,
                all_sets,
            ),
        ))
    }

    fn try_build_range_prefix(
        byte_sets_raw: &[Vec<u8>],
        anchor_pos: usize,
    ) -> Option<crate::accel::FwdPrefixSearch> {
        let anchor_bytes = &byte_sets_raw[anchor_pos];
        let freq_sum: u32 = anchor_bytes
            .iter()
            .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u32)
            .sum();
        if freq_sum >= crate::prefix::SKIP_FREQ_THRESHOLD {
            return None;
        }
        let tset = crate::accel::TSet::from_bytes(anchor_bytes);
        let ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
        if ranges.is_empty() || ranges.len() > 3 {
            return None;
        }
        let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
            .iter()
            .map(|bytes| crate::accel::TSet::from_bytes(bytes))
            .collect();
        if cfg!(feature = "debug") {
            eprintln!(
                "  [bdfa-prefix-range] anchor=pos{} ranges={:?} len={}",
                anchor_pos,
                ranges,
                byte_sets_raw.len()
            );
        }
        Some(crate::accel::FwdPrefixSearch::Range(
            crate::simd::FwdRangeSearch::new(byte_sets_raw.len(), anchor_pos, ranges, all_sets),
        ))
    }

    pub fn counted_best(node: NodeId, b: &RegexBuilder) -> u32 {
        b.get_extra(node) >> 16
    }

    fn register(&mut self, node: NodeId, b: &RegexBuilder) -> u16 {
        if let Some(&sid) = self.state_map.get(&node) {
            return sid;
        }
        let sid = self.states.len() as u16;
        let mut match_step = 0u32;
        let mut match_best = 0u32;
        if node.0 > NodeId::BOT.0 {
            debug_assert_eq!(b.get_kind(node), Kind::Ordered);
            if node.left(b) == NodeId::BOT {
                let best = Self::counted_best(node, b);
                if best > 0 {
                    match_step = b.get_extra(node) & 0xFFFF;
                    match_best = best;
                }
            }
        }
        if cfg!(feature = "debug") {
            eprintln!(
                "  [bounded] register state {} node={} step={} best={}",
                sid,
                b.pp(node),
                match_step,
                match_best,
            );
        }
        self.states.push(node);
        self.state_map.insert(node, sid);
        self.match_rel.push(match_step);
        self.match_end_off.push(match_step - match_best);
        self.table
            .resize(self.table.len() + (1usize << self.mt_log), 0u32);
        sid
    }

    #[inline(always)]
    pub fn transition(
        &mut self,
        b: &mut RegexBuilder,
        state: u16,
        mt_idx: usize,
    ) -> Result<u32, Error> {
        let delta = (state as usize) << self.mt_log | mt_idx;
        let cached = self.table[delta];
        if cached != 0 {
            return Ok(cached);
        }
        self.transition_slow(b, state, mt_idx)
    }

    fn derive_chain(b: &mut RegexBuilder, head: NodeId, mt: TSetId) -> Result<Vec<NodeId>, Error> {
        let mut result = Vec::new();
        let mut cur = head;
        while cur.0 > NodeId::BOT.0 {
            debug_assert_eq!(b.get_kind(cur), Kind::Ordered);
            let chain = cur.right(b);
            let body = cur.left(b);
            if body == NodeId::BOT {
                let best = Self::counted_best(cur, b);
                if best > 0 {
                    let step = (b.get_extra(cur) & 0xFFFF) + 1;
                    let bumped = b.mk_ordered(NodeId::BOT, NodeId::MISSING, (best << 16) | step);
                    result.push(bumped);
                }
                cur = chain;
                continue;
            }
            let der = b.der(cur, Nullability::CENTER).map_err(Error::Algebra)?;
            let next = transition_term(b, der, mt);
            if next != NodeId::BOT
                && !(next.left(b) == NodeId::BOT && Self::counted_best(next, b) == 0)
            {
                result.push(next);
            }
            cur = chain;
        }
        Ok(result)
    }

    fn rebuild_chain(b: &mut RegexBuilder, candidates: &[NodeId]) -> NodeId {
        let mut chain = NodeId::MISSING;
        for &node in candidates.iter().rev() {
            let body = node.left(b);
            let packed = b.get_extra(node);
            let next = b.mk_ordered(body, chain, packed);
            if next != NodeId::BOT {
                chain = next;
            }
        }
        chain
    }

    #[cold]
    #[inline(never)]
    fn transition_slow(
        &mut self,
        b: &mut RegexBuilder,
        state: u16,
        mt_idx: usize,
    ) -> Result<u32, Error> {
        let head = self.states[state as usize];
        let mt = self.minterms[mt_idx];

        let mut candidates = Self::derive_chain(b, head, mt)?;

        let spawn_der = b
            .der(self.initial_node, Nullability::CENTER)
            .map_err(Error::Algebra)?;
        let spawn_next = transition_term(b, spawn_der, mt);
        if spawn_next != NodeId::BOT && !candidates.contains(&spawn_next) {
            candidates.push(spawn_next);
        }

        let new_head = Self::rebuild_chain(b, &candidates);
        let next_sid = self.register(new_head, b);

        if cfg!(feature = "debug") {
            eprintln!(
                "  [bdfa-slow] state={} mt={} head={} candidates=[{}] new_head={} -> sid={}",
                state,
                mt_idx,
                b.pp(head),
                candidates
                    .iter()
                    .map(|n| b.pp(*n))
                    .collect::<Vec<_>>()
                    .join(", "),
                b.pp(new_head),
                next_sid,
            );
        }

        let rel = self.match_rel[next_sid as usize];
        let packed = (rel << 16) | next_sid as u32;
        let delta = (state as usize) << self.mt_log | mt_idx;
        self.table[delta] = packed;
        Ok(packed)
    }
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Prefix {
    None = 0,
    Search = 1,
    Literal = 2,
}

#[inline(never)]
fn bdfa_inner<const PREFIX: u8>(
    table: *const u32,
    ml: *const u8,
    data: *const u8,
    mt_log: u32,
    initial: u16,
    match_end_off: *const u32,
    mut state: u16,
    mut pos: usize,
    len: usize,
    match_buf: *mut Match,
    match_cap: usize,
) -> (u16, usize, usize) {
    let mut mc: usize = 0;
    unsafe {
        while pos < len {
            if PREFIX != Prefix::None as u8 && state == initial {
                return (state, pos, mc);
            }
            let mt = *ml.add(*data.add(pos) as usize) as usize;
            let delta = (state as usize) << mt_log | mt;
            let entry = *table.add(delta);
            if entry == 0 {
                return (state, pos, mc);
            }
            let rel = entry >> 16;
            if rel > 0 && mc >= match_cap {
                return (state, pos, mc);
            }
            state = (entry & 0xFFFF) as u16;
            if rel > 0 {
                let end_off = *match_end_off.add(state as usize);
                let end = pos + 1 - end_off as usize;
                *match_buf.add(mc) = Match {
                    start: pos + 1 - rel as usize,
                    end,
                };
                mc += 1;
                state = initial;
                pos = end;
                continue;
            }
            pos += 1;
        }
        (state, pos, mc)
    }
}

pub(crate) fn bdfa_scan<const PREFIX: u8, const ISMATCH: bool>(
    bounded: &mut BDFA,
    b: &mut RegexBuilder,
    input: &[u8],
    matches: &mut Vec<Match>,
) -> Result<bool, Error> {
    let initial = bounded.initial;
    let mt_log = bounded.mt_log;
    let ml = bounded.minterms_lookup;
    let len = input.len();
    let mut state = initial;
    let mut pos: usize = 0;

    if PREFIX == Prefix::None as u8 {
        let data = input.as_ptr();
        if !ISMATCH {
            matches.reserve(2048);
        }
        let mut scratch: [Match; 1] = [Match { start: 0, end: 0 }];
        loop {
            if !ISMATCH && matches.len() == matches.capacity() {
                matches.reserve(matches.capacity().max(256));
            }
            let spare = if ISMATCH {
                1
            } else {
                matches.capacity() - matches.len()
            };
            let buf_ptr = if ISMATCH {
                scratch.as_mut_ptr()
            } else {
                unsafe { matches.as_mut_ptr().add(matches.len()) }
            };
            let table = bounded.table.as_ptr();
            let meo = bounded.match_end_off.as_ptr();
            let (s, p, mc) = bdfa_inner::<{ Prefix::None as u8 }>(
                table,
                ml.as_ptr(),
                data,
                mt_log,
                initial,
                meo,
                state,
                pos,
                len,
                buf_ptr,
                spare,
            );
            state = s;
            pos = p;
            if ISMATCH && mc > 0 {
                return Ok(true);
            }
            unsafe { matches.set_len(matches.len() + mc) };
            if pos >= len {
                break;
            }
            let mt = ml[input[pos] as usize] as usize;
            let entry = bounded.transition(b, state, mt)?;
            state = (entry & 0xFFFF) as u16;
            let rel = entry >> 16;
            if rel > 0 {
                if ISMATCH {
                    return Ok(true);
                }
                let end_off = bounded.match_end_off[state as usize];
                let end = pos + 1 - end_off as usize;
                matches.push(Match {
                    start: pos + 1 - rel as usize,
                    end,
                });
                state = initial;
                pos = end;
            } else {
                pos += 1;
            }
        }
    } else {
        'main: loop {
            if pos >= len {
                break;
            }

            if state == initial {
                let found = bounded.prefix.as_ref().unwrap().find_fwd(input, pos);
                match found {
                    Some(p) => {
                        if PREFIX == Prefix::Literal as u8 {
                            pos = p + bounded.prefix_len;
                            state = bounded.after_prefix;
                        } else {
                            pos = p;
                            for _ in 0..bounded.prefix_len {
                                if pos >= len {
                                    break;
                                }
                                let mt = ml[input[pos] as usize] as usize;
                                let delta = (state as usize) << mt_log | mt;
                                let entry = bounded.table[delta];
                                let entry = if entry != 0 {
                                    entry
                                } else {
                                    bounded.transition(b, state, mt)?
                                };
                                state = (entry & 0xFFFF) as u16;
                                if state == initial {
                                    break;
                                }
                                pos += 1;
                            }
                        }
                        let rel = bounded.match_rel[state as usize];
                        if rel > 0 {
                            if ISMATCH {
                                return Ok(true);
                            }
                            let end_off = bounded.match_end_off[state as usize];
                            let end = pos - end_off as usize + 1;
                            matches.push(Match {
                                start: pos - rel as usize + 1,
                                end,
                            });
                            state = initial;
                            pos = end;
                        }
                        continue 'main;
                    }
                    None => break 'main,
                }
            }

            unsafe {
                let table = bounded.table.as_ptr();
                let data = input.as_ptr();
                let ml_ptr = ml.as_ptr();
                let meo = bounded.match_end_off.as_ptr();

                while pos < len {
                    let mt = *ml_ptr.add(*data.add(pos) as usize) as usize;
                    let delta = (state as usize) << mt_log | mt;
                    let entry = *table.add(delta);
                    if entry == 0 {
                        break;
                    }
                    let rel = entry >> 16;
                    state = (entry & 0xFFFF) as u16;
                    if state == initial {
                        continue 'main;
                    }
                    if rel > 0 {
                        if ISMATCH {
                            return Ok(true);
                        }
                        let end_off = *meo.add(state as usize);
                        let end = pos + 1 - end_off as usize;
                        matches.push(Match {
                            start: pos + 1 - rel as usize,
                            end,
                        });
                        state = initial;
                        pos = end;
                        continue 'main;
                    }
                    pos += 1;
                }
            }

            if pos >= len {
                break;
            }
            let mt = ml[input[pos] as usize] as usize;
            let entry = bounded.transition(b, state, mt)?;
            state = (entry & 0xFFFF) as u16;
            let rel = entry >> 16;
            if rel > 0 {
                if ISMATCH {
                    return Ok(true);
                }
                let end_off = bounded.match_end_off[state as usize];
                let end = pos + 1 - end_off as usize;
                matches.push(Match {
                    start: pos + 1 - rel as usize,
                    end,
                });
                state = initial;
                pos = end;
            } else {
                pos += 1;
            }
        }
    }

    if state != initial {
        let node = bounded.states[state as usize];
        if node != NodeId::MISSING {
            let mut cands: Vec<(usize, usize)> = Vec::new();
            let mut cur = node;
            while cur.0 > NodeId::BOT.0 {
                let packed = b.get_extra(cur);
                let best = (packed >> 16) as usize;
                if best > 0 {
                    if ISMATCH {
                        return Ok(true);
                    }
                    let start = len - (packed & 0xFFFF) as usize;
                    cands.push((start, start + best));
                }
                cur = cur.right(b);
            }
            cands.sort_by(|a, c| a.0.cmp(&c.0).then(c.1.cmp(&a.1)));
            let mut last_end = matches.last().map_or(0, |m| m.end);
            for (start, end) in cands {
                if start < last_end {
                    continue;
                }
                matches.push(Match { start, end });
                last_end = end;
            }
        }
    }

    Ok(false)
}

impl Regex {
    pub(crate) fn find_all_fwd_bounded(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        debug_assert!(!input.is_empty());
        let crate::RegexInner {
            b,
            bounded,
            matches: matches_buf,
            ..
        } = &mut *self.inner.lock().unwrap();
        let bounded = bounded.as_mut().unwrap();
        matches_buf.clear();
        match &bounded.prefix {
            Some(p) if p.is_literal() => {
                bdfa_scan::<{ Prefix::Literal as u8 }, false>(bounded, b, input, matches_buf)?;
            }
            Some(_) => {
                bdfa_scan::<{ Prefix::Search as u8 }, false>(bounded, b, input, matches_buf)?;
            }
            None => {
                bdfa_scan::<{ Prefix::None as u8 }, false>(bounded, b, input, matches_buf)?;
            }
        }
        if self.always_nullable {
            let len = input.len();
            let mut filled = Vec::with_capacity(len + 1);
            let mut cursor = 0usize;
            for m in matches_buf.iter() {
                for pos in cursor..m.start {
                    filled.push(Match { start: pos, end: pos });
                }
                filled.push(*m);
                cursor = m.end.max(m.start + 1);
            }
            for pos in cursor..len {
                filled.push(Match { start: pos, end: pos });
            }
            if filled.last().map(|x| x.start) != Some(len) {
                filled.push(Match { start: len, end: len });
            }
            return Ok(filled);
        }
        Ok(matches_buf.clone())
    }
}
