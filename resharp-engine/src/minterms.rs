use std::collections::HashSet;

use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder, TRegex, TRegexId};

pub(crate) struct PartitionTree {
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
            Kind::Begin | Kind::End | Kind::Tag => {}
            Kind::Pred => {
                sets.insert(node_id.pred_tset(b));
            }
            Kind::Union | Kind::Concat | Kind::Inter => {
                stack.push(node_id.left(b));
                stack.push(node_id.right(b));
            }
            Kind::Lookahead | Kind::Lookbehind | Kind::Ordered => {
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
