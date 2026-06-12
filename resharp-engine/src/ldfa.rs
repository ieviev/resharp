use std::collections::{HashMap, HashSet};

use rustc_hash::FxHashMap;

use resharp_algebra::nulls::{
    EID_NONE, NullState, Nullability, NullsId, collect_nulls,
};
pub(crate) use resharp_algebra::nulls::has_any_null;
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{NodeId, RegexBuilder};

use crate::accel::MintermSearchValue;
use crate::minterms::{collect_sets, collect_tregex_leaves, transition_term, PartitionTree};
use crate::scan::{
    collect_rev, collect_rev_complex, fwd_update, register_state, scan_fwd, scan_fwd_first_null,
    scan_fwd_verify, ScanTables,
};
pub(crate) use crate::scan::{collect_max_fwd, collect_max_rev};
use crate::{Error, Match};

pub const NO_MATCH: usize = usize::MAX;
pub const DFA_MISSING: u16 = 0;
pub const DFA_DEAD: u16 = 1;
pub const DFA_INITIAL: u16 = 2;

#[inline(always)]
fn as_sid(s: u32) -> Result<u16, Error> {
    u16::try_from(s).map_err(|_| Error::CapacityExceeded)
}

#[inline(always)]
fn found(pos: usize) -> Option<usize> {
    (pos != NO_MATCH).then_some(pos)
}

#[inline(always)]
fn center_or_end(at_end: bool) -> Nullability {
    if at_end { Nullability::END } else { Nullability::CENTER }
}

#[inline(always)]
pub(crate) fn dfa_delta(state: u32, mt: u32, mt_log: u32) -> usize {
    (state << mt_log | mt) as usize
}

const SKIP_FREQ_THRESHOLD: u32 = 75_000;

fn skip_is_profitable(bytes: &[u8]) -> bool {
    if bytes.len() >= 256 {
        return false;
    }
    let freq_sum: u32 = bytes
        .iter()
        .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u32)
        .sum();
    if freq_sum < SKIP_FREQ_THRESHOLD {
        return true;
    }
    if bytes.len() > 128 {
        let complement_freq: u32 = (0u32..256)
            .filter(|&b| !bytes.contains(&(b as u8)))
            .map(|b| crate::simd::BYTE_FREQ[b as usize] as u32)
            .sum();
        return complement_freq < SKIP_FREQ_THRESHOLD;
    }
    false
}
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct LDFA {
    pub pruned: u16,
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub prune_memo: FxHashMap<NodeId, NodeId>,
    pub begin_table: Vec<u16>,
    pub center_table: Vec<u16>,
    pub effects_id: Vec<u16>,
    pub effects: Vec<Vec<NullState>>,
    pub center_effect_id: Vec<u16>,
    pub mt_log: u32,
    #[cfg_attr(feature = "serialize", serde(with = "crate::dump::array256"))]
    pub mt_lookup: [u8; 256],
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub minterms: Vec<TSetId>,
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub state_nodes: Vec<NodeId>,
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub node_to_state: HashMap<NodeId, u16>,
    pub skip_ids: Vec<u8>,
    pub skip_searchers: Vec<MintermSearchValue>,
    pub prefix_skip: Option<crate::accel::RevTeddySearch>,
    pub max_capacity: usize,
    pub is_forward: bool,
    pub has_anchors: bool,
    pub initial_nullability: Nullability,
}

impl LDFA {
    pub fn new_rev(
        b: &mut RegexBuilder,
        initial: NodeId,
        max_capacity: usize,
    ) -> Result<LDFA, Error> {
        Self::new_inner(b, initial, max_capacity, false)
    }

    pub fn new_fwd(
        b: &mut RegexBuilder,
        initial: NodeId,
        max_capacity: usize,
    ) -> Result<LDFA, Error> {
        Self::new_inner(b, initial, max_capacity, true)
    }

    fn new_inner(
        b: &mut RegexBuilder,
        initial: NodeId,
        max_capacity: usize,
        is_forward: bool,
    ) -> Result<LDFA, Error> {
        let sets = collect_sets(b, initial);
        let minterms = PartitionTree::generate_minterms(sets, b.solver());
        let u8_lookup = PartitionTree::minterms_lookup(&minterms, b.solver());
        let max_capacity = max_capacity.min(65535);

        // state 0 = uncomputed, state 1 = dead
        let mut state_nodes: Vec<NodeId> = vec![NodeId::MISSING, NodeId::BOT];
        let mut node_to_state: HashMap<NodeId, u16> = HashMap::new();
        node_to_state.insert(NodeId::BOT, DFA_DEAD);

        let mut effects_id: Vec<u16> = vec![0u16; 2]; // slots 0,1 (MISSING, DEAD)
        let mut center_effect_id: Vec<u16> = vec![EID_NONE as u16; 2];
        let mut effects: Vec<Vec<NullState>> = Vec::new();

        let mut prune_memo: FxHashMap<NodeId, NodeId> = FxHashMap::default();

        // state 2
        register_state(
            &mut state_nodes,
            &mut node_to_state,
            &mut effects_id,
            &mut center_effect_id,
            &mut effects,
            b,
            initial,
            true,
        );

        // state 3
        let initial_pruned = b.prune_begin(initial);

        let pruned_sid = register_state(
            &mut state_nodes,
            &mut node_to_state,
            &mut effects_id,
            &mut center_effect_id,
            &mut effects,
            b,
            initial_pruned,
            false,
        );

        let der0 = b.der(initial, Nullability::BEGIN)?;
        let mut begin_table = vec![DFA_DEAD; minterms.len()];
        for (idx, mt) in minterms.iter().enumerate() {
            let mut t = transition_term(b, der0, *mt);
            if is_forward {
                t = b.prune_fwd(t, &mut prune_memo);
            } else {
                t = b.prune_rev(t, &mut prune_memo);
            }
            let sid = register_state(
                &mut state_nodes,
                &mut node_to_state,
                &mut effects_id,
                &mut center_effect_id,
                &mut effects,
                b,
                t,
                false,
            );
            if state_nodes.len() > max_capacity {
                return Err(Error::CapacityExceeded);
            }
            begin_table[idx] = sid;
        }
        let num_minterms = minterms.len() as u32;
        let mt_log = (num_minterms as usize).next_power_of_two().trailing_zeros();
        let stride = 1usize << mt_log;
        let center_table_size = state_nodes.len() * stride;
        let mut center_table = vec![DFA_MISSING; center_table_size];
        for mt_idx in 0..minterms.len() {
            center_table[dfa_delta(DFA_DEAD as u32, mt_idx as u32, mt_log)] = DFA_DEAD;
        }

        while effects.len() < b.nulls_count() {
            effects.push(b.nulls_entry_vec(effects.len() as u32));
        }
        let skip_ids = vec![0u8; state_nodes.len()];
        let mut dfa = LDFA {
            pruned: pruned_sid,
            prune_memo,
            begin_table,
            center_table,
            effects_id,
            effects,
            center_effect_id,
            mt_log,
            mt_lookup: u8_lookup,
            minterms,
            state_nodes,
            node_to_state,
            skip_ids,
            skip_searchers: Vec::new(),
            prefix_skip: None,
            max_capacity,
            is_forward,
            has_anchors: b.contains_anchors(initial),
            initial_nullability: b.nullability(initial),
        };
        dfa.ensure_capacity(pruned_sid);

        Ok(dfa)
    }

    #[inline(always)]
    pub fn dfa_delta(&self, state_id: u16, mt: u32) -> usize {
        dfa_delta(state_id as u32, mt, self.mt_log)
    }

    pub fn ensure_capacity(&mut self, state_id: u16) {
        let cap = state_id as usize + 1;
        if cap > self.effects_id.len() {
            let new_len = self.effects_id.len().max(4) * 2;
            let new_len = new_len.max(cap);
            self.effects_id.resize(new_len, 0u16);
            self.center_effect_id.resize(new_len, EID_NONE as u16);
        }
        let stride = 1usize << self.mt_log;
        let needed = cap * stride;
        if needed > self.center_table.len() {
            let new_len = self.center_table.len().max(4) * 2;
            let new_len = new_len.max(needed);
            self.center_table.resize(new_len, DFA_MISSING);
        }
        if cap > self.skip_ids.len() {
            let new_len = self.skip_ids.len().max(4) * 2;
            let new_len = new_len.max(cap);
            self.skip_ids.resize(new_len, 0u8);
        }
        debug_assert!(self.center_table.len() >= cap * stride);
    }

    pub fn get_or_register(&mut self, b: &mut RegexBuilder, node: NodeId) -> u16 {
        let sid = register_state(
            &mut self.state_nodes,
            &mut self.node_to_state,
            &mut self.effects_id,
            &mut self.center_effect_id,
            &mut self.effects,
            b,
            node,
            false,
        );
        self.ensure_capacity(sid);
        sid
    }

    fn try_create(&mut self, b: &mut RegexBuilder, curr: u32) {
        if let Ok(sid) = u16::try_from(curr) {
            self.create_state(b, sid).ok();
        }
    }

    #[inline(always)]
    pub fn lazy_transition(
        &mut self,
        b: &mut RegexBuilder,
        state_id: u16,
        minterm_idx: u32,
    ) -> Result<u16, Error> {
        let delta = self.dfa_delta(state_id, minterm_idx);
        if self.center_table[delta] != DFA_MISSING {
            return Ok(self.center_table[delta]);
        }
        self.lazy_transition_slow(b, state_id, minterm_idx)
    }

    #[cold]
    #[inline(never)]
    fn lazy_transition_slow(
        &mut self,
        b: &mut RegexBuilder,
        state_id: u16,
        minterm_idx: u32,
    ) -> Result<u16, Error> {
        if state_id == DFA_DEAD {
            return Ok(DFA_DEAD);
        }
        self.ensure_capacity(state_id);
        self.create_state(b, state_id)?;
        let delta = self.dfa_delta(state_id, minterm_idx);
        Ok(self.center_table[delta])
    }

    #[allow(dead_code)]
    pub fn precompile(&mut self, b: &mut RegexBuilder, threshold: usize) -> bool {
        use std::collections::VecDeque;
        let mut worklist: VecDeque<u16> = VecDeque::new();
        let mut visited = HashSet::new();

        for &sid in &self.begin_table {
            if sid > DFA_DEAD {
                worklist.push_back(sid);
            }
        }

        let stride = 1usize << self.mt_log;
        while let Some(sid) = worklist.pop_front() {
            if !visited.insert(sid) {
                continue;
            }
            if visited.len() > threshold {
                return false;
            }
            self.ensure_capacity(sid);
            if self.create_state(b, sid).is_err() {
                return false;
            }
            let base = (sid as usize) * stride;
            for mt_idx in 0..self.minterms.len() {
                let next_sid = self.center_table[base | mt_idx];
                if next_sid > DFA_DEAD && !visited.contains(&next_sid) {
                    worklist.push_back(next_sid);
                }
            }
        }

        true
    }

    pub fn has_nonnullable_cycle(&self, b: &mut RegexBuilder, budget: usize) -> bool {
        use std::collections::VecDeque;

        let mut seed_nodes: Vec<NodeId> = Vec::new();
        for &sid in &self.begin_table {
            if sid > DFA_DEAD {
                let node = self.state_nodes[sid as usize];
                if node.0 > NodeId::BOT.0 {
                    seed_nodes.push(node);
                }
            }
        }

        let mut visited = HashSet::new();
        let mut worklist: VecDeque<NodeId> = VecDeque::new();
        let mut successors: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

        for &node in &seed_nodes {
            if visited.insert(node) {
                worklist.push_back(node);
            }
        }

        while let Some(node) = worklist.pop_front() {
            if visited.len() > budget {
                return true;
            }
            let sder = match b.der(node, Nullability::CENTER) {
                Ok(d) => d,
                Err(_) => return true,
            };
            let mut leaves = Vec::new();
            collect_tregex_leaves(b, sder, &mut leaves);
            let mut succs = Vec::new();
            for next in leaves {
                if next.0 > NodeId::BOT.0 {
                    succs.push(next);
                    if visited.insert(next) {
                        worklist.push_back(next);
                    }
                }
            }
            successors.insert(node, succs);
        }

        let nonnull: HashSet<NodeId> = visited
            .iter()
            .copied()
            .filter(|&node| b.get_nulls_id(node) == NullsId::EMPTY)
            .collect();

        if nonnull.is_empty() {
            return false;
        }

        let mut color: HashMap<NodeId, u8> = HashMap::new();
        let mut stack: Vec<(NodeId, usize)> = Vec::new();
        for &start in &nonnull {
            if color.get(&start).copied().unwrap_or(0) != 0 {
                continue;
            }
            stack.push((start, 0));
            color.insert(start, 1);
            while let Some((node, idx)) = stack.last_mut() {
                let succs = successors.get(node).map(|v| v.as_slice()).unwrap_or(&[]);
                if *idx >= succs.len() {
                    color.insert(*node, 2);
                    stack.pop();
                    continue;
                }
                let next = succs[*idx];
                *idx += 1;
                if !nonnull.contains(&next) {
                    continue;
                }
                match color.get(&next).copied().unwrap_or(0) {
                    1 => return true,
                    0 => {
                        color.insert(next, 1);
                        stack.push((next, 0));
                    }
                    _ => {}
                }
            }
        }
        false
    }

    pub(crate) fn create_state(
        &mut self,
        b: &mut RegexBuilder,
        state_id: u16,
    ) -> Result<(), Error> {
        let node = self.state_nodes[state_id as usize];
        if node == NodeId::MISSING {
            return Ok(());
        }
        let sder = b.der(node, Nullability::CENTER).map_err(Error::Algebra)?;
        for mt_idx in 0..self.minterms.len() {
            let delta = self.dfa_delta(state_id, mt_idx as u32);
            if self.center_table[delta] != DFA_MISSING {
                continue;
            }
            let mt = self.minterms[mt_idx];
            let mut next_node = transition_term(b, sder, mt);
            if self.is_forward {
                next_node = b.prune_fwd(next_node, &mut self.prune_memo);
            } else {
                next_node = b.prune_rev(next_node, &mut self.prune_memo);
            }
            if self.state_nodes.len() >= self.max_capacity {
                return Err(Error::CapacityExceeded);
            }
            let next_sid = self.get_or_register(b, next_node);
            self.ensure_capacity(next_sid);
            let delta = self.dfa_delta(state_id, mt_idx as u32);
            self.center_table[delta] = next_sid;
        }
        if crate::simd::has_simd() {
            self.try_build_skip_simd(b, state_id.into());
        }
        Ok(())
    }

    fn try_build_skip_simd(&mut self, b: &mut RegexBuilder, state: usize) {
        if self.skip_ids[state] != 0 {
            return;
        }
        let node = self.state_nodes[state];
        if node == NodeId::MISSING || node == NodeId::BOT {
            return;
        }

        let sder = match b.der(node, Nullability::CENTER) {
            Ok(d) => d,
            Err(_) => return,
        };

        let mut notany = TSetId::EMPTY;
        let mut stack = vec![(sder, TSetId::FULL)];
        b.iter_sat(
            &mut stack,
            &mut (|b, next, set| {
                if next == node {
                    notany = b.solver().or_id(notany, set)
                }
            }),
        );
        let any = b.solver().not_id(notany);

        let bytes = b.solver().collect_bytes(any);
        if bytes.len() == 256 {
            return;
        }
        if bytes.is_empty() {
            self.skip_ids[state] = self.get_or_create_skip_all();
            return;
        }
        if bytes.len() <= 3 {
            self.skip_ids[state] = self.get_or_create_skip_exact(bytes);
            return;
        }
        if let Some(sid) = self.try_build_range_skip(&bytes) {
            self.skip_ids[state] = sid;
        }
    }

    fn try_build_range_skip(&mut self, bytes: &[u8]) -> Option<u8> {
        let tset = crate::accel::TSet::from_bytes(bytes);
        let ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
        if ranges.is_empty() || ranges.len() > 4 {
            return None;
        }
        if !skip_is_profitable(bytes) {
            return None;
        }
        if bytes.len() > 128 && (256 - bytes.len()) < 16 {
            return None;
        }
        Some(self.get_or_create_skip_range(ranges))
    }

    pub(crate) fn ensure_dead_skip(&mut self) {
        if self.skip_ids.len() > DFA_DEAD as usize && self.skip_ids[DFA_DEAD as usize] == 0 {
            self.skip_ids[DFA_DEAD as usize] = self.get_or_create_skip_all();
        }
    }

    pub(crate) fn ensure_pruned_skip(&mut self) {
        if self.prefix_skip.is_some() {
            let p = self.pruned as usize;
            if p < self.skip_ids.len() && self.skip_ids[p] == 0 {
                self.skip_ids[p] = self.get_or_create_skip_all();
            }
        }
    }

    fn get_or_create_skip_all(&mut self) -> u8 {
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if matches!(s, MintermSearchValue::All) {
                return (i + 1) as u8;
            }
        }
        self.skip_searchers.push(MintermSearchValue::All);
        self.skip_searchers.len() as u8
    }

    fn get_or_create_skip_exact(&mut self, mut bytes: Vec<u8>) -> u8 {
        bytes.sort();
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if let MintermSearchValue::Exact(ref e) = s {
                if e.bytes() == bytes {
                    return (i + 1) as u8;
                }
            }
        }
        self.skip_searchers
            .push(MintermSearchValue::Exact(crate::simd::RevSearchBytes::new(
                bytes,
            )));
        self.skip_searchers.len() as u8
    }

    fn get_or_create_skip_range(&mut self, mut ranges: Vec<(u8, u8)>) -> u8 {
        ranges.sort();
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if let MintermSearchValue::Range(ref r) = s {
                if r.ranges() == ranges {
                    return (i + 1) as u8;
                }
            }
        }
        self.skip_searchers.push(MintermSearchValue::Range(
            crate::simd::RevSearchRanges::new(ranges),
        ));
        self.skip_searchers.len() as u8
    }

    #[inline(always)]
    fn scan_tables(&self, data: &[u8]) -> ScanTables {
        ScanTables {
            center_table: self.center_table.as_ptr(),
            effects: self.effects.as_ptr(),
            center_effect_id: self.center_effect_id.as_ptr(),
            data: data.as_ptr(),
            minterms_lookup: self.mt_lookup.as_ptr(),
            mt_log: self.mt_log,
        }
    }

    #[inline(always)]
    fn dispatch_scan_fwd(
        &self,
        tables: &ScanTables,
        curr: u32,
        pos: usize,
        end: usize,
        max_end: usize,
    ) -> (u32, usize, usize, bool) {
        if self.can_skip() {
            scan_fwd::<true>(
                tables,
                self.effects_id.as_ptr(),
                &self.skip_ids,
                &self.skip_searchers,
                curr,
                pos,
                end,
                max_end,
            )
        } else {
            scan_fwd::<false>(
                tables,
                self.effects_id.as_ptr(),
                &[],
                &[],
                curr,
                pos,
                end,
                max_end,
            )
        }
    }

    #[inline(always)]
    fn dispatch_scan_fwd_verify(
        &self,
        tables: &ScanTables,
        curr: u32,
        pos: usize,
        end: usize,
        max_end: usize,
    ) -> (u32, usize, usize, bool) {
        if self.can_skip() {
            scan_fwd_verify::<true>(
                tables,
                self.effects_id.as_ptr(),
                &self.skip_ids,
                &self.skip_searchers,
                curr,
                pos,
                end,
                max_end,
            )
        } else {
            scan_fwd_verify::<false>(
                tables,
                self.effects_id.as_ptr(),
                &[],
                &[],
                curr,
                pos,
                end,
                max_end,
            )
        }
    }

    #[inline(always)]
    fn dispatch_collect_rev<const EARLY_EXIT: bool, const INITIAL_SKIP: bool>(
        &self,
        tables: &ScanTables,
        prefix_ptr: *const crate::accel::RevTeddySearch,
        curr: u32,
        pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> (u32, usize, bool) {
        if self.can_skip() {
            collect_rev::<EARLY_EXIT, true, INITIAL_SKIP>(
                tables,
                &self.skip_ids,
                &self.skip_searchers,
                prefix_ptr,
                curr,
                pos,
                data,
                nulls,
                self.pruned as u32,
            )
        } else {
            collect_rev::<EARLY_EXIT, false, false>(
                tables,
                &[],
                &[],
                std::ptr::null(),
                curr,
                pos,
                data,
                nulls,
                0,
            )
        }
    }

    /// attempt to scan from this pos: may return non-match
    pub(crate) fn scan_fwd_optional(
        &mut self,
        b: &mut RegexBuilder,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<Option<usize>, Error> {
        let mut max_end: usize = if self.initial_nullability.has(if pos_begin == 0 {
            Nullability::BEGIN
        } else {
            Nullability::CENTER
        }) {
            pos_begin
        } else {
            NO_MATCH
        };

        let mt = self.mt_lookup[data[pos_begin] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD as u32 {
            return Ok(found(max_end));
        }
        let end = data.len();
        let mut pos = pos_begin + 1;

        collect_max_fwd(
            &self.effects_id,
            &self.effects,
            curr,
            pos,
            center_or_end(pos == end),
            &mut max_end,
        );

        if pos == end {
            return Ok(found(max_end));
        }

        loop {
            let tables = self.scan_tables(data);
            let (state, new_pos, new_max, cache_miss) =
                self.dispatch_scan_fwd(&tables, curr, pos, end, max_end);
            max_end = new_max;

            if !cache_miss {
                break;
            }

            let sid = as_sid(state)?;
            self.create_state(b, sid)?;

            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            curr = self.center_table[self.dfa_delta(sid, mt)] as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.create_state(b, as_sid(curr)?)?;

            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                center_or_end(pos == end),
                &mut max_end,
            );

            if pos == end {
                break;
            }
        }

        Ok(found(max_end))
    }

    /// scan_fwd_all is guaranteed to find a match, given a valid, pre-checked start position
    #[inline(never)]
    pub fn scan_fwd_all(
        &mut self,
        b: &mut RegexBuilder,
        nulls: &[usize],
        data: &[u8],
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        if nulls.is_empty() {
            return Ok(());
        }
        let data_end = data.len();

        let mut next_start = 0usize;

        let mut l_pos: usize;
        let mut i = nulls.len();

        if nulls[nulls.len() - 1] == 0 {
            i = i - 1;
            l_pos = 0;
            let mut l_max_end: usize = if self.initial_nullability.has(Nullability::BEGIN) {
                0
            } else {
                NO_MATCH
            };

            // manually take first step
            let mt = self.mt_lookup[data[l_pos] as usize] as u32;
            let mut l_state = self.begin_table[mt as usize] as _;
            l_pos = 1;

            loop {
                let tables = self.scan_tables(data);
                let (state, new_pos, new_max, cache_miss) =
                    self.dispatch_scan_fwd(&tables, l_state, l_pos, data_end, l_max_end);
                l_max_end = new_max;
                if cache_miss {
                    let (flush_state, flush_pos) = if new_pos >= data_end {
                        (state as u32, new_pos)
                    } else {
                        let mt = self.mt_lookup[data[new_pos] as usize] as u32;
                        let new_state = self.lazy_transition(b, as_sid(state)?, mt)? as u32;
                        l_pos = new_pos + 1;
                        l_state = new_state;
                        if l_pos != data_end {
                            continue;
                        }
                        (new_state, l_pos)
                    };
                    l_max_end = unsafe {
                        fwd_update::<true>(
                            self.effects_id.as_ptr(),
                            self.effects.as_ptr(),
                            flush_state as u32,
                            flush_pos,
                            l_max_end,
                        )
                    };
                }
                debug_assert_ne!(
                    NO_MATCH, l_max_end,
                    "find_all: forward scan found no end for reverse-proposed start 0"
                );
                if l_max_end != NO_MATCH {
                    matches.push(Match {
                        start: 0,
                        end: l_max_end,
                    });
                    next_start = l_max_end;
                }
                break;
            }
        }

        while i != 0 {
            i = i - 1;
            l_pos = nulls[i];
            if l_pos < next_start {
                continue;
            }
            if l_pos == data_end {
                matches.push(Match {
                    start: l_pos,
                    end: l_pos,
                });
                break;
            }

            let mut l_state = DFA_INITIAL as u32;
            let mut l_max_end = NO_MATCH;
            loop {
                let tables = self.scan_tables(data);
                let (state, new_pos, new_max, cache_miss) =
                    self.dispatch_scan_fwd(&tables, l_state, l_pos, data_end, l_max_end);
                l_max_end = new_max;
                if cache_miss {
                    debug_assert!(new_pos >= l_pos, "backwards");
                    let mt = self.mt_lookup[data[new_pos] as usize] as u32;
                    let next_state = self.lazy_transition(b, as_sid(state)?, mt)? as u32;
                    l_pos = new_pos + 1;
                    l_state = next_state;
                    if l_pos != data_end {
                        continue;
                    }
                    l_max_end = unsafe {
                        fwd_update::<true>(
                            self.effects_id.as_ptr(),
                            self.effects.as_ptr(),
                            l_state,
                            l_pos,
                            l_max_end,
                        )
                    };
                    debug_assert_ne!(
                        NO_MATCH, l_max_end,
                        "find_all: forward scan found no end for reverse-proposed start"
                    );
                    if l_max_end != NO_MATCH {
                        matches.push(Match {
                            start: nulls[i],
                            end: l_max_end,
                        });
                        next_start = l_max_end;
                    }
                    break;
                }
                debug_assert!(
                    l_max_end >= nulls[i],
                    "unexpected end {} > {}",
                    l_max_end,
                    nulls[i]
                );
                debug_assert_ne!(
                    NO_MATCH, l_max_end,
                    "find_all: forward scan found no end for reverse-proposed start"
                );
                if l_max_end != NO_MATCH {
                    matches.push(Match {
                        start: nulls[i],
                        end: l_max_end,
                    });
                    next_start = l_max_end;
                }
                break;
            }
        }

        Ok(())
    }

    pub fn walk_input(
        &mut self,
        b: &mut RegexBuilder,
        pos: usize,
        len: usize,
        data: &[u8],
    ) -> Result<u32, Error> {
        let (mut state, start_i) = if pos == 0 {
            let mt = self.mt_lookup[data[pos] as usize];
            (self.begin_table[mt as usize], 1usize)
        } else {
            (self.pruned, 0usize)
        };
        if state <= DFA_DEAD {
            return Ok(0);
        }
        for i in start_i..len {
            let mt = self.mt_lookup[data[pos + i] as usize] as u32;
            state = self.lazy_transition(b, state, mt)?;
            if state <= DFA_DEAD {
                return Ok(0);
            }
        }
        Ok(state as u32)
    }

    /// Advance the DFA from `state`/`pos_begin` until a CENTER-nullable state or end of input.
    /// Returns `(state, pos, hit_null)`: `hit_null=true` means CENTER-nullable (verify with
    /// `has_any_null`); `hit_null=false` means either DFA_DEAD or input exhausted.
    pub(crate) fn scan_fwd_first_null_from(
        &mut self,
        b: &mut RegexBuilder,
        state: u32,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<(u32, usize, bool), Error> {
        if state <= DFA_DEAD as u32 {
            return Ok((state, pos_begin, false));
        }
        if has_any_null(&self.effects_id, &self.effects, state, Nullability::CENTER) {
            return Ok((state, pos_begin, true));
        }
        let end = data.len();
        let mut pos = pos_begin;
        let mut curr = state;
        if pos >= end {
            return Ok((curr, pos, false));
        }
        loop {
            let tables = self.scan_tables(data);
            let (state_out, new_pos, hit_null, cache_miss) =
                self.dispatch_scan_fwd_first_null(&tables, curr, pos, end);
            if hit_null {
                return Ok((state_out, new_pos, true));
            }
            if !cache_miss {
                return Ok((state_out, new_pos, false));
            }
            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, as_sid(state_out)?, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                return Ok((curr, pos, false));
            }
            self.try_create(b, curr);
            if has_any_null(&self.effects_id, &self.effects, curr, Nullability::CENTER) {
                return Ok((curr, pos, true));
            }
            if pos >= end {
                return Ok((curr, pos, false));
            }
        }
    }

    #[inline(always)]
    fn dispatch_scan_fwd_first_null(
        &self,
        tables: &ScanTables,
        curr: u32,
        pos: usize,
        end: usize,
    ) -> (u32, usize, bool, bool) {
        if self.can_skip() {
            scan_fwd_first_null::<true>(
                tables,
                self.effects_id.as_ptr(),
                &self.skip_ids,
                &self.skip_searchers,
                curr,
                pos,
                end,
            )
        } else {
            scan_fwd_first_null::<false>(tables, self.effects_id.as_ptr(), &[], &[], curr, pos, end)
        }
    }

    /// scan forward from a state and pos
    pub fn scan_fwd_from(
        &mut self,
        b: &mut RegexBuilder,
        state: u32,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<Option<usize>, Error> {
        if state <= DFA_DEAD as u32 {
            return Ok(None);
        }
        let end = data.len();
        let mut pos = pos_begin;
        let mut curr = state;
        let mut max_end: usize = NO_MATCH;

        collect_max_fwd(
            &self.effects_id,
            &self.effects,
            curr,
            pos,
            center_or_end(pos >= end),
            &mut max_end,
        );

        if pos >= end {
            return Ok(found(max_end));
        }

        loop {
            let tables = self.scan_tables(data);
            let (state_out, new_pos, new_max, cache_miss) =
                self.dispatch_scan_fwd_verify(&tables, curr, pos, end, max_end);
            max_end = new_max;

            if !cache_miss {
                break;
            }

            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, as_sid(state_out)?, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.try_create(b, curr);

            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                center_or_end(pos >= end),
                &mut max_end,
            );

            if pos >= end {
                break;
            }
        }

        Ok(found(max_end))
    }

    pub fn scan_rev_from(
        &mut self,
        b: &mut RegexBuilder,
        end: usize,
        begin: usize,
        data: &[u8],
    ) -> Result<Option<usize>, Error> {
        if end == 0 || end > data.len() || end <= begin {
            return Ok(None);
        }
        let start_pos = end - 1;
        let mt = self.mt_lookup[data[start_pos] as usize] as u32;
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD as u32 {
            return Ok(None);
        }

        let mut min_start = NO_MATCH;
        collect_max_rev(
            &self.effects_id,
            &self.effects,
            curr,
            start_pos,
            center_or_end(start_pos == begin),
            &mut min_start,
        );

        let mut pos = start_pos;
        while pos > begin {
            pos -= 1;
            let mt = self.mt_lookup[data[pos] as usize] as u32;
            let delta = dfa_delta(curr, mt, self.mt_log);
            let next = self.center_table[delta];
            if next == DFA_MISSING {
                curr = self.lazy_transition(b, as_sid(curr)?, mt)? as u32;
                self.try_create(b, curr);
            } else {
                curr = next as u32;
            }
            if curr <= DFA_DEAD as u32 {
                break;
            }
            collect_max_rev(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                center_or_end(pos == begin),
                &mut min_start,
            );
        }

        Ok(found(min_start))
    }

    pub(crate) fn can_skip(&self) -> bool {
        self.prefix_skip.is_some() || !self.skip_searchers.is_empty()
    }

    pub fn collect_rev(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        self.collect_rev_inner::<false>(b, start_pos, data, nulls)
    }

    pub fn collect_rev_first(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        self.collect_rev_inner::<true>(b, start_pos, data, nulls)
    }

    #[cold]
    pub fn len_1_rev(&mut self, curr: u32, nulls: &mut Vec<usize>) -> Result<(), Error> {
        collect_nulls(
            &self.effects_id,
            &self.effects,
            curr,
            0,
            Nullability::END,
            nulls,
        );
        Ok(())
    }

    /// whole input rev to pos 0
    fn collect_rev_inner<const EARLY_EXIT: bool>(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        #[cfg(feature = "debug")]
        {
            let node = self.state_nodes[DFA_INITIAL as usize];
            let eid = self.effects_id[DFA_INITIAL as usize];
            eprintln!("[rev0] eid={eid} node={:.80}", b.pp(node));
        }

        let mut curr = self.begin_table[self.mt_lookup[data[start_pos] as usize] as usize] as u32;
        if data.len() == 1 {
            return self.len_1_rev(curr, nulls);
        }
        collect_nulls(
            &self.effects_id,
            &self.effects,
            curr,
            start_pos,
            Nullability::CENTER,
            nulls,
        );

        #[cfg(feature = "debug")]
        {
            let node = self.state_nodes[curr as usize];
            let eid = self.effects_id[curr as usize];
            eprintln!(
                "[rev pos={start_pos}] state={curr} eid={eid} node={:.80} nulls={nulls:?}",
                b.pp(node)
            );
        }

        if let Some(preskip) = self.prefix_skip.as_ref() {
            return self.collect_rev_prefix::<EARLY_EXIT>(
                b,
                preskip as *const crate::accel::RevTeddySearch,
                start_pos,
                curr,
                data,
                nulls,
            );
        }

        if EARLY_EXIT && !nulls.is_empty() {
            return Ok(());
        }

        let mut pos = start_pos;

        loop {
            let tables = self.scan_tables(data);
            let (state, new_pos, cache_miss) = self.dispatch_collect_rev::<EARLY_EXIT, false>(
                &tables,
                std::ptr::null(),
                curr,
                pos,
                data,
                nulls,
            );

            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }

            if !cache_miss {
                self.handle_rev_end(b, as_sid(state)?, data, nulls)?;
                break;
            }

            let sid = as_sid(state)?;
            self.create_state(b, sid)?;

            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            let delta = self.dfa_delta(sid, mt);
            curr = self.center_table[delta] as u32;
            pos = new_pos;

            if curr <= DFA_DEAD as u32 {
                break;
            }

            collect_nulls(&self.effects_id, &self.effects, curr, pos, center_or_end(pos == 0), nulls);
            #[cfg(feature = "debug")]
            {
                let node = self.state_nodes[curr as usize];
                let eid = self.effects_id[curr as usize];
                eprintln!(
                    "[rev pos={pos}] state={curr} eid={eid} node={} nulls={nulls:?}",
                    b.pp(node)
                );
            }
            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }
        }

        Ok(())
    }

    fn collect_rev_prefix<const EARLY_EXIT: bool>(
        &mut self,
        b: &mut RegexBuilder,
        prefix_ptr: *const crate::accel::RevTeddySearch,
        start_pos: usize,
        start_state: u32,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        let mut curr = start_state;
        let mut pos = start_pos;
        if cfg!(feature = "debug") {
            // eprintln!("  [rev_prefix] after_collect_nulls nulls={:?}", nulls);
        }
        if EARLY_EXIT && !nulls.is_empty() {
            return Ok(());
        }

        if !self.has_anchors {
            pos = data.len();
            curr = self.pruned as u32;
        }

        loop {
            let tables = self.scan_tables(data);
            let (state, new_pos, cache_miss) = self.dispatch_collect_rev::<EARLY_EXIT, true>(
                &tables, prefix_ptr, curr, pos, data, nulls,
            );

            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }

            if cache_miss {
                let sid = as_sid(state)?;
                self.create_state(b, sid)?;
                curr = sid as u32;
                pos = new_pos + 1;
                continue;
            } else {
                self.handle_rev_end(b, as_sid(state)?, data, nulls)?;
                break;
            }
        }

        Ok(())
    }

    #[cfg_attr(not(feature = "debug"), allow(unused_variables))]
    fn handle_rev_end(
        &mut self,
        b: &mut RegexBuilder,
        sid: u16,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        let mt = self.mt_lookup[data[0] as usize] as u32;
        #[cfg(feature = "debug")]
        {
            let delta = self.dfa_delta(sid, mt);
            let cached = if delta < self.center_table.len() {
                self.center_table[delta]
            } else {
                DFA_MISSING
            };
            eprintln!("[handle_rev_end] sid={sid} mt={mt} delta={delta} center_cached={cached}");
        }
        let new_state = self.lazy_transition(b, sid, mt)?;
        #[cfg(feature = "debug")]
        {
            let node = self.state_nodes[sid as usize];
            let eid = self.effects_id[sid as usize];
            eprintln!(
                "[pre end] sid={sid} new_state={new_state} eid={eid} node={:.80} nulls={nulls:?}",
                b.pp(node)
            );
        }
        #[cfg(feature = "debug")]
        {
            let node = self.state_nodes[new_state as usize];
            let eid = self.effects_id[new_state as usize];
            eprintln!(
                "[rev end] state={new_state} eid={eid} node={:.80} nulls={nulls:?}",
                b.pp(node)
            );
        }

        let effect = self.effects_id[new_state as usize] as u32;
        collect_rev_complex(self.effects.as_ptr(), effect, 0, Nullability::END, nulls);
        Ok(())
    }
}

