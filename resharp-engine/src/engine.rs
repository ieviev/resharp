use std::collections::{BTreeSet, HashMap, HashSet};

use resharp_algebra::nulls::{NullState, Nullability};
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder, TRegex, TRegexId};

use crate::accel::MintermSearchValue;
use crate::{Error, Match};

pub const NO_MATCH: usize = usize::MAX;
pub const DFA_MISSING: u16 = 0;
pub const DFA_DEAD: u16 = 1;

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

/// prefix character sets from the reversed pattern.
pub fn calc_prefix_sets(
    b: &mut RegexBuilder,
    rev_start: NodeId,
) -> Result<Vec<TSetId>, crate::Error> {
    calc_prefix_sets_inner(b, rev_start, true)
}

/// prefix walk for potential match start positions.
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

/// fwd literal prefix for patterns with no star-stripping.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub fn build_strict_literal_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let sets = calc_prefix_sets_inner(b, node, false)?;
    if sets.is_empty() {
        return Ok(None);
    }
    let byte_sets: Vec<Vec<u8>> = sets.iter().map(|&s| b.solver().collect_bytes(s)).collect();
    if !byte_sets.iter().all(|bs| bs.len() == 1) {
        return Ok(None);
    }
    let needle: Vec<u8> = byte_sets.iter().map(|bs| bs[0]).collect();
    let lit = crate::simd::FwdLiteralSearch::new(&needle);
    if crate::simd::BYTE_FREQ[lit.rare_byte() as usize] >= 100 {
        return Ok(None);
    }
    Ok(Some(crate::accel::FwdPrefixSearch::Literal(lit)))
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn build_strict_literal_prefix(
    _b: &mut RegexBuilder,
    _node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    Ok(None)
}

/// fwd prefix search, picking the rarest position for memchr.
pub fn build_fwd_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    if !crate::simd::has_simd() {
        return Ok((None, false));
    }

    build_fwd_prefix_simd(b, node)
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn build_fwd_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let stripped = get_prefix_node(b, node) != node;
    let sets = calc_potential_start(b, node, 16, 64)?;
    if sets.is_empty() {
        return Ok((None, false));
    }

    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();

    let lit_len = byte_sets_raw.iter().take_while(|bs| bs.len() == 1).count();
    if cfg!(feature = "debug-nulls") {
        eprintln!("  [fwd-prefix] lit_len={} total={}", lit_len, byte_sets_raw.len());
    }
    if lit_len >= 3 {
        let needle: Vec<u8> = byte_sets_raw[..lit_len].iter().map(|bs| bs[0]).collect();
        let lit = crate::simd::FwdLiteralSearch::new(&needle);
        if cfg!(feature = "debug-nulls") {
            let freq = crate::simd::BYTE_FREQ[lit.rare_byte() as usize];
            eprintln!("  [fwd-prefix] literal {:?} rare={} freq={}", std::str::from_utf8(&needle).unwrap_or("?"), lit.rare_byte() as char, freq);
        }
        if lit_len == byte_sets_raw.len() || crate::simd::BYTE_FREQ[lit.rare_byte() as usize] < 100 {
            return Ok((
                Some(crate::accel::FwdPrefixSearch::Literal(lit)),
                stripped,
            ));
        }
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
        return Ok((None, false));
    }
    freqs.sort_by_key(|&(_, f)| f);

    let rarest_idx = freqs[0].0;
    let rarest_len = byte_sets_raw[rarest_idx].len();

    #[cfg(target_arch = "x86_64")]
    if rarest_len > 16 {
        return try_build_fwd_range_prefix(&byte_sets_raw, rarest_idx, stripped);
    }
    // TODO: impl for neon
    #[cfg(not(target_arch = "x86_64"))]
    if rarest_len > 16 {
        return Ok((None, false));
    }

    let freq_order: Vec<usize> = freqs.iter().map(|&(i, _)| i).collect();

    if cfg!(feature = "debug-nulls") {
        for &(i, f) in &freqs {
            eprintln!("  [fwd-prefix] pos={} bytes={} freq={}", i, byte_sets_raw[i].len(), f);
        }
        eprintln!("  [fwd-prefix] anchor=pos{} ({} bytes)", freq_order[0], byte_sets_raw[freq_order[0]].len());
    }

    let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();

    Ok((
        Some(crate::accel::FwdPrefixSearch::Prefix(
            crate::simd::FwdPrefixSearch::new(sets.len(), &freq_order, &byte_sets_raw, all_sets),
        )),
        stripped,
    ))
}

const MAX_RANGE_SETS: usize = 3;

#[cfg(target_arch = "x86_64")]
fn try_build_fwd_range_prefix(
    byte_sets_raw: &[Vec<u8>],
    anchor_pos: usize,
    stripped: bool,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let anchor_bytes = &byte_sets_raw[anchor_pos];
    if !skip_is_profitable(anchor_bytes) {
        return Ok((None, false));
    }
    let tset = crate::accel::TSet::from_bytes(anchor_bytes);
    let ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
    if ranges.is_empty() || ranges.len() > MAX_RANGE_SETS {
        return Ok((None, false));
    }
    let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();
    if cfg!(feature = "debug-nulls") {
        eprintln!(
            "  [fwd-prefix-range] anchor=pos{} ranges={:?} len={}",
            anchor_pos, ranges, byte_sets_raw.len()
        );
    }
    Ok((
        Some(crate::accel::FwdPrefixSearch::Range(
            crate::simd::FwdRangeSearch::new(byte_sets_raw.len(), anchor_pos, ranges, all_sets),
        )),
        stripped,
    ))
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn build_fwd_prefix_simd(
    _b: &mut RegexBuilder,
    _node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    Ok((None, false))
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

const SKIP_FREQ_THRESHOLD: u32 = 2000;

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
    pub initial: u16,
    pub begin_table: Vec<u16>,
    pub center_table: Vec<u16>,
    pub effects_id: Vec<u16>,
    pub effects: Vec<Vec<NullState>>,
    pub num_minterms: u32,
    pub mt_log: u32,
    pub minterms_lookup: [u8; 256],

    pub minterms: Vec<TSetId>,
    pub state_nodes: Vec<NodeId>,
    pub node_to_state: HashMap<NodeId, u16>,
    pub skip_ids: Vec<u8>,
    pub skip_searchers: Vec<MintermSearchValue>,
    pub prefix_skip: Option<crate::accel::RevPrefixSearch>,
    pub(crate) _prefix_transition: u32, // reserved: DFA state after consuming prefix (unused, per-byte walk used instead)
    pub max_capacity: usize,
}

impl LDFA {
    pub fn new(
        b: &mut RegexBuilder,
        initial: NodeId,
        max_capacity: usize,
    ) -> Result<LDFA, Error> {
        let sets = collect_sets(b, initial);
        let minterms = PartitionTree::generate_minterms(sets, b.solver());
        let u8_lookup = PartitionTree::minterms_lookup(&minterms, b.solver());

        let max_capacity = max_capacity.min(65535);

        // state 0 = uncomputed, state 1 = dead
        let mut state_nodes: Vec<NodeId> = vec![NodeId::MISSING, NodeId::BOT];
        let mut node_to_state: HashMap<NodeId, u16> = HashMap::new();
        node_to_state.insert(NodeId::BOT, DFA_DEAD);

        let mut effects_id: Vec<u16> = vec![0u16; 2]; // slots 0,1

        let initial_sid = state_nodes.len() as u16;
        state_nodes.push(initial);
        node_to_state.insert(initial, initial_sid);
        let initial_eff_id = b.get_nulls_id(initial);
        effects_id.push(initial_eff_id.0 as u16);

        let der0 = b.der(initial, Nullability::BEGIN)?;
        let mut begin_table = vec![DFA_DEAD; minterms.len()];
        for (idx, mt) in minterms.iter().enumerate() {
            let t = transition_term(b, der0, *mt);
            let sid = register_state(&mut state_nodes, &mut node_to_state, &mut effects_id, b, t);
            if state_nodes.len() > max_capacity {
                return Err(Error::CapacityExceeded);
            }
            begin_table[idx] = sid;
        }
        let num_minterms = minterms.len() as u32;
        let mt_log = (num_minterms as usize).next_power_of_two().trailing_zeros();
        let stride = 1usize << mt_log;
        let center_table_size = state_nodes.len() * stride;
        let center_table = vec![DFA_MISSING; center_table_size];

        let effects = b.nulls_as_vecs();

        let skip_ids = vec![0u8; state_nodes.len()];

        Ok(LDFA {
            initial: initial_sid,
            begin_table,
            center_table,
            effects_id,
            effects,
            num_minterms,
            mt_log,
            minterms_lookup: u8_lookup,
            minterms,
            state_nodes,
            node_to_state,
            skip_ids,
            skip_searchers: Vec::new(),
            prefix_skip: None,
            _prefix_transition: DFA_MISSING as u32,
            max_capacity,
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

        let node = self.state_nodes[state_id as usize];
        if node == NodeId::MISSING {
            return Ok(DFA_DEAD);
        }
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
            if sid > DFA_DEAD {
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
            if node == NodeId::MISSING {
                continue;
            }
            self.ensure_capacity(sid);
            let sder = match b.der(node, Nullability::CENTER) {
                Ok(d) => d,
                Err(_) => {
                    return false;
                }
            };

            for mt_idx in 0..self.minterms.len() {
                let mt = self.minterms[mt_idx];
                let next_node = transition_term(b, sder, mt);
                let next_sid = self.get_or_register(b, next_node);
                if self.state_nodes.len() > self.max_capacity {
                    return false;
                }
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

    pub(crate) fn precompile_state(
        &mut self,
        b: &mut RegexBuilder,
        state_id: u16,
    ) -> Result<(), Error> {
        if state_id == DFA_DEAD {
            return Ok(());
        }
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
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        if crate::simd::has_simd() {
            self.try_build_skip_simd(_state, false);
        }
    }

    fn try_build_skip_force(&mut self, _state: usize) {
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
        if crate::simd::has_simd() {
            self.try_build_skip_simd(_state, true);
        }
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn try_build_skip_simd(&mut self, state: usize, force: bool) {
        let num_mt = self.num_minterms as usize;
        let stride = 1usize << self.mt_log;
        if state >= self.skip_ids.len() || self.skip_ids[state] != 0 {
            return;
        }
        let is_nullable = state < self.effects_id.len() && self.effects_id[state] != 0;
        if !force && is_nullable && !self.can_skip() {
            return;
        }
        let base = state * stride;
        if base + stride > self.center_table.len() {
            return;
        }
        let row = &self.center_table[base..base + num_mt];
        let zeros = row.iter().filter(|&&x| x == DFA_MISSING).count();
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
        let mut bytes = Vec::new();
        for &mt in &non_self_mts {
            for (byte, &m) in self.minterms_lookup.iter().enumerate() {
                if m as usize == mt {
                    bytes.push(byte as u8);
                }
            }
        }
        if bytes.len() <= 3 {
            self.skip_ids[state] = self.get_or_create_skip_exact(bytes);
            return;
        }
        if let Some(sid) = self.try_build_range_skip(&bytes) {
            self.skip_ids[state] = sid;
        }
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
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

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn get_or_create_skip_exact(&mut self, mut bytes: Vec<u8>) -> u8 {
        bytes.sort();
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if let MintermSearchValue::Exact(ref e) = s {
                if e.bytes() == &bytes {
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

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn get_or_create_skip_range(&mut self, mut ranges: Vec<(u8, u8)>) -> u8 {
        ranges.sort();
        for (i, s) in self.skip_searchers.iter().enumerate() {
            if let MintermSearchValue::Range(ref r) = s {
                if r.ranges() == &ranges {
                    return (i + 1) as u8;
                }
            }
        }
        self.skip_searchers.push(MintermSearchValue::Range(
            crate::simd::RevSearchRanges::new(ranges),
        ));
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

        let use_skip = self.can_skip();
        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                mt_log: self.mt_log,
            };

            let (state, new_pos, new_max, cache_miss) = if use_skip {
                scan_fwd_skip(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    end,
                    max_end,
                )
            } else {
                scan_fwd_noskip(&tables, curr, pos, end, max_end)
            };

            max_end = new_max;

            if !cache_miss {
                break;
            }

            let sid = state as u16;
            self.precompile_state(b, sid)?;
            self.try_build_skip(sid as usize);

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            curr = self.center_table[self.dfa_delta(sid, mt)] as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.precompile_state(b, curr as u16)?;
            self.try_build_skip(curr as usize);
            if cfg!(feature = "debug-nulls") {
                eprintln!("  [fwd-miss] sid={} curr={} skip_ids=[{},{}]",
                    sid, curr,
                    self.skip_ids.get(sid as usize).copied().unwrap_or(255),
                    self.skip_ids.get(curr as usize).copied().unwrap_or(255));
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
        max_length: Option<u32>,
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let data_end = data.len();
        if data_end == 0 || nulls.is_empty() {
            return Ok(());
        }

        let mut skip_until = 0usize;
        let mut skip_rebuilt = false;
        let mut use_skip = self.can_skip();
        if cfg!(feature = "debug-nulls") {
            eprintln!("  [scan_fwd_all] can_skip={} searchers={} nulls={}",
                use_skip, self.skip_searchers.len(), nulls.len());
        }

        for &begin_pos in nulls.iter().rev() {
            if begin_pos < skip_until || begin_pos >= data_end {
                continue;
            }

            let end = match max_length {
                Some(ml) => (begin_pos + ml as usize).min(data_end),
                None => data_end,
            };

            let mt = self.minterms_lookup[data[begin_pos] as usize];
            let mut curr = self.begin_table[mt as usize] as u32;
            if curr <= DFA_DEAD as u32 {
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
                        mt_log: self.mt_log,
                    };

                    let (state, new_pos, new_max, cache_miss) = if use_skip {
                        scan_fwd_skip(
                            &tables,
                            &self.skip_ids,
                            &self.skip_searchers,
                            curr,
                            pos,
                            end,
                            max_end,
                        )
                    } else {
                        scan_fwd_noskip(&tables, curr, pos, end, max_end)
                    };
                    max_end = new_max;

                    if !cache_miss {
                        break;
                    }

                    let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
                    curr = self.lazy_transition(b, state as u16, mt)? as u32;
                    pos = new_pos + 1;
                    if curr <= DFA_DEAD as u32 {
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

            if !skip_rebuilt && nulls.len() > 64 && self.state_nodes.len() < 256 {
                skip_rebuilt = true;
                self.build_skip_all(b);
                use_skip = self.can_skip();
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

    /// O(N·S) hardened forward scan.
    ///
    /// tracks all candidate starts simultaneously (grouped by DFA state)
    /// instead of scanning forward from each candidate independently.
    /// avoids the O(N²) worst case of `scan_fwd_all` on patterns like
    /// `.*[^A-Z]|[A-Z]` with dense reverse-scan candidates.
    #[inline(never)]
    pub fn scan_fwd_all_hardened(
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

        // each entry: (dfa_state, match_start, max_end)
        let mut active: Vec<(u32, usize, usize)> = Vec::with_capacity(64);
        let mut new_active: Vec<(u32, usize, usize)> = Vec::with_capacity(64);
        // dead entries awaiting emission: (start, max_end)
        let mut dead: Vec<(usize, usize)> = Vec::new();
        let mut skip_until: usize = 0;

        // nulls is descending; walk from end for ascending order
        if cfg!(feature = "debug-nulls") {
            eprintln!("  [hardened-fwd] nulls={:?} max_length={:?}", nulls, max_length);
        }
        let mut null_idx = nulls.len();
        let first_pos = nulls[nulls.len() - 1];

        for pos in first_pos..data_end {
            let mt = self.minterms_lookup[data[pos] as usize] as u32;
            new_active.clear();

            // 1. transition existing active slots on data[pos]
            for i in 0..active.len() {
                let (state, start, max_end) = active[i];

                let end_limit = match max_length {
                    Some(ml) => (start + ml as usize).min(data_end),
                    None => data_end,
                };

                if pos >= end_limit {
                    if max_end > 0 {
                        dead.push((start, max_end));
                    }
                    continue;
                }

                let delta = self.dfa_delta(state as u16, mt);
                let next = if delta < self.center_table.len()
                    && self.center_table[delta] != DFA_MISSING
                {
                    self.center_table[delta] as u32
                } else {
                    self.lazy_transition(b, state as u16, mt)? as u32
                };

                if next <= DFA_DEAD as u32 {
                    if max_end > 0 {
                        dead.push((start, max_end));
                    }
                    continue;
                }

                let mut me = max_end;
                let next_pos = pos + 1;
                let mask = if next_pos >= end_limit {
                    Nullability::END
                } else {
                    Nullability::CENTER
                };
                collect_nulls_fwd(
                    &self.effects_id,
                    &self.effects,
                    next,
                    next_pos,
                    mask,
                    &mut me,
                );

                if next_pos >= end_limit {
                    if me > 0 {
                        dead.push((start, me));
                    }
                    continue;
                }

                new_active.push((next, start, me));
            }

            // 2. activate new starts at this position
            while null_idx > 0 && nulls[null_idx - 1] == pos {
                null_idx -= 1;
                let state = self.begin_table[mt as usize] as u32;
                if state > DFA_DEAD as u32 {
                    let end_limit = match max_length {
                        Some(ml) => (pos + ml as usize).min(data_end),
                        None => data_end,
                    };
                    let next_pos = pos + 1;
                    let mut me = 0usize;
                    let mask = if next_pos >= end_limit {
                        Nullability::END
                    } else {
                        Nullability::CENTER
                    };
                    collect_nulls_fwd(
                        &self.effects_id,
                        &self.effects,
                        state,
                        next_pos,
                        mask,
                        &mut me,
                    );
                    if next_pos >= end_limit {
                        if me > 0 {
                            dead.push((pos, me));
                        }
                    } else {
                        new_active.push((state, pos, me));
                    }
                }
            }

            // 3. dedup: for each DFA state keep at most 2 entries
            //    (earliest start, plus earliest-with-match if different)
            hardened_prune(&mut new_active, &mut dead);

            if dead.len() > 50_000 {
                return Err(Error::CapacityExceeded);
            }

            // 4. drain dead entries that are safe to emit
            //    (no active entry has an earlier start that could produce a match)
            if !dead.is_empty() {
                let min_active_start = new_active.iter().map(|e| e.1).min().unwrap_or(usize::MAX);
                if min_active_start > skip_until {
                    dead.sort_unstable_by_key(|d| d.0);
                    let mut write = 0;
                    for i in 0..dead.len() {
                        let (start, max_end) = dead[i];
                        if max_end == 0 || start < skip_until {
                            continue;
                        }
                        if start < min_active_start {
                            matches.push(Match { start, end: max_end });
                            skip_until = if max_end > start { max_end } else { start + 1 };
                        } else {
                            dead[write] = dead[i];
                            write += 1;
                        }
                    }
                    dead.truncate(write);
                }
            }

            // prune active entries superseded by emitted matches
            new_active.retain(|e| e.1 >= skip_until);

            std::mem::swap(&mut active, &mut new_active);
        }

        // final drain: flush remaining active entries into dead
        for &(_, start, max_end) in &active {
            if max_end > 0 {
                dead.push((start, max_end));
            }
        }
        dead.sort_unstable_by_key(|d| d.0);
        for &(start, max_end) in &dead {
            if start >= skip_until && max_end > 0 {
                matches.push(Match { start, end: max_end });
                skip_until = if max_end > start { max_end } else { start + 1 };
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
        if state <= DFA_DEAD {
            return Ok(0);
        }
        for i in 1..len {
            let mt = self.minterms_lookup[data[pos + i] as usize] as u32;
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
                mt_log: self.mt_log,
            };

            #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
            let (state_out, new_pos, new_max, cache_miss) = if self.can_skip() {
                scan_fwd_skip(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    end,
                    max_end,
                )
            } else {
                scan_fwd_noskip(&tables, curr, pos, end, max_end)
            };
            #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
            let (state_out, new_pos, new_max, cache_miss) =
                scan_fwd_noskip(&tables, curr, pos, end, max_end);
            max_end = new_max;

            if !cache_miss {
                break;
            }

            let mt = self.minterms_lookup[data[new_pos] as usize] as u32;
            curr = self.lazy_transition(b, state_out as u16, mt)? as u32;
            pos = new_pos + 1;
            if curr <= DFA_DEAD as u32 {
                break;
            }

            self.precompile_state(b, curr as u16).ok();
            self.try_build_skip(curr as usize);

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

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    fn compute_skip_simd(
        &mut self,
        _b: &mut RegexBuilder,
        _rev_start: NodeId,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
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
            self._prefix_transition = sid as u32;

            self.prefix_skip = Some(crate::accel::RevPrefixSearch::new(
                sets.len(),
                &byte_sets_raw,
                all_sets,
            ));
        } else {
            let bytes = b.solver().collect_bytes(sets[0]);
            let ini = self.initial as usize;
            if ini < self.effects_id.len() && self.effects_id[ini] != 0 {
                return Ok(());
            }
            if bytes.len() <= 3 {
                if self.skip_ids.len() <= ini {
                    self.skip_ids.resize(ini + 1, 0u8);
                }
                self.skip_ids[ini] = self.get_or_create_skip_exact(bytes);
            } else if let Some(sid) = self.try_build_range_skip(&bytes) {
                if self.skip_ids.len() <= ini {
                    self.skip_ids.resize(ini + 1, 0u8);
                }
                self.skip_ids[ini] = sid;
            }
        }

        Ok(())
    }

    pub fn compute_fwd_skip(&mut self, b: &mut RegexBuilder) {
        self.compute_fwd_skip_inner(b, 64);
    }

    pub(crate) fn compute_fwd_skip_inner(&mut self, b: &mut RegexBuilder, limit: usize) {
        if !crate::simd::has_simd() || self.max_capacity < 64 {
            return;
        }
        use std::collections::VecDeque;
        let mut todo: VecDeque<u16> = VecDeque::new();
        let mut visited = HashSet::new();
        let ini = self.initial;
        if ini > DFA_DEAD {
            todo.push_back(ini);
        }
        // also seed from begin_table entries
        for &s in &self.begin_table {
            if s > DFA_DEAD && !visited.contains(&s) {
                todo.push_back(s);
            }
        }
        while let Some(sid) = todo.pop_front() {
            if !visited.insert(sid) || visited.len() > limit {
                continue;
            }
            if self.precompile_state(b, sid).is_err() {
                break;
            }
            let num_mt = self.minterms.len();
            for mt_idx in 0..num_mt {
                let delta = self.dfa_delta(sid, mt_idx as u32);
                if delta < self.center_table.len() {
                    let next = self.center_table[delta];
                    if next > DFA_DEAD && !visited.contains(&next) {
                        todo.push_back(next);
                    }
                }
            }
        }
        self.sync_effects(b);
        // build skip: non-nullable first to seed can_skip(), then nullable
        let states: Vec<u16> = visited.iter().copied().collect();
        for &sid in &states {
            let s = sid as usize;
            let is_nullable = s < self.effects_id.len() && self.effects_id[s] != 0;
            if !is_nullable {
                self.try_build_skip(s);
            }
        }
        for &sid in &states {
            let s = sid as usize;
            self.try_build_skip(s);
        }
        // if all reachable states are nullable, force-build the first viable skip
        if cfg!(feature = "debug-nulls") {
            eprintln!("  [fwd-skip] visited={} can_skip={}", states.len(), self.can_skip());
        }
        if !self.can_skip() {
            for &sid in &states {
                self.try_build_skip_force(sid as usize);
                if self.can_skip() {
                    // now build remaining nullable states
                    for &sid2 in &states {
                        self.try_build_skip(sid2 as usize);
                    }
                    break;
                }
            }
        }
    }

    pub(crate) fn can_skip(&self) -> bool {
        !self.skip_searchers.is_empty()
    }

    pub(crate) fn build_skip_all(&mut self, b: &mut RegexBuilder) {
        let n = self.state_nodes.len();
        for sid in 2..n {
            let _ = self.precompile_state(b, sid as u16);
        }
        self.sync_effects(b);
        if !self.can_skip() {
            for sid in 0..n {
                self.try_build_skip_force(sid);
                if self.can_skip() { break; }
            }
        }
        let n2 = self.state_nodes.len();
        for sid in 0..n2 {
            self.try_build_skip(sid);
        }
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

    fn collect_rev_inner<const EARLY_EXIT: bool>(
        &mut self,
        b: &mut RegexBuilder,
        start_pos: usize,
        data: &[u8],
        nulls: &mut Vec<usize>,
    ) -> Result<(), Error> {
        if self.prefix_skip.is_some() {
            let prefix_ptr =
                self.prefix_skip.as_ref().unwrap() as *const crate::accel::RevPrefixSearch;
            return self.collect_rev_prefix::<EARLY_EXIT>(b, prefix_ptr, start_pos, data, nulls);
        }

        let mt = self.minterms_lookup[data[start_pos] as usize];
        let mut curr = self.begin_table[mt as usize] as u32;
        if curr <= DFA_DEAD as u32 {
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
        if EARLY_EXIT && !nulls.is_empty() {
            return Ok(());
        }

        let mut pos = start_pos;

        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                mt_log: self.mt_log,
            };

            let use_skip = self.can_skip();
            let (state, new_pos, cache_miss) = if use_skip {
                collect_rev_skip::<EARLY_EXIT>(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    data,
                    nulls,
                )
            } else {
                collect_rev_noskip::<EARLY_EXIT>(&tables, curr, pos, nulls)
            };

            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }

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

            if curr <= DFA_DEAD as u32 {
                break;
            }

            let mask = if pos == 0 {
                Nullability::END
            } else {
                Nullability::CENTER
            };
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
            if curr <= DFA_DEAD as u32 {
                return Ok(());
            }
            pos = match_pos;
        } else {
            curr = self.initial as u32;
            pos = match_pos + 1;
        }
        let prefix_end = match_pos + 1 - prefix_len;
        while pos > prefix_end {
            pos -= 1;
            let mt = self.minterms_lookup[data[pos] as usize] as u32;
            let mut delta = self.dfa_delta(curr as u16, mt);
            if delta >= self.center_table.len() || self.center_table[delta] == DFA_MISSING {
                self.precompile_state(b, curr as u16)?;
                delta = self.dfa_delta(curr as u16, mt);
            }
            curr = self.center_table[delta] as u32;
            if curr <= DFA_DEAD as u32 {
                return Ok(());
            }
        }

        let mask = if pos == 0 {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);
        if EARLY_EXIT && !nulls.is_empty() {
            return Ok(());
        }

        if pos == 0 {
            return Ok(());
        }

        loop {
            let tables = ScanTables {
                center_table: self.center_table.as_ptr(),
                effects_id: self.effects_id.as_ptr(),
                effects: self.effects.as_ptr(),
                data: data.as_ptr(),
                minterms_lookup: self.minterms_lookup.as_ptr(),
                mt_log: self.mt_log,
            };

            let use_skip = self.can_skip();
            let (state, new_pos, cache_miss) = if use_skip {
                collect_rev_skip::<EARLY_EXIT>(
                    &tables,
                    &self.skip_ids,
                    &self.skip_searchers,
                    curr,
                    pos,
                    data,
                    nulls,
                )
            } else {
                collect_rev_noskip::<EARLY_EXIT>(&tables, curr, pos, nulls)
            };

            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
            }

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

            if curr <= DFA_DEAD as u32 {
                break;
            }

            let mask = if pos == 0 {
                Nullability::END
            } else {
                Nullability::CENTER
            };
            collect_nulls(&self.effects_id, &self.effects, curr, pos, mask, nulls);
            if EARLY_EXIT && !nulls.is_empty() {
                return Ok(());
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
    mt_log: u32,
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
fn collect_rev_noskip<const EARLY_EXIT: bool>(
    t: &ScanTables,
    mut curr: u32,
    mut pos: usize,
    nulls: &mut Vec<usize>,
) -> (u32, usize, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
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
                    if EARLY_EXIT { return (curr, pos, false); }
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
                    if EARLY_EXIT && !nulls.is_empty() { return (curr, pos, false); }
                }
            }
            let delta = (curr << mt_log | mt) as usize;
            let next = *center_table.add(delta);
            if next == DFA_MISSING {
                return (curr, pos, true); // cache miss
            }
            if next == DFA_DEAD {
                return (DFA_DEAD as u32, pos, false); // dead state
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
fn collect_rev_skip<const EARLY_EXIT: bool>(
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
    let mt_log = t.mt_log;
    let mut prev_eid: u32 = 0;
    while pos != 0 {
        let sid = skip_ids[curr as usize];
        if sid != 0 {
            // flush deferred null before skip
            if prev_eid != 0 {
                if prev_eid == 1 {
                    nulls.push(pos);
                    if EARLY_EXIT { return (curr, pos, false); }
                } else {
                    collect_rev_complex(effects, prev_eid, pos, Nullability::CENTER, nulls);
                    if EARLY_EXIT && !nulls.is_empty() { return (curr, pos, false); }
                }
                prev_eid = 0;
            }
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
            // nullable self-loop: batch-emit for skipped positions
            unsafe {
                let eid = *effects_id.add(curr as usize) as u32;
                if eid != 0 && pos < old_pos {
                    if EARLY_EXIT {
                        if eid == 1 {
                            nulls.push(old_pos - 1);
                        } else {
                            collect_rev_complex(effects, eid, old_pos - 1, Nullability::CENTER, nulls);
                        }
                        if !nulls.is_empty() { return (curr, pos, false); }
                    } else {
                        if eid == 1 {
                            for p in (pos..old_pos).rev() {
                                nulls.push(p);
                            }
                        } else {
                            for p in (pos..old_pos).rev() {
                                collect_rev_complex(effects, eid, p, Nullability::CENTER, nulls);
                            }
                        }
                    }
                }
            }
        }

        while pos != 0 {
            pos -= 1;
            unsafe {
                let mt = *minterms_lookup.add(*t.data.add(pos) as usize) as u32;
                if prev_eid != 0 {
                    if prev_eid == 1 {
                        nulls.push(pos + 1);
                        if EARLY_EXIT { return (curr, pos, false); }
                    } else {
                        collect_rev_complex(effects, prev_eid, pos + 1, Nullability::CENTER, nulls);
                        if EARLY_EXIT && !nulls.is_empty() { return (curr, pos, false); }
                    }
                }
                let delta = (curr << mt_log | mt) as usize;
                let next = *center_table.add(delta);
                if next == DFA_MISSING {
                    return (curr, pos, true); // cache miss
                }
                if next == DFA_DEAD {
                    return (DFA_DEAD as u32, pos, false); // dead state
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

/// for each DFA state, keep at most 2 entries:
/// 1. the entry with the smallest start (primary)
/// 2. if primary has max_end == 0: also the earliest entry with max_end > 0 (backup)
/// result is sorted by start ascending.
fn hardened_prune(active: &mut Vec<(u32, usize, usize)>, dead: &mut Vec<(usize, usize)>) {
    if active.len() <= 1 {
        return;
    }
    active.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut write = 0;
    let mut read = 0;
    while read < active.len() {
        let state = active[read].0;
        let group_start = read;
        while read < active.len() && active[read].0 == state {
            read += 1;
        }
        // primary = smallest start
        let primary = active[group_start];
        active[write] = primary;
        write += 1;
        let discard_from = if primary.2 == 0 {
            // primary has no match: keep first backup that does
            let mut kept = group_start + 1;
            for j in (group_start + 1)..read {
                if active[j].2 > 0 {
                    active[write] = active[j];
                    write += 1;
                    kept = j + 1;
                    break;
                }
            }
            kept
        } else {
            group_start + 1
        };
        // entries discarded from the active set still have valid matches;
        // push them to dead so they can be emitted later.
        // correctness: same-state entries get identical collect_nulls_fwd
        // updates. if the kept entry's max_end later grows past a dead
        // entry's start, the dead entry is covered and will be skipped.
        // if it doesn't grow, the dead entry's max_end is already final.
        for j in discard_from..read {
            if active[j].2 > 0 {
                dead.push((active[j].1, active[j].2));
            }
        }
    }
    active.truncate(write);
    active.sort_unstable_by_key(|e| e.1);
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
    let mt_log = t.mt_log;
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
            let delta = (curr << mt_log | mt) as usize;
            let next = *center_table.add(delta);
            if next == DFA_MISSING {
                return (curr, pos, max_end, true); // cache miss
            }
            if next == DFA_DEAD {
                return (DFA_DEAD as u32, pos, max_end, false); // dead state
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

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn scan_fwd_skip(
    t: &ScanTables,
    skip_ids: &[u8],
    skip_searchers: &[MintermSearchValue],
    mut curr: u32,
    mut pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects_id = t.effects_id;
    let effects = t.effects;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;

    while pos < end {
        let sid = skip_ids[curr as usize];
        if sid != 0 {
            let searcher = &skip_searchers[sid as usize - 1];
            let haystack = unsafe { std::slice::from_raw_parts(data.add(pos), end - pos) };
            match searcher.find_fwd(haystack) {
                Some(offset) => {
                    // nullable self-loop: only need max position for max_end
                    unsafe {
                        let eid = *effects_id.add(curr as usize) as u32;
                        if eid != 0 && offset > 0 {
                            let skip_end_pos = pos + offset;
                            if eid == 1 {
                                max_end = max_end.max(skip_end_pos);
                            } else {
                                max_end = scan_fwd_complex(
                                    effects,
                                    eid,
                                    skip_end_pos,
                                    Nullability::CENTER,
                                    max_end,
                                );
                            }
                        }
                    }
                    pos += offset;
                }
                None => {
                    // no non-self-loop byte: entire rest is self-loop
                    unsafe {
                        let eid = *effects_id.add(curr as usize) as u32;
                        if eid != 0 {
                            if eid == 1 {
                                max_end = max_end.max(end);
                            } else {
                                max_end =
                                    scan_fwd_complex(effects, eid, end, Nullability::END, max_end);
                            }
                        }
                    }
                    return (curr, end, max_end, false);
                }
            }
        }

        let mut prev_eid: u32 = 0;
        while pos < end {
            unsafe {
                let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
                if prev_eid != 0 {
                    if prev_eid == 1 {
                        max_end = max_end.max(pos);
                    } else {
                        max_end =
                            scan_fwd_complex(effects, prev_eid, pos, Nullability::CENTER, max_end);
                    }
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
                prev_eid = *effects_id.add(curr as usize) as u32;
            }
            pos += 1;
            if skip_ids[curr as usize] != 0 {
                // flush deferred prev_eid before returning to skip loop
                if prev_eid != 0 {
                    let mask = if pos >= end {
                        Nullability::END
                    } else {
                        Nullability::CENTER
                    };
                    if prev_eid == 1 {
                        max_end = max_end.max(pos);
                    } else {
                        max_end = scan_fwd_complex(effects, prev_eid, pos, mask, max_end);
                    }
                    prev_eid = 0;
                }
                break;
            }
        }
        if pos >= end && prev_eid != 0 {
            if prev_eid == 1 {
                max_end = max_end.max(pos);
            } else {
                max_end = scan_fwd_complex(effects, prev_eid, pos, Nullability::END, max_end);
            }
        }
    }
    (curr, pos, max_end, false)
}


#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn scan_fwd_skip(
    _t: &ScanTables,
    _skip_ids: &[u8],
    _skip_searchers: &[MintermSearchValue],
    _curr: u32,
    _pos: usize,
    _end: usize,
    _max_end: usize,
) -> (u32, usize, usize, bool) {
    unreachable!("scan_fwd_skip requires SIMD")
}

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

/// bounded DFA for opportunistic matching with known max_length.
/// only exists for a slight (20-30%) performance boost on short patterns
pub struct BDFA {
    initial_node: NodeId,
    /// states as Counted node chains.
    pub states: Vec<NodeId>,
    state_map: HashMap<NodeId, u16>,
    /// packed transition table: entry = (match_rel << 16) | next_state.
    /// 0 = uncached sentinel.
    pub table: Vec<u32>,
    /// match rel per state (0 = no match).
    pub match_rel: Vec<u32>,
    /// number of minterms.
    pub num_mt: usize,
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
            num_mt,
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
        if cfg!(feature = "debug-nulls") {
            let byte_counts: Vec<usize> = prefix_sets.iter().map(|&s| b.solver_ref().collect_bytes(s).len()).collect();
            eprintln!("  [bdfa-build-prefix] linear_sets={} bytes={:?}", prefix_sets.len(), byte_counts);
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
        let sets = calc_potential_start(b, pattern_node, 16, 64)?;
        if cfg!(feature = "debug-nulls") {
            eprintln!("  [bdfa-prefix-potential] node={:?} sets={}", pattern_node, sets.len());
        }
        if sets.is_empty() {
            return Ok(());
        }
        let byte_sets_raw: Vec<Vec<u8>> = sets
            .iter()
            .map(|&s| b.solver_ref().collect_bytes(s))
            .collect();
        if cfg!(feature = "debug-nulls") {
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

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn build_prefix_search(byte_sets_raw: &[Vec<u8>]) -> Option<crate::accel::FwdPrefixSearch> {
        if byte_sets_raw.iter().all(|bs| bs.len() == 1) {
            let needle: Vec<u8> = byte_sets_raw.iter().map(|bs| bs[0]).collect();
            let lit = crate::simd::FwdLiteralSearch::new(&needle);
            if crate::simd::BYTE_FREQ[lit.rare_byte() as usize] >= 100 {
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

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    fn try_build_range_prefix(
        byte_sets_raw: &[Vec<u8>],
        anchor_pos: usize,
    ) -> Option<crate::accel::FwdPrefixSearch> {
        let anchor_bytes = &byte_sets_raw[anchor_pos];
        if !skip_is_profitable(anchor_bytes) {
            return None;
        }
        let tset = crate::accel::TSet::from_bytes(anchor_bytes);
        let ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
        if ranges.is_empty() || ranges.len() > MAX_RANGE_SETS {
            return None;
        }
        let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
            .iter()
            .map(|bytes| crate::accel::TSet::from_bytes(bytes))
            .collect();
        if cfg!(feature = "debug-nulls") {
            eprintln!(
                "  [bdfa-prefix-range] anchor=pos{} ranges={:?} len={}",
                anchor_pos, ranges, byte_sets_raw.len()
            );
        }
        Some(crate::accel::FwdPrefixSearch::Range(
            crate::simd::FwdRangeSearch::new(byte_sets_raw.len(), anchor_pos, ranges, all_sets),
        ))
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    fn build_prefix_search(_byte_sets_raw: &[Vec<u8>]) -> Option<crate::accel::FwdPrefixSearch> {
        None
    }

    /// best match rel from packed extra.
    pub fn counted_best(node: NodeId, b: &RegexBuilder) -> u32 {
        (b.get_extra(node) >> 16) as u32
    }

    fn register(&mut self, node: NodeId, b: &RegexBuilder) -> u16 {
        if let Some(&sid) = self.state_map.get(&node) {
            return sid;
        }
        let sid = self.states.len() as u16;
        // leftmost with body=BOT and best>0 -> match
        let body = node.left(b);
        let rel = if body == NodeId::BOT {
            Self::counted_best(node, b)
        } else {
            0
        };
        if cfg!(feature = "debug-nulls") {
            eprintln!(
                "  [bounded] register state {} node={} rel={}",
                sid,
                b.pp(node),
                rel,
            );
        }
        self.states.push(node);
        self.state_map.insert(node, sid);
        self.match_rel.push(rel);
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
            let der = b.der(cur, Nullability::CENTER).map_err(Error::Algebra)?;
            let next = transition_term(b, der, mt);
            if next != NodeId::BOT {
                result.push(next);
            } else {
                let best = Self::counted_best(next, b);
                if best > 0 {
                    result.push(next);
                }
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

        if cfg!(feature = "debug-nulls") {
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
