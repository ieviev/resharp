use resharp_algebra::nulls::Nullability;
use resharp_algebra::solver::{Solver, TSetId};
use resharp_algebra::{NodeId, RegexBuilder, TRegex, TRegexId};
use std::collections::BTreeSet;

use crate::Error;

pub(crate) fn collect_derivative_targets(
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
        collect_derivative_targets(b, der, TSetId::FULL, &mut targets);

        // when not stripping, include self-loop byte sets in the union
        // so the full set of bytes at this position is captured
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
///
/// Returns an empty vec when no tight linear prefix exists.  When non-empty,
/// every byte NOT in a returned set drives the rev DFA to dead - the sets are
/// safe to use as a skip trigger with no false positives.
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
            collect_derivative_targets(b, der, TSetId::FULL, &mut targets);

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

/// All candidate prefix-set sequences for a pattern.
///
/// Computed once at pattern-compile time by [`PrefixSets::compute`].
/// [`select_prefix_simd`] uses [`PrefixSets::rarity`] to compare candidates
/// and pick the best SIMD anchor.
#[allow(dead_code)]
pub struct PrefixSets {
    /// Tight anchored fwd prefix.  Every match starts exactly at a SIMD hit.
    pub fwd_anchored: Vec<TSetId>,
    /// Potential-start fwd sets (full node, self-loop bytes included).
    pub fwd_potential: Vec<TSetId>,
    /// Potential-start fwd sets after stripping a leading `_*`.
    pub fwd_potential_stripped: Vec<TSetId>,
    /// Tight anchored rev prefix.  Every match ends with this byte sequence
    /// (read right-to-left).
    pub rev_anchored: Vec<TSetId>,
    /// Potential-start rev sets.
    pub rev_potential: Vec<TSetId>,
}

impl PrefixSets {
    /// Compute all prefix-set sequences for `node` (fwd) and `rev_start`
    /// (already reversed, not yet stripped).
    pub fn compute(
        b: &mut RegexBuilder,
        node: NodeId,
        rev_start: NodeId,
    ) -> Result<Self, crate::Error> {
        let stripped_node = b.strip_prefix_safe(node);

        let fwd_anchored = {
            let n = b.prune_begin(node);
            let n = b.strip_prefix_safe(n);
            calc_prefix_sets(b, n)?
        };
        let fwd_potential = calc_potential_start(b, node, 16, 64, false)?;
        let fwd_potential_stripped = calc_potential_start(b, stripped_node, 16, 64, false)?;
        let rev_anchored = calc_prefix_sets(b, rev_start)?;
        let mut rev_potential = calc_potential_start_prune(b, rev_start, 16, 64, true)?;
        if rev_potential.is_empty() {
            if let Ok(body) = b.strip_lb(node) {
                if body != node {
                    if let Ok(body_rev) = b.reverse(body) {
                        if let Ok(bare) = b.strip_lb(body_rev) {
                            rev_potential = calc_potential_start(b, bare, 16, 64, false)?;
                        }
                    }
                }
            }
        }

        Ok(Self {
            fwd_anchored,
            fwd_potential,
            fwd_potential_stripped,
            rev_anchored,
            rev_potential,
        })
    }

    /// Lower is rarer and more profitable for SIMD skip. `u64::MAX` for an empty sequence.
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    pub fn rarity(b: &mut RegexBuilder, sets: &[TSetId]) -> u64 {
        rarest_freq(b, sets)
    }

    /// Estimated cost of scanning with this prefix (lower = faster)
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
    pub fn scan_cost(b: &mut RegexBuilder, sets: &[TSetId], is_rev: bool) -> u64 {
        if sets.is_empty() {
            return u64::MAX;
        }
        let byte_sets: Vec<Vec<u8>> = sets.iter().map(|&s| b.solver().collect_bytes(s)).collect();
        let freqs: Vec<u64> = byte_sets
            .iter()
            .map(|bs| {
                bs.iter()
                    .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u64)
                    .sum()
            })
            .collect();
        let num_simd = freqs.len().min(3);
        if num_simd == 0 {
            return u64::MAX;
        }
        let total = TOTAL_BYTE_FREQ as f64;

        if !is_rev {
            let lit_len = byte_sets.iter().take_while(|bs| bs.len() == 1).count();
            if lit_len >= 3 {
                let rare_f = byte_sets[..lit_len]
                    .iter()
                    .map(|bs| crate::simd::BYTE_FREQ[bs[0] as usize] as u64)
                    .min()
                    .unwrap() as f64;
                if lit_len == byte_sets.len() || rare_f < RARE_BYTE_FREQ_LIMIT as f64 {
                    let cost = 0.1_f64 + (rare_f / total) * 20.0;
                    return (cost * 1e9) as u64;
                }
            }
        }

        // teddy path: pick the window this backend actually uses
        let max_off = if is_rev { freqs.len() - num_simd } else { 0 };
        let mut best_prod = f64::INFINITY;
        for off in 0..=max_off {
            let p: f64 = freqs[off..off + num_simd]
                .iter()
                .map(|&f| f as f64)
                .product();
            if p < best_prod {
                best_prod = p;
            }
        }
        let fire = best_prod / total.powi(num_simd as i32);
        // teddy kernel: ~3 loads + shuffles per 32 B
        let cost = 0.3_f64 + fire * 200.0;
        (cost * 1e9) as u64
    }
}

const SKIP_FREQ_THRESHOLD: u32 = 75_000;
const TEDDY_MAX_FREQ_SUM: u64 = 25_000;
// sum of BYTE_FREQ[0..256] in the corpus
const TOTAL_BYTE_FREQ: u64 = 252_052;
// contributes no meaningful filtering (essentially a wildcard).
const TEDDY_WEAK_POSITION_FREQ: u64 = 100_000;
// when to use memchr instead of a full prefix
const TEDDY_MEMCHR_MAX_FREQ: u64 = 2_500;
const TEDDY_MEMCHR_MAX_FREQ_F: u64 = 1_500;
const RARE_BYTE_FREQ_LIMIT: u16 = 25_000;

pub(crate) fn skip_is_profitable(bytes: &[u8]) -> bool {
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

/// Forward literal prefix for patterns with no `_*` stripping.
/// Returns `Some` only when the pattern has a tight literal prefix and the
/// rarest byte in it is not too common.
pub fn build_strict_literal_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    #[cfg(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    ))]
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
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        all(target_arch = "wasm32", target_feature = "simd128")
    )))]
    {
        let _ = (b, node);
        Ok(None)
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

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
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
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn try_build_fwd_search_raw(
    byte_sets_raw: &[Vec<u8>],
) -> Result<Option<crate::accel::FwdPrefixSearch>, crate::Error> {
    let lit_len = byte_sets_raw.iter().take_while(|bs| bs.len() == 1).count();
    if cfg!(feature = "debug") {
        eprintln!(
            "  [fwd-prefix] lit_len={} total={} sets={:?}",
            lit_len,
            byte_sets_raw.len(),
            byte_sets_raw
                .iter()
                .map(|bs| if bs.len() <= 4 {
                    format!("{:?}", bs)
                } else {
                    format!("[{}b]", bs.len())
                })
                .collect::<Vec<_>>()
        );
    }
    if lit_len >= 3 {
        let needle: Vec<u8> = byte_sets_raw[..lit_len].iter().map(|bs| bs[0]).collect();
        let lit = crate::simd::FwdLiteralSearch::new(&needle);
        if cfg!(feature = "debug") {
            let freq = crate::simd::BYTE_FREQ[lit.rare_byte() as usize];
            eprintln!(
                "  [fwd-prefix] literal {:?} rare={} freq={}",
                std::str::from_utf8(&needle).unwrap_or("?"),
                lit.rare_byte() as char,
                freq
            );
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
        for &(i, f) in &freqs {
            eprintln!(
                "  [fwd-prefix] pos={} bytes={} freq={}",
                i,
                byte_sets_raw[i].len(),
                f
            );
        }
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

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
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

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn build_fwd_prefix_from_sets(
    b: &mut RegexBuilder,
    full_sets: &[TSetId],
    stripped_sets: &[TSetId],
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    // Prefer stripped when it is meaningfully rarer (≥4× rarity advantage).
    let full_rarity = PrefixSets::rarity(b, full_sets);
    let stripped_rarity = PrefixSets::rarity(b, stripped_sets);
    if !stripped_sets.is_empty() && (full_sets.is_empty() || stripped_rarity * 4 < full_rarity) {
        if let Some(fp) = try_build_fwd_search(b, stripped_sets)? {
            return Ok((Some(fp), true));
        }
    }

    if !full_sets.is_empty() {
        if let Some(fp) = try_build_fwd_search(b, full_sets)? {
            return Ok((Some(fp), false));
        }
    }
    if !stripped_sets.is_empty() {
        if let Some(fp) = try_build_fwd_search(b, stripped_sets)? {
            return Ok((Some(fp), true));
        }
    }

    Ok((None, false))
}

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
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

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn try_build_fwd_range_prefix(
    byte_sets_raw: &[Vec<u8>],
    anchor_pos: usize,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    let anchor_bytes = &byte_sets_raw[anchor_pos];
    let freq_sum: u32 = anchor_bytes
        .iter()
        .map(|&b| crate::simd::BYTE_FREQ[b as usize] as u32)
        .sum();
    if freq_sum >= SKIP_FREQ_THRESHOLD {
        if cfg!(feature = "debug") {
            eprintln!(
                "  [fwd-prefix-range] reject: {} bytes, freq_sum={} >= {}",
                anchor_bytes.len(),
                freq_sum,
                SKIP_FREQ_THRESHOLD
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

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
fn build_fwd_prefix_simd(
    _b: &mut RegexBuilder,
    _node: NodeId,
) -> Result<(Option<crate::accel::FwdPrefixSearch>, bool), crate::Error> {
    Ok((None, false))
}

/// Build a `RevPrefixSearch` from byte sets, or return `None` if the sets are
/// too wide to be useful.  `len >= 2` required (single-byte case is handled by
/// the DFA skip system).
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
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
        eprintln!(
            "  [rev-prefix] total={} sets={:?}",
            byte_sets_raw.len(),
            byte_sets_raw
                .iter()
                .map(|bs| if bs.len() <= 4 {
                    format!("{:?}", bs)
                } else {
                    format!("[{}b]", bs.len())
                })
                .collect::<Vec<_>>()
        );
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
        eprintln!(
            "  [rev-prefix] tail_offset={} window_freqs={:?}",
            tail_offset, freq_sums
        );
    }
    let rarest_freq_sum = *freq_sums.iter().min().unwrap_or(&u64::MAX);
    if rarest_freq_sum > TEDDY_MAX_FREQ_SUM {
        if cfg!(feature = "debug") {
            eprintln!("  [rev-prefix] reject: max sum={}", rarest_freq_sum,);
        }
        return None;
    }
    let narrow = freq_sums
        .iter()
        .filter(|&&f| f <= TEDDY_WEAK_POSITION_FREQ)
        .count();
    if narrow < 2 && rarest_freq_sum > TEDDY_MEMCHR_MAX_FREQ {
        if cfg!(feature = "debug") {
            eprintln!(
                "  [rev-prefix] reject: memchr-degenerate, rarest_freq={} > {} (narrow={})",
                rarest_freq_sum, TEDDY_MEMCHR_MAX_FREQ, narrow
            );
        }
        return None;
    }
    // Combined hit rate ≈ ∏(freq_i) / TOTAL_BYTE_FREQ^num_simd.  Threshold
    // 12/256 ≈ 4.7%.
    let combined_freq: u128 = freq_sums.iter().map(|&f| f as u128).product();
    let threshold: u128 = 12 * (TOTAL_BYTE_FREQ as u128).pow(num_simd as u32) / 256;
    if cfg!(feature = "debug") {
        eprintln!(
            "  [rev-prefix] freq_sums={:?} combined={} threshold={}",
            freq_sums, combined_freq, threshold
        );
    }
    if combined_freq > threshold {
        if cfg!(feature = "debug") {
            eprintln!("  [rev-prefix] reject: combined_freq > threshold");
        }
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
    #[allow(dead_code)]
    AnchoredFwdLb(crate::accel::FwdPrefixSearch),

    /// `calc_potential_start` prefix - Teddy-accelerated, may have false positives.
    ///
    /// The rev DFA walk after each candidate position must verify nullability.
    /// Positions where the DFA does not become nullable are silently skipped.
    /// The `RevPrefixSearch` lives in `LDFA::prefix_skip`.
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

/// Select the best prefix acceleration for a compiled pattern.
pub(crate) fn select_prefix(
    b: &mut RegexBuilder,
    node: NodeId,
    rev_start: NodeId,
    has_look: bool,
    min_len: u32,
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevPrefixSearch>), Error> {
    if !crate::simd::has_simd() {
        return Ok((None, None));
    }
    select_prefix_simd(b, node, rev_start, has_look, min_len)
}

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
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

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
))]
fn select_prefix_simd(
    b: &mut RegexBuilder,
    node: NodeId,
    rev_start: NodeId,
    has_look: bool,
    min_len: u32,
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevPrefixSearch>), Error> {
    use resharp_algebra::nulls::NullsId;
    if min_len == 0 {
        return Ok((None, None));
    }
    let sets = PrefixSets::compute(b, node, rev_start)?;

    // lower cost wins
    let fwd_cost = PrefixSets::scan_cost(b, &sets.fwd_potential, false).min(PrefixSets::scan_cost(
        b,
        &sets.fwd_potential_stripped,
        false,
    ));
    let rev_cost = PrefixSets::scan_cost(b, &sets.rev_anchored, true).min(PrefixSets::scan_cost(
        b,
        &sets.rev_potential,
        true,
    ));
    let rev_usable = b.get_nulls_id(rev_start) == NullsId::EMPTY
        && (!sets.rev_anchored.is_empty() || !sets.rev_potential.is_empty());
    let fwd_better = fwd_cost < rev_cost;
    if cfg!(feature = "debug") {
        eprintln!(
            "  [prefix-select] fwd_cost={} rev_cost={} rev_usable={} fwd_better={}",
            fwd_cost, rev_cost, rev_usable, fwd_better
        );
    }

    if has_look {
        let body = strip_leading_lookbehind(b, node);
        if body != node && node.right(b) == body {
            use resharp_algebra::Kind;
            let lb = node.left(b);
            if b.get_kind(lb) == Kind::Lookbehind {
                let lb_inner = b.get_lookbehind_inner(lb);
                let lb_nonbegin = b.nonbegins(lb_inner);
                let mut lb_stripped = lb_nonbegin;
                loop {
                    let after_strip = b.strip_prefix_safe(lb_stripped);
                    let after_nb = b.nonbegins(after_strip);
                    if after_nb == lb_stripped {
                        break;
                    }
                    lb_stripped = after_nb;
                }
                let lb_fixed = b.get_fixed_length(lb_stripped);
                if matches!(lb_fixed, Some(1..=4)) {
                    let lb_body = b.mk_concat(lb_stripped, body);
                    let (fp, stripped) = build_fwd_prefix(b, lb_body)?;
                    if let (Some(fp), false) = (fp, stripped) {
                        let _ = fp;
                        if !(rev_usable && !fwd_better) {
                            return Ok((Some(PrefixKind::AnchoredFwdLb(fp)), None));
                        }
                    }
                }
            }
        }
    }

    // try to build a rev prefix searcher. returns None if rev isn't
    // usable or neither set yields a searcher.
    let try_rev = |b: &mut RegexBuilder| -> Option<(PrefixKind, crate::accel::RevPrefixSearch)> {
        if !rev_usable {
            return None;
        }
        if !sets.rev_anchored.is_empty() {
            if let Some(s) = build_rev_prefix_search(b, &sets.rev_anchored) {
                return Some((PrefixKind::AnchoredRev, s));
            }
        }
        if !sets.rev_potential.is_empty() {
            if let Some(s) = build_rev_prefix_search(b, &sets.rev_potential) {
                return Some((PrefixKind::PotentialStart, s));
            }
        }
        None
    };

    if !has_look || !node.contains_lookbehind(b) {
        let (fp, stripped) =
            build_fwd_prefix_from_sets(b, &sets.fwd_potential, &sets.fwd_potential_stripped)?;

        if let Some(fp) = fp {
            if !fwd_better {
                if let Some((k, s)) = try_rev(b) {
                    return Ok((Some(k), Some(s)));
                }
            }
            let kind = if stripped {
                PrefixKind::UnanchoredFwd(fp)
            } else {
                PrefixKind::AnchoredFwd(fp)
            };
            return Ok((Some(kind), None));
        }
        // strict literal fallback (no _* stripping, exact literal)
        if b.is_infinite(node) {
            if let Some(fp) = build_strict_literal_prefix(b, node)? {
                return Ok((Some(PrefixKind::AnchoredFwd(fp)), None));
            }
        }
    }

    if let Some((k, s)) = try_rev(b) {
        return Ok((Some(k), Some(s)));
    }
    Ok((None, None))
}

#[cfg(not(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
fn select_prefix_simd(
    _b: &mut RegexBuilder,
    _node: NodeId,
    _rev_start: NodeId,
    _has_look: bool,
    _min_len: u32,
) -> Result<(Option<PrefixKind>, Option<crate::accel::RevPrefixSearch>), Error> {
    Ok((None, None))
}
