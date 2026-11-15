use std::collections::HashSet;

use resharp_algebra::solver::{Solver, TSet, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder, TRegex, TRegexId};

pub(crate) struct PartitionTree;

impl PartitionTree {
    pub fn generate_minterms(sets: HashSet<TSetId>, solver: &mut Solver) -> Vec<TSetId> {
        let mut sets: Vec<TSetId> = sets.into_iter().collect();
        sets.sort();
        let mut classes: Vec<TSet> = vec![Solver::full()];
        for set in sets {
            let s = solver.get_set(set);
            let mut next = Vec::with_capacity(classes.len() * 2);
            for c in classes {
                let inside = c & s;
                let outside = c & !s;
                if !Solver::is_empty(&inside) {
                    next.push(inside);
                }
                if !Solver::is_empty(&outside) {
                    next.push(outside);
                }
            }
            classes = next;
        }
        let mut ids: Vec<TSetId> = classes.into_iter().map(|c| solver.get_id(c)).collect();
        ids.sort();
        ids
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minterms_partition_bytes_and_refine_each_set() {
        let mut solver = Solver::new();
        let ranges = [
            (b'a', b'm'),
            (b'h', b'z'),
            (0u8, 0x7f),
            (b'0', b'9'),
            (0xC2, 0xDF),
        ];
        let sets: HashSet<TSetId> = ranges
            .iter()
            .map(|&(lo, hi)| solver.range_to_set_id(lo, hi))
            .collect();
        let mts = PartitionTree::generate_minterms(sets.clone(), &mut solver);
        let mut union = Solver::empty();
        for (i, &m) in mts.iter().enumerate() {
            for &o in &mts[i + 1..] {
                assert!(solver.and_id(m, o) == TSetId::EMPTY, "minterms overlap");
            }
            union = union | solver.get_set(m);
        }
        assert!(
            union == Solver::full(),
            "minterms do not cover the byte universe"
        );
        for &s in &sets {
            for &m in &mts {
                let inter = solver.and_id(m, s);
                assert!(inter == TSetId::EMPTY || inter == m, "set splits a minterm");
            }
        }
        let lookup = PartitionTree::minterms_lookup(&mts, &mut solver);
        for byte in 0..=255u8 {
            let m = solver.get_set(mts[lookup[byte as usize] as usize]);
            assert!(
                m.contains_byte(byte),
                "lookup maps byte {byte} to the wrong minterm"
            );
        }
    }
}
