use resharp_algebra::nulls::Nullability;
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{Kind, NodeId, RegexBuilder};
use std::collections::BTreeSet;

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
            b.collect_der_targets(der, TSetId::FULL, &mut targets);

            for &(target, char_set) in &targets {
                if exclude_initial && target == initial_node {
                    continue;
                }
                if target == NodeId::BOT {
                    continue;
                }
                union_set = b.solver().or_id(union_set, char_set);
                next_nodes.insert(target);
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

#[derive(Clone, Debug)]
pub struct PrefixSet {
    pub sets: Vec<TSetId>,
    /// per-byte cost (lower = faster). `u64::MAX` for empty
    pub cost: u64,
}

/// Prefix sets for both directions
pub struct PrefixSets {
    /// Tight anchored fwd prefix.  Every match starts exactly at a SIMD hit.
    // pub fwd_anchored: PrefixSet,
    /// Potential-start fwd sets (full node, self-loop bytes included).
    pub fwd_potential: PrefixSet,
    /// Potential-start fwd sets after stripping a leading `_*`.
    pub fwd_potential_stripped: PrefixSet,
    /// Tight anchored rev prefix.  Every match ends with this byte sequence
    /// (read right-to-left).
    pub rev_anchored: PrefixSet,
    /// Potential-start rev sets.
    pub rev_potential: PrefixSet,
}

impl PrefixSets {
    /// Compute all prefix-set sequences for `node` (fwd) and `rev_start`
    /// (already reversed, not yet stripped), along with body shape and the
    /// estimated per-byte scan costs for each direction.
    pub fn compute(
        b: &mut RegexBuilder,
        node: NodeId,
        rev_start: NodeId,
    ) -> Result<Self, crate::Error> {
        let fwd_body = strip_leading_lookbehind(b, node);
        let stripped_node = b.strip_prefix_safe(node);
        let fwd_body_stripped = strip_leading_lookbehind(b, stripped_node);

        // let fwd_anchored_sets = {
        //     let n = b.prune_begin(node);
        //     let n = b.strip_prefix_safe(n);
        //     calc_prefix_sets(b, n)?
        // };
        let fwd_potential_sets = calc_potential_start(b, fwd_body, 16, 64, false)?;
        let fwd_potential_stripped_sets =
            calc_potential_start(b, fwd_body_stripped, 16, 64, false)?;
        let rev_anchored_sets = calc_prefix_sets(b, rev_start)?;
        let mut rev_potential_sets = calc_potential_start_prune(b, rev_start, 16, 64, true)?;
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

        // let fwd_anchored = mk(fwd_anchored_sets, Direction::Fwd);
        let fwd_potential = mk(fwd_potential_sets, Direction::Fwd);
        let fwd_potential_stripped = mk(fwd_potential_stripped_sets, Direction::Fwd);
        let rev_anchored = mk(rev_anchored_sets, Direction::Rev);
        let rev_potential = mk(rev_potential_sets, Direction::Rev);
        Ok(Self {
            // fwd_anchored,
            fwd_potential,
            fwd_potential_stripped,
            rev_anchored,
            rev_potential,
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
    let num_simd = freqs.len().min(3);
    if num_simd == 0 {
        return u64::MAX;
    }
    let total = TOTAL_BYTE_FREQ as f64;
    let mut best_prod = f64::INFINITY;
    for off in 0..=freqs.len() - num_simd {
        let p: f64 = freqs[off..off + num_simd]
            .iter()
            .map(|&f| f as f64)
            .product();
        if p < best_prod {
            best_prod = p;
        }
    }
    let fire = best_prod / total.powi(num_simd as i32);

    let (scan_per_byte, verify_per_fire) = match dir {
        Direction::Rev => (0.5, 20.0),
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

/// Shape of the body *after* the prefix, controlling fwd-direction verify cost.
#[derive(Copy, Clone, Debug)]
pub enum NodeShape {
    /// Body is `_*` after the prefix — fwd verify is O(1) (saturate to EOI).
    TrailingStar,
    /// Body is bounded length — fwd verify is a small constant.
    Bounded,
    /// Body contains an unbounded wildcard (`_+`, `[^x]+`, ...) before more
    /// constraints — fwd verify per hit is O(remaining input).
    Unbounded,
}

pub(crate) const SKIP_FREQ_THRESHOLD: u32 = 75_000;

/// Threshold above which a byte set is treated as wildcard-like.
const WIDE_SET_BYTES: u32 = 200;

/// Classify body shape past the fwd prefix to set verify cost.
fn classify_body_shape(
    b: &mut RegexBuilder,
    fwd_body: NodeId,
    fwd_potential: &[TSetId],
) -> NodeShape {
    if b.ends_with_ts(fwd_body) {
        return NodeShape::TrailingStar;
    }
    match fwd_potential.last() {
        Some(&last) if b.solver().byte_count(last) > WIDE_SET_BYTES => NodeShape::Unbounded,
        _ => NodeShape::Bounded,
    }
}
const TEDDY_MAX_FREQ_SUM: u64 = 25_000;
// sum of BYTE_FREQ[0..256] in the corpus
const TOTAL_BYTE_FREQ: u64 = 252_052;
// contributes no meaningful filtering (essentially a wildcard).
const TEDDY_WEAK_POSITION_FREQ: u64 = 100_000;
// when to use memchr instead of a full prefix
const TEDDY_MEMCHR_MAX_FREQ: u64 = 2_500;
const TEDDY_MEMCHR_MAX_FREQ_F: u64 = 1_500;
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

/// Forward prefix search, picking the rarest position for the SIMD anchor.
///
/// Returns `(searcher, stripped)`.  `stripped` is true when a leading `_*` was
/// removed - the returned position is a potential *end* position for the match,
/// not the guaranteed start.
pub fn build_fwd_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    if !crate::simd::has_simd() {
        return Ok((None, false));
    }
    build_fwd_prefix_simd(b, node)
}

fn try_build_fwd_search(
    b: &mut RegexBuilder,
    sets: &[TSetId],
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();
    try_build_fwd_search_raw(&byte_sets_raw)
}

/// Core of `try_build_fwd_search`, operating on raw byte sets to avoid
/// requiring a `RegexBuilder`.
fn try_build_fwd_search_raw(
    byte_sets_raw: &[Vec<u8>],
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let lit_len = byte_sets_raw.iter().take_while(|bs| bs.len() == 1).count();
    if cfg!(feature = "debug") {
        // eprintln!(
        //     "  [fwd-prefix] lit_len={} total={} sets={:?}",
        //     lit_len,
        //     byte_sets_raw.len(),
        //     byte_sets_raw
        //         .iter()
        //         .map(|bs| if bs.len() <= 4 {
        //             format!("{:?}", bs)
        //         } else {
        //             format!("[{}b]", bs.len())
        //         })
        //         .collect::<Vec<_>>()
        // );
    }
    if lit_len >= 3 {
        let needle: Vec<u8> = byte_sets_raw[..lit_len].iter().map(|bs| bs[0]).collect();
        let lit = crate::simd::FwdLiteralSearch::new(&needle);
        if cfg!(feature = "debug") {
            // let freq = crate::simd::BYTE_FREQ[lit.rare_byte() as usize];
            // eprintln!(
            //     "  [fwd-prefix] literal {:?} rare={} freq={}",
            //     std::str::from_utf8(&needle).unwrap_or("?"),
            //     lit.rare_byte() as char,
            //     freq
            // );
        }
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
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx).map(|r| r.0);
    }

    if rarest_len > 16 {
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx).map(|r| r.0);
    }

    // Reject Teddy when the rarest position is too common (high false-positive
    // rate). Try a range-based prefix first; if that also fails, skip entirely.
    if rarest_freq_sum > TEDDY_MAX_FREQ_SUM {
        return try_build_fwd_range_prefix(byte_sets_raw, rarest_idx).map(|r| r.0);
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
    _stripped_sets: &[TSetId],
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    if !full_sets.is_empty() {
        if let Some(fp) = try_build_fwd_search(b, full_sets)? {
            return Ok((Some(fp), false));
        }
    }
    Ok((None, false))
}

fn build_fwd_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let stripped_node = b.strip_prefix_safe(node);
    let full_sets = calc_potential_start(b, node, 16, 64, false)?;
    let stripped_sets = calc_potential_start(b, stripped_node, 16, 64, false)?;
    build_fwd_prefix_from_sets(b, &full_sets, &stripped_sets)
}

const MAX_RANGE_SETS: usize = 3;

fn try_build_fwd_range_prefix(
    byte_sets_raw: &[Vec<u8>],
    anchor_pos: usize,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let anchor_bytes = &byte_sets_raw[anchor_pos];
    let freq_sum: u32 = anchor_bytes
        .iter()
        .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u32)
        .sum();
    // Space (0x20) is saturated at u16::MAX (65535); we want to reject it as
    // a sole anchor since it's the most common byte in typical text.
    const RANGE_FREQ_THRESHOLD: u32 = 65_535;
    if freq_sum >= RANGE_FREQ_THRESHOLD {
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

/// Build a `RevPrefixSearch` from byte sets, or return `None` if the sets are
/// too wide to be useful.  `len >= 2` required (single-byte case is handled by
/// the DFA skip system).
pub(crate) fn build_rev_prefix_search(
    b: &mut RegexBuilder,
    sets: &[TSetId],
) -> Option<crate::accel::RevPrefixSearch> {
    if sets.len() < 1 {
        return None;
    }
    let byte_sets_raw: Vec<Vec<u8>> = sets
        .iter()
        .map(|&set| b.solver().collect_bytes(set))
        .collect();
    if cfg!(feature = "debug") {
        // eprintln!(
        //     "  [rev-prefix] total={} sets={:?}",
        //     byte_sets_raw.len(),
        //     byte_sets_raw
        //         .iter()
        //         .map(|bs| if bs.len() <= 4 {
        //             format!("{:?}", bs)
        //         } else {
        //             format!("[{}b]", bs.len())
        //         })
        //         .collect::<Vec<_>>()
        // );
    }
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
    if cfg!(feature = "debug") {
        // eprintln!(
        //     "  [rev-prefix] tail_offset={} window_freqs={:?}",
        //     tail_offset, freq_sums
        // );
    }
    let rarest_freq_sum = *freq_sums.iter().min().unwrap_or(&u64::MAX);
    if rarest_freq_sum > TEDDY_MAX_FREQ_SUM {
        // if cfg!(feature = "debug") {
        //     eprintln!("  [rev-prefix] reject: max sum={}", rarest_freq_sum,);
        // }
        return None;
    }
    let narrow = freq_sums
        .iter()
        .filter(|&&f| f <= TEDDY_WEAK_POSITION_FREQ)
        .count();
    if narrow < 2 && rarest_freq_sum > TEDDY_MEMCHR_MAX_FREQ {
        if cfg!(feature = "debug") {
            // eprintln!(
            //     "  [rev-prefix] reject: memchr-degenerate, rarest_freq={} > {} (narrow={})",
            //     rarest_freq_sum, TEDDY_MEMCHR_MAX_FREQ, narrow
            // );
        }
        return None;
    }
    // Combined hit rate ≈ ∏(freq_i) / TOTAL_BYTE_FREQ^num_simd.  Threshold
    // 12/256 ≈ 4.7%.
    let combined_freq: u128 = freq_sums.iter().map(|&f| f as u128).product();
    let threshold: u128 = 12 * (TOTAL_BYTE_FREQ as u128).pow(num_simd as u32) / 256;
    if combined_freq > threshold {
        // if cfg!(feature = "debug") {
        //     eprintln!("  [rev-prefix] reject: combined_freq > threshold");
        // }
        return None;
    }
    let window = &byte_sets_raw[tail_offset..tail_offset + num_simd];
    let all_sets: Vec<crate::accel::TSet> = window
        .iter()
        .map(|bytes| crate::accel::TSet::from_bytes(bytes))
        .collect();
    Some(crate::accel::RevPrefixSearch::new(
        num_simd,
        window,
        all_sets,
        tail_offset,
    ))
}

/// Runtime prefix acceleration
#[cfg_attr(debug_assertions, derive(Debug))]
pub enum PrefixKind {
    /// `calc_prefix_sets` on the rev DFA succeeded.
    ///
    /// Every match ends with this byte sequence (read right-to-left).  Bytes
    /// outside the set drive the rev DFA to dead - skipping them is safe and
    /// exact.  The `RevPrefixSearch` lives in `LDFA::prefix_skip`.
    AnchoredRev,

    /// Forward literal prefix with no `_*` stripping.
    ///
    /// Every match starts at the returned SIMD hit position - guaranteed anchor.
    /// The forward DFA confirms the match end from there.
    AnchoredFwd(crate::accel::FwdPrefixSearch),

    /// Forward `_*`-stripped potential-start prefix.
    ///
    /// Finds candidate positions that are on the shortest path to a match end.
    /// The match may start before the candidate - a leftward walk of the fwd DFA
    /// from the initial state extends the match start backwards.
    UnanchoredFwd(crate::accel::FwdPrefixSearch),

    /// Forward prefix for patterns with a leading lookbehind (e.g. `\b`, `^`).
    ///
    /// The SIMD anchor uses the first bytes of the body after stripping leading
    /// lookbehinds.  Every hit is a candidate match start.  The runtime verifies
    /// the full pattern - including the leading lookbehind - by initialising the
    /// full-pattern fwd DFA (`fwd_lb`) with the preceding byte as context.
    AnchoredFwdLb(crate::accel::FwdPrefixSearch),

    /// Reverse potential start, may have false positives.
    PotentialStart,
}

impl PrefixKind {
    /// Return `true` if this variant uses the fwd scanning path.
    #[cfg(feature = "diag")]
    pub(crate) fn is_fwd(&self) -> bool {
        matches!(
            self,
            PrefixKind::AnchoredFwd(_)
                | PrefixKind::UnanchoredFwd(_)
                | PrefixKind::AnchoredFwdLb(_)
        )
    }

    #[cfg(feature = "diag")]
    pub(crate) fn is_rev(&self) -> bool {
        matches!(self, PrefixKind::AnchoredRev | PrefixKind::PotentialStart)
    }

    pub(crate) fn fwd_search(&self) -> Option<&crate::accel::FwdPrefixSearch> {
        match self {
            PrefixKind::AnchoredFwd(s)
            | PrefixKind::UnanchoredFwd(s)
            | PrefixKind::AnchoredFwdLb(s) => Some(s),
            _ => None,
        }
    }
}

/// Try to build a rev-side `PrefixKind` from any rev-DFA node (not just
/// `ts_rev_start`). Used both by the standard prefix-selection path and by
/// the convergence-prefix path which feeds in a peeled state node.
///
/// Returns the chosen `PrefixKind` (always `AnchoredRev` or `PotentialStart`)
/// paired with a `RevPrefixSearch` for the runtime, or `None` if no usable
/// rev prefix can be extracted.
#[allow(dead_code)] // used by convergence_prefix feature; see CONVERGENCE.md S7
pub(crate) fn try_rev_prefix(
    b: &mut RegexBuilder,
    rev_node: NodeId,
) -> Result<Option<(PrefixKind, crate::accel::RevPrefixSearch)>, Error> {
    use resharp_algebra::nulls::NullsId;
    if b.get_nulls_id(rev_node) != NullsId::EMPTY {
        return Ok(None);
    }
    let anchored = calc_prefix_sets(b, rev_node)?;
    if !anchored.is_empty() {
        if let Some(s) = build_rev_prefix_search(b, &anchored) {
            return Ok(Some((PrefixKind::AnchoredRev, s)));
        }
    }
    let potential = calc_potential_start_prune(b, rev_node, 16, 64, true)?;
    if !potential.is_empty() {
        if let Some(s) = build_rev_prefix_search(b, &potential) {
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
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevPrefixSearch>), Error> {
    if !crate::simd::has_simd() {
        return Ok((None, None));
    }
    let (kind, skip) = select_prefix_simd(b, node, rev_start, has_look, min_len)?;
    // Convergence override (rev-prefix); only when no fwd prefix already chosen.
    let fwd_already = matches!(
        kind,
        Some(
            PrefixKind::AnchoredFwd(_)
                | PrefixKind::UnanchoredFwd(_)
                | PrefixKind::AnchoredFwdLb(_)
        )
    );
    #[cfg(feature = "convergence_prefix")]
    if !fwd_already {
        let mut conv_ldfa = match crate::engine::LDFA::new(b, rev_start, max_cap) {
            Ok(l) => l,
            Err(_) => return Ok((kind, skip)),
        };
        if let Some((conv_kind, conv_skip)) =
            try_convergence_prefix(b, node, &mut conv_ldfa, rev_start)?
        {
            return Ok((Some(conv_kind), Some(conv_skip)));
        }
    }
    let _ = fwd_already;
    let _ = max_cap;
    Ok((kind, skip))
}

#[cfg(feature = "convergence_prefix")]
fn try_convergence_prefix(
    b: &mut RegexBuilder,
    fwd_node: NodeId,
    rev_ldfa: &mut crate::engine::LDFA,
    rev_start: NodeId,
) -> Result<Option<(PrefixKind, crate::accel::RevPrefixSearch)>, Error> {
    const MAX_DEPTH: u32 = 12;
    let (fwd_min, _) = b.get_min_max_length(fwd_node);
    if fwd_min == 0 {
        return Ok(None);
    }
    // Try strict convergence first; fall back to relaxed.
    let attempt = |conv_node,
                   peel: u32,
                   b: &mut RegexBuilder|
     -> Result<Option<(PrefixKind, crate::accel::RevPrefixSearch)>, Error> {
        let Some((kind, search)) = try_rev_prefix(b, conv_node)? else {
            return Ok(None);
        };
        if let Some(fl) = b.get_fixed_length(fwd_node) {
            if peel as u64 + search.len() as u64 > fl as u64 {
                return Ok(None);
            }
        }
        Ok(Some((kind, search.add_tail_offset(peel))))
    };
    if let Some((conv_node, peel)) =
        crate::find_strict_convergence_node(b, rev_ldfa, rev_start, MAX_DEPTH)
    {
        if let Some(out) = attempt(conv_node, peel, b)? {
            return Ok(Some(out));
        }
    }
    Ok(None)
}

fn strip_leading_lookbehind(b: &RegexBuilder, mut node: NodeId) -> NodeId {
    use resharp_algebra::Kind;
    loop {
        if b.get_kind(node) != Kind::Concat {
            break;
        }
        if b.get_kind(node.left(b)) != Kind::Lookbehind {
            break;
        }
        node = node.right(b);
    }
    node
}

fn contains_lookahead_rel_max(b: &RegexBuilder, start: NodeId) -> bool {
    use std::collections::HashSet;
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack = vec![start];
    while let Some(n) = stack.pop() {
        if n == NodeId::MISSING || !visited.insert(n) {
            continue;
        }
        let kind = b.get_kind(n);
        if kind == Kind::Lookahead && b.get_extra(n) == u32::MAX {
            return true;
        }
        match kind {
            Kind::Pred | Kind::Begin | Kind::End => {}
            Kind::Star | Kind::Compl => {
                stack.push(n.left(b));
            }
            _ => {
                stack.push(n.left(b));
                stack.push(n.right(b));
            }
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
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevPrefixSearch>), Error> {
    use resharp_algebra::nulls::NullsId;
    // min_len==0 disables skip-by-prefix; AnchoredFwdLb still works (lb walk preserves empty matches).
    if min_len == 0 {
        if has_look && node.contains_lookbehind(b) {
            if let Some(fp) = try_build_fwd_lb(b, node)? {
                return Ok((Some(PrefixKind::AnchoredFwdLb(fp)), None));
            }
        }
        return Ok((None, None));
    }
    let sets = PrefixSets::compute(b, node, rev_start)?;

    #[cfg(feature = "debug")]
    {
        let mut all = vec![
            ("rev anc", &sets.rev_anchored.sets, sets.rev_anchored.cost),
            ("rev pot", &sets.rev_potential.sets, sets.rev_potential.cost),
            ("fwd pot", &sets.fwd_potential.sets, sets.fwd_potential.cost),
            (
                "fwd str",
                &sets.fwd_potential_stripped.sets,
                sets.fwd_potential_stripped.cost,
            ),
        ];
        all.sort_by_key(|(_, _, c)| *c);
        for (name, s, cost) in all {
            println!("  [sets] {} {:?} cost={}", name, pp_sets(b, s), cost);
        }
    }

    let fwd_cost = sets
        .fwd_potential
        .cost
        .min(sets.fwd_potential_stripped.cost);
    let rev_cost = sets.rev_anchored.cost.min(sets.rev_potential.cost);
    let rev_usable = b.get_nulls_id(rev_start) == NullsId::EMPTY
        && (!sets.rev_anchored.sets.is_empty() || !sets.rev_potential.sets.is_empty());
    let fwd_wins = fwd_cost < rev_cost;

    // Build whichever fwd candidate is possible for this pattern. A leading
    // lookbehind rules out plain fwd scanning (the DFA would start without
    // prior-byte context); instead we splice the lb bytes onto the body and
    // build AnchoredFwdLb, which walks lb_len bytes back at each candidate.
    // AnchoredFwd is unsound when the fwd pattern contains a `Lookahead(_, _,
    // u32::MAX)` state. That form is created by `attempt_rw_concat_2` when a
    // `Lookahead(la_body, MISSING, 0)` is concatenated with a center-nullable
    // tail (e.g. `.*`); the resulting state's nullability can fire at non-end-
    // of-input positions during fwd scanning, producing spurious matches.
    // Patterns with a leading lookbehind are still safe via AnchoredFwdLb. For
    // patterns with the unsound rel=MAX form, fall through to the sound
    // llmatch path (rev_collect + fwd_scan).
    let fwd_candidate = if has_look && node.contains_lookbehind(b) {
        try_build_fwd_lb(b, node)?.map(PrefixKind::AnchoredFwdLb)
    } else if has_look && contains_lookahead_rel_max(b, node) {
        None
    } else {
        let (fp, stripped) = build_fwd_prefix_from_sets(
            b,
            &sets.fwd_potential.sets,
            &sets.fwd_potential_stripped.sets,
        )?;
        match fp {
            Some(fp) if stripped => Some(PrefixKind::UnanchoredFwd(fp)),
            Some(fp) => Some(PrefixKind::AnchoredFwd(fp)),
            // strict literal fallback (e.g. `_*FOO` where potential_start is
            // too wide but the exact literal still helps).
            // strict literal fallback: use the leading fixed literal.
            None if b.is_infinite(node) => {
                build_strict_literal_prefix(b, node)?.map(PrefixKind::AnchoredFwd)
            }
            None => None,
        }
    };

    let try_rev = |b: &mut RegexBuilder| -> Option<(PrefixKind, crate::accel::RevPrefixSearch)> {
        if !rev_usable {
            return None;
        }
        // Use the pre-computed sets (same as `try_rev_prefix` would compute)
        // to avoid recomputing. Keep behavior identical to pre-refactor.
        if !sets.rev_anchored.sets.is_empty() {
            if let Some(s) = build_rev_prefix_search(b, &sets.rev_anchored.sets) {
                return Some((PrefixKind::AnchoredRev, s));
            }
        }
        if !sets.rev_potential.sets.is_empty() {
            if let Some(s) = build_rev_prefix_search(b, &sets.rev_potential.sets) {
                return Some((PrefixKind::PotentialStart, s));
            }
        }
        None
    };

    // Decision: if fwd built AND won on cost, use it. Otherwise try rev;
    // if rev also fails, fall back to whatever fwd we did build. Previous
    // revisions had a bug where the lb-strip path would return None after
    // rejecting fwd on cost and then failing to build rev.
    if fwd_wins {
        if let Some(kind) = fwd_candidate {
            return Ok((Some(kind), None));
        }
    }
    if let Some((kind, s)) = try_rev(b) {
        return Ok((Some(kind), Some(s)));
    }
    if let Some(kind) = fwd_candidate {
        return Ok((Some(kind), None));
    }
    Ok((None, None))
}

/// Build an `AnchoredFwdLb` fwd-prefix searcher for a pattern whose outermost
/// structure is `Concat(Lookbehind, body)` with a fixed lb length in 1..=4.
/// Returns `None` if any structural precondition fails or the resulting fwd
/// prefix isn't anchored (a stripped/unanchored result can't be combined with
/// the walk-back-lb-bytes trick).
fn try_build_fwd_lb(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, Error> {
    use resharp_algebra::Kind;
    let body = strip_leading_lookbehind(b, node);
    if body == node || node.right(b) != body {
        return Ok(None);
    }
    let lb = node.left(b);
    if b.get_kind(lb) != Kind::Lookbehind {
        return Ok(None);
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
    if !matches!(b.get_fixed_length(lb_stripped), Some(1..=4)) {
        return Ok(None);
    }
    // Reject when body's leading star absorbs the lb byte(s) (`^X*Y` with X ⊇ lb-bytes).
    if body_absorbs_lb(b, body, lb_stripped)? {
        #[cfg(feature = "debug")]
        eprintln!("  [fwd-lb] reject: body's leading star absorbs lb byte(s)");
        return Ok(None);
    }
    let lb_body = b.mk_concat(lb_stripped, body);
    let (fp, stripped) = build_fwd_prefix(b, lb_body)?;
    // an unanchored (_*-stripped) prefix has lost the lb bytes we just
    // spliced on; can't combine it with walk-back-lb verification.
    if stripped {
        return Ok(None);
    }
    Ok(fp)
}

/// True iff body's leading wide set is a superset of lb's last byte set.
fn body_absorbs_lb(b: &mut RegexBuilder, body: NodeId, lb: NodeId) -> Result<bool, crate::Error> {
    let body_first = calc_potential_start(b, body, 1, 64, false)?;
    let lb_first = calc_potential_start(b, lb, 1, 64, false)?;
    let (Some(&bf), Some(&lf)) = (body_first.first(), lb_first.first()) else {
        return Ok(false);
    };
    let body_bytes = b.solver().collect_bytes(bf);
    let lb_bytes = b.solver().collect_bytes(lf);
    // Body's first set must be wide (uninformative as a Teddy fingerprint)
    // AND must be a superset of the lb's byte alphabet.
    if body_bytes.len() < 64 {
        return Ok(false);
    }
    let body_set: std::collections::BTreeSet<u8> = body_bytes.iter().copied().collect();
    Ok(lb_bytes.iter().all(|b| body_set.contains(b)))
}
