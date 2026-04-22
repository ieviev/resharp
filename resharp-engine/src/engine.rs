use std::collections::{HashMap, HashSet};

use rustc_hash::FxHashMap;

use resharp_algebra::nulls::{NullState, Nullability, NullsId};
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder, TRegex, TRegexId};

use crate::accel::MintermSearchValue;
use crate::prefix::{calc_potential_start, calc_prefix_sets_inner};
use crate::{Error, Match};

pub const NO_MATCH: usize = usize::MAX;
pub const DFA_MISSING: u16 = 0;
pub const DFA_DEAD: u16 = 1;
pub const DFA_INITIAL: u16 = 2;

struct PartitionTree {
    sets: Vec<TSetId>,
    lefts: Vec<u32>,
    rights: Vec<u32>,
}

impl PartitionTree {
    const NO_CHILD: u32 = u32::MAX;

    fn new(set: TSetId) -> PartitionTree {
        PartitionTree {
            sets: vec![set],
            lefts: vec![Self::NO_CHILD],
            rights: vec![Self::NO_CHILD],
        }
    }

    fn push(&mut self, set: TSetId) -> u32 {
        let idx = self.sets.len() as u32;
        self.sets.push(set);
        self.lefts.push(Self::NO_CHILD);
        self.rights.push(Self::NO_CHILD);
        idx
    }

    fn refine(&mut self, idx: u32, other: TSetId, solver: &mut Solver) {
        let set = self.sets[idx as usize];
        let this_and_other = solver.and_id(set, other);
        if this_and_other != TSetId::EMPTY {
            let notother = solver.not_id(other);
            let this_minus_other = solver.and_id(set, notother);
            if this_minus_other != TSetId::EMPTY {
                if self.lefts[idx as usize] == Self::NO_CHILD {
                    let l = self.push(this_and_other);
                    let r = self.push(this_minus_other);
                    self.lefts[idx as usize] = l;
                    self.rights[idx as usize] = r;
                } else {
                    let l = self.lefts[idx as usize];
                    let r = self.rights[idx as usize];
                    self.refine(l, other, solver);
                    self.refine(r, other, solver);
                }
            }
        }
    }

    fn get_leaf_sets(&self) -> Vec<TSetId> {
        let mut leaves = Vec::new();
        let mut stack = vec![0u32];
        while let Some(idx) = stack.pop() {
            if self.lefts[idx as usize] == Self::NO_CHILD {
                leaves.push(self.sets[idx as usize]);
            } else {
                stack.push(self.lefts[idx as usize]);
                stack.push(self.rights[idx as usize]);
            }
        }
        leaves
    }

    pub fn generate_minterms(sets: HashSet<TSetId>, solver: &mut Solver) -> Vec<TSetId> {
        let mut pt = PartitionTree::new(TSetId::FULL);
        for set in sets {
            pt.refine(0, set, solver);
        }
        let mut lsets = pt.get_leaf_sets();
        lsets[1..].sort();
        lsets
    }

    pub fn minterms_lookup(minterms: &[TSetId], solver: &mut Solver) -> [u8; 256] {
        let mut lookup = [0u8; 256];
        if minterms.len() <= 1 {
            return lookup;
        }
        let mut mt_index = 1u8;
        for m in minterms.iter().skip(1) {
            for i in 0..4 {
                for j in 0..64 {
                    let nthbit = 1u64 << j;
                    if solver.has_bit_set(*m, i, nthbit) {
                        let cc = (i * 64 + j) as u8;
                        lookup[cc as usize] = mt_index;
                    }
                }
            }
            mt_index += 1;
        }
        lookup
    }
}

pub fn collect_sets(b: &RegexBuilder, start_id: NodeId) -> HashSet<TSetId> {
    let mut visited = HashSet::new();
    let mut sets = HashSet::new();
    let mut stack = vec![start_id];
    while let Some(node_id) = stack.pop() {
        if visited.contains(&node_id) {
            continue;
        }
        visited.insert(node_id);
        match b.get_kind(node_id) {
            Kind::Begin | Kind::End => {}
            Kind::Pred => {
                sets.insert(node_id.pred_tset(b));
            }
            Kind::Union | Kind::Concat | Kind::Inter => {
                stack.push(node_id.left(b));
                stack.push(node_id.right(b));
            }
            Kind::Lookahead | Kind::Lookbehind | Kind::Counted => {
                stack.push(node_id.left(b));
                stack.push(node_id.right(b));
            }
            Kind::Star | Kind::Compl => {
                stack.push(node_id.left(b));
            }
        }
    }
    sets
}

pub fn transition_term(b: &mut RegexBuilder, der: TRegexId, set: TSetId) -> NodeId {
    let mut term = b.get_tregex(der);
    loop {
        match *term {
            TRegex::Leaf(node_id) => return node_id,
            TRegex::ITE(cond, _then, _else) => {
                if b.solver().is_sat_id(set, cond) {
                    term = b.get_tregex(_then);
                } else {
                    term = b.get_tregex(_else);
                }
            }
        }
    }
}

pub(crate) fn collect_tregex_leaves(b: &RegexBuilder, tregex: TRegexId, out: &mut Vec<NodeId>) {
    let mut stack = vec![tregex];
    let mut visited = HashSet::new();
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            continue;
        }
        match *b.get_tregex(id) {
            TRegex::Leaf(node_id) => out.push(node_id),
            TRegex::ITE(_, then_br, else_br) => {
                stack.push(then_br);
                stack.push(else_br);
            }
        }
    }
}

const SKIP_FREQ_THRESHOLD: u32 = 75_000;
const RARE_BYTE_FREQ_LIMIT: u16 = 25_000;

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
pub struct LDFA {
    pub pruned: u16,
    pub prune_memo: FxHashMap<NodeId, NodeId>,
    pub begin_table: Vec<u16>,
    pub center_table: Vec<u16>,
    pub effects_id: Vec<u16>,
    pub effects: Vec<Vec<NullState>>,
    pub center_effect_id: Vec<u16>,
    pub mt_log: u32,
    pub mt_lookup: [u8; 256],
    pub minterms: Vec<TSetId>,
    pub state_nodes: Vec<NodeId>,
    pub node_to_state: HashMap<NodeId, u16>,
    pub skip_ids: Vec<u8>,
    pub skip_searchers: Vec<MintermSearchValue>,
    pub prefix_skip: Option<crate::accel::RevPrefixSearch>,
    pub max_capacity: usize,
    pub is_forward: bool,
}

impl LDFA {
    pub fn new(b: &mut RegexBuilder, initial: NodeId, max_capacity: usize) -> Result<LDFA, Error> {
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
        let _ = register_state(
            &mut state_nodes,
            &mut node_to_state,
            &mut effects_id,
            &mut center_effect_id,
            &mut effects,
            b,
            initial,
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
            center_table[(DFA_DEAD as usize) << mt_log | mt_idx] = DFA_DEAD;
        }

        while effects.len() < b.nulls_count() {
            effects.push(b.nulls_entry_vec(effects.len() as u32));
        }
        let skip_ids = vec![0u8; state_nodes.len()];

        Ok(LDFA {
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
        })
    }

    #[inline(always)]
    pub fn dfa_delta(&self, state_id: u16, mt: u32) -> usize {
        ((state_id as u32) << self.mt_log | mt) as usize
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
    }

    pub fn get_or_register(&mut self, b: &mut RegexBuilder, node: NodeId) -> u16 {
        register_state(
            &mut self.state_nodes,
            &mut self.node_to_state,
            &mut self.effects_id,
            &mut self.center_effect_id,
            &mut self.effects,
            b,
            node,
        )
    }

    #[inline(always)]
    pub fn lazy_transition(
        &mut self,
        b: &mut RegexBuilder,
        state_id: u16,
        minterm_idx: u32,
    ) -> Result<u16, Error> {
        let delta = self.dfa_delta(state_id, minterm_idx);
        if delta < self.center_table.len() && self.center_table[delta] != DFA_MISSING {
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
                let next_sid = self.center_table[base + mt_idx];
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
            if delta < self.center_table.len() && self.center_table[delta] != DFA_MISSING {
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    fn try_build_range_skip(&mut self, bytes: &[u8]) -> Option<u8> {
        let tset = crate::accel::TSet::from_bytes(bytes);
        let ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
        if ranges.is_empty() || ranges.len() > 3 {
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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
        prefix_ptr: *const crate::accel::RevPrefixSearch,
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

    pub fn scan_fwd_slow(
        &mut self,
        b: &mut RegexBuilder,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<usize, Error> {
        let empty_mask = if pos_begin == 0 {
            Nullability::BEGIN
        } else {
            Nullability::CENTER
        };
        let has_empty = has_any_null(
            &self.effects_id,
            &self.effects,
            DFA_INITIAL as u32,
            empty_mask,
        );

        let mt = self.mt_lookup[data[pos_begin] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD as u32 {
            return Ok(if has_empty { pos_begin } else { NO_MATCH });
        }

        let end = data.len();
        let mut pos = pos_begin + 1;
        let mut max_end: usize = 0;

        let mask = if pos == end {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        collect_max_fwd(
            &self.effects_id,
            &self.effects,
            curr,
            pos,
            mask,
            &mut max_end,
        );

        if pos == end {
            return Ok(self.resolve_max_end(max_end, has_empty, pos_begin));
        }

        loop {
            let tables = self.scan_tables(data);
            let (state, new_pos, new_max, cache_miss) =
                self.dispatch_scan_fwd(&tables, curr, pos, end, max_end);
            max_end = new_max;

            if !cache_miss {
                break;
            }

            let sid = state as u16;
            self.create_state(b, sid)?;

            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            curr = self.center_table[self.dfa_delta(sid, mt)] as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.create_state(b, curr as u16)?;
            if cfg!(feature = "debug") {
                eprintln!(
                    "  [fwd-miss] sid={} curr={} skip_ids=[{},{}]",
                    sid,
                    curr,
                    self.skip_ids.get(sid as usize).copied().unwrap_or(255),
                    self.skip_ids.get(curr as usize).copied().unwrap_or(255)
                );
            }

            let mask = if pos == end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                mask,
                &mut max_end,
            );

            if pos == end {
                break;
            }
        }

        Ok(self.resolve_max_end(max_end, has_empty, pos_begin))
    }

    #[inline]
    fn resolve_max_end(&self, max_end: usize, has_empty: bool, pos_begin: usize) -> usize {
        if max_end > 0 {
            max_end
        } else if has_empty {
            pos_begin
        } else {
            NO_MATCH
        }
    }

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
        let use_skip = self.can_skip();
        if cfg!(feature = "debug") {
            eprintln!(
                "  [scan_fwd_all] can_skip={} searchers={} nulls={}",
                use_skip,
                self.skip_searchers.len(),
                nulls.len()
            );
        }

        let mut l_pos: usize;
        let mut i = nulls.len();

        if nulls[nulls.len() - 1] == 0 {
            i = i - 1;
            l_pos = 0;
            let mut l_max_end = 0;

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
                        let new_state = self.lazy_transition(b, state as u16, mt)? as u32;
                        l_pos = new_pos + 1;
                        l_state = new_state;
                        if l_pos != data_end {
                            continue;
                        }
                        (new_state, l_pos)
                    };
                    let effect = self.effects_id[flush_state as usize] as u32;
                    l_max_end = collect_nulls_fwd_complex(
                        self.effects.as_ptr(),
                        effect,
                        flush_pos,
                        Nullability::END,
                        l_max_end,
                    );
                }

                matches.push(Match {
                    start: 0,
                    end: l_max_end,
                });
                next_start = l_max_end;
                break;
            }
        }

        while i != 0 {
            i = i - 1;
            l_pos = nulls[i];
            if l_pos < next_start {
                continue;
            }
            if cfg!(feature = "debug") {
                println!(
                    "  [scan_fwd_all] begin_pos={}, data_end={}",
                    l_pos, data_end
                );
            }
            if l_pos == data_end {
                matches.push(Match {
                    start: l_pos,
                    end: l_pos,
                });
                break;
            }

            let mut l_state = DFA_INITIAL as u32;
            let mut l_max_end = 0;
            loop {
                let tables = self.scan_tables(data);
                let (state, new_pos, new_max, cache_miss) =
                    self.dispatch_scan_fwd(&tables, l_state, l_pos, data_end, l_max_end);
                l_max_end = new_max;
                if cache_miss {
                    debug_assert!(new_pos >= l_pos, "backwards");
                    let mt = self.mt_lookup[data[new_pos] as usize] as u32;
                    let next_state = self.lazy_transition(b, state as u16, mt)? as u32;
                    if cfg!(feature = "debug") {
                        println!(
                            "[{} '{}'] {} -> {}",
                            new_pos,
                            data[new_pos] as char,
                            state,
                            b.pp(self.state_nodes[next_state as usize])
                        );
                    }
                    l_pos = new_pos + 1;
                    l_state = next_state;
                    if l_pos != data_end {
                        continue;
                    }
                    let effect = self.effects_id[l_state as usize] as u32;
                    l_max_end = collect_nulls_fwd_complex(
                        self.effects.as_ptr(),
                        effect,
                        l_pos,
                        Nullability::END,
                        l_max_end,
                    );
                    matches.push(Match {
                        start: nulls[i],
                        end: l_max_end,
                    });
                    next_start = l_max_end;
                    break;
                }
                debug_assert!(
                    l_max_end >= nulls[i],
                    "unexpected end {} > {}",
                    l_max_end,
                    nulls[i]
                );
                matches.push(Match {
                    start: nulls[i],
                    end: l_max_end,
                });
                next_start = l_max_end;
                break;
            }
        }

        Ok(())
    }

    /// Spawn a register from null at `np`, fast-forward through data[np+1..limit).
    fn spawn_from_null(
        &mut self,
        b: &mut RegexBuilder,
        data: &[u8],
        data_end: usize,
        np: usize,
        limit: usize,
    ) -> Result<Option<(u16, usize)>, Error> {
        let smt = self.mt_lookup[data[np] as usize];
        let ss = self.begin_table[smt as usize];
        if ss <= DFA_DEAD {
            return Ok(None);
        }
        let mut me = 0usize;
        let init_eid = self
            .effects_id
            .get(DFA_INITIAL as usize)
            .copied()
            .unwrap_or(0);
        if init_eid != 0 {
            let pre_mask = if np == 0 {
                Nullability::BEGIN
            } else {
                Nullability::CENTER
            };
            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                DFA_INITIAL as u32,
                np,
                pre_mask,
                &mut me,
            );
        }
        collect_max_fwd(
            &self.effects_id,
            &self.effects,
            ss as u32,
            np + 1,
            if np + 1 >= data_end {
                Nullability::END
            } else {
                Nullability::CENTER
            },
            &mut me,
        );
        let mut rs = ss;
        let mut fp = np + 1;
        while fp < limit {
            let fmt = self.mt_lookup[data[fp] as usize] as u32;
            let ns = self.lazy_transition(b, rs, fmt)?;
            if ns <= DFA_DEAD {
                rs = DFA_DEAD;
                break;
            }
            rs = ns;
            fp += 1;
            let mask = if fp >= data_end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                rs as u32,
                fp,
                mask,
                &mut me,
            );
        }
        Ok(Some((rs, me)))
    }

    pub fn scan_fwd_ordered(
        &mut self,
        b: &mut RegexBuilder,
        nulls: &[usize],
        data: &[u8],
        max_length: Option<u32>,
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let data_end = data.len();
        if data_end == 0 || nulls.is_empty() {
            return Ok(());
        }

        let mut regs: Vec<(u16, usize, usize)> = Vec::with_capacity(8);
        let mut next_regs: Vec<(u16, usize, usize)> = Vec::with_capacity(8);
        let mut dead: Vec<(usize, usize)> = Vec::new();
        let mut skip_until: usize = 0;

        let mut null_idx = nulls.len();
        let first_pos = nulls[nulls.len() - 1];
        let mut pos = first_pos;

        // respawn modes: walk nulls[null_idx..] and spawn from each.
        //   0 = break on first live, ignore dead         (fast-path EOF)
        //   1 = break on first live, emit dead as match  (fast-path DFA_DEAD)
        //   2 = push all live (state-deduped), pile dead (post dead-flush)
        fn respawn<const MODE: u8>(
            eng: &mut LDFA,
            b: &mut RegexBuilder,
            nulls: &[usize],
            null_idx: usize,
            data: &[u8],
            data_end: usize,
            pos: usize,
            regs: &mut Vec<(u16, usize, usize)>,
            matches: &mut Vec<Match>,
            dead: &mut Vec<(usize, usize)>,
            skip_until: &mut usize,
        ) -> Result<(), Error> {
            for ni in null_idx..nulls.len() {
                let np = nulls[ni];
                if np < *skip_until {
                    break;
                }
                if np >= pos || np + 1 > data_end {
                    continue;
                }
                let Some((rs, reg_me)) = eng.spawn_from_null(b, data, data_end, np, pos)? else {
                    continue;
                };
                if rs > DFA_DEAD {
                    if MODE == 2 {
                        if !regs.iter().any(|r| r.0 == rs) {
                            regs.push((rs, np, reg_me));
                        }
                    } else {
                        regs.push((rs, np, reg_me));
                        return Ok(());
                    }
                } else if reg_me > 0 {
                    match MODE {
                        1 => {
                            matches.push(Match {
                                start: np,
                                end: reg_me,
                            });
                            *skip_until = reg_me.max(np + 1);
                        }
                        2 => dead.push((np, reg_me)),
                        _ => {}
                    }
                }
            }
            Ok(())
        }

        while pos < data_end {
            if regs.is_empty() {
                if null_idx == 0 {
                    break;
                }
                let next_null = nulls[null_idx - 1];
                if next_null > pos {
                    pos = next_null;
                }
                if pos >= data_end {
                    break;
                }
            }

            let has_spawn = null_idx > 0 && nulls[null_idx - 1] == pos;
            if has_spawn {
                while null_idx > 0 && nulls[null_idx - 1] == pos {
                    null_idx -= 1;
                }
            }

            if regs.is_empty() && !has_spawn {
                pos += 1;
                continue;
            }

            // single-component fast path (no spawn at this position)
            if regs.len() == 1 && !has_spawn && max_length.is_none() {
                let run_end = if null_idx > 0 {
                    nulls[null_idx - 1].min(data_end)
                } else {
                    data_end
                };
                let (mut ldfa_state, reg_start, mut me) = (regs[0].0 as u32, regs[0].1, regs[0].2);

                loop {
                    let tables = self.scan_tables(data);
                    let (s, p, m, cache_miss) = scan_single_component(
                        &tables,
                        self.effects_id.as_ptr(),
                        ldfa_state,
                        pos,
                        run_end,
                        data_end,
                        me,
                    );
                    me = m;
                    if !cache_miss {
                        ldfa_state = s;
                        pos = p;
                        break;
                    }
                    let miss_mt = self.mt_lookup[data[p] as usize] as u32;
                    let next = self.lazy_transition(b, s as u16, miss_mt)?;
                    if next <= DFA_DEAD {
                        ldfa_state = DFA_DEAD as u32;
                        pos = p;
                        break;
                    }
                    ldfa_state = next as u32;
                    pos = p + 1;
                    let mask = if pos >= data_end {
                        Nullability::END
                    } else {
                        Nullability::CENTER
                    };
                    collect_max_fwd(
                        &self.effects_id,
                        &self.effects,
                        ldfa_state,
                        pos,
                        mask,
                        &mut me,
                    );
                    if pos >= run_end {
                        break;
                    }
                }

                if ldfa_state != DFA_DEAD as u32 {
                    if pos >= data_end && me > 0 && reg_start >= skip_until {
                        matches.push(Match {
                            start: reg_start,
                            end: me,
                        });
                        skip_until = if me > reg_start { me } else { reg_start + 1 };
                        regs.clear();
                        respawn::<0>(
                            self,
                            b,
                            nulls,
                            null_idx,
                            data,
                            data_end,
                            pos,
                            &mut regs,
                            matches,
                            &mut dead,
                            &mut skip_until,
                        )?;
                    } else {
                        regs[0] = (ldfa_state as u16, reg_start, me);
                    }
                    continue;
                }

                regs.clear();
                if me > 0 && reg_start >= skip_until {
                    matches.push(Match {
                        start: reg_start,
                        end: me,
                    });
                    skip_until = if me > reg_start { me } else { reg_start + 1 };

                    if !dead.is_empty() {
                        skip_until = flush_dead(&mut dead, matches, skip_until, usize::MAX);
                    }

                    respawn::<1>(
                        self,
                        b,
                        nulls,
                        null_idx,
                        data,
                        data_end,
                        pos,
                        &mut regs,
                        matches,
                        &mut dead,
                        &mut skip_until,
                    )?;
                }
                if regs.is_empty() {
                    pos += 1;
                }
                continue;
            }

            // multi-register path: transition existing registers + handle spawn
            let mt = self.mt_lookup[data[pos] as usize] as u32;
            let next_pos = pos + 1;
            next_regs.clear();

            for &(state, start, max_end) in &regs {
                let base = (state as usize) << self.mt_log;
                let mut next_state = if base + mt as usize <= self.center_table.len() {
                    self.center_table[base + mt as usize]
                } else {
                    0
                };

                if next_state == 0 {
                    let ns = self.lazy_transition(b, state, mt)?;
                    next_state = ns;
                }

                if next_state <= DFA_DEAD {
                    if max_end > 0 {
                        if cfg!(feature = "debug") {
                            eprintln!("  [dead-die] ({}, {}) state={}", start, max_end, state);
                        }
                        dead.push((start, max_end));
                    }
                    continue;
                }

                let mut new_me = max_end;
                let end_limit = match max_length {
                    Some(ml) => (start + ml as usize).min(data_end),
                    None => data_end,
                };
                let mask = if next_pos >= end_limit {
                    Nullability::END
                } else {
                    Nullability::CENTER
                };
                collect_max_fwd(
                    &self.effects_id,
                    &self.effects,
                    next_state as u32,
                    next_pos,
                    mask,
                    &mut new_me,
                );

                let dup = next_regs.iter().position(|r| r.0 == next_state);
                if let Some(di) = dup {
                    if start < next_regs[di].1 {
                        if next_regs[di].2 > 0 {
                            dead.push((next_regs[di].1, next_regs[di].2));
                        }
                        next_regs[di] = (next_state, start, new_me);
                    } else if new_me > 0 {
                        dead.push((start, new_me));
                    }
                } else {
                    next_regs.push((next_state, start, new_me));
                }
            }

            // handle spawn: begin_table already consumes data[pos]
            if has_spawn {
                let mut spawn_me = 0usize;
                let init_eid = self
                    .effects_id
                    .get(DFA_INITIAL as usize)
                    .copied()
                    .unwrap_or(0);
                if init_eid != 0 {
                    let pre_mask = if pos == 0 {
                        Nullability::BEGIN
                    } else {
                        Nullability::CENTER
                    };
                    collect_max_fwd(
                        &self.effects_id,
                        &self.effects,
                        DFA_INITIAL as u32,
                        pos,
                        pre_mask,
                        &mut spawn_me,
                    );
                }

                let smt = self.mt_lookup[data[pos] as usize];
                let spawn_state = self.begin_table[smt as usize];
                if spawn_state <= DFA_DEAD {
                    if spawn_me > 0 && pos >= skip_until {
                        dead.push((pos, spawn_me));
                    }
                } else {
                    let end_limit = match max_length {
                        Some(ml) => (pos + ml as usize).min(data_end),
                        None => data_end,
                    };
                    let mask = if next_pos >= end_limit {
                        Nullability::END
                    } else {
                        Nullability::CENTER
                    };
                    collect_max_fwd(
                        &self.effects_id,
                        &self.effects,
                        spawn_state as u32,
                        next_pos,
                        mask,
                        &mut spawn_me,
                    );

                    if next_regs.iter().any(|r| r.0 == spawn_state) {
                        if spawn_me > 0 && pos >= skip_until {
                            dead.push((pos, spawn_me));
                        }
                    } else {
                        next_regs.push((spawn_state, pos, spawn_me));
                    }
                }
            }

            std::mem::swap(&mut regs, &mut next_regs);
            pos = next_pos;

            if !dead.is_empty() {
                let min_active = regs.iter().map(|r| r.1).min().unwrap_or(usize::MAX);
                if min_active > skip_until {
                    if cfg!(feature = "debug") {
                        eprintln!(
                            "  [dead-flush] pos={} min_active={} skip={} dead={:?} regs={:?}",
                            pos, min_active, skip_until, dead, regs
                        );
                    }
                    let old_skip = skip_until;
                    skip_until = flush_dead(&mut dead, matches, skip_until, min_active);
                    if skip_until == old_skip {
                        continue;
                    }
                    regs.retain(|r| r.1 >= skip_until);
                    respawn::<2>(
                        self,
                        b,
                        nulls,
                        null_idx,
                        data,
                        data_end,
                        pos,
                        &mut regs,
                        matches,
                        &mut dead,
                        &mut skip_until,
                    )?;
                }
            }
        }

        for &(_, start, max_end) in &regs {
            if max_end > 0 {
                dead.push((start, max_end));
            }
        }
        regs.clear();
        if !dead.is_empty() {
            skip_until = flush_dead(&mut dead, matches, skip_until, usize::MAX);
        }
        loop {
            let m = self.respawn_first(b, nulls, null_idx, data, data_end, skip_until, data_end);
            match m {
                Some((s, e)) => {
                    matches.push(Match { start: s, end: e });
                    skip_until = if e > s { e } else { s + 1 };
                }
                None => break,
            }
        }

        Ok(())
    }

    fn respawn_first(
        &mut self,
        b: &mut RegexBuilder,
        nulls: &[usize],
        null_idx: usize,
        data: &[u8],
        data_end: usize,
        skip_until: usize,
        limit: usize,
    ) -> Option<(usize, usize)> {
        for ni in (null_idx..nulls.len()).rev() {
            let np = nulls[ni];
            if np < skip_until || np >= limit || np + 1 > data_end {
                continue;
            }
            if let Some((_, me)) = self.spawn_from_null(b, data, data_end, np, data_end).ok()? {
                if me > 0 {
                    return Some((np, me));
                }
            }
            return None;
        }
        None
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

    /// scan forward from a precomputed DFA state.
    pub fn scan_fwd_from(
        &mut self,
        b: &mut RegexBuilder,
        state: u32,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<usize, Error> {
        if state <= DFA_DEAD as u32 {
            return Ok(NO_MATCH);
        }
        let end = data.len();
        let mut pos = pos_begin;
        let mut curr = state;
        let mut max_end: usize = 0;

        collect_max_fwd(
            &self.effects_id,
            &self.effects,
            curr,
            pos,
            Nullability::CENTER,
            &mut max_end,
        );

        if pos >= end {
            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                Nullability::END,
                &mut max_end,
            );
            return Ok(if max_end > 0 { max_end } else { NO_MATCH });
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
            curr = self.lazy_transition(b, state_out as u16, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.create_state(b, curr as u16).ok();

            let mask = if pos >= end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_max_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                mask,
                &mut max_end,
            );

            if pos >= end {
                break;
            }
        }

        Ok(if max_end > 0 { max_end } else { NO_MATCH })
    }

    /// Reverse analog of `scan_fwd_from`: starting from `end` (exclusive), scan
    /// backwards to `begin` (inclusive) using the bare reverse DFA, and return
    /// the minimum (leftmost) match-start position, or `NO_MATCH`.
    pub fn scan_rev_from(
        &mut self,
        b: &mut RegexBuilder,
        end: usize,
        begin: usize,
        data: &[u8],
    ) -> Result<usize, Error> {
        if end == 0 || end > data.len() || end <= begin {
            return Ok(NO_MATCH);
        }
        let start_pos = end - 1;
        let mt = self.mt_lookup[data[start_pos] as usize] as u32;
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD as u32 {
            return Ok(NO_MATCH);
        }

        let mut min_start = NO_MATCH;
        let mask = if start_pos == begin {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        collect_max_rev(
            &self.effects_id,
            &self.effects,
            curr,
            start_pos,
            mask,
            &mut min_start,
        );

        let mut pos = start_pos;
        while pos > begin {
            pos -= 1;
            let mt = self.mt_lookup[data[pos] as usize] as u32;
            let delta = (curr << self.mt_log | mt) as usize;
            let next = if delta < self.center_table.len() {
                self.center_table[delta]
            } else {
                DFA_MISSING
            };
            if next == DFA_MISSING {
                curr = self.lazy_transition(b, curr as u16, mt)? as u32;
                self.create_state(b, curr as u16).ok();
            } else {
                curr = next as u32;
            }
            if curr <= DFA_DEAD as u32 {
                break;
            }
            let mask = if pos == begin {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_max_rev(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                mask,
                &mut min_start,
            );
        }

        Ok(min_start)
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
            // eprintln!("  [rev0]: {}", b.pp(self.state_nodes[DFA_INITIAL as usize]));
            eprintln!(
                "  [rev0]: {:.20}",
                b.pp(self.state_nodes[DFA_INITIAL as usize])
            );
        }

        let mut curr = self.begin_table[self.mt_lookup[data[start_pos] as usize] as usize] as u32;
        #[cfg(feature = "debug")]
        {
            eprintln!("rev1: {:.30}", b.pp(self.state_nodes[curr as usize]));
        }
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

        if let Some(preskip) = self.prefix_skip.as_ref() {
            return self.collect_rev_prefix::<EARLY_EXIT>(
                b,
                preskip as *const crate::accel::RevPrefixSearch,
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
                if cfg!(feature = "debug") {
                    eprintln!(
                        "  [collect_rev] no cache miss, state={} pos={}",
                        state, new_pos
                    );
                }

                let sid = state as u16;
                let mt = self.mt_lookup[data[0] as usize] as u32;
                let new_state = self.lazy_transition(b, sid, mt)?;
                let effect = self.effects_id[new_state as usize] as u32;
                collect_rev_complex(self.effects.as_ptr(), effect, 0, Nullability::END, nulls);
                break;
            }

            let sid = state as u16;
            self.create_state(b, sid)?;

            let mt = self.mt_lookup[data[new_pos] as usize] as u32;
            let delta = self.dfa_delta(sid, mt);
            curr = self.center_table[delta] as u32;
            pos = new_pos;

            if curr <= DFA_DEAD as u32 {
                break;
            }

            let mask = if pos == 0 {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            if cfg!(feature = "debug") {
                if self.effects_id[curr as usize] > 0 {
                    eprintln!(
                        "  [effect] pos={} eid=1 push={:.20}",
                        pos,
                        b.pp(self.state_nodes[curr as usize])
                    );
                }
            }
            collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);
            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }
        }

        Ok(())
    }

    fn collect_rev_prefix<const EARLY_EXIT: bool>(
        &mut self,
        b: &mut RegexBuilder,
        prefix_ptr: *const crate::accel::RevPrefixSearch,
        start_pos: usize,
        start_state: u32,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        let mut curr = start_state;
        let mut pos = start_pos;

        if cfg!(feature = "debug") {
            eprintln!("  [rev_prefix] after_collect_nulls nulls={:?}", nulls);
        }
        if EARLY_EXIT && !nulls.is_empty() {
            return Ok(());
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
                let sid = state as u16;
                self.create_state(b, sid)?;
                curr = sid as u32;
                pos = new_pos + 1;
                continue;
            } else {
                let sid = state as u16;
                let mt = self.mt_lookup[data[0] as usize] as u32;
                let new_state = self.lazy_transition(b, sid, mt)?;
                let effect = self.effects_id[new_state as usize] as u32;
                collect_rev_complex(self.effects.as_ptr(), effect, 0, Nullability::END, nulls);
                break;
            }
        }

        Ok(())
    }
}

pub(crate) fn has_any_null(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    mask: Nullability,
) -> bool {
    let eid = effects_id[state as usize] as u32;
    if eid == 0 {
        return false;
    }
    if eid == EID_ALWAYS0 {
        return mask.has(Nullability::ALWAYS);
    }
    if eid == EID_CENTER0 {
        return mask.has(Nullability::CENTER);
    }
    effects[eid as usize].iter().any(|n| n.mask.has(mask))
}

// same as resharp-algebra/src/nulls.rs, just explicit
const EID_NONE: u32 = 0;
const EID_CENTER0: u32 = 1;
const EID_ALWAYS0: u32 = 2;
const EID_BEGIN0: u32 = 3;
const EID_END0: u32 = 4;

#[inline(always)]
fn collect_nulls(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    nulls: &mut Vec<usize>,
) {
    let eid = effects_id[state as usize] as u32;
    if eid != 0 {
        // Fast paths for the four well-known NullsIds (no heap indirection).
        match eid {
            EID_ALWAYS0 => {
                if mask.has(Nullability::ALWAYS) {
                    if cfg!(feature = "debug") {
                        eprintln!(
                            "  [collect_nulls] state={} pos={} eid=1 push={}",
                            state, pos, pos
                        );
                    }
                    nulls.push(pos);
                }
            }
            EID_CENTER0 => {
                if mask.has(Nullability::CENTER) {
                    if cfg!(feature = "debug") {
                        eprintln!(
                            "  [collect_nulls] state={} pos={} eid=2 push={}",
                            state, pos, pos
                        );
                    }
                    nulls.push(pos);
                }
            }
            EID_BEGIN0 => {
                if mask.has(Nullability::BEGIN) {
                    nulls.push(pos);
                }
            }
            EID_END0 => {
                if mask.has(Nullability::END) {
                    nulls.push(pos);
                }
            }
            _ => {
                for n in &effects[eid as usize] {
                    if n.mask.has(mask) {
                        if cfg!(feature = "debug") {
                            eprintln!(
                                "  [collect_nulls] state={} pos={} eid={} rel={} push={}",
                                state,
                                pos,
                                eid,
                                n.rel,
                                pos + n.rel as usize
                            );
                        }
                        nulls.push(pos + n.rel as usize);
                    }
                }
            }
        }
    }
}

struct ScanTables {
    center_table: *const u16,
    center_effect_id: *const u16,
    effects: *const Vec<NullState>,
    data: *const u8,
    minterms_lookup: *const u8,
    mt_log: u32,
}

#[cold]
#[inline(never)]
fn collect_rev_center_simple(
    effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    nulls: &mut Vec<usize>,
) {
    unsafe {
        let v = &*effects.add(eid as usize);
        for n in v {
            nulls.push(pos + n.rel as usize);
        }
    }
}

#[cold]
#[inline(never)]
fn collect_rev_complex(
    effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    mask: Nullability,
    nulls: &mut Vec<usize>,
) {
    unsafe {
        let effects_vec = &*effects.add(eid as usize);
        for n in effects_vec {
            if n.mask.has(mask) {
                nulls.push(pos + n.rel as usize);
            }
        }
    }
}

/// Emit matches from `dead` with start in [skip_until, emit_limit).
/// Entries >= emit_limit are retained. Returns updated skip_until.
fn flush_dead(
    dead: &mut Vec<(usize, usize)>,
    matches: &mut Vec<Match>,
    mut skip_until: usize,
    emit_limit: usize,
) -> usize {
    dead.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut w = 0;
    for i in 0..dead.len() {
        if dead[i].1 == 0 {
            continue;
        }
        if w > 0 && dead[w - 1].0 == dead[i].0 {
            continue;
        }
        dead[w] = dead[i];
        w += 1;
    }
    dead.truncate(w);

    let mut retained = 0;
    let mut max_skipped_end: usize = 0;
    for i in 0..dead.len() {
        let (s, e) = dead[i];
        if s < skip_until {
            if e > max_skipped_end {
                max_skipped_end = e;
            }
            continue;
        }
        if s < emit_limit {
            let e = e.max(max_skipped_end);
            max_skipped_end = 0;
            matches.push(Match { start: s, end: e });
            skip_until = if e > s { e } else { s + 1 };
        } else {
            dead[retained] = (s, e.max(max_skipped_end));
            max_skipped_end = 0;
            retained += 1;
        }
    }
    dead.truncate(retained);
    skip_until
}

#[inline(always)]
fn collect_max<const REV: bool>(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    let eid = effects_id[state as usize] as u32;
    if eid != 0 {
        if eid == 1 {
            if mask.has(Nullability::ALWAYS) {
                if REV {
                    *best = (*best).min(pos);
                } else {
                    *best = (*best).max(pos);
                }
            }
            return;
        }
        for n in &effects[eid as usize] {
            if n.mask.has(mask) {
                let candidate = if REV {
                    pos + n.rel as usize
                } else {
                    pos - n.rel as usize
                };
                if REV {
                    *best = (*best).min(candidate);
                } else {
                    *best = (*best).max(candidate);
                }
            }
        }
    }
}

#[inline(always)]
fn collect_max_fwd(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    collect_max::<false>(effects_id, effects, state, pos, mask, best);
}

#[inline(always)]
fn collect_max_rev(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    collect_max::<true>(effects_id, effects, state, pos, mask, best);
}

#[inline(never)]
fn collect_rev<const EARLY_EXIT: bool, const SKIP: bool, const INITIAL_SKIP: bool>(
    t: &ScanTables,
    skip_ids: &[u8],
    skip_searchers: &[MintermSearchValue],
    prefix_ptr: *const crate::accel::RevPrefixSearch,
    mut curr: u32,
    mut pos: usize,
    data: &[u8],
    nulls: &mut Vec<usize>,
    pruned_id: u32,
) -> (u32, usize, bool) {
    let center_table = t.center_table;
    let center_effect_id = t.center_effect_id;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
    while pos > 1 {
        if SKIP {
            let sid = skip_ids[curr as usize];
            if sid != 0 {
                if INITIAL_SKIP && curr == pruned_id {
                    // SAFETY: unreachable unless prefix_ptr is non-null
                    match unsafe { &*prefix_ptr }.find_rev(data, pos) {
                        Some(skip_pos) => {
                            if pos != skip_pos {
                                pos = skip_pos + 1;
                                let eid = unsafe { *center_effect_id.add(curr as usize) };
                                if eid == EID_CENTER0 as _ {
                                    nulls.push(pos + 1);
                                } else if eid != EID_NONE as _ {
                                    collect_rev_center_simple(
                                        t.effects,
                                        eid as u32,
                                        pos + 1,
                                        nulls,
                                    );
                                }
                            }
                        }
                        None => {
                            pos = 0;
                            continue;
                        }
                    }
                } else {
                    let searcher = &skip_searchers[sid as usize - 1];
                    match searcher.find_rev(&data[..pos]) {
                        Some(skip_pos) => {
                            debug_assert!(pos != skip_pos);
                            let eid = unsafe { *center_effect_id.add(curr as usize) };
                            if eid == EID_CENTER0 as _ {
                                nulls.extend((skip_pos + 1..pos).rev());
                            } else if eid != EID_NONE as _ {
                                for p in (skip_pos + 1..pos).rev() {
                                    collect_rev_center_simple(t.effects, eid as u32, p, nulls);
                                }
                            }
                            pos = skip_pos + 1;
                        }
                        None => pos = 1,
                    }
                }
            }
        }
        pos -= 1;
        unsafe {
            let mt = *minterms_lookup.add(*data.as_ptr().add(pos) as usize) as u32;
            let next = *center_table.add((curr << mt_log | mt) as usize);
            if next == DFA_MISSING {
                return (curr, pos, true);
            }
            curr = next as u32;

            let eid = *center_effect_id.add(curr as usize);
            if eid == EID_CENTER0 as _ {
                nulls.push(pos);
                if EARLY_EXIT {
                    return (curr, pos, false);
                }
            } else if eid != EID_NONE as _ {
                collect_rev_center_simple(t.effects, eid as u32, pos, nulls);
                if EARLY_EXIT && !nulls.is_empty() {
                    return (curr, pos, false);
                }
            }
        }
    }

    // eprintln!("nulls! {:?}",nulls);

    (curr, 1, false)
}

#[cold]
#[inline(never)]
fn collect_nulls_fwd_complex(
    effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    mask: Nullability,
    max_end: usize,
) -> usize {
    let mut result = max_end;
    unsafe {
        let effects_vec = &*effects.add(eid as usize);
        for n in effects_vec {
            if n.mask.has(mask) {
                result = result.max(pos - n.rel as usize);
            }
        }
    }
    result
}

#[cold]
#[inline(never)]
fn collect_nulls_fwd_center_simple(
    center_effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    max_end: usize,
) -> usize {
    let mut result = max_end;
    unsafe {
        let v = &*center_effects.add(eid as usize);
        for n in v {
            result = result.max(pos - n.rel as usize);
        }
    }
    result
}

#[inline(always)]
unsafe fn fwd_center_update(
    center_effect_id: *const u16,
    center_effects: *const Vec<NullState>,
    state: u32,
    pos: usize,
    max_end: usize,
) -> usize {
    let eid = *center_effect_id.add(state as usize);
    if eid == EID_NONE as u16 {
        max_end
    } else if eid == EID_CENTER0 as u16 {
        max_end.max(pos)
    } else {
        collect_nulls_fwd_center_simple(center_effects, eid as u32, pos, max_end)
    }
}

#[inline(never)]
fn scan_single_component(
    t: &ScanTables,
    effects_id: *const u16,
    mut curr: u32,
    mut pos: usize,
    end: usize,
    data_end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let center_effect_id = t.center_effect_id;
    let center_effects = t.effects;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
    let mut prev_state: u32 = curr;
    let mut has_prev = false;
    while pos < end {
        unsafe {
            let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
            if has_prev {
                max_end =
                    fwd_center_update(center_effect_id, center_effects, prev_state, pos, max_end);
            }
            let delta = (curr << mt_log | mt) as usize;
            let next = *center_table.add(delta);
            if next == DFA_MISSING {
                return (curr, pos, max_end, true);
            }
            if next == DFA_DEAD {
                return (DFA_DEAD as u32, pos, max_end, false);
            }
            curr = next as u32;
            prev_state = curr;
            has_prev = true;
        }
        pos += 1;
    }
    if has_prev {
        if pos >= data_end {
            unsafe {
                let eid = *effects_id.add(prev_state as usize) as u32;
                if eid == 1 {
                    max_end = max_end.max(pos);
                } else if eid != 0 {
                    max_end =
                        collect_nulls_fwd_complex(t.effects, eid, pos, Nullability::END, max_end);
                }
            }
        } else {
            unsafe {
                max_end =
                    fwd_center_update(center_effect_id, center_effects, prev_state, pos, max_end);
            }
        }
    }
    (curr, pos, max_end, false)
}

#[inline(never)]
fn scan_fwd_verify<const SKIP: bool>(
    t: &ScanTables,
    effects_id: *const u16,
    skip_ids: &[u8],
    skip_searchers: &[MintermSearchValue],
    mut curr: u32,
    mut pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects = t.effects;
    let center_effect_id = t.center_effect_id;
    let center_effects = t.effects;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;

    'outer: while pos < end {
        if SKIP {
            #[cfg(any(
                target_arch = "x86_64",
                target_arch = "aarch64",
                all(target_arch = "wasm32", target_feature = "simd128")
            ))]
            {
                let sid = skip_ids[curr as usize];
                if sid != 0 {
                    let searcher = &skip_searchers[sid as usize - 1];
                    let haystack = unsafe { std::slice::from_raw_parts(data.add(pos), end - pos) };
                    match searcher.find_fwd(haystack) {
                        Some(offset) => {
                            // nullable self-loop: only need max position for max_end
                            if offset > 0 {
                                unsafe {
                                    max_end = fwd_center_update(
                                        center_effect_id,
                                        center_effects,
                                        curr,
                                        pos + offset,
                                        max_end,
                                    );
                                }
                            }
                            pos += offset;
                        }
                        None => {
                            // no non-self-loop byte: entire rest is self-loop (END boundary)
                            unsafe {
                                let eid = *effects_id.add(curr as usize) as u32;
                                if eid == 1 {
                                    max_end = max_end.max(end);
                                } else if eid != 0 {
                                    max_end = collect_nulls_fwd_complex(
                                        effects,
                                        eid,
                                        end,
                                        Nullability::END,
                                        max_end,
                                    );
                                }
                            }
                            return (curr, end, max_end, false);
                        }
                    }
                }
            }
        }

        let mut prev_state: u32 = curr;
        let mut has_prev = false;
        while pos < end {
            unsafe {
                let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
                if has_prev {
                    max_end = fwd_center_update(
                        center_effect_id,
                        center_effects,
                        prev_state,
                        pos,
                        max_end,
                    );
                }
                let delta = (curr << mt_log | mt) as usize;
                let next = *center_table.add(delta);
                if next == DFA_MISSING {
                    return (curr, pos, max_end, true);
                }
                if next == DFA_DEAD {
                    return (DFA_DEAD as u32, pos, max_end, false);
                }
                curr = next as u32;
                prev_state = curr;
                has_prev = true;
            }
            pos += 1;
            if SKIP && skip_ids[curr as usize] != 0 {
                // flush deferred prev_state before returning to skip loop
                if has_prev {
                    if pos >= end {
                        unsafe {
                            let eid = *effects_id.add(prev_state as usize) as u32;
                            if eid == 1 {
                                max_end = max_end.max(pos);
                            } else if eid != 0 {
                                max_end = collect_nulls_fwd_complex(
                                    effects,
                                    eid,
                                    pos,
                                    Nullability::END,
                                    max_end,
                                );
                            }
                        }
                    } else {
                        unsafe {
                            max_end = fwd_center_update(
                                center_effect_id,
                                center_effects,
                                prev_state,
                                pos,
                                max_end,
                            );
                        }
                    }
                }
                continue 'outer;
            }
        }
        if has_prev {
            // END boundary: use full effects for non-hot path.
            unsafe {
                let eid = *effects_id.add(prev_state as usize) as u32;
                if eid == 1 {
                    max_end = max_end.max(pos);
                } else if eid != 0 {
                    max_end =
                        collect_nulls_fwd_complex(effects, eid, pos, Nullability::END, max_end);
                }
            }
        }
        if !SKIP {
            break 'outer;
        }
    }
    (curr, pos, max_end, false)
}

#[inline(never)]
fn scan_fwd<const SKIP: bool>(
    t: &ScanTables,
    effects_id: *const u16,
    skip_ids: &[u8],
    skip_searchers: &[MintermSearchValue],
    mut l_state: u32,
    mut l_pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects = t.effects;
    let center_effect_id = t.center_effect_id;
    let center_effects = t.effects;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
    unsafe {
        while l_state != DFA_DEAD as u32 {
            if SKIP {
                #[cfg(any(
                    target_arch = "x86_64",
                    target_arch = "aarch64",
                    all(target_arch = "wasm32", target_feature = "simd128")
                ))]
                {
                    let sid = skip_ids[l_state as usize];
                    if sid != 0 {
                        let searcher = &skip_searchers[sid as usize - 1];
                        let haystack = std::slice::from_raw_parts(data.add(l_pos), end - l_pos);
                        match searcher.find_fwd(haystack) {
                            Some(offset) => {
                                if offset > 0 {
                                    max_end = fwd_center_update(
                                        center_effect_id,
                                        center_effects,
                                        l_state,
                                        l_pos + offset,
                                        max_end,
                                    );
                                }
                                l_pos += offset;
                            }
                            None => {
                                // no non-self-loop byte: entire rest is self-loop
                                let eid = *effects_id.add(l_state as usize) as u32;
                                if eid != 0 {
                                    if eid == 1 {
                                        max_end = max_end.max(end);
                                    } else {
                                        max_end = collect_nulls_fwd_complex(
                                            effects,
                                            eid,
                                            end,
                                            Nullability::END,
                                            max_end,
                                        );
                                    }
                                }
                                return (l_state, end, max_end, false);
                            }
                        }
                    }
                }
            }
            max_end = fwd_center_update(center_effect_id, center_effects, l_state, l_pos, max_end);
            let mt = *minterms_lookup.add(*data.add(l_pos) as usize) as u32;
            let delta = (l_state << mt_log | mt) as usize;
            let next = *center_table.add(delta) as u32;
            if next == DFA_MISSING as u32 {
                return (l_state, l_pos, max_end, true);
            }
            if next == DFA_DEAD as u32 {
                return (DFA_DEAD as u32, l_pos, max_end, false);
            }
            // keeping for debugging
            // eprintln!("[pos] {:?}; {}->{}", l_pos, l_state, next);
            l_state = next;
            l_pos += 1;
            if l_pos == end {
                let effect_id = *effects_id.add(l_state as usize) as u32;
                max_end =
                    collect_nulls_fwd_complex(effects, effect_id, l_pos, Nullability::END, max_end);
                l_state = DFA_DEAD as _;
            }
        }
    }
    (l_state, l_pos, max_end, false)
}

fn register_state(
    state_nodes: &mut Vec<NodeId>,
    node_to_state: &mut HashMap<NodeId, u16>,
    effects_id: &mut Vec<u16>,
    center_effect_id: &mut Vec<u16>,
    effects: &mut Vec<Vec<NullState>>,
    b: &mut RegexBuilder,
    node: NodeId,
) -> u16 {
    if let Some(&sid) = node_to_state.get(&node) {
        return sid;
    }
    let sid = state_nodes.len() as u16;
    state_nodes.push(node);
    node_to_state.insert(node, sid);
    let eff_id = b.get_nulls_id(node);
    let eid = b.center_nulls_id(eff_id);
    if sid as usize >= effects_id.len() {
        effects_id.resize(sid as usize + 1, 0u16);
    }
    if sid as usize >= center_effect_id.len() {
        center_effect_id.resize(sid as usize + 1, EID_NONE as u16);
    }
    effects_id[sid as usize] = eff_id.0 as u16;
    center_effect_id[sid as usize] = eid.0 as u16;
    while effects.len() <= eff_id.0 as usize || effects.len() <= eid.0 as usize {
        effects.push(b.nulls_entry_vec(effects.len() as u32));
    }
    sid
}

/// bounded DFA for opportunistic matching with known max_length.
/// only exists for a slight (20-30%) performance boost on short patterns
/// when two DFAs arent necessary
/// this is basically derivative based Aho-Corasick
pub struct BDFA {
    initial_node: NodeId,
    /// states as Counted node chains.
    pub states: Vec<NodeId>,
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
    minterms: Vec<TSetId>,
    /// byte -> minterm index.
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
    /// construct from a pattern node.
    pub fn new(b: &mut RegexBuilder, pattern_node: NodeId) -> Result<Self, Error> {
        let initial_node = b.mk_counted(pattern_node, NodeId::MISSING, 0);
        let sets = collect_sets(b, initial_node);
        let minterms = PartitionTree::generate_minterms(sets, b.solver());
        let minterms_lookup = PartitionTree::minterms_lookup(&minterms, b.solver());
        let num_mt = minterms.len();
        let mt_log = num_mt.next_power_of_two().trailing_zeros();
        let stride = 1usize << mt_log;

        // state 0 = uncached sentinel (unused), state 1 = MISSING (no active candidates)
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
        let mut prefix_sets = calc_prefix_sets_inner(b, pattern_node, false)?;
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
                None => return Ok(()), // shouldn't happen
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
        let sets = calc_potential_start(b, pattern_node, 16, 64, false)?;
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
        // PREFIX=1 (Teddy) transitions manually - no after_prefix needed
        self.prefix = Some(search);
        self.prefix_len = sets.len();
        Ok(())
    }

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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

    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    fn try_build_range_prefix(
        byte_sets_raw: &[Vec<u8>],
        anchor_pos: usize,
    ) -> Option<crate::accel::FwdPrefixSearch> {
        let anchor_bytes = &byte_sets_raw[anchor_pos];
        if !crate::prefix::skip_is_profitable(anchor_bytes) {
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

    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    fn build_prefix_search(_byte_sets_raw: &[Vec<u8>]) -> Option<crate::accel::FwdPrefixSearch> {
        None
    }

    /// best match rel from packed extra.
    pub fn counted_best(node: NodeId, b: &RegexBuilder) -> u32 {
        b.get_extra(node) >> 16
    }

    fn register(&mut self, node: NodeId, b: &RegexBuilder) -> u16 {
        if let Some(&sid) = self.state_map.get(&node) {
            return sid;
        }
        let sid = self.states.len() as u16;
        // walk chain to find best match among body=BOT nodes
        let mut match_step = 0u32;
        let mut match_best = 0u32;
        let mut cur = node;
        while cur.0 > NodeId::BOT.0 {
            debug_assert_eq!(b.get_kind(cur), Kind::Counted);
            let body = cur.left(b);
            if body == NodeId::BOT {
                let best = Self::counted_best(cur, b);
                if best > match_best {
                    let packed = b.get_extra(cur);
                    match_step = packed & 0xFFFF;
                    match_best = best;
                }
            }
            cur = cur.right(b);
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

    /// transition from state on minterm. returns packed (rel << 16 | next_state).
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
            debug_assert_eq!(b.get_kind(cur), Kind::Counted);
            let chain = cur.right(b);
            let body = cur.left(b);
            if body == NodeId::BOT {
                // skip dead wrappers, keep pending matches
                if Self::counted_best(cur, b) > 0 {
                    result.push(cur);
                }
                cur = chain;
                continue;
            }
            let der = b.der(cur, Nullability::CENTER).map_err(Error::Algebra)?;
            let next = transition_term(b, der, mt);
            if next != NodeId::BOT {
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
            let next = b.mk_counted(body, chain, packed);
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
