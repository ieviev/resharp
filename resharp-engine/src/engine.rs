use std::collections::{BTreeSet, HashMap, HashSet};

use resharp_algebra::nulls::{NullState, Nullability};
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder, TRegex, TRegexId};

use crate::accel::MintermSearchValue;
use crate::{Error, Match};

pub const NO_MATCH: usize = usize::MAX;
pub const DFA_DEAD: u32 = 1; // 0 = missing

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
    loop {
        match stack.pop() {
            Some(node_id) => {
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
                    Kind::Lookahead | Kind::Lookbehind => {
                        stack.push(node_id.left(b));
                        stack.push(node_id.right(b));
                    }
                    Kind::Star | Kind::Compl => {
                        stack.push(node_id.left(b));
                    }
                }
            }
            None => break,
        }
    }
    sets
}

fn collect_derivative_targets(
    b: &mut RegexBuilder,
    der: TRegexId,
    path_set: TSetId,
    targets: &mut Vec<(NodeId, TSetId)>,
) {
    let term = b.get_tregex(der).clone();
    match term {
        TRegex::Leaf(target) => {
            if let Some(entry) = targets.iter_mut().find(|(t, _)| *t == target) {
                entry.1 = b.solver().or_id(entry.1, path_set);
            } else {
                targets.push((target, path_set));
            }
        }
        TRegex::ITE(cond, then_branch, else_branch) => {
            let then_path = b.solver().and_id(path_set, cond);
            collect_derivative_targets(b, then_branch, then_path, targets);
            let not_cond = b.solver().not_id(cond);
            let else_path = b.solver().and_id(path_set, not_cond);
            collect_derivative_targets(b, else_branch, else_path, targets);
        }
    }
}

/// strip leading stars and lookbehinds from reversed pattern for prefix computation.
fn get_prefix_node(b: &mut RegexBuilder, node: NodeId) -> NodeId {
    match b.get_kind(node) {
        Kind::Concat => {
            let head = node.left(b);
            let tail = node.right(b);
            match b.get_kind(head) {
                Kind::Star => get_prefix_node(b, tail),
                Kind::Lookbehind => {
                    let lb_inner = b.get_lookbehind_inner(head);
                    let replaced = b.mk_concat(lb_inner, tail);
                    get_prefix_node(b, replaced)
                }
                _ => node,
            }
        }
        Kind::Lookahead => node.left(b),
        _ => node,
    }
}

fn calc_prefix_sets_inner(
    b: &mut RegexBuilder,
    start: NodeId,
    strip_prefix: bool,
) -> Result<Vec<TSetId>, crate::Error> {
    let mut result = Vec::new();
    let prefix_start = if strip_prefix {
        get_prefix_node(b, start)
    } else {
        start
    };
    let mut node = prefix_start;
    let mut redundant = BTreeSet::new();
    redundant.insert(NodeId::BOT);
    redundant.insert(start);
    redundant.insert(prefix_start);

    loop {
        if !result.is_empty() && redundant.contains(&node) {
            break;
        }
        if b.any_nonbegin_nullable(node) {
            break;
        }

        let der = b
            .der(node, Nullability::CENTER)
            .map_err(crate::Error::Algebra)?;
        let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
        collect_derivative_targets(b, der, TSetId::FULL, &mut targets);

        targets.retain(|(t, _)| !redundant.contains(t));

        if targets.len() == 1 {
            let (target, char_set) = targets[0];
            if target == node {
                result.clear();
                break;
            }
            result.push(char_set);
            node = target;
        } else {
            break;
        }
    }

    Ok(result)
}

/// linear prefix character sets from the reversed pattern.
pub fn calc_prefix_sets(
    b: &mut RegexBuilder,
    rev_start: NodeId,
) -> Result<Vec<TSetId>, crate::Error> {
    calc_prefix_sets_inner(b, rev_start, true)
}

/// prefix walk
pub fn calc_potential_start(
    b: &mut RegexBuilder,
    rev_start: NodeId,
    max_prefix_len: usize,
    max_frontier_size: usize,
) -> Result<Vec<TSetId>, crate::Error> {
    let start = get_prefix_node(b, rev_start);
    let mut nodes: BTreeSet<NodeId> = BTreeSet::new();
    let mut redundant: BTreeSet<NodeId> = BTreeSet::new();
    redundant.insert(NodeId::BOT);
    nodes.insert(start);
    redundant.insert(start);

    let mut result = Vec::new();

    loop {
        if nodes.is_empty() || nodes.len() > max_frontier_size || result.len() >= max_prefix_len {
            break;
        }

        if nodes.iter().any(|&n| b.any_nonbegin_nullable(n)) {
            break;
        }

        let mut union_set = TSetId::EMPTY;
        let mut next_nodes: BTreeSet<NodeId> = BTreeSet::new();

        for &node in &nodes.clone() {
            let der = b
                .der(node, Nullability::CENTER)
                .map_err(crate::Error::Algebra)?;
            let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
            collect_derivative_targets(b, der, TSetId::FULL, &mut targets);

            for &(target, char_set) in &targets {
                if !redundant.contains(&target) {
                    union_set = b.solver().or_id(union_set, char_set);
                    next_nodes.insert(target);
                }
            }
        }

        if next_nodes.is_empty() || union_set == TSetId::EMPTY {
            break;
        }

        result.push(union_set);
        nodes = next_nodes;
    }

    Ok(result)
}

/// build a forward prefix search, picking the rarest BFS position for memchr.
pub fn build_fwd_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    if !crate::simd::has_simd() {
        return Ok(None);
    }

    build_fwd_prefix_simd(b, node)
}

#[cfg(target_arch = "x86_64")]
fn build_fwd_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let sets = calc_potential_start(b, node, 16, 64)?;
    if sets.is_empty() {
        return Ok(None);
    }

    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();

    // pure literal: every position is a single byte - use memchr + memcmp
    if byte_sets_raw.iter().all(|bs| bs.len() == 1) {
        let needle: Vec<u8> = byte_sets_raw.iter().map(|bs| bs[0]).collect();
        return Ok(Some(crate::accel::FwdPrefixSearch::Literal(
            crate::simd::FwdLiteralSearch::new(&needle),
        )));
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
        return Ok(None);
    }
    freqs.sort_by_key(|&(_, f)| f);

    if byte_sets_raw[freqs[0].0].len() > 16 {
        return Ok(None);
    }

    let freq_order: Vec<usize> = freqs.iter().map(|&(i, _)| i).collect();

    let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();

    Ok(Some(crate::accel::FwdPrefixSearch::Prefix(
        crate::simd::FwdPrefixSearch::new(sets.len(), &freq_order, &byte_sets_raw, all_sets),
    )))
}

#[cfg(not(target_arch = "x86_64"))]
fn build_fwd_prefix_simd(
    _b: &mut RegexBuilder,
    _node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    Ok(None)
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

pub struct LazyDFA {
    pub initial: u16,
    pub begin_table: Vec<u16>,
    pub center_table: Vec<u16>,
    pub effects_id: Vec<u16>,
    pub effects: Vec<Vec<NullState>>,
    pub num_minterms: u32,
    pub minterms_lookup: [u8; 256],

    pub minterms: Vec<TSetId>,
    pub state_nodes: Vec<NodeId>,
    pub node_to_state: HashMap<NodeId, u16>,
    pub skip_ids: Vec<u8>,
    pub skip_searchers: Vec<MintermSearchValue>,
    pub prefix_skip: Option<crate::accel::RevPrefixSearch>,
    pub prefix_transition: u32,
    pub max_capacity: usize,
}

impl LazyDFA {
    pub fn new(
        b: &mut RegexBuilder,
        initial: NodeId,
        max_capacity: usize,
    ) -> Result<LazyDFA, Error> {
        let sets = collect_sets(b, initial);
        let minterms = PartitionTree::generate_minterms(sets, b.solver());
        let u8_lookup = PartitionTree::minterms_lookup(&minterms, b.solver());

        let max_capacity = max_capacity.min(65535);

        // state 0 = uncomputed, state 1 = dead
        let mut state_nodes: Vec<NodeId> = vec![NodeId::MISSING, NodeId::BOT];
        let mut node_to_state: HashMap<NodeId, u16> = HashMap::new();
        node_to_state.insert(NodeId::BOT, DFA_DEAD as u16);

        let mut effects_id: Vec<u16> = vec![0u16; 2]; // slots 0,1

        // register initial state
        let initial_sid = state_nodes.len() as u16;
        state_nodes.push(initial);
        node_to_state.insert(initial, initial_sid);
        let initial_eff_id = b.get_nulls_id(initial);
        effects_id.push(initial_eff_id.0 as u16);

        let der0 = b.der(initial, Nullability::BEGIN)?;
        let mut begin_table = vec![DFA_DEAD as u16; minterms.len()];
        for (idx, mt) in minterms.iter().enumerate() {
            let t = transition_term(b, der0, *mt);
            let sid = register_state(&mut state_nodes, &mut node_to_state, &mut effects_id, b, t);
            if state_nodes.len() > max_capacity {
                return Err(Error::CapacityExceeded);
            }
            begin_table[idx] = sid;
        }
        let num_minterms = minterms.len() as u32;
        let center_table_size = state_nodes.len() * num_minterms as usize;
        let center_table = vec![0u16; center_table_size];

        let effects = b.nulls_as_vecs();

        let skip_ids = vec![0u8; state_nodes.len()];

        Ok(LazyDFA {
            initial: initial_sid,
            begin_table,
            center_table,
            effects_id,
            effects,
            num_minterms,
            minterms_lookup: u8_lookup,
            minterms,
            state_nodes,
            node_to_state,
            skip_ids,
            skip_searchers: Vec::new(),
            prefix_skip: None,
            prefix_transition: 0,
            max_capacity,
        })
    }

    #[inline(always)]
    pub fn dfa_delta(&self, state_id: u16, mt: u32) -> usize {
        (state_id as u32 * self.num_minterms + mt) as usize
    }

    pub fn ensure_capacity(&mut self, state_id: u16) {
        let cap = state_id as usize + 1;
        if cap > self.effects_id.len() {
            let new_len = self.effects_id.len().max(4) * 2;
            let new_len = new_len.max(cap);
            self.effects_id.resize(new_len, 0u16);
        }
        let needed = cap * self.num_minterms as usize;
        if needed > self.center_table.len() {
            let new_len = self.center_table.len().max(4) * 2;
            let new_len = new_len.max(needed);
            self.center_table.resize(new_len, 0u16);
        }
        if cap > self.skip_ids.len() {
            let new_len = self.skip_ids.len().max(4) * 2;
            let new_len = new_len.max(cap);
            self.skip_ids.resize(new_len, 0u8);
        }
    }

    pub fn get_or_register(&mut self, b: &RegexBuilder, node: NodeId) -> u16 {
        register_state(
            &mut self.state_nodes,
            &mut self.node_to_state,
            &mut self.effects_id,
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
        if delta < self.center_table.len() && self.center_table[delta] != 0 {
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
        if state_id == DFA_DEAD as u16 {
            return Ok(DFA_DEAD as u16);
        }

        let node = self.state_nodes[state_id as usize];
        let sder = b.der(node, Nullability::CENTER).map_err(Error::Algebra)?;
        let mt = self.minterms[minterm_idx as usize];
        let next_node = transition_term(b, sder, mt);
        if self.state_nodes.len() >= self.max_capacity {
            return Err(Error::CapacityExceeded);
        }
        let next_sid = self.get_or_register(b, next_node);
        self.ensure_capacity(next_sid);
        self.sync_effects(b);

        let delta = self.dfa_delta(state_id, minterm_idx);
        self.center_table[delta] = next_sid;

        Ok(next_sid)
    }

    fn sync_effects(&mut self, b: &RegexBuilder) {
        let n = b.nulls_count();
        while self.effects.len() < n {
            self.effects
                .push(b.nulls_entry_vec(self.effects.len() as u32));
        }
    }

    pub fn precompile(&mut self, b: &mut RegexBuilder, threshold: usize) -> bool {
        use std::collections::VecDeque;
        let mut todo: VecDeque<u16> = VecDeque::new();
        let mut visited = HashSet::new();

        for &sid in &self.begin_table {
            if sid > DFA_DEAD as u16 {
                todo.push_back(sid);
            }
        }

        while let Some(sid) = todo.pop_front() {
            if visited.contains(&sid) {
                continue;
            }
            if visited.len() >= threshold {
                return false;
            }
            visited.insert(sid);

            let node = self.state_nodes[sid as usize];
            self.ensure_capacity(sid);

            for mt_idx in 0..self.minterms.len() {
                let sder = match b.der(node, Nullability::CENTER) {
                    Ok(d) => d,
                    Err(_) => {
                        return false;
                    }
                };
                let mt = self.minterms[mt_idx];
                let next_node = transition_term(b, sder, mt);
                let next_sid = self.get_or_register(b, next_node);
                self.ensure_capacity(next_sid);
                let delta = self.dfa_delta(sid, mt_idx as u32);
                self.center_table[delta] = next_sid;
                if !visited.contains(&next_sid) {
                    todo.push_back(next_sid);
                }
            }
        }

        self.sync_effects(b);
        self.build_skip_info(&visited);

        true
    }

    fn precompile_state(&mut self, b: &mut RegexBuilder, state_id: u16) -> Result<(), Error> {
        if state_id == DFA_DEAD as u16 {
            return Ok(());
        }
        let node = self.state_nodes[state_id as usize];
        let sder = b.der(node, Nullability::CENTER).map_err(Error::Algebra)?;
        for mt_idx in 0..self.minterms.len() {
            let delta = self.dfa_delta(state_id, mt_idx as u32);
            if delta < self.center_table.len() && self.center_table[delta] != 0 {
                continue;
            }
            let mt = self.minterms[mt_idx];
            let next_node = transition_term(b, sder, mt);
            let next_sid = self.get_or_register(b, next_node);
            self.ensure_capacity(next_sid);
            let delta = self.dfa_delta(state_id, mt_idx as u32);
            self.center_table[delta] = next_sid;
        }
        self.sync_effects(b);
        Ok(())
    }

    fn build_skip_info(&mut self, visited: &HashSet<u16>) {
        for &sid in visited {
            self.try_build_skip(sid as usize);
        }
    }

    fn try_build_skip(&mut self, _state: usize) {
        #[cfg(target_arch = "x86_64")]
        if crate::simd::has_simd() {
            self.try_build_skip_simd(_state);
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn try_build_skip_simd(&mut self, state: usize) {
        let num_mt = self.num_minterms as usize;
        if state >= self.skip_ids.len() || self.skip_ids[state] != 0 {
            return;
        }
        // nullable states must record positions - can't skip
        if state < self.effects_id.len() && self.effects_id[state] != 0 {
            return;
        }
        let base = state * num_mt;
        if base + num_mt > self.center_table.len() {
            return;
        }
        let row = &self.center_table[base..base + num_mt];
        let zeros = row.iter().filter(|&&x| x == 0).count();
        if zeros > 0 {
            return;
        }
        let self_id = state as u16;
        let mut non_self_mts = Vec::new();
        for (mt, &next) in row.iter().enumerate() {
            if next != self_id {
                non_self_mts.push(mt);
            }
        }
        if non_self_mts.is_empty() {
            return;
        }
        if non_self_mts.len() > 3 {
            return;
        }
        let mut bytes = Vec::new();
        for &mt in &non_self_mts {
            for (byte, &m) in self.minterms_lookup.iter().enumerate() {
                if m as usize == mt {
                    bytes.push(byte as u8);
                }
            }
        }
        if bytes.len() >= 1 && bytes.len() <= 3 {
            self.skip_ids[state] = self.get_or_create_skip(bytes);
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn get_or_create_skip(&mut self, mut bytes: Vec<u8>) -> u8 {
        bytes.sort();
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if s.bytes() == &bytes {
                return (i + 1) as u8;
            }
        }
        self.skip_searchers.push(MintermSearchValue::new(bytes));
        self.skip_searchers.len() as u8
    }

    pub fn scan_fwd(
        &mut self,
        b: &mut RegexBuilder,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<usize, Error> {
        let has_empty = has_any_null(
            &self.effects_id,
            &self.effects,
            self.initial as u32,
            Nullability::BEGIN,
        );

        let mt = self.minterms_lookup[data[pos_begin] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD {
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
        collect_nulls_fwd(
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
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                num_mt: self.num_minterms,
            };

            let (state, new_pos, new_max, cache_miss) =
                scan_fwd_noskip(&tables, curr, pos, end, max_end);

            max_end = new_max;

            if !cache_miss {
                break;
            }

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, state as u16, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD {
                break;
            }

            let mask = if pos == end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls_fwd(
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
        let end = data.len();
        if end == 0 || nulls.is_empty() {
            return Ok(());
        }

        let mut skip_until = 0usize;

        for &begin_pos in nulls.iter().rev() {
            if begin_pos < skip_until || begin_pos >= end {
                continue;
            }

            let mt = self.minterms_lookup[data[begin_pos] as usize];
            let mut curr = self.begin_table[mt as usize] as u32;
            if curr <= DFA_DEAD {
                continue;
            }

            let mut pos = begin_pos + 1;
            let mut max_end: usize = 0;

            let init_mask = if pos >= end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls_fwd(
                &self.effects_id,
                &self.effects,
                curr,
                pos,
                init_mask,
                &mut max_end,
            );

            if pos < end {
                loop {
                    let tables = ScanTables {
                        center_table: self.center_table.as_ptr(),
                        effects_id: self.effects_id.as_ptr(),
                        effects: self.effects.as_ptr(),
                        data: data.as_ptr(),
                        minterms_lookup: self.minterms_lookup.as_ptr(),
                        num_mt: self.num_minterms,
                    };

                    let (state, new_pos, new_max, cache_miss) =
                        scan_fwd_noskip(&tables, curr, pos, end, max_end);
                    max_end = new_max;

                    if !cache_miss {
                        break;
                    }

                    let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
                    curr = self.lazy_transition(b, state as u16, mt)? as u32;
                    pos = new_pos + 1;
                    if curr <= DFA_DEAD {
                        break;
                    }

                    let mask = if pos >= end {
                        Nullability::END
                    } else {
                        Nullability::CENTER
                    };
                    collect_nulls_fwd(
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
            }

            if max_end > 0 {
                matches.push(Match {
                    start: begin_pos,
                    end: max_end,
                });
                skip_until = if max_end > begin_pos {
                    max_end
                } else {
                    begin_pos + 1
                };
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
        let mt = self.minterms_lookup[data[pos] as usize];
        let mut state = self.begin_table[mt as usize];
        if state <= DFA_DEAD as u16 {
            return Ok(0);
        }
        for i in 1..len {
            let mt = self.minterms_lookup[data[pos + i] as usize] as u32;
            state = self.lazy_transition(b, state, mt)?;
            if state <= DFA_DEAD as u16 {
                return Ok(0);
            }
        }
        Ok(state as u32)
    }

    /// scan forward from a precomputed DFA state, skipping begin_table lookup.
    pub fn scan_fwd_from(
        &mut self,
        b: &mut RegexBuilder,
        state: u32,
        pos_begin: usize,
        data: &[u8],
    ) -> Result<usize, Error> {
        if state <= DFA_DEAD {
            return Ok(NO_MATCH);
        }
        let end = data.len();
        let mut pos = pos_begin;
        let mut curr = state;
        let mut max_end: usize = 0;

        collect_nulls_fwd(
            &self.effects_id,
            &self.effects,
            curr,
            pos,
            Nullability::CENTER,
            &mut max_end,
        );

        if pos >= end {
            collect_nulls_fwd(
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
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                num_mt: self.num_minterms,
            };

            let (state_out, new_pos, new_max, cache_miss) =
                scan_fwd_noskip(&tables, curr, pos, end, max_end);
            max_end = new_max;

            if !cache_miss {
                break;
            }

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, state_out as u16, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD {
                break;
            }

            let mask = if pos >= end {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls_fwd(
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

    pub fn compute_skip(&mut self, b: &mut RegexBuilder, rev_start: NodeId) -> Result<(), Error> {
        if !crate::simd::has_simd() {
            return Ok(());
        }
        self.compute_skip_simd(b, rev_start)
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn compute_skip_simd(&mut self, _b: &mut RegexBuilder, _rev_start: NodeId) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    fn compute_skip_simd(&mut self, b: &mut RegexBuilder, rev_start: NodeId) -> Result<(), Error> {
        // bail if rev_start is nullable - prefix skip would miss nullable paths
        if b.get_nulls_id(rev_start) != resharp_algebra::nulls::NullsId::EMPTY {
            return Ok(());
        }
        let prefix = calc_prefix_sets(b, rev_start)?;

        let sets = if !prefix.is_empty() {
            prefix
        } else {
            calc_potential_start(b, rev_start, 16, 64)?
        };

        if sets.is_empty() {
            return Ok(());
        }

        if sets.len() >= 2 && !b.contains_look(rev_start) {
            let byte_sets_raw: Vec<Vec<u8>> = sets
                .iter()
                .map(|&set| b.solver().collect_bytes(set))
                .collect();
            let num_simd = sets.len().min(3);
            let rarest_simd = byte_sets_raw[..num_simd]
                .iter()
                .map(|bs| bs.len())
                .min()
                .unwrap_or(256);
            if rarest_simd > 12 {
                return Ok(());
            }
            let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
                .iter()
                .map(|bytes| crate::accel::TSet::from_bytes(bytes))
                .collect();
            // pre-compute the DFA state after consuming all prefix bytes
            let mut current_node = self.state_nodes[self.initial as usize];
            for &set in &sets {
                let der = b
                    .der(current_node, Nullability::CENTER)
                    .map_err(crate::Error::Algebra)?;
                let mt = self
                    .minterms
                    .iter()
                    .find(|&&mt| b.solver().is_sat_id(mt, set))
                    .copied()
                    .expect("prefix set must intersect some minterm");
                current_node = transition_term(b, der, mt);
            }
            let sid = self.get_or_register(b, current_node);
            self.ensure_capacity(sid);
            self.prefix_transition = sid as u32;

            self.prefix_skip = Some(crate::accel::RevPrefixSearch::new(
                sets.len(),
                &byte_sets_raw,
                all_sets,
            ));
        } else {
            let bytes = b.solver().collect_bytes(sets[0]);
            let ini = self.initial as usize;
            if bytes.len() >= 1
                && bytes.len() <= 3
                && (ini >= self.effects_id.len() || self.effects_id[ini] == 0)
            {
                if self.skip_ids.len() <= ini {
                    self.skip_ids.resize(ini + 1, 0u8);
                }
                self.skip_ids[ini] = self.get_or_create_skip(bytes);
            }
        }

        Ok(())
    }

    fn can_skip(&self) -> bool {
        !self.skip_searchers.is_empty()
    }

    pub fn collect_rev(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        if self.prefix_skip.is_some() {
            let prefix_ptr =
                self.prefix_skip.as_ref().unwrap() as *const crate::accel::RevPrefixSearch;
            return self.collect_rev_prefix(b, prefix_ptr, start_pos, data, nulls);
        }

        let mt = self.minterms_lookup[data[start_pos] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD {
            return Ok(());
        }

        let begin_mask = if start_pos == 0 {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        collect_nulls(
            &self.effects_id,
            &self.effects,
            curr,
            start_pos,
            begin_mask,
            nulls,
        );

        let mut pos = start_pos;

        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                num_mt: self.num_minterms,
            };

            let use_skip = self.can_skip();
            let (state, new_pos, cache_miss) = if use_skip {
                collect_rev_skip(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    data,
                    nulls,
                )
            } else {
                collect_rev_noskip(&tables, curr, pos, nulls)
            };

            if !cache_miss {
                if cfg!(feature = "debug-nulls") {
                    eprintln!(
                        "  [collect_rev] no cache miss, state={} pos={}",
                        state, new_pos
                    );
                }
                break;
            }

            if cfg!(feature = "debug-nulls") {
                eprintln!("  [collect_rev] CACHE MISS state={} pos={}", state, new_pos);
            }

            let sid = state as u16;
            self.precompile_state(b, sid)?;
            self.try_build_skip(sid as usize);

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            let delta = self.dfa_delta(sid, mt);
            curr = self.center_table[delta] as u32;
            pos = new_pos;

            if curr <= DFA_DEAD {
                break;
            }

            let mask = if pos == 0 {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);
        }

        Ok(())
    }

    fn collect_rev_prefix(
        &mut self,
        b: &mut RegexBuilder,
        prefix_ptr: *const crate::accel::RevPrefixSearch,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        let prefix_skip = unsafe { &*prefix_ptr };
        let prefix_len = prefix_skip.len();

        let Some(match_pos) = prefix_skip.find_rev(data, start_pos) else {
            return Ok(());
        };

        // begin_table (BEGIN derivative) can only be used at start_pos.
        // when match_pos < start_pos, the skipped bytes are in _* and the
        // DFA stays in the initial state, so we start from self.initial as u32.
        let mut curr;
        let mut pos;
        if match_pos == start_pos {
            let mt = self.minterms_lookup[data[start_pos] as usize];
            curr = self.begin_table[mt as usize] as u32;
            if curr <= DFA_DEAD {
                return Ok(());
            }
            pos = match_pos;
        } else {
            curr = self.initial as u32;
            pos = match_pos + 1;
        }
        // walk prefix bytes through center_table
        let prefix_end = match_pos + 1 - prefix_len;
        while pos > prefix_end {
            pos -= 1;
            let mt = self.minterms_lookup[data[pos] as usize] as u32;
            let delta = self.dfa_delta(curr as u16, mt);
            if delta >= self.center_table.len() || self.center_table[delta] == 0 {
                self.precompile_state(b, curr as u16)?;
                let delta = self.dfa_delta(curr as u16, mt);
                curr = self.center_table[delta] as u32;
            } else {
                curr = self.center_table[delta] as u32;
            }
            if curr <= DFA_DEAD {
                return Ok(());
            }
        }

        let mask = if pos == 0 {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);

        if pos == 0 {
            return Ok(());
        }

        // continue from pos to 0
        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                num_mt: self.num_minterms,
            };

            let use_skip = self.can_skip();
            let (state, new_pos, cache_miss) = if use_skip {
                collect_rev_skip(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    data,
                    nulls,
                )
            } else {
                collect_rev_noskip(&tables, curr, pos, nulls)
            };

            if !cache_miss {
                break;
            }

            let sid = state as u16;
            self.precompile_state(b, sid)?;
            self.try_build_skip(sid as usize);

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            let delta = self.dfa_delta(sid, mt);
            curr = self.center_table[delta] as u32;
            pos = new_pos;

            if curr <= DFA_DEAD {
                break;
            }

            let mask = if pos == 0 {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);
        }

        Ok(())
    }

    pub fn any_nullable_rev(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
    ) -> Result<bool, Error> {
        let mt = self.minterms_lookup[data[start_pos] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD {
            return Ok(false);
        }

        if has_any_null(&self.effects_id, &self.effects, curr, Nullability::CENTER) {
            return Ok(true);
        }

        let mut pos = start_pos;

        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                num_mt: self.num_minterms,
            };

            let (state, new_pos, found, cache_miss) = any_null_rev_noskip(&tables, curr, pos);

            if found {
                return Ok(true);
            }

            if !cache_miss {
                curr = state;
                pos = new_pos;
                break;
            }

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, state as u16, mt)? as u32;
            pos = new_pos;
            if curr <= DFA_DEAD {
                break;
            }

            if has_any_null(&self.effects_id, &self.effects, curr, Nullability::CENTER) {
                return Ok(true);
            }
        }

        if pos == 0 && curr > DFA_DEAD {
            if has_any_null(&self.effects_id, &self.effects, curr, Nullability::END) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

fn has_any_null(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    mask: Nullability,
) -> bool {
    let eid = effects_id[state as usize] as u32;
    if eid == 0 {
        return false;
    }
    if eid == 1 {
        return mask.has(Nullability::ALWAYS);
    }
    effects[eid as usize].iter().any(|n| n.mask.has(mask))
}

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
        if eid == 1 {
            if mask.has(Nullability::ALWAYS) {
                if cfg!(feature = "debug-nulls") {
                    eprintln!(
                        "  [collect_nulls] state={} pos={} eid=1 push={}",
                        state, pos, pos
                    );
                }
                nulls.push(pos);
            }
            return;
        }
        for n in &effects[eid as usize] {
            if n.mask.has(mask) {
                if cfg!(feature = "debug-nulls") {
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

struct ScanTables {
    center_table: *const u16,
    effects_id: *const u16,
    effects: *const Vec<NullState>,
    data: *const u8,
    minterms_lookup: *const u8,
    num_mt: u32,
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

#[inline(always)]
fn collect_nulls_fwd(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    max_end: &mut usize,
) {
    let eid = effects_id[state as usize] as u32;
    if eid != 0 {
        if eid == 1 {
            if mask.has(Nullability::ALWAYS) {
                *max_end = (*max_end).max(pos);
            }
            return;
        }
        for n in &effects[eid as usize] {
            if n.mask.has(mask) {
                *max_end = (*max_end).max(pos - n.rel as usize);
            }
        }
    }
}

#[inline(never)]
fn collect_rev_noskip(
    t: &ScanTables,
    mut curr: u32,
    mut pos: usize,
    nulls: &mut Vec<usize>,
) -> (u32, usize, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let num_mt = t.num_mt;
    let mut prev_eid: u32 = 0;
    while pos != 0 {
        pos -= 1;

        unsafe {
            let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
            if prev_eid != 0 {
                if prev_eid == 1 {
                    if cfg!(feature = "debug-nulls") {
                        eprintln!(
                            "  [rev_noskip] state={} pos={} eid=1 push={}",
                            curr,
                            pos,
                            pos + 1
                        );
                    }
                    nulls.push(pos + 1);
                } else {
                    if cfg!(feature = "debug-nulls") {
                        eprintln!(
                            "  [rev_noskip] state={} pos={} eid={} push_complex at {}",
                            curr,
                            pos,
                            prev_eid,
                            pos + 1
                        );
                    }
                    collect_rev_complex(t.effects, prev_eid, pos + 1, Nullability::CENTER, nulls);
                }
            }
            let delta = (curr * num_mt + mt) as usize;
            let next = *center_table.add(delta);
            if next == 0 {
                return (curr, pos, true); // cache miss
            }
            if next == DFA_DEAD as u16 {
                return (DFA_DEAD, pos, false); // dead state
            }
            curr = next as u32;
            prev_eid = *effects_id.add(curr as usize) as u32;
        }
    }

    if prev_eid != 0 {
        if prev_eid == 1 {
            nulls.push(0);
        } else {
            collect_rev_complex(t.effects, prev_eid, 0, Nullability::END, nulls);
        }
    }
    (curr, 0, false)
}

#[inline(never)]
fn any_null_rev_noskip(t: &ScanTables, mut curr: u32, mut pos: usize) -> (u32, usize, bool, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let minterms_lookup = t.minterms_lookup;
    let num_mt = t.num_mt;
    while pos != 0 {
        pos -= 1;

        unsafe {
            let mt = *minterms_lookup.add(*t.data.add(pos) as usize) as u32;
            let delta = (curr * num_mt + mt) as usize;
            let next = *center_table.add(delta);
            if next == 0 {
                return (curr, pos, false, true); // cache miss
            }
            if next == DFA_DEAD as u16 {
                return (DFA_DEAD, pos, false, false); // dead state
            }
            curr = next as u32;
            let eid = *effects_id.add(curr as usize) as u32;
            if eid != 0 {
                if eid == 1 {
                    return (curr, pos, true, false);
                }
                let effects_vec = &*t.effects.add(eid as usize);
                if effects_vec.iter().any(|n| n.mask.has(Nullability::CENTER)) {
                    return (curr, pos, true, false);
                }
            }
        }
    }
    (curr, 0, false, false)
}

#[inline(never)]
fn collect_rev_skip(
    t: &ScanTables,
    skip_ids: &[u8],
    skip_searchers: &[MintermSearchValue],
    mut curr: u32,
    mut pos: usize,
    data: &[u8],
    nulls: &mut Vec<usize>,
) -> (u32, usize, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let effects = t.effects;
    let minterms_lookup = t.minterms_lookup;
    let num_mt = t.num_mt;
    while pos != 0 {
        let sid = skip_ids[curr as usize];
        if sid != 0 {
            let searcher = &skip_searchers[sid as usize - 1];
            let old_pos = pos;
            match searcher.find_rev(&data[..pos]) {
                Some(skip_pos) => {
                    pos = skip_pos + 1;
                }
                None => {
                    pos = 1;
                }
            }
            if cfg!(feature = "debug-nulls") {
                eprintln!("  [rev_skip] state={} skip {} -> {}", curr, old_pos, pos);
            }
        }

        let mut prev_eid: u32 = 0;
        while pos != 0 {
            pos -= 1;
            unsafe {
                let mt = *minterms_lookup.add(*t.data.add(pos) as usize) as u32;
                if prev_eid != 0 {
                    if prev_eid == 1 {
                        nulls.push(pos + 1);
                    } else {
                        collect_rev_complex(effects, prev_eid, pos + 1, Nullability::CENTER, nulls);
                    }
                }
                let delta = (curr * num_mt + mt) as usize;
                let next = *center_table.add(delta);
                if next == 0 {
                    return (curr, pos, true); // cache miss
                }
                if next == DFA_DEAD as u16 {
                    return (DFA_DEAD, pos, false); // dead state
                }
                curr = next as u32;
                prev_eid = *effects_id.add(curr as usize) as u32;
            }
            if skip_ids[curr as usize] != 0 {
                break;
            }
        }
        if pos == 0 && prev_eid != 0 {
            if prev_eid == 1 {
                nulls.push(0);
            } else {
                collect_rev_complex(effects, prev_eid, 0, Nullability::END, nulls);
            }
        }
    }
    (curr, 0, false)
}

#[cold]
#[inline(never)]
fn scan_fwd_complex(
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

#[inline(never)]
fn scan_fwd_noskip(
    t: &ScanTables,
    mut curr: u32,
    mut pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let num_mt = t.num_mt;
    let mut prev_eid: u32 = 0;
    while pos < end {
        unsafe {
            let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
            if prev_eid != 0 {
                if prev_eid == 1 {
                    max_end = max_end.max(pos);
                } else {
                    max_end =
                        scan_fwd_complex(t.effects, prev_eid, pos, Nullability::CENTER, max_end);
                }
            }
            let delta = (curr * num_mt + mt) as usize;
            let next = *center_table.add(delta);
            if next == 0 {
                return (curr, pos, max_end, true); // cache miss
            }
            if next == DFA_DEAD as u16 {
                return (DFA_DEAD, pos, max_end, false); // dead state
            }
            curr = next as u32;
            prev_eid = *effects_id.add(curr as usize) as u32;
        }
        pos += 1;
    }
    if prev_eid != 0 {
        if prev_eid == 1 {
            max_end = max_end.max(pos);
        } else {
            max_end = scan_fwd_complex(t.effects, prev_eid, pos, Nullability::END, max_end);
        }
    }
    (curr, pos, max_end, false)
}

/// register a node as a DFA state, returning its StateId (u16).
fn register_state(
    state_nodes: &mut Vec<NodeId>,
    node_to_state: &mut HashMap<NodeId, u16>,
    effects_id: &mut Vec<u16>,
    b: &RegexBuilder,
    node: NodeId,
) -> u16 {
    if let Some(&sid) = node_to_state.get(&node) {
        return sid;
    }
    let sid = state_nodes.len() as u16;
    state_nodes.push(node);
    node_to_state.insert(node, sid);
    let eff_id = b.get_nulls_id(node);
    if sid as usize >= effects_id.len() {
        effects_id.resize(sid as usize + 1, 0u16);
    }
    effects_id[sid as usize] = eff_id.0 as u16;
    sid
}
