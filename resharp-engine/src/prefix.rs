use resharp_algebra::nulls::Nullability;
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder};
use std::collections::{BTreeMap, BTreeSet};

use crate::Error;

#[cfg(feature = "debug")]
fn pp_sets(b: &RegexBuilder, sets: &[TSetId]) -> String {
    sets.iter()
        .map(|&s| b.solver_ref().pp(s))
        .collect::<Vec<_>>()
        .join(";")
}

pub(crate) fn calc_prefix_sets_inner(
    b: &mut RegexBuilder,
    start: NodeId,
    strip_prefix: bool,
) -> Result<Vec<TSetId>, crate::Error> {
    let mut result = Vec::new();
    let mut node = start;
    let mut redundant = BTreeSet::new();
    redundant.insert(NodeId::BOT);
    redundant.insert(start);
    let mut visited: BTreeSet<NodeId> = BTreeSet::new();

    loop {
        if !result.is_empty() && redundant.contains(&node) {
            break;
        }

        if !result.is_empty() && !visited.insert(node) {
            result.clear();
            break;
        }

        if b.any_nonbegin_nullable(node) {
            break;
        }

        let der = b
            .der(node, Nullability::CENTER)
            .map_err(crate::Error::Algebra)?;
        let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
        b.collect_der_targets(der, TSetId::FULL, &mut targets);
        let full_union = if !strip_prefix {
            targets
                .iter()
                .filter(|(t, _)| *t != NodeId::BOT)
                .fold(TSetId::EMPTY, |acc, &(_, cs)| b.solver().or_id(acc, cs))
        } else {
            TSetId::EMPTY
        };

        targets.retain(|(t, _)| !redundant.contains(t));

        if targets.is_empty() {
            result.clear();
            break;
        }

        if targets.len() == 1 {
            let (target, char_set) = targets[0];
            if target == node {
                result.clear();
                break;
            }
            let set = if !strip_prefix && full_union != TSetId::EMPTY {
                full_union
            } else {
                char_set
            };
            result.push(set);
            node = target;
        } else {
            break;
        }
    }

    Ok(result)
}

/// True (anchored) prefix sets from the reversed pattern.
pub fn calc_prefix_sets(
    b: &mut RegexBuilder,
    rev_start: NodeId,
) -> Result<Vec<TSetId>, crate::Error> {
    let rev_start = b.nonbegins(rev_start);
    let safe = b.strip_prefix_safe(rev_start);
    calc_prefix_sets_inner(b, safe, true)
}

/// potential start prefix, but does not guarantee the match starts here.
/// eg .*a.* -> a does guarantee there is a match, but not where it starts
pub fn calc_potential_start_prune(
    b: &mut RegexBuilder,
    node: NodeId,
    max_prefix_len: usize,
    max_frontier_size: usize,
    exclude_initial: bool,
) -> Result<Vec<TSetId>, crate::Error> {
    let node = b.prune_begin(node);
    let node = b.strip_prefix_safe(node);
    calc_potential_start(b, node, max_prefix_len, max_frontier_size, exclude_initial)
}

/// potential start prefix, may have false positives, but no false negatives.
pub fn calc_potential_start(
    b: &mut RegexBuilder,
    initial_node: NodeId,
    max_prefix_len: usize,
    max_frontier_size: usize,
    exclude_initial: bool,
) -> Result<Vec<TSetId>, crate::Error> {
    let mut nodes: BTreeSet<NodeId> = BTreeSet::new();
    nodes.insert(initial_node);
    let mut depth: BTreeMap<NodeId, usize> = BTreeMap::new();
    depth.insert(initial_node, 0);

    let mut result = Vec::new();
    let mut step: usize = 0;

    let mut sat_stack: Vec<(resharp_algebra::TRegexId, TSetId)> = Vec::new();

    loop {
        if nodes.is_empty() || nodes.len() > max_frontier_size || result.len() >= max_prefix_len {
            break;
        }

        if nodes.iter().any(|&n| b.any_nonbegin_nullable(n)) {
            break;
        }

        let mut union_set = TSetId::EMPTY;
        let mut next_nodes: BTreeSet<NodeId> = BTreeSet::new();
        let next_step = step + 1;

        for &node in &nodes.clone() {
            let der = b
                .der(node, Nullability::CENTER)
                .map_err(crate::Error::Algebra)?;
            sat_stack.push((der, TSetId::FULL));
            b.iter_sat(&mut sat_stack, &mut |b, target, char_set| {
                if exclude_initial && target == initial_node {
                    return;
                }
                if target == NodeId::BOT {
                    return;
                }
                union_set = b.solver().or_id(union_set, char_set);
                next_nodes.insert(target);
                depth.entry(target).or_insert(next_step);
            });
        }

        if next_nodes.is_empty() || union_set == TSetId::EMPTY {
            if next_nodes.is_empty() {
                result.clear();
            }
            break;
        }

        result.push(union_set);
        nodes = next_nodes;
        step = next_step;
    }

    Ok(result)
}

fn collect_loop_factored_bodies(b: &RegexBuilder, init: NodeId) -> Option<Vec<NodeId>> {
    let mut bodies = Vec::new();
    let mut stack = vec![init];
    while let Some(n) = stack.pop() {
        if n.is_inter(b) {
            stack.push(n.left(b));
            stack.push(n.right(b));
        } else if n.is_concat(b) && n.left(b) == NodeId::TS {
            bodies.push(n.right(b));
        } else {
            return None;
        }
    }
    Some(bodies)
}

fn synthesize_inter_constraint(b: &mut RegexBuilder, init: NodeId) -> Option<NodeId> {
    if !init.is_inter(b) {
        return None;
    }
    let bodies = collect_loop_factored_bodies(b, init)?;
    if bodies.is_empty() {
        return None;
    }
    Some(b.mk_unions(bodies.into_iter()))
}

/// Detect a reverse start `[_*] ~(_*X) tail`. Returns `(rc, boundary, tail)`
/// where `rc` is the begin-relaxed node `~(_*X) tail`, `boundary = [^X]`.
pub(crate) fn rev_boundary_shape(
    b: &mut RegexBuilder,
    rev_start: NodeId,
) -> Option<(NodeId, TSetId, NodeId)> {
    let stripped = if rev_start.is_concat(b) && rev_start.left(b) == NodeId::TS {
        rev_start.right(b)
    } else {
        rev_start
    };
    let rc = b.prune_begin_eps(stripped);
    if !rc.is_concat(b) {
        return None;
    }
    let lead = rc.left(b);
    let tail = rc.right(b);
    if !lead.is_compl(b) {
        return None;
    }
    let inner = lead.left(b);
    if !inner.is_concat(b) || inner.left(b) != NodeId::TS {
        return None;
    }
    let pred = inner.right(b);
    if !pred.is_pred(b) {
        return None;
    }
    let cc = pred.pred_tset(b);
    let boundary = b.solver().not_id(cc);
    if boundary == TSetId::EMPTY {
        return None;
    }
    Some((rc, boundary, tail))
}

fn calc_rev_boundary_prefix(
    b: &mut RegexBuilder,
    rev_start: NodeId,
) -> Result<Option<Vec<TSetId>>, crate::Error> {
    let Some((_, boundary, tail)) = rev_boundary_shape(b, rev_start) else {
        return Ok(None);
    };
    let tail_sets = calc_potential_start(b, tail, 16, 64, false)?;
    if tail_sets.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(tail_sets.len() + 1);
    out.push(boundary);
    out.extend(tail_sets);
    Ok(Some(out))
}

pub(crate) fn calc_combined_prefix(
    b: &mut RegexBuilder,
    init: NodeId,
    fingerprint_depth: usize,
    max_prefix_len: usize,
    max_frontier_size: usize,
) -> Result<Vec<TSetId>, crate::Error> {
    let potential = calc_potential_start(b, init, max_prefix_len, max_frontier_size, true)?;
    let head = if let Some(c) = synthesize_inter_constraint(b, init) {
        let constrained = b.mk_inter(init, c);
        let mut h =
            calc_potential_start(b, constrained, fingerprint_depth, max_frontier_size, false)?;
        h.truncate(fingerprint_depth);
        h
    } else {
        Vec::new()
    };
    if head.is_empty() {
        return Ok(potential);
    }
    let mut out = potential;
    if out.len() < head.len() {
        return Ok(head);
    }
    for (i, &h) in head.iter().enumerate() {
        out[i] = b.solver().and_id(out[i], h);
    }
    Ok(out)
}

#[derive(Clone, Debug)]
pub struct PrefixSet {
    pub sets: Vec<TSetId>,
    /// per-byte cost (lower = faster). `u64::MAX` for empty
    pub cost: u64,
}

/// Prefix sets for both directions.
pub struct PrefixSets {
    /// Potential-start fwd sets (full node, self-loop bytes included).
    pub fwd_potential: PrefixSet,
    /// Potential-start fwd sets after stripping a leading `_*`.
    pub fwd_potential_stripped: PrefixSet,
    /// Tight anchored rev prefix (right-to-left).
    pub rev_anchored: PrefixSet,
    /// Fingerprint head intersected with potential-start tail; narrower than bare potential-start.
    pub rev_potential: PrefixSet,
    /// `rev_start` with the leading `_*`/begin pruned: the mandatory reverse
    /// body. This is the canonical node to search for an interior literal.
    pub rev_stripped: NodeId,
}

impl PrefixSets {
    /// Compute all prefix sets for `node` (fwd) and `rev_start` (reversed, not yet stripped).
    pub fn compute(
        b: &mut RegexBuilder,
        node: NodeId,
        rev_start: NodeId,
    ) -> Result<Self, crate::Error> {
        let fwd_body = strip_leading_lookbehind(b, node);
        let stripped_node = b.strip_prefix_safe(node);
        let fwd_body_stripped = strip_leading_lookbehind(b, stripped_node);
        let fwd_potential_sets = calc_potential_start(b, fwd_body, 16, 64, false)?;
        let fwd_potential_stripped_sets =
            calc_potential_start(b, fwd_body_stripped, 16, 64, false)?;
        let rev_anchored_sets = calc_prefix_sets(b, rev_start)?;
        let rev_combined_init = {
            let n = b.prune_begin(rev_start);
            b.strip_prefix_safe(n)
        };
        let rev_stripped = rev_combined_init;
        let mut rev_potential_sets =
            if let Some(s) = calc_rev_boundary_prefix(b, rev_start)? {
                s
            } else {
                calc_combined_prefix(b, rev_combined_init, 3, 16, 64)?
            };
        if rev_potential_sets.is_empty() {
            if let Ok(body) = b.strip_lb(node) {
                if body != node {
                    if let Ok(body_rev) = b.reverse(body) {
                        if let Ok(bare) = b.strip_lb(body_rev) {
                            rev_potential_sets = calc_potential_start(b, bare, 16, 64, false)?;
                        }
                    }
                }
            }
        }

        let body_shape = classify_body_shape(b, fwd_body, &fwd_potential_sets);
        let mut mk = |sets: Vec<TSetId>, dir: Direction| PrefixSet {
            cost: cost_for(b, &sets, dir, body_shape),
            sets,
        };

        let fwd_potential = mk(fwd_potential_sets, Direction::Fwd);
        let fwd_potential_stripped = mk(fwd_potential_stripped_sets, Direction::Fwd);
        let rev_anchored = mk(rev_anchored_sets, Direction::Rev);
        let rev_potential = mk(rev_potential_sets, Direction::Rev);
        Ok(Self {
            fwd_potential,
            fwd_potential_stripped,
            rev_anchored,
            rev_potential,
            rev_stripped,
        })
    }

    /// Lower is rarer and more profitable for SIMD skip. `u64::MAX` for an empty sequence.
    #[allow(dead_code)]
    pub fn rarity(b: &mut RegexBuilder, sets: &[TSetId]) -> u64 {
        rarest_freq(b, sets)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Direction {
    Fwd,
    Rev,
}

/// Cost wrapper that handles the non-SIMD target stub.
fn cost_for(b: &mut RegexBuilder, sets: &[TSetId], dir: Direction, body_shape: NodeShape) -> u64 {
    scan_cost(b, sets, dir, body_shape)
}

/// Estimated per-byte scan cost: `scan_per_byte + fire_rate * verify_per_fire`.
fn scan_cost(b: &mut RegexBuilder, sets: &[TSetId], dir: Direction, body_shape: NodeShape) -> u64 {
    if sets.is_empty() {
        return u64::MAX;
    }
    let counts: Vec<usize> = sets
        .iter()
        .map(|&s| b.solver().collect_bytes(s).len())
        .collect();
    let freqs: Vec<u64> = sets
        .iter()
        .map(|&s| {
            b.solver()
                .collect_bytes(s)
                .iter()
                .map(|&byte| crate::simd::BYTE_FREQ[byte as usize] as u64)
                .sum()
        })
        .collect();
    let total = TOTAL_BYTE_FREQ as f64;
    let rarest = freqs
        .iter()
        .zip(counts.iter())
        .enumerate()
        .filter(|&(_, (&f, _))| f > 0)
        .min_by_key(|&(_, (&f, _))| f);
    let single_position = matches!(dir, Direction::Fwd)
        && rarest.is_some_and(|(_, (_, &c))| c > 16);
    let fire = if single_position {
        rarest.map(|(_, (&f, _))| f as f64).unwrap_or(total) / total
    } else {
        let mut nz: Vec<u64> = freqs.iter().copied().filter(|&f| f > 0).collect();
        if nz.is_empty() {
            return u64::MAX;
        }
        nz.sort_unstable();
        let num_simd = nz.len().min(3);
        let prod: f64 = nz[..num_simd].iter().map(|&f| f as f64).product();
        prod / total.powi(num_simd as i32)
    };

    let (scan_per_byte, verify_per_fire) = match dir {
        Direction::Rev => (0.05, 20.0),
        Direction::Fwd => (
            0.05,
            match body_shape {
                NodeShape::TrailingStar => 1.0,
                NodeShape::Bounded => 50.0,
                NodeShape::Unbounded => 5000.0,
            },
        ),
    };
    let cost = scan_per_byte + fire * verify_per_fire;
    (cost * 1e9) as u64
}

/// Shape of the node after prefix, controlling fwd-direction verify cost.
#[derive(Copy, Clone, Debug)]
pub enum NodeShape {
    TrailingStar,
    Bounded,
    Unbounded,
}

pub(crate) const SKIP_FREQ_THRESHOLD: u32 = 75_000;

/// Threshold above which a byte set is treated as wildcard-like.
const WIDE_SET_BYTES: u32 = 200;

fn is_pure_trailing_run(b: &mut RegexBuilder, node: NodeId) -> bool {
    let mut cur = node;
    loop {
        if cur.is_star(b) {
            return true;
        }
        if cur.is_lookahead(b) {
            cur = cur.right(b);
            continue;
        }
        if cur.is_inter(b) {
            let (l, r) = (cur.left(b), cur.right(b));
            cur = if l.is_compl(b) { r } else { l };
            continue;
        }
        if !cur.is_concat(b) {
            return false;
        }
        let left = cur.left(b);
        if b.get_min_max_length(left).1 == u32::MAX {
            return false;
        }
        cur = cur.right(b);
    }
}

/// Classify body shape past the fwd prefix to set verify cost.
fn classify_body_shape(
    b: &mut RegexBuilder,
    fwd_body: NodeId,
    fwd_potential: &[TSetId],
) -> NodeShape {
    if b.ends_with_ts(fwd_body) {
        return NodeShape::TrailingStar;
    }
    let rarest_wide = !fwd_potential.is_empty()
        && fwd_potential
            .iter()
            .map(|&s| b.solver().byte_count(s))
            .min()
            .is_some_and(|c| c > 16);
    if is_pure_trailing_run(b, fwd_body) {
        return NodeShape::TrailingStar;
    }
    if rarest_wide && b.get_min_max_length(fwd_body).1 == u32::MAX {
        return NodeShape::Unbounded;
    }
    match fwd_potential.last() {
        Some(&last) if b.solver().byte_count(last) > WIDE_SET_BYTES => NodeShape::Unbounded,
        _ => NodeShape::Bounded,
    }
}
#[cfg(feature = "convergence_prefix")]
const CONV_PENALTY: u64 = 8;
#[cfg(feature = "convergence_prefix")]
const CONV_WIDE_LOOP_BYTES: u32 = 128;
#[cfg(feature = "convergence_prefix")]
const CONV_BOUNDED_MAX: u32 = 12;

#[cfg(feature = "convergence_prefix")]
fn conv_b_interior_unbounded(b: &mut RegexBuilder, b_node: NodeId) -> bool {
    use resharp_algebra::nulls::Nullability;
    let mut seen_wide_unbounded = false;
    let mut curr = b_node;
    loop {
        let is_concat = curr.is_concat(b);
        let head = if is_concat { curr.left(b) } else { curr };
        let (hmin, hmax) = b.get_min_max_length(head);
        if seen_wide_unbounded && hmin > 0 {
            return true;
        }
        if hmax == u32::MAX {
            let lead = match b.der(head, Nullability::CENTER) {
                Ok(d) => {
                    let mut stack = vec![(d, TSetId::FULL)];
                    let mut acc = TSetId::EMPTY;
                    b.iter_sat(&mut stack, &mut |bb, _n, set| {
                        acc = bb.solver().or_id(acc, set);
                    });
                    acc
                }
                Err(_) => b.solver().not_id(TSetId::EMPTY),
            };
            if b.solver().byte_count(lead) >= CONV_WIDE_LOOP_BYTES {
                seen_wide_unbounded = true;
            }
        }
        if is_concat {
            curr = curr.right(b);
        } else {
            break;
        }
    }
    false
}
const TEDDY_MAX_FREQ_SUM: u64 = 25_000;
// sum of BYTE_FREQ[0..256] in the corpus
pub(crate) const TOTAL_BYTE_FREQ: u64 = 252_052;
/// a position must be at least this rare to count as a selective Teddy lane;
/// a bare multi-class fingerprint with no rare anchor is rejected
const TEDDY_WEAK_POSITION_FREQ: u64 = 8_000;
// when to use memchr instead of a full prefix
const TEDDY_MEMCHR_MAX_FREQ: u64 = 2_500;
const TEDDY_MEMCHR_MAX_FREQ_F: u64 = 1_500;
#[cfg(feature = "convergence_prefix")]
const CONV_MEMCHR_MAX: u64 = 5_000;
const RARE_BYTE_FREQ_LIMIT: u16 = 25_000;

/// Forward literal prefix for patterns with no `_*` stripping.
/// Returns `Some` only when the pattern has a tight literal prefix and the
/// rarest byte in it is not too common.
pub fn build_strict_literal_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    {
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
        if crate::simd::BYTE_FREQ[lit.rare_byte() as usize] >= RARE_BYTE_FREQ_LIMIT {
            return Ok(None);
        }
        Ok(Some(crate::accel::FwdPrefixSearch::Literal(lit)))
    }
}

pub fn build_fwd_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    if !crate::simd::has_simd() {
        return Ok(None);
    }
    build_fwd_prefix_simd(b, node)
}

fn try_build_fwd_search(
    b: &mut RegexBuilder,
    sets: &[TSetId],
    allow_common: bool,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();
    try_build_fwd_search_raw(&byte_sets_raw, allow_common)
}

fn try_build_fwd_search_raw(
    byte_sets_raw: &[Vec<u8>],
    allow_common: bool,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let lit_len = byte_sets_raw.iter().take_while(|bs| bs.len() == 1).count();
    if lit_len >= 3 {
        let needle: Vec<u8> = byte_sets_raw[..lit_len].iter().map(|bs| bs[0]).collect();
        let lit = crate::simd::FwdLiteralSearch::new(&needle);
        if lit_len == byte_sets_raw.len()
            || crate::simd::BYTE_FREQ[lit.rare_byte() as usize] < RARE_BYTE_FREQ_LIMIT
        {
            return Ok(Some(crate::accel::FwdPrefixSearch::Literal(lit)));
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
        return Ok(None);
    }
    freqs.sort_by_key(|&(_, f)| f);

    let rarest_idx = freqs[0].0;
    let rarest_freq_sum = freqs[0].1;
    let rarest_len = byte_sets_raw[rarest_idx].len();

    let narrow_positions = byte_sets_raw
        .iter()
        .map(|bs| {
            bs.iter()
                .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u64)
                .sum::<u64>()
        })
        .filter(|&f| f <= TEDDY_WEAK_POSITION_FREQ)
        .count();
    let non_full_positions = byte_sets_raw.iter().filter(|bs| bs.len() < 256).count();
    if byte_sets_raw.len() > 1 && non_full_positions <= 1 {
        if cfg!(feature = "debug") {
            eprintln!(
                "  [fwd-prefix] reject: only {} discriminating position(s) in {}-byte prefix",
                non_full_positions,
                byte_sets_raw.len()
            );
        }
        return Ok(None);
    }
    let degenerate = byte_sets_raw.len() == 1;
    if degenerate && rarest_freq_sum > TEDDY_MEMCHR_MAX_FREQ_F {
        let _ = narrow_positions;
        if cfg!(feature = "debug") {
            eprintln!(
                "  [fwd-prefix] teddy-degenerate, trying range: rarest_freq={} > {} (narrow_positions={})",
                rarest_freq_sum, TEDDY_MEMCHR_MAX_FREQ_F, narrow_positions
            );
        }
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx, allow_common).map(|r| r.0);
    }

    if rarest_len > 16 {
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx, false).map(|r| r.0);
    }

    // Reject Teddy when the rarest position is too common (high false-positive
    // rate). Try a range-based prefix first; if that also fails, skip entirely.
    if rarest_freq_sum > TEDDY_MAX_FREQ_SUM {
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx, false).map(|r| r.0);
    }

    let freq_order: Vec<usize> = freqs.iter().map(|&(i, _)| i).collect();

    if cfg!(feature = "debug") {
        let _ = &freqs;
        eprintln!(
            "  [fwd-prefix] anchor=pos{} ({} bytes)",
            freq_order[0],
            byte_sets_raw[freq_order[0]].len()
        );
    }

    let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();

    Ok(Some(crate::accel::FwdPrefixSearch::Prefix(
        crate::simd::FwdPrefixSearch::new(
            byte_sets_raw.len(),
            &freq_order,
            byte_sets_raw,
            all_sets,
        ),
    )))
}

fn rarest_freq(b: &mut RegexBuilder, sets: &[TSetId]) -> u64 {
    sets.iter()
        .map(|&s| {
            b.solver()
                .collect_bytes(s)
                .iter()
                .map(|&byte| crate::simd::BYTE_FREQ[byte as usize] as u64)
                .sum::<u64>()
        })
        .min()
        .unwrap_or(u64::MAX)
}

fn build_fwd_prefix_from_sets(
    b: &mut RegexBuilder,
    full_sets: &[TSetId],
    allow_common: bool,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    if !full_sets.is_empty() {
        return try_build_fwd_search(b, full_sets, allow_common);
    }
    Ok(None)
}

fn every_first_byte_is_full_match(b: &mut RegexBuilder, node: NodeId) -> bool {
    let der = match b.der(node, Nullability::CENTER) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
    b.collect_der_targets(der, TSetId::FULL, &mut targets);
    let mut any = false;
    for (t, _) in targets {
        if t == NodeId::BOT {
            continue;
        }
        any = true;
        if !b.nullability(t).has(Nullability::CENTER) {
            return false;
        }
    }
    any
}

fn build_fwd_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let full_sets = calc_potential_start(b, node, 16, 64, false)?;
    let allow_common = every_first_byte_is_full_match(b, node);
    build_fwd_prefix_from_sets(b, &full_sets, allow_common)
}

const MAX_RANGE_SETS: usize = 3;

fn try_build_fwd_range_prefix(
    byte_sets_raw: &[Vec<u8>],
    anchor_pos: usize,
    allow_common: bool,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let anchor_bytes = &byte_sets_raw[anchor_pos];
    let freq_sum: u32 = anchor_bytes
        .iter()
        .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u32)
        .sum();
    // Space (0x20) is saturated at u16::MAX (65535); we want to reject it as
    // a sole anchor since it's the most common byte in typical text.
    const RANGE_FREQ_THRESHOLD: u32 = 65_535;
    if !allow_common && freq_sum >= RANGE_FREQ_THRESHOLD {
        if cfg!(feature = "debug") {
            eprintln!(
                "  [fwd-prefix-range] reject: {} bytes, freq_sum={} >= {}",
                anchor_bytes.len(),
                freq_sum,
                RANGE_FREQ_THRESHOLD
            );
        }
        return Ok((None, false));
    }
    let tset = crate::accel::TSet::from_bytes(anchor_bytes);
    let exact_ranges: Vec<(u8, u8)> = Solver::pp_collect_ranges(&tset).into_iter().collect();
    if exact_ranges.is_empty() {
        return Ok((None, false));
    }
    let ranges: Vec<(u8, u8)> = if exact_ranges.len() <= MAX_RANGE_SETS {
        exact_ranges
    } else {
        let ascii_only: Vec<u8> = anchor_bytes.iter().copied().filter(|&b| b < 0x80).collect();
        let has_high = anchor_bytes.iter().any(|&b| b >= 0x80);
        if !has_high {
            return Ok((None, false));
        }
        let ascii_tset = crate::accel::TSet::from_bytes(&ascii_only);
        let mut coarse: Vec<(u8, u8)> =
            Solver::pp_collect_ranges(&ascii_tset).into_iter().collect();
        coarse.push((0x80, 0xFF));
        if coarse.len() > MAX_RANGE_SETS {
            return Ok((None, false));
        }
        if cfg!(feature = "debug") {
            eprintln!(
                "  [fwd-prefix-range] coarsened {} ranges -> {} (high-byte fold)",
                exact_ranges.len(),
                coarse.len()
            );
        }
        coarse
    };
    let all_sets: Vec<crate::accel::TSet> = byte_sets_raw
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();
    if cfg!(feature = "debug") {
        eprintln!(
            "  [fwd-prefix-range] anchor=pos{} ranges={:?} len={}",
            anchor_pos,
            ranges,
            byte_sets_raw.len()
        );
    }
    Ok((
        Some(crate::accel::FwdPrefixSearch::Range(
            crate::simd::FwdRangeSearch::new(byte_sets_raw.len(), anchor_pos, ranges, all_sets),
        )),
        false,
    ))
}

/// Build a `RevTeddySearch` from byte sets, or return `None` if the sets are
/// too wide to be useful.  `len >= 2` required (single-byte case is handled by
/// the DFA skip system).
pub(crate) fn build_rev_prefix_search(
    b: &mut RegexBuilder,
    sets: &[TSetId],
    memchr_max: u64,
) -> Option<crate::accel::RevTeddySearch> {
    if sets.len() < 1 {
        return None;
    }
    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();
    let num_simd = sets.len().min(3);
    // per-position freq for every position in the full rev prefix
    let pos_freq: Vec<u64> = byte_sets_raw
        .iter()
        .map(|bs| {
            bs.iter()
                .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u64)
                .sum::<u64>()
        })
        .collect();
    let mut tail_offset = 0usize;
    let mut best_prod = u128::MAX;
    for off in 0..=byte_sets_raw.len() - num_simd {
        let prod: u128 = pos_freq[off..off + num_simd]
            .iter()
            .map(|&f| f as u128)
            .product();
        if prod < best_prod {
            best_prod = prod;
            tail_offset = off;
        }
    }
    let freq_sums: Vec<u64> = pos_freq[tail_offset..tail_offset + num_simd].to_vec();
    let rarest_freq_sum = *freq_sums.iter().min().unwrap_or(&u64::MAX);
    if rarest_freq_sum > TEDDY_MAX_FREQ_SUM {
        return None;
    }
    let narrow = freq_sums
        .iter()
        .filter(|&&f| f <= TEDDY_WEAK_POSITION_FREQ)
        .count();
    if narrow < 2 && rarest_freq_sum > memchr_max {
        return None;
    }
    let combined_freq: u128 = freq_sums.iter().map(|&f| f as u128).product();
    let threshold: u128 = 12 * (TOTAL_BYTE_FREQ as u128).pow(num_simd as u32) / 256;
    if combined_freq > threshold {
        return None;
    }
    let window = &byte_sets_raw[tail_offset..tail_offset + num_simd];
    let all_sets: Vec<crate::accel::TSet> = window
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();
    Some(crate::accel::RevTeddySearch::new(
        num_simd,
        window,
        all_sets,
        tail_offset,
    ))
}

/// Runtime prefix acceleration
#[cfg_attr(debug_assertions, derive(Debug))]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize, Clone)
)]
pub enum PrefixKind {
    AnchoredRev,
    AnchoredFwd(crate::accel::FwdPrefixSearch),
    AnchoredFwdLb(crate::accel::FwdPrefixSearch),
    PotentialStart,
    #[cfg(feature = "convergence_prefix")]
    Convergence,
}

impl PrefixKind {
    #[cfg(feature = "diag")]
    pub(crate) fn is_fwd(&self) -> bool {
        matches!(
            self,
            PrefixKind::AnchoredFwd(_) | PrefixKind::AnchoredFwdLb(_)
        )
    }

    #[cfg(feature = "diag")]
    pub(crate) fn is_rev(&self) -> bool {
        #[cfg(feature = "convergence_prefix")]
        return matches!(
            self,
            PrefixKind::AnchoredRev | PrefixKind::PotentialStart | PrefixKind::Convergence
        );
        #[cfg(not(feature = "convergence_prefix"))]
        matches!(self, PrefixKind::AnchoredRev | PrefixKind::PotentialStart)
    }
}

#[allow(dead_code)]
pub(crate) fn try_rev_prefix(
    b: &mut RegexBuilder,
    rev_node: NodeId,
) -> Result<Option<(PrefixKind, crate::accel::RevTeddySearch)>, Error> {
    use resharp_algebra::nulls::NullsId;
    if b.get_nulls_id(rev_node) != NullsId::EMPTY {
        return Ok(None);
    }
    let anchored = calc_prefix_sets(b, rev_node)?;
    if !anchored.is_empty() {
        if let Some(s) = build_rev_prefix_search(b, &anchored, TEDDY_MEMCHR_MAX_FREQ) {
            return Ok(Some((PrefixKind::AnchoredRev, s)));
        }
    }
    let potential = calc_potential_start_prune(b, rev_node, 16, 64, true)?;
    if !potential.is_empty() {
        if let Some(s) = build_rev_prefix_search(b, &potential, TEDDY_MEMCHR_MAX_FREQ) {
            return Ok(Some((PrefixKind::PotentialStart, s)));
        }
    }
    Ok(None)
}

pub(crate) fn select_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
    rev_start: NodeId,
    has_look: bool,
    min_len: u32,
    max_cap: usize,
    no_fwd_prefix: bool,
    hardened: bool,
    force_convergence: bool,
) -> Result<
    (
        Option<PrefixKind>,
        Option<(crate::accel::RevTeddySearch, Option<NodeId>, Option<NodeId>)>,
        bool,
    ),
    Error,
> {
    if !crate::simd::has_simd() {
        return Ok((None, None, false));
    }
    let _ = force_convergence;
    let (kind, skip, fwd_wins, selected_cost, rev_stripped) =
        select_prefix_simd(b, node, rev_start, has_look, min_len, no_fwd_prefix, hardened)?;
    #[cfg(not(feature = "convergence_prefix"))]
    let _ = (selected_cost, rev_stripped);
    #[cfg(feature = "convergence_prefix")]
    if let Some((conv_kind, conv_skip, conv_node, b_node, conv_cost)) =
        try_convergence_prefix(b, node, rev_stripped, force_convergence)?
    {
        let penalty = if matches!(kind, Some(PrefixKind::PotentialStart)) {
            1
        } else {
            CONV_PENALTY
        };
        if force_convergence
            || kind.is_none()
            || conv_cost.saturating_mul(penalty) < selected_cost
        {
            return Ok((
                Some(conv_kind),
                Some((conv_skip, Some(conv_node), Some(b_node))),
                false,
            ));
        }
    }
    let _ = max_cap;
    Ok((kind, skip.map(|s| (s, None, None)), fwd_wins))
}

#[cfg(feature = "convergence_prefix")]
const RESUME_STOPPER_MIN: u64 = 30_000;

#[cfg(feature = "convergence_prefix")]
fn loop_stopper_set(b: &mut RegexBuilder, body: NodeId) -> Result<TSetId, Error> {
    let der = b
        .der(body, resharp_algebra::nulls::Nullability::CENTER)
        .map_err(crate::Error::Algebra)?;
    let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
    b.collect_der_targets(der, TSetId::FULL, &mut targets);
    let leading = targets
        .iter()
        .filter(|(t, _)| *t != NodeId::BOT)
        .fold(TSetId::EMPTY, |acc, &(_, cs)| b.solver().or_id(acc, cs));
    Ok(b.solver().not_id(leading))
}

#[cfg(feature = "convergence_prefix")]
fn set_byte_freq(b: &mut RegexBuilder, set: TSetId) -> u64 {
    b.solver()
        .collect_bytes(set)
        .iter()
        .map(|&c| crate::simd::BYTE_FREQ[c as usize] as u64)
        .sum()
}

#[cfg(feature = "convergence_prefix")]
fn resume_loops_die_fast(
    b: &mut RegexBuilder,
    conv_node: NodeId,
    run: &[TSetId],
) -> Result<bool, Error> {
    let run_union = run
        .iter()
        .fold(TSetId::EMPTY, |acc, &s| b.solver().or_id(acc, s));
    let mut spine: Vec<NodeId> = Vec::new();
    let mut curr = conv_node;
    loop {
        let is_concat = curr.is_concat(b);
        spine.push(if is_concat { curr.left(b) } else { curr });
        if is_concat {
            curr = curr.right(b);
        } else {
            break;
        }
    }
    let last_mandatory = spine
        .iter()
        .rposition(|&h| b.get_min_max_length(h).0 > 0);
    for (idx, &head) in spine.iter().enumerate() {
        if !head.is_star(b) {
            continue;
        }
        if last_mandatory.is_none_or(|m| idx > m) {
            continue;
        }
        let stopper = loop_stopper_set(b, head.left(b))?;
        if b.solver().is_sat_id(stopper, run_union) {
            continue;
        }
        if set_byte_freq(b, stopper) < RESUME_STOPPER_MIN {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(feature = "convergence_prefix")]
fn convergence_right_node(b: &RegexBuilder, fwd_node: NodeId, run: &[TSetId]) -> Option<NodeId> {
    fn head_byte(b: &RegexBuilder, h: NodeId) -> Option<u8> {
        if !h.is_pred(b) {
            return None;
        }
        let bytes = b.solver_ref().collect_bytes(h.pred_tset(b));
        (bytes.len() == 1).then(|| bytes[0])
    }
    let lit: Vec<u8> = run
        .iter()
        .rev()
        .map(|&s| {
            let bytes = b.solver_ref().collect_bytes(s);
            (bytes.len() == 1).then(|| bytes[0])
        })
        .collect::<Option<Vec<u8>>>()?;
    let mut spine: Vec<(NodeId, NodeId)> = Vec::new();
    let mut curr = fwd_node;
    loop {
        let is_concat = curr.is_concat(b);
        let head = if is_concat { curr.left(b) } else { curr };
        spine.push((curr, head));
        if is_concat {
            curr = curr.right(b);
        } else {
            break;
        }
    }
    let n = spine.len();
    if lit.is_empty() || lit.len() > n {
        return None;
    }
    let mut found: Option<usize> = None;
    for start in 0..=(n - lit.len()) {
        if (0..lit.len()).all(|k| head_byte(b, spine[start + k].1) == Some(lit[k])) {
            if found.is_some() {
                return None;
            }
            found = Some(start);
        }
    }
    let end = found? + lit.len();
    Some(if end >= n { NodeId::EPS } else { spine[end].0 })
}

#[cfg(feature = "convergence_prefix")]
fn conv_leading_set(b: &mut RegexBuilder, node: NodeId) -> Result<TSetId, Error> {
    let d = b.der(node, Nullability::CENTER)?;
    let mut stack = vec![(d, TSetId::FULL)];
    let mut lead = TSetId::EMPTY;
    b.iter_sat(&mut stack, &mut |bb, n, set| {
        if n != NodeId::BOT {
            lead = bb.solver().or_id(lead, set);
        }
    });
    Ok(lead)
}

fn conv_der_through_set(b: &mut RegexBuilder, node: NodeId, s: TSetId) -> Result<NodeId, Error> {
    let der = b.der(node, Nullability::CENTER)?;
    let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
    b.collect_der_targets(der, TSetId::FULL, &mut targets);
    for (t, set) in targets {
        if t != NodeId::BOT && b.solver().and_id(set, s) != TSetId::EMPTY {
            return Ok(t);
        }
    }
    Ok(NodeId::BOT)
}

fn conv_run_boundary_ambiguous(
    b: &mut RegexBuilder,
    conv_node: NodeId,
    run: &[TSetId],
    b_node: NodeId,
) -> Result<bool, Error> {
    if run.is_empty() {
        return Ok(false);
    }
    let mut run_core = TSetId::FULL;
    for &s in run {
        run_core = b.solver().and_id(run_core, s);
    }
    if run_core == TSetId::EMPTY {
        return Ok(false);
    }
    let b_lead = conv_leading_set(b, b_node)?;
    if b.solver().and_id(run_core, b_lead) == TSetId::EMPTY {
        return Ok(false);
    }
    let mut left = conv_node;
    for &s in run.iter().rev() {
        left = conv_der_through_set(b, left, s)?;
        if left == NodeId::BOT {
            return Ok(true);
        }
    }
    let left_lead = conv_leading_set(b, left)?;
    Ok(b.solver().and_id(run_core, left_lead) == TSetId::EMPTY)
}

#[cfg(feature = "convergence_prefix")]
fn try_convergence_prefix(
    b: &mut RegexBuilder,
    fwd_node: NodeId,
    rev_stripped: NodeId,
    force: bool,
) -> Result<Option<(PrefixKind, crate::accel::RevTeddySearch, NodeId, NodeId, u64)>, Error> {
    let (fwd_min, fwd_max) = b.get_min_max_length(fwd_node);
    if fwd_min == 0 {
        return Ok(None);
    }
    if !force
        && fwd_max <= CONV_BOUNDED_MAX
        && !b.contains_anchors(fwd_node)
        && !fwd_node.contains_lookaround(b)
    {
        return Ok(None);
    }
    let Some((conv_node, run, l_rep)) = crate::find_inner_literal(b, rev_stripped)
    else {
        return Ok(None);
    };
    let Some(b_node) = convergence_right_node(b, fwd_node, &run) else {
        return Ok(None);
    };
    let (b_min, b_max) = b.get_min_max_length(b_node);
    if b_max == u32::MAX
        && (b_min >= 2 || b_node.contains_lookaround(b))
        && conv_run_boundary_ambiguous(b, conv_node, &run, b_node)?
    {
        return Ok(None);
    }
    if !force && !resume_loops_die_fast(b, conv_node, &run)? {
        return Ok(None);
    }
    let avoid_l = {
        let any_non_l = b.mk_pred_not(l_rep);
        b.mk_star(any_non_l)
    };
    let without_l = b.mk_inter(fwd_node, avoid_l);
    if b.is_empty_lang(without_l) != Some(true) {
        return Ok(None);
    }
    let Some(search) = build_rev_prefix_search(b, &run, CONV_MEMCHR_MAX) else {
        return Ok(None);
    };
    if !force && conv_b_interior_unbounded(b, b_node) {
        return Ok(None);
    }
    let b_potential = calc_potential_start(b, b_node, 16, 64, false)?;
    let b_shape = classify_body_shape(b, b_node, &b_potential);
    let conv_cost = scan_cost(b, &run, Direction::Fwd, b_shape);
    Ok(Some((
        PrefixKind::Convergence,
        search,
        conv_node,
        b_node,
        conv_cost,
    )))
}

fn strip_leading_lookbehind(b: &RegexBuilder, mut node: NodeId) -> NodeId {
    loop {
        if !node.is_concat(b) {
            break;
        }
        if !node.left(b).is_lookbehind(b) {
            break;
        }
        node = node.right(b);
    }
    node
}

fn node_lead_bytes(b: &mut RegexBuilder, node: NodeId) -> TSetId {
    use resharp_algebra::nulls::Nullability;
    match b.der(node, Nullability::CENTER) {
        Ok(d) => {
            let mut stack = vec![(d, TSetId::FULL)];
            let mut acc = TSetId::EMPTY;
            b.iter_sat(&mut stack, &mut |bb, _n, set| {
                acc = bb.solver().or_id(acc, set);
            });
            acc
        }
        Err(_) => b.solver().not_id(TSetId::EMPTY),
    }
}

fn loop_body_class(b: &mut RegexBuilder, node: NodeId) -> Option<TSetId> {
    if node.is_star(b) {
        return Some(node_lead_bytes(b, node));
    }
    if node.is_lookahead(b) {
        let tail = node.right(b);
        if let Some(s) = loop_body_class(b, tail) {
            return Some(s);
        }
        return Some(node_lead_bytes(b, node));
    }
    if node.is_concat(b) {
        let left = node.left(b);
        return loop_body_class(b, left);
    }
    if node.is_inter(b) {
        let (l, r) = (node.left(b), node.right(b));
        if let Some(s) = loop_body_class(b, l) {
            return Some(s);
        }
        return loop_body_class(b, r);
    }
    None
}

fn fwd_interior_quadratic(b: &mut RegexBuilder, node: NodeId) -> bool {
    let mut seen_swallowing_loop = false;
    let mut prior_leads: Vec<TSetId> = Vec::new();
    let mut mandatory_prefix: Vec<TSetId> = Vec::new();
    let mut curr = node;
    loop {
        let is_concat = curr.is_concat(b);
        let head = if is_concat { curr.left(b) } else { curr };
        let (hmin, hmax) = b.get_min_max_length(head);
        let lead = node_lead_bytes(b, head);
        let single_position = hmin == 1 && hmax == 1;
        if seen_swallowing_loop && hmin > 0 {
            let absorbed = single_position
                && mandatory_prefix
                    .iter()
                    .any(|&m| b.solver().and_id(m, lead) == m);
            if !absorbed {
                return true;
            }
        }
        if hmax == u32::MAX && !prior_leads.is_empty() {
            let body = loop_body_class(b, head).unwrap_or(lead);
            let loop_chains_candidates = prior_leads
                .iter()
                .all(|&p| b.solver().is_sat_id(p, body));
            if loop_chains_candidates {
                if !is_pure_trailing_run(b, head) {
                    return true;
                }
                seen_swallowing_loop = true;
            }
        }
        if single_position {
            mandatory_prefix.push(lead);
        }
        prior_leads.push(lead);
        if is_concat {
            curr = curr.right(b);
        } else {
            break;
        }
    }
    false
}

fn select_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
    rev_start: NodeId,
    has_look: bool,
    min_len: u32,
    no_fwd_prefix: bool,
    hardened: bool,
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevTeddySearch>, bool, u64, NodeId), Error> {
    use resharp_algebra::nulls::NullsId;
    if min_len == 0 {
        if !no_fwd_prefix && has_look && node.contains_lookbehind(b) {
            if let Some(fp) = try_build_fwd_lb(b, node)? {
                return Ok((Some(PrefixKind::AnchoredFwdLb(fp)), None, false, u64::MAX, NodeId::BOT));
            }
        }
        return Ok((None, None, false, u64::MAX, NodeId::BOT));
    }
    let sets = PrefixSets::compute(b, node, rev_start)?;
    let rev_stripped = sets.rev_stripped;

    let fwd_cost = sets
        .fwd_potential
        .cost
        .min(sets.fwd_potential_stripped.cost);
    let rev_cost = sets.rev_anchored.cost.min(sets.rev_potential.cost);
    let rev_usable = b.get_nulls_id(rev_start) == NullsId::EMPTY
        && (!sets.rev_anchored.sets.is_empty() || !sets.rev_potential.sets.is_empty());
    let (_, max_len) = b.get_min_max_length(node);
    let bounded = max_len != u32::MAX;
    let fwd_quad = !bounded && fwd_interior_quadratic(b, node);
    let fwd_wins = !fwd_quad && (bounded || fwd_cost < rev_cost);

    let fwd_candidate = if fwd_quad {
        None
    } else if no_fwd_prefix {
        if !hardened && fwd_wins {
            if has_look && node.contains_lookbehind(b) {
                match try_build_fwd_lb(b, node)? {
                    Some(fp) => Some(PrefixKind::AnchoredFwdLb(fp)),
                    None => {
                        try_build_fwd_neg_lb(b, node)?.map(|(fp, _)| PrefixKind::AnchoredFwd(fp))
                    }
                }
            } else {
                let allow_common = every_first_byte_is_full_match(b, node);
                build_fwd_prefix_from_sets(b, &sets.fwd_potential.sets, allow_common)?
                    .map(PrefixKind::AnchoredFwd)
            }
        } else {
            None
        }
    } else if has_look && node.contains_lookbehind(b) {
        match try_build_fwd_lb(b, node)? {
            Some(fp) => Some(PrefixKind::AnchoredFwdLb(fp)),
            None => try_build_fwd_neg_lb(b, node)?.map(|(fp, _)| PrefixKind::AnchoredFwd(fp)),
        }
    } else {
        let allow_common = every_first_byte_is_full_match(b, node);
        let fp = build_fwd_prefix_from_sets(b, &sets.fwd_potential.sets, allow_common)?;
        match fp {
            Some(fp) => Some(PrefixKind::AnchoredFwd(fp)),
            None if b.is_infinite(node) => {
                build_strict_literal_prefix(b, node)?.map(PrefixKind::AnchoredFwd)
            }
            None => None,
        }
    };
    let try_rev = |b: &mut RegexBuilder| -> Option<(PrefixKind, crate::accel::RevTeddySearch)> {
        if !rev_usable {
            return None;
        }
        if !sets.rev_anchored.sets.is_empty() {
            if let Some(s) = build_rev_prefix_search(b, &sets.rev_anchored.sets, TEDDY_MEMCHR_MAX_FREQ)
            {
                return Some((PrefixKind::AnchoredRev, s));
            }
        }
        if !sets.rev_potential.sets.is_empty() {
            if let Some(s) =
                build_rev_prefix_search(b, &sets.rev_potential.sets, TEDDY_MEMCHR_MAX_FREQ)
            {
                return Some((PrefixKind::PotentialStart, s));
            }
        }
        None
    };

    if fwd_wins || no_fwd_prefix {
        if let Some(kind) = fwd_candidate {
            return Ok((Some(kind), None, fwd_wins, fwd_cost, rev_stripped));
        }
    }
    if let Some((kind, s)) = try_rev(b) {
        return Ok((Some(kind), Some(s), false, rev_cost, rev_stripped));
    }
    if let Some(kind) = fwd_candidate {
        return Ok((Some(kind), None, fwd_wins, fwd_cost, rev_stripped));
    }
    Ok((None, None, false, u64::MAX, rev_stripped))
}

/// The positive byte-class a leading lookbehind contributes to a fwd prefix,
/// plus the lb length consumed. None when there is no usable fixed-length class.
pub(crate) fn fwd_lb_class(b: &mut RegexBuilder, lb: NodeId) -> Option<(NodeId, u32)> {
    if let Some(pred) = b.neg_lookbehind_prev_pred(lb) {
        return Some((pred, 1));
    }
    let lb_inner = b.get_lookbehind_inner(lb);
    let mut lb_stripped = b.nonbegins(lb_inner);
    loop {
        let stripped = b.strip_prefix_safe(lb_stripped);
        let after = b.nonbegins(stripped);
        if after == lb_stripped {
            break;
        }
        lb_stripped = after;
    }
    match b.get_fixed_length(lb_stripped) {
        Some(len @ 1..=64) => Some((lb_stripped, len)),
        _ => None,
    }
}

fn try_build_fwd_lb(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, Error> {
    #[cfg(feature = "debug")]
    eprintln!("  [try_build_fwd_lb] node={:?}", b.pp(node));
    let body = strip_leading_lookbehind(b, node);
    if body == node || node.right(b) != body {
        return Ok(None);
    }
    let lb = node.left(b);
    if !lb.is_lookbehind(b) {
        return Ok(None);
    }
    let Some((lb_stripped, _)) = fwd_lb_class(b, lb) else {
        return Ok(None);
    };
    if body_absorbs_lb(b, body, lb_stripped)? {
        #[cfg(feature = "debug")]
        eprintln!("  [fwd-lb] reject: body's leading star absorbs lb byte(s)");
        return Ok(None);
    }
    let lb_body = b.mk_concat(lb_stripped, body);
    #[cfg(feature = "debug")]
    eprintln!("  [try_build_fwd_lb] lb_stripped={:?}, body={:?}, lb_body={:?}", b.pp(lb_stripped), b.pp(body), b.pp(lb_body));
    let result = build_fwd_prefix(b, lb_body);
    #[cfg(feature = "debug")]
    eprintln!("  [try_build_fwd_lb] result={:?}", result.as_ref().map(|_| "Some"));
    result
}

/// One forbidden fixed-length suffix: a sequence of single-byte classes. A
/// candidate match start at `p` is matched (forbidden) iff `p >= len` and
/// `input[p-len+i]` is in `classes[i]` for all `i`.
#[cfg_attr(debug_assertions, derive(Debug))]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize, Clone)
)]
pub struct NegLbTerm {
    pub len: usize,
    pub classes: Vec<[u64; 4]>,
}

impl NegLbTerm {
    #[inline]
    fn matches(&self, input: &[u8], start: usize) -> bool {
        if start < self.len {
            return false;
        }
        let base = start - self.len;
        for (i, set) in self.classes.iter().enumerate() {
            let byte = input[base + i];
            if set[(byte >> 6) as usize] & (1u64 << (byte & 63)) == 0 {
                return false;
            }
        }
        true
    }
}

/// Fixed-length negative lookbehind verifier: a candidate start at `p` is
/// rejected iff any forbidden term matches the bytes before `p`.
#[cfg_attr(debug_assertions, derive(Debug))]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize, Clone)
)]
pub struct NegLb {
    pub terms: Vec<NegLbTerm>,
}

impl NegLb {
    #[inline]
    pub(crate) fn rejects(&self, input: &[u8], start: usize) -> bool {
        self.terms.iter().any(|t| t.matches(input, start))
    }
}

/// Parse a negative term `\A~(_*X)` into its single-byte class sequence, where
/// `X` is a `Pred` chain. Anything wider bails to `None`.
fn parse_neg_term(b: &mut RegexBuilder, term: NodeId) -> Option<Vec<TSetId>> {
    if !term.is_concat(b) || term.left(b) != NodeId::BEGIN {
        return None;
    }
    let compl = term.right(b);
    if !compl.is_compl(b) {
        return None;
    }
    let body_ts = compl.left(b);
    if !body_ts.is_concat(b) || body_ts.left(b) != NodeId::TS {
        return None;
    }
    let mut x = body_ts.right(b);
    let mut seq = Vec::new();
    loop {
        match b.get_kind(x) {
            Kind::Pred => {
                seq.push(x.pred_tset(b));
                break;
            }
            Kind::Concat if x.left(b).is_pred(b) => {
                seq.push(x.left(b).pred_tset(b));
                x = x.right(b);
            }
            _ => return None,
        }
    }
    if seq.is_empty() || seq.len() > 64 {
        return None;
    }
    Some(seq)
}

/// Detect a leading negative lookbehind `(?<!X)body`, where the lookbehind inner
/// is one negative term or an intersection `~(_*X1) & ~(_*X2) & ...` (e.g. a
/// hand-written guard merged with a `\b`). Returns `(body, per-term sequences)`.
/// Every term must be a single-byte-class chain; otherwise `None`.
fn neg_lb_body_and_seq(b: &mut RegexBuilder, node: NodeId) -> Option<(NodeId, Vec<Vec<TSetId>>)> {
    if !node.is_concat(b) {
        return None;
    }
    let lb = node.left(b);
    let body = node.right(b);
    if !lb.is_lookbehind(b) {
        return None;
    }
    let inner = b.get_lookbehind_inner(lb);
    let mut terms = Vec::new();
    let mut cur = inner;
    while cur.is_inter(b) {
        terms.push(parse_neg_term(b, cur.left(b))?);
        cur = cur.right(b);
    }
    terms.push(parse_neg_term(b, cur)?);
    Some((body, terms))
}

/// Build a body-literal forward prefix for a leading fixed-length negative
/// lookbehind. The lookbehind is verified separately by [`NegLb`]; the literal
/// is a necessary condition on the consumed bytes so the prefilter is sound.
pub(crate) fn try_build_fwd_neg_lb(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<(crate::accel::FwdPrefixSearch, NegLb)>, Error> {
    let Some((body, terms)) = neg_lb_body_and_seq(b, node) else {
        return Ok(None);
    };
    let Some(search) = build_fwd_prefix(b, body)? else {
        return Ok(None);
    };
    let neg = build_neg_lb(b, &terms);
    Ok(Some((search, neg)))
}

fn build_neg_lb(b: &mut RegexBuilder, terms: &[Vec<TSetId>]) -> NegLb {
    let terms = terms
        .iter()
        .map(|seq| {
            let classes = seq
                .iter()
                .map(|&set| {
                    let mut bits = [0u64; 4];
                    for byte in b.solver().collect_bytes(set) {
                        bits[(byte >> 6) as usize] |= 1u64 << (byte & 63);
                    }
                    bits
                })
                .collect();
            NegLbTerm {
                len: seq.len(),
                classes,
            }
        })
        .collect();
    NegLb { terms }
}

/// Recompute the [`NegLb`] verifier for a node already selected for a body-only
/// forward prefix (used at construction to attach the reject test).
pub(crate) fn neg_lb_classes(b: &mut RegexBuilder, node: NodeId) -> Option<NegLb> {
    let (_, terms) = neg_lb_body_and_seq(b, node)?;
    Some(build_neg_lb(b, &terms))
}

fn body_absorbs_lb(b: &mut RegexBuilder, body: NodeId, lb: NodeId) -> Result<bool, crate::Error> {
    let body_first = calc_potential_start(b, body, 1, 64, false)?;
    let lb_first = calc_potential_start(b, lb, 1, 64, false)?;
    let (Some(&bf), Some(&lf)) = (body_first.first(), lb_first.first()) else {
        return Ok(false);
    };
    let body_bytes = b.solver().collect_bytes(bf);
    let lb_bytes = b.solver().collect_bytes(lf);
    if body_bytes.len() < 64 {
        return Ok(false);
    }
    let body_set: std::collections::BTreeSet<u8> = body_bytes.iter().copied().collect();
    Ok(lb_bytes.iter().all(|b| body_set.contains(b)))
}
