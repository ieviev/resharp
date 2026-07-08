//! regex engine with intersection, complement, and lookarounds
//!
//! # quick start
//!
//! ```
//! let re = resharp::Regex::new(r"\d{3}-\d{4}").unwrap();
//! let matches = re.find_all(b"call 555-1234 or 555-5678").unwrap();
//! assert_eq!(matches.len(), 2);
//! ```
//!
//! # options
//!
//! use [`RegexOptions`] with [`Regex::with_options`] for non-default settings:
//!
//! ```
//! use resharp::{Regex, RegexOptions};
//!
//! let re = Regex::with_options(
//!     r"hello world",
//!     RegexOptions::default()
//!         .case_insensitive(true)
//!         .dot_matches_new_line(true),
//! ).unwrap();
//! assert!(re.is_match(b"Hello World").unwrap());
//! ```
//!
//! # escaping user input
//!
//! use [`escape`] to safely embed literal strings in patterns:
//!
//! ```
//! let user_input = "file (1).txt";
//! let pattern = format!(r"^{}$", resharp::escape(user_input));
//! let re = resharp::Regex::new(&pattern).unwrap();
//! assert!(re.is_match(b"file (1).txt").unwrap());
//! ```

#![deny(missing_docs)]

#[cfg(not(any(
    target_arch = "x86_64",
    all(target_arch = "aarch64", target_feature = "neon"),
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
compile_error!(
    "resharp requires a SIMD-capable target: x86_64, aarch64 with target_feature=neon, or wasm32 with target_feature=simd128"
);

pub(crate) mod accel;
pub(crate) mod bdfa;
pub(crate) mod ldfa;
pub(crate) mod fas;
pub(crate) mod minterms;
pub(crate) mod fwd;
pub(crate) mod ismatch;
pub(crate) mod prefix;
pub(crate) mod scan;

#[cfg(feature = "stream")]
pub(crate) mod stream;
#[cfg(feature = "stream")]
pub use stream::StreamState;

#[cfg(feature = "serialize")]
pub mod dump;
#[cfg(feature = "serialize")]
#[allow(missing_docs)]
pub use dump::RegexDump;
#[cfg(feature = "serialize")]
#[allow(missing_docs)]
pub use bdfa::BDFA;
#[cfg(feature = "serialize")]
#[allow(missing_docs)]
pub use ldfa::LDFA;
#[cfg(feature = "serialize")]
#[allow(missing_docs)]
pub use prefix::{NegLb, NegLbTerm, PrefixKind};

pub(crate) mod simd;

#[cfg(feature = "diag")]
pub use prefix::calc_potential_start;
#[cfg(feature = "diag")]
pub use prefix::calc_potential_start_prune;
#[cfg(feature = "diag")]
pub use prefix::calc_prefix_sets;
#[cfg(feature = "diag")]
pub use prefix::PrefixSets;
#[cfg(feature = "diag")]
pub use simd::{force_scalar_scope, ForceScalarGuard};
pub(crate) use resharp_algebra::nulls::{Nullability, StartPositions};
pub(crate) use resharp_algebra::solver::TSetId;
use resharp_algebra::Kind;
#[doc(hidden)]
pub use resharp_algebra::NodeId;
#[doc(hidden)]
pub use resharp_algebra::RegexBuilder;

/// escape all resharp meta characters in `text`, returning a pattern
/// that matches the literal string.
///
/// ```
/// assert_eq!(resharp::escape("a+b"), r"a\+b");
/// ```
pub use resharp_parser::escape;
/// like [`escape`] but appends to an existing buffer.
pub use resharp_parser::escape_into;

use std::sync::Mutex;

/// error from compiling or matching a regex.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// parse failure.
    Parse(Box<resharp_parser::ParseError>),
    /// algebra error (unsupported pattern, anchor limit).
    Algebra(resharp_algebra::ResharpError),
    /// DFA state cache exceeded `max_dfa_capacity`.
    CapacityExceeded,
    /// pattern produced more algebra nodes than the engine supports.
    PatternTooLarge,
    /// serialization or deserialization failure.
    Serialize(String),
    /// internal invariant violated; indicates a bug in the engine.
    InternalError(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "parse error: {}", e),
            Error::Algebra(e) => write!(f, "{}", e),
            Error::CapacityExceeded => write!(f, "DFA state capacity exceeded"),
            Error::PatternTooLarge => write!(f, "pattern too large"),
            Error::Serialize(ref s) => write!(f, "serialization error: {}", s),
            Error::InternalError(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Parse(e) => Some(e),
            Error::Algebra(e) => Some(e),
            Error::CapacityExceeded => None,
            Error::PatternTooLarge => None,
            Error::Serialize(_) => None,
            Error::InternalError(_) => None,
        }
    }
}

impl From<resharp_parser::ParseError> for Error {
    fn from(e: resharp_parser::ParseError) -> Self {
        Error::Parse(Box::new(e))
    }
}

impl From<resharp_algebra::ResharpError> for Error {
    fn from(e: resharp_algebra::ResharpError) -> Self {
        Error::Algebra(e)
    }
}

/// configuration for pattern compilation and engine behavior.
///
/// all options have sensible defaults via [`Default`]. use the builder
/// methods to override:
///
/// ```
/// use resharp::RegexOptions;
///
/// let opts = RegexOptions::default()
///     .unicode(false)           // ASCII-only \w, \d, \s
///     .case_insensitive(true)   // global (?i)
///     .dot_matches_new_line(true); // . matches \n
/// ```
/// Coverage of `\w`/`\d`/`\s` and the width of `.` / negated classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnicodeMode {
    /// ASCII `\w`/`\d`/`\s`; `.` and negated classes step byte-by-byte. Fastest.
    Ascii,
    /// `\w` covers scripts up to 2-byte UTF-8 (Latin, Greek, Cyrillic, Hebrew, Arabic, ...);
    /// `\d` and `\s` are ASCII.
    #[default]
    Default,
    /// Full Unicode `\w`/`\d`/`\s` (incl. CJK and historic scripts, up to 4-byte UTF-8);
    /// `.` and negated classes match one full codepoint.
    Full,
    /// ASCII `\w`/`\d`/`\s`, but `.`, `[^...]`, `\W`/`\D`/`\S` match one full codepoint.
    /// Matches default JS `RegExp` behavior (no `u` flag).
    Javascript,
}

/// Regex configuration, passed to [`Regex::with_options`].
pub struct RegexOptions {
    /// max cached DFA states, clamped to `u16::MAX` (default: `u16::MAX`).
    pub max_dfa_capacity: usize,
    /// max lookahead context distance (default: 800).
    pub lookahead_context_max: u32,
    /// Unicode coverage for `\w`/`\d`/`\s` and width of `.` / negated classes
    /// (default: `UnicodeMode::Default`).
    pub unicode: UnicodeMode,
    /// global case-insensitive matching (default: false).
    pub case_insensitive: bool,
    /// `.` matches `\n` (default: false). `_` always matches any byte.
    pub dot_matches_new_line: bool,
    /// `^` and `$` match at line boundaries (`\n`) in addition to text
    /// boundaries (default: true). Disable with `(?-m)` inline or this flag.
    pub multiline: bool,
    /// allow whitespace and `#` comments in the pattern (default: false).
    pub ignore_whitespace: bool,
    /// use hardened forward scan (default: false).
    /// slower, but prevents O(n^2) all-matches blowup on adversarial combinations.
    pub hardened: bool,
    /// remove the default pattern size limit for very large regexes (default: false).
    pub unbounded_size: bool,
    #[doc(hidden)]
    pub force_convergence: bool,
    /// disable all prefix-acceleration. much slower,
    /// but can be useful for sanity checks and serialization
    #[doc(hidden)]
    pub disable_prefixes: bool,
}

impl Default for RegexOptions {
    fn default() -> Self {
        Self {
            max_dfa_capacity: u16::MAX as usize,
            lookahead_context_max: 800,
            unicode: UnicodeMode::Default,
            case_insensitive: false,
            dot_matches_new_line: false,
            multiline: true,
            ignore_whitespace: false,
            hardened: false,
            unbounded_size: false,
            force_convergence: false,
            disable_prefixes: false,
        }
    }
}

impl RegexOptions {
    /// set Unicode coverage for `\w`/`\d`/`\s` and width of `.` / negated classes.
    pub fn unicode(mut self, mode: UnicodeMode) -> Self {
        self.unicode = mode;
        self
    }
    /// set case-insensitive mode.
    pub fn case_insensitive(mut self, yes: bool) -> Self {
        self.case_insensitive = yes;
        self
    }
    /// set dot-matches-newline mode.
    pub fn dot_matches_new_line(mut self, yes: bool) -> Self {
        self.dot_matches_new_line = yes;
        self
    }
    /// `^`/`$` match at `\n` (default: true), set false to make `^`/`$` same as `\A`/`\z`.
    pub fn multiline(mut self, yes: bool) -> Self {
        self.multiline = yes;
        self
    }
    /// set ignore-whitespace (verbose) mode.
    pub fn ignore_whitespace(mut self, yes: bool) -> Self {
        self.ignore_whitespace = yes;
        self
    }
    /// enable hardened mode for untrusted patterns: uses only O(N·S) forward scan (~5-20x constant overhead).
    pub fn hardened(mut self, yes: bool) -> Self {
        self.hardened = yes;
        self
    }
    #[doc(hidden)]
    pub fn force_convergence(mut self, yes: bool) -> Self {
        self.force_convergence = yes;
        self
    }
    /// disable parser and algebra size caps.
    /// the defaults are generous; if you hit them, splitting the pattern into
    /// several smaller regexes is almost always the better fix than raising the limit.
    pub fn unbounded_size(mut self, yes: bool) -> Self {
        self.unbounded_size = yes;
        self
    }
}

/// byte-offset range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Match {
    /// inclusive start.
    pub start: usize,
    /// exclusive end.
    pub end: usize,
}

pub(crate) struct RegexInner {
    pub(crate) b: RegexBuilder,
    pub(crate) fwd: ldfa::LDFA,
    pub(crate) fwd_ts: ldfa::LDFA,
    #[cfg_attr(not(feature = "stream"), allow(dead_code))]
    pub(crate) rev: Option<ldfa::LDFA>,
    pub(crate) rev_ts: ldfa::LDFA,
    #[cfg(feature = "convergence_prefix")]
    pub(crate) conv_b: Option<ldfa::LDFA>,
    #[cfg(feature = "stream")]
    pub(crate) stream: stream::StreamInit,
    pub(crate) nulls: StartPositions,
    pub(crate) matches: Vec<Match>,
    pub(crate) bounded: Option<bdfa::BDFA>,
    pub(crate) fas: Option<fas::FwdDFA>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct InitialNodeFlags(u8);

impl InitialNodeFlags {
    const HAS_ANCHORS: u8 = 1 << 0;
    const HAS_LB: u8 = 1 << 1;
    const HAS_LA: u8 = 1 << 2;

    pub(crate) fn new(has_anchors: bool, has_lb: bool, has_la: bool) -> Self {
        let mut bits = 0;
        if has_anchors {
            bits |= Self::HAS_ANCHORS;
        }
        if has_lb {
            bits |= Self::HAS_LB;
        }
        if has_la {
            bits |= Self::HAS_LA;
        }
        InitialNodeFlags(bits)
    }

    #[inline]
    pub(crate) fn has_anchors(self) -> bool {
        self.0 & Self::HAS_ANCHORS != 0
    }
    
    #[inline]
    #[allow(dead_code)]
    pub(crate) fn has_lb(self) -> bool {
        self.0 & Self::HAS_LB != 0
    }
    #[inline]
    pub(crate) fn has_la(self) -> bool {
        self.0 & Self::HAS_LA != 0
    }
}

/// Lazily compiled regex instance.
/// Uses Mutex for interior mutability.
pub struct Regex {
    pub(crate) inner: Mutex<RegexInner>,
    pub(crate) prefix: Option<prefix::PrefixKind>,
    pub(crate) fixed_length: Option<u32>,
    pub(crate) empty_nullable: bool,
    pub(crate) always_nullable: bool,
    pub(crate) star_loop: bool,
    /// node = ⊥
    /// found to be trivially unmatchable, not guaranteed before full expansion
    pub(crate) is_empty_lang: bool,
    #[allow(dead_code)]
    pub(crate) fwd_begin_anchored: bool,
    pub(crate) fwd_lb_stripped: bool,
    #[allow(dead_code)]
    pub(crate) rev_end_anchored: bool,
    /// rev = _*, skip rev pass entirely
    pub(crate) initial_nullability: Nullability,
    pub(crate) fwd_end_nullable: bool,
    // unfinished experimental optimizations, will not put these in yet
    // `Y·_*` shape: at most one match. skip rev+fwd.
    // pub(crate) trailing_star_anchored_left: bool,
    // pub(crate) trailing_star_branch_left: bool,
    pub(crate) hardened: bool,
    #[allow(dead_code)]
    pub(crate) has_bounded: bool,
    pub(crate) bounded_safe_find_all: bool,
    pub(crate) lb_check_bytes: u8,
    pub(crate) fwd_lb_begin_nullable: bool,
    pub(crate) fwd_lb_body_nullable: bool,
    pub(crate) init_flags: InitialNodeFlags,
    #[cfg(feature = "convergence_prefix")]
    pub(crate) conv_prefix: bool,
    /// fixed-length negative lookbehind reject test paired with an `AnchoredFwd`
    /// body-literal prefix; verifies the lookbehind the SIMD prefix cannot.
    pub(crate) neg_lb: Option<prefix::NegLb>,
    pub(crate) find_all: FindAll,
    pub(crate) class_plus: Option<[u64; 4]>,
    #[cfg(feature = "stream")]
    pub(crate) stream_cache: stream::StreamCache,
}

/// cached dispatch decision for `find_all`. computed once at construction.
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FindAll {
    /// language is ⊥; never matches.
    EmptyLang,
    /// `\A`-anchored; single forward scan from byte 0.
    Anchored,
    /// `\z`-anchored; single reverse scan from the string end.
    EndAnchored,
    /// O(N·S) bulk scan for untrusted patterns.
    Hardened,
    /// SIMD forward-prefix anchored scan.
    FwdPrefix,
    /// SIMD forward-prefix with leading lookbehind.
    FwdLbPrefix,
    /// bounded-DFA fwd-only scan.
    Bounded,
    /// generic rev-collect + fwd-verify.
    Dfa,
    /// language is `CLASS+` for one frequent byte class; emit maximal runs.
    ClassPlus,
}

// not a security measure. only flags obvious cases where hardening results in better performance
#[derive(Clone, Copy, Default)]
struct Hardening {
    full: bool,
    no_fwd_prefix: bool,
}

/// auto-hardening heuristic, flags some patterns for hardened forward scan.
fn auto_harden(b: &mut RegexBuilder, start: NodeId, has_anchors: bool) -> Hardening {
    const NODE_BUDGET: usize = 128;
    const LARGE_COVER: u32 = 128;
    let opener = opener_class(b, start);
    if opener == TSetId::EMPTY {
        return Hardening::default();
    }
    let opener_full = b.solver().is_full_id(opener);
    let Some(graph) = build_partial_graph(b, start, NODE_BUDGET) else {
        return Hardening::default();
    };
    if graph
        .nodes
        .iter()
        .any(|&n| n.is_compl(b))
    {
        return Hardening::default();
    }
    let mut pure_star: Vec<bool> = vec![false; graph.nodes.len()];
    for (i, &n) in graph.nodes.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if n.nullability(b) != resharp_algebra::nulls::Nullability::ALWAYS {
            continue;
        }
        if graph.edges[i].len() == 1 {
            let e = &graph.edges[i][0];
            if e.dst == i && b.solver().is_full_id(e.set) {
                pure_star[i] = true;
            }
        }
    }
    if !has_anchors
        && graph.edges[0].len() == 1
        && graph.edges[0][0].dst == 0
        && b.solver().is_full_id(graph.edges[0][0].set)
    {
        return Hardening::default();
    }

    let reach = transitive_closure(&graph);
    let sccs = sccs_from_reach(&reach);
    let mut node_scc: Vec<usize> = vec![0; graph.nodes.len()];
    for (sid, scc) in sccs.iter().enumerate() {
        for &n in scc {
            node_scc[n] = sid;
        }
    }
    let start_in_cycle = sccs[node_scc[0]].len() > 1 || graph.edges[0].iter().any(|e| e.dst == 0);
    let total_wide_self_loops = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !pure_star[*i])
        .filter(|(i, _)| {
            let self_cov = graph.edges[*i]
                .iter()
                .filter(|e| e.dst == *i)
                .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
            b.solver().byte_count(self_cov) >= 2
        })
        .count();
    let (min_len, _) = b.get_min_max_length(start);
    const SHORT_PREFIX: u32 = 3;
    const ENTRY_BYTES: u32 = 2;
    for (i, &n) in graph.nodes.iter().enumerate() {
        if n.nullability(b) == resharp_algebra::nulls::Nullability::NEVER {
            continue;
        }
        let scc = &sccs[node_scc[i]];
        let scc_non_trivial = scc.len() > 1 || graph.edges[i].iter().any(|e| e.dst == i);
        if !scc_non_trivial {
            continue;
        }
        let scc_set: std::collections::HashSet<usize> = scc.iter().copied().collect();
        let in_scc_cov = graph.edges[i]
            .iter()
            .filter(|e| scc_set.contains(&e.dst))
            .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
        if b.solver().byte_count(in_scc_cov) < LARGE_COVER {
            continue;
        }
        if i == 0 {
            return Hardening {
                full: true,
                no_fwd_prefix: true,
            };
        }
        let start_to_i = graph.edges[0]
            .iter()
            .filter(|e| e.dst == i)
            .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
        let entry_wide = b.solver().byte_count(start_to_i) >= ENTRY_BYTES;
        let start_is_union = start.is_union(b);
        if !has_anchors && min_len <= SHORT_PREFIX && entry_wide && start_is_union {
            return Hardening {
                full: true,
                no_fwd_prefix: true,
            };
        }
    }
    let mut no_fwd_prefix = false;
    let opener_wide = opener_full || b.solver().byte_count(opener) >= LARGE_COVER;
    for scc in sccs {
        let non_trivial = scc.len() > 1 || graph.edges[scc[0]].iter().any(|e| e.dst == scc[0]);
        if !non_trivial {
            continue;
        }
        if scc.iter().all(|&n| pure_star[n]) {
            continue;
        }
        let scc_set: std::collections::HashSet<usize> = scc.iter().copied().collect();
        if scc_set.contains(&0) {
            continue; // (3b) start in SCC
        }
        let sticky = scc.iter().all(|&n| {
            let cover = graph.edges[n]
                .iter()
                .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
            b.solver().is_full_id(cover)
        });
        const SPIN_FREQ_THRESHOLD: u64 = crate::prefix::TOTAL_BYTE_FREQ / 2;
        let scc_set_local: std::collections::HashSet<usize> = scc.iter().copied().collect();
        let has_wide_spin = scc.iter().any(|&n| {
            let in_scc_cover = graph.edges[n]
                .iter()
                .filter(|e| scc_set_local.contains(&e.dst))
                .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
            let freq: u64 = b
                .solver()
                .collect_bytes(in_scc_cover)
                .iter()
                .map(|&byte| crate::simd::BYTE_FREQ[byte as usize] as u64)
                .sum();
            freq >= SPIN_FREQ_THRESHOLD
        });
        if !has_wide_spin {
            continue;
        }
        let restartable = scc.iter().any(|&n| {
            graph.edges[n]
                .iter()
                .any(|e| scc_set.contains(&e.dst) && b.solver().is_sat_id(e.set, opener))
        });
        if !restartable {
            continue;
        }
        if !has_anchors {
            no_fwd_prefix = true;
        }
        let start_branches = graph.edges[0].len() >= 2;
        let scc_branches = scc.iter().any(|&n| graph.edges[n].len() >= 3);
        if !start_branches && total_wide_self_loops <= 1 {
            continue;
        }
        let start_escapes_scc = if has_anchors {
            let start_into_scc = graph.edges[0]
                .iter()
                .filter(|e| scc_set.contains(&e.dst))
                .count();
            graph.edges[0].len() > start_into_scc
        } else {
            let cover = graph.edges[0]
                .iter()
                .filter(|e| scc_set.contains(&e.dst) || scc.iter().any(|&s| reach[e.dst][s]))
                .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
            !b.solver().is_full_id(cover)
        };
        if start_escapes_scc && !start_in_cycle {
            continue;
        }
        if min_len <= SHORT_PREFIX && sticky && opener_wide && (start_branches || scc_branches) {
            return Hardening {
                full: true,
                no_fwd_prefix: true,
            };
        }
    }
    if no_fwd_prefix {
        return Hardening {
            full: false,
            no_fwd_prefix: true,
        };
    }
    Hardening::default()
}

struct Edge {
    dst: usize,
    set: TSetId,
}

struct Graph {
    edges: Vec<Vec<Edge>>,
    nodes: Vec<NodeId>,
}

const BUILD_PARTIAL_GRAPH_CREATION_BUDGET: u32 = 100_000;

fn build_partial_graph(b: &mut RegexBuilder, start: NodeId, budget: usize) -> Option<Graph> {
    use std::collections::HashMap;
    let mut idx: HashMap<NodeId, usize> = HashMap::from([(start, 0)]);
    let mut edges: Vec<Vec<Edge>> = vec![Vec::new()];
    let mut nodes: Vec<NodeId> = vec![start];
    let mut queue: Vec<(usize, NodeId)> = vec![(0, start)];
    let mut overflow = false;
    let node_budget_start = b.num_nodes();
    while let Some((u, node)) = queue.pop() {
        if b.num_nodes().wrapping_sub(node_budget_start) > BUILD_PARTIAL_GRAPH_CREATION_BUDGET {
            return None;
        }
        let sder = b.der(node, Nullability::CENTER).ok()?;
        let mut stack = vec![(sder, TSetId::FULL)];
        b.iter_sat(&mut stack, &mut |_, next, set| {
            let dst = *idx.entry(next).or_insert_with(|| {
                if edges.len() >= budget {
                    overflow = true;
                    return usize::MAX;
                }
                let i = edges.len();
                edges.push(Vec::new());
                nodes.push(next);
                queue.push((i, next));
                i
            });
            if dst != usize::MAX {
                edges[u].push(Edge { dst, set });
            }
        });
        if overflow {
            return None;
        }
    }
    Some(Graph { edges, nodes })
}

fn transitive_closure(graph: &Graph) -> Vec<Vec<bool>> {
    let n = graph.edges.len();
    let mut r = vec![vec![false; n]; n];
    for i in 0..n {
        for e in &graph.edges[i] {
            r[i][e.dst] = true;
        }
    }
    for k in 0..n {
        for i in 0..n {
            if !r[i][k] {
                continue;
            }
            for j in 0..n {
                if r[k][j] {
                    r[i][j] = true;
                }
            }
        }
    }
    r
}

// extract SCCs from a reach matrix: i,j share an SCC iff each reaches the other.
fn sccs_from_reach(reach: &[Vec<bool>]) -> Vec<Vec<usize>> {
    let n = reach.len();
    let mut visited = vec![false; n];
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    for i in 0..n {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        let mut scc = vec![i];
        for j in (i + 1)..n {
            if !visited[j] && reach[i][j] && reach[j][i] {
                visited[j] = true;
                scc.push(j);
            }
        }
        sccs.push(scc);
    }
    sccs
}

fn opener_class(b: &mut RegexBuilder, start: NodeId) -> TSetId {
    let sder = match b.der(start, Nullability::CENTER) {
        Ok(d) => d,
        Err(_) => return TSetId::EMPTY,
    };
    let mut stack = vec![(sder, TSetId::FULL)];
    let mut acc = TSetId::EMPTY;
    b.iter_sat(
        &mut stack,
        &mut (|bb, next, set| {
            if next.0 > NodeId::BOT.0 {
                acc = bb.solver().or_id(acc, set);
            }
        }),
    );
    acc
}

fn collect_union_branches(b: &RegexBuilder, node: NodeId, out: &mut Vec<NodeId>) {
    if node.is_union(b) {
        collect_union_branches(b, node.left(b), out);
        collect_union_branches(b, node.right(b), out);
    } else {
        out.push(node);
    }
}

fn first_lb_in_branch(b: &RegexBuilder, node: NodeId) -> Option<NodeId> {
    if node.is_lookbehind(b) {
        return Some(node);
    }
    if node.is_concat(b) {
        return first_lb_in_branch(b, node.left(b));
    }
    None
}

fn lb_is_unbounded(b: &RegexBuilder, lb_node: NodeId) -> bool {
    let inner = b.get_lookbehind_inner(lb_node);
    let rest = if inner.is_concat(b) && inner.left(b).is_star(b) {
        inner.right(b)
    } else {
        inner
    };
    b.get_min_max_length(rest).1 == u32::MAX
}

fn any_unbounded_lookback(b: &RegexBuilder, node: NodeId) -> bool {
    if !node.contains_lookbehind(b) {
        return false;
    }
    if node.is_lookbehind(b) && lb_is_unbounded(b, node) {
        return true;
    }
    [node.left(b), node.right(b)]
        .into_iter()
        .any(|c| c != NodeId::MISSING && any_unbounded_lookback(b, c))
}

/// heuristic checks if we can support an union with lookbehinds,
/// we wont determine which one matched so we require them to be disjoint
fn union_branches_distinguishable(b: &mut RegexBuilder, union_node: NodeId) -> bool {
    let mut branches = Vec::new();
    collect_union_branches(b, union_node, &mut branches);
    union_branches_distinguishable_list(b, &branches)
}

fn union_branches_distinguishable_list(b: &mut RegexBuilder, branches: &[NodeId]) -> bool {
    let any_lb = branches.iter().any(|n| n.contains_lookbehind(b));
    if !any_lb {
        return true;
    }
    let union_node_len = branches
        .iter()
        .fold((0u32, 0u32), |(mn, mx), &n| {
            let (bn, bx) = b.get_min_max_length(n);
            (mn.min(bn), mx.max(bx))
        });
    // lookbehind defers its reverse-pass null emission by its lookback length.
    if union_node_len.1 > 0 && branches.iter().any(|&br| any_unbounded_lookback(b, br)) {
        return false;
    }
    let any_anchors = branches.iter().any(|&br| b.contains_anchors(br));
    if any_anchors
        && branches
            .iter()
            .any(|&br| br.contains_lookbehind(b) && first_lb_in_branch(b, br).is_none())
    {
        return false;
    }
    // this is outside of formally verified territory, careful with changes
    let fixed_lens: Option<Vec<u32>> = branches.iter().map(|&br| b.get_fixed_length(br)).collect();
    if let Some(lens) = fixed_lens {
        if lens.iter().all(|&l| l == lens[0]) {
            return true;
        }
    }
    let mut firsts: Vec<(bool, TSetId, Option<NodeId>, Option<u32>)> =
        Vec::with_capacity(branches.len());
    for &br in branches {
        let has_lb = br.contains_lookbehind(b);
        let lb_node = if has_lb {
            first_lb_in_branch(b, br)
        } else {
            None
        };
        let stripped = match b.strip_lb(br) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let sets = match prefix::calc_potential_start_prune(b, stripped, 1, 64, false) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let (bmin, bmax) = b.get_min_max_length(br);
        let first = match sets.first() {
            Some(&s) => s,
            None => {
                if bmin == 0 {
                    return false;
                }
                continue;
            }
        };
        let fixed_len = if bmin == bmax { Some(bmin) } else { None };
        firsts.push((has_lb, first, lb_node, fixed_len));
    }
    for i in 0..firsts.len() {
        if !firsts[i].0 {
            continue;
        }
        for j in 0..firsts.len() {
            if i == j {
                continue;
            }
            let inter = b.solver().and_id(firsts[i].1, firsts[j].1);
            if inter == TSetId::EMPTY {
                continue;
            }
            let lb_same = match (firsts[i].2, firsts[j].2) {
                (Some(ni), Some(nj)) => {
                    b.get_lookbehind_inner(ni) == b.get_lookbehind_inner(nj)
                }
                (None, None) => true,
                _ => false,
            };
            if lb_same {
                continue;
            }
            let same_fixed = matches!(
                (firsts[i].3, firsts[j].3),
                (Some(a), Some(c)) if a == c
            );
            if !same_fixed {
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compatibility {
    LookaroundUnion,
}

fn combine_compatibility(
    left: Option<Compatibility>,
    right: Option<Compatibility>,
) -> Option<Compatibility> {
    left.or(right)
}

fn ensure_supported_rec(
    b: &mut RegexBuilder,
    node: NodeId,
    at_start: bool,
    strict_lb_start: bool,
    memo: &mut std::collections::HashSet<(NodeId, bool, bool)>,
) -> Result<Option<Compatibility>, resharp_algebra::ResharpError> {
    if !node.contains_lookaround(b) {
        return Ok(None);
    }
    if !memo.insert((node, at_start, strict_lb_start)) {
        return Ok(None);
    }
    match b.get_kind(node) {
        Kind::Union => {
            let (l, r) = (node.left(b), node.right(b));
            let has_lb = l.contains_lookbehind(b) || r.contains_lookbehind(b);
            if has_lb && !union_branches_distinguishable(b, node) {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            let left = ensure_supported_rec(b, l, at_start, strict_lb_start, memo)?;
            let right = ensure_supported_rec(b, r, at_start, strict_lb_start, memo)?;
            let union = if has_lb {
                Some(Compatibility::LookaroundUnion)
            } else {
                None
            };
            Ok(combine_compatibility(
                union,
                combine_compatibility(left, right),
            ))
        }
        Kind::Inter => {
            let (l, r) = (node.left(b), node.right(b));
            // distributing (A|B) & C eagerly to (A&C)|(B&C)
            // to unlock some patterns outside of RE# fragment like `(^abc|def)&.*`
            for (u, other) in [(l, r), (r, l)] {
                if u.is_union(b) && u.contains_lookbehind(b) {
                    if strict_lb_start && !at_start {
                        return Err(resharp_algebra::ResharpError::UnsupportedPattern);
                    }
                    let mut branches = Vec::new();
                    collect_union_branches(b, u, &mut branches);
                    let distributed_branches: Vec<NodeId> =
                        branches.iter().map(|&br| b.mk_inter(br, other)).collect();
                    if !union_branches_distinguishable_list(b, &distributed_branches) {
                        return Err(resharp_algebra::ResharpError::UnsupportedPattern);
                    }
                    let other_compatibility =
                        ensure_supported_rec(b, other, at_start, strict_lb_start, memo)?;
                    return Ok(combine_compatibility(
                        Some(Compatibility::LookaroundUnion),
                        other_compatibility,
                    ));
                }
            }
            let left = ensure_supported_rec(b, l, at_start, strict_lb_start, memo)?;
            let right = ensure_supported_rec(b, r, at_start, strict_lb_start, memo)?;
            Ok(combine_compatibility(left, right))
        }
        Kind::Concat => {
            let left = node.left(b);
            let right = node.right(b);
            let (_, left_max) = b.get_min_max_length(left);
            if left_max > 0 && right.is_union(b) && right.contains_lookbehind(b) {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            if left.is_union(b) && left.contains_lookbehind(b) {
                if strict_lb_start && !at_start {
                    return Err(resharp_algebra::ResharpError::UnsupportedPattern);
                }
                let mut branches = Vec::new();
                collect_union_branches(b, left, &mut branches);
                let distributed_branches: Vec<NodeId> =
                    branches.iter().map(|&br| b.mk_concat(br, right)).collect();
                if union_branches_distinguishable_list(b, &distributed_branches) {
                    let right_compatibility =
                        ensure_supported_rec(b, right, at_start, strict_lb_start, memo)?;
                    return Ok(combine_compatibility(
                        Some(Compatibility::LookaroundUnion),
                        right_compatibility,
                    ));
                } else {
                    return Err(resharp_algebra::ResharpError::UnsupportedPattern);
                }
            }
            let left_compatibility = ensure_supported_rec(b, left, at_start, strict_lb_start, memo)?;
            let (_, left_max) = b.get_min_max_length(left);
            let right_compatibility =
                ensure_supported_rec(b, right, at_start && left_max == 0, strict_lb_start, memo)?;
            Ok(combine_compatibility(
                left_compatibility,
                right_compatibility,
            ))
        }
        Kind::Star => {
            if node.left(b).contains_lookaround(b) {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            ensure_supported_rec(b, node.left(b), at_start, strict_lb_start, memo)
        }
        Kind::Ordered => ensure_supported_rec(b, node.left(b), at_start, strict_lb_start, memo),
        Kind::Compl => ensure_supported_rec(b, node.left(b), at_start, strict_lb_start, memo),
        Kind::Lookbehind => {
            let prev = node.right(b);
            let (_, prev_max) = if prev == NodeId::MISSING {
                (0, 0)
            } else {
                b.get_min_max_length(prev)
            };
            if !at_start || prev_max > 0 {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            let left = ensure_supported_rec(b, node.left(b), at_start, strict_lb_start, memo)?;
            let right = ensure_supported_rec(b, prev, at_start, strict_lb_start, memo)?;
            Ok(combine_compatibility(left, right))
        }
        Kind::Lookahead => {
            let left = ensure_supported_rec(b, node.left(b), at_start, strict_lb_start, memo)?;
            let right = ensure_supported_rec(b, node.right(b), at_start, strict_lb_start, memo)?;
            Ok(combine_compatibility(left, right))
        }
        Kind::Pred => Ok(None),
        Kind::Begin => Ok(None),
        Kind::End => Ok(None),
    }
}

fn ensure_begin_leading(
    b: &RegexBuilder,
    node: NodeId,
    at_start: bool,
    memo: &mut std::collections::HashSet<(NodeId, bool)>,
) -> Result<(), resharp_algebra::ResharpError> {
    if !b.contains_anchors(node) {
        return Ok(());
    }
    if !memo.insert((node, at_start)) {
        return Ok(());
    }
    match b.get_kind(node) {
        Kind::Begin => {
            if at_start {
                Ok(())
            } else {
                Err(resharp_algebra::ResharpError::UnsupportedPattern)
            }
        }
        Kind::End | Kind::Pred => Ok(()),
        Kind::Concat => {
            let l = node.left(b);
            ensure_begin_leading(b, l, at_start, memo)?;
            let (lmin, _) = b.get_min_max_length(l);
            ensure_begin_leading(b, node.right(b), at_start && lmin == 0, memo)
        }
        Kind::Union | Kind::Inter => {
            ensure_begin_leading(b, node.left(b), at_start, memo)?;
            ensure_begin_leading(b, node.right(b), at_start, memo)
        }
        Kind::Star | Kind::Ordered => ensure_begin_leading(b, node.left(b), false, memo),
        Kind::Compl => Ok(()),
        Kind::Lookbehind | Kind::Lookahead => Ok(()),
    }
}

fn ensure_supported(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<Option<Compatibility>, resharp_algebra::ResharpError> {
    ensure_begin_leading(b, node, true, &mut std::collections::HashSet::new())?;
    ensure_supported_rec(b, node, true, true, &mut std::collections::HashSet::new())
}

impl Regex {
    /// compile a pattern with default options.
    ///
    /// ```
    /// let re = resharp::Regex::new(r"\b\w+\b").unwrap();
    /// ```
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        Self::with_options(pattern, RegexOptions::default())
    }

    /// compile a pattern with custom [`RegexOptions`].
    ///
    /// ```
    /// use resharp::{Regex, RegexOptions};
    ///
    /// let re = Regex::with_options(
    ///     r"hello",
    ///     RegexOptions::default().case_insensitive(true),
    /// ).unwrap();
    /// assert!(re.is_match(b"HELLO").unwrap());
    /// ```
    pub fn with_options(pattern: &str, opts: RegexOptions) -> Result<Regex, Error> {
        let mut b = RegexBuilder::new();
        b.lookahead_context_max = opts.lookahead_context_max;
        let pflags = resharp_parser::PatternFlags {
            unicode: opts.unicode != UnicodeMode::Ascii,
            full_unicode: opts.unicode == UnicodeMode::Full,
            ascii_perl_classes: opts.unicode == UnicodeMode::Javascript,
            case_insensitive: opts.case_insensitive,
            dot_matches_new_line: opts.dot_matches_new_line,
            multiline: opts.multiline,
            ignore_whitespace: opts.ignore_whitespace,
            expanded_ast_limit: if opts.unbounded_size {
                u64::MAX
            } else {
                resharp_parser::DEFAULT_EXPANDED_AST_LIMIT
            },
            max_list_len: if opts.unbounded_size {
                usize::MAX
            } else {
                resharp_parser::DEFAULT_MAX_LIST_LEN
            },
            max_repeat: if opts.unbounded_size {
                u32::MAX
            } else {
                resharp_parser::DEFAULT_MAX_REPEAT
            },
            max_depth: if opts.unbounded_size {
                usize::MAX
            } else {
                resharp_parser::DEFAULT_MAX_DEPTH
            },
        };
        let node = resharp_parser::parse_ast_with(&mut b, pattern, &pflags)?;
        Self::from_node_inner(b, node, opts, pattern.len())
    }

    /// build from a pre-constructed AST node.
    #[doc(hidden)]
    pub fn from_node(b: RegexBuilder, node: NodeId, opts: RegexOptions) -> Result<Regex, Error> {
        Self::from_node_inner(b, node, opts, 0)
    }

    fn from_node_inner(
        mut b: RegexBuilder,
        node: NodeId,
        opts: RegexOptions,
        pattern_len: usize,
    ) -> Result<Regex, Error> {
        // sanity check
        let node_limit = if opts.unbounded_size {
            usize::MAX
        } else {
            200_000
        };
        if b.tree_size(node, node_limit) >= node_limit {
            return Err(Error::PatternTooLarge);
        }
        let _compatibility = ensure_supported(&mut b, node)?;

        let empty_nullable = b
            .nullability_emptystring(node)
            .has(Nullability::EMPTYSTRING);
        let initial_nullability = b.nullability(node);

        let node_fwd_simpl = b.simplify_fwd_initial(node);
        let fwd_start = b.strip_lb(node_fwd_simpl)?;
        let fwd_end_nullable = b.nullability(fwd_start).has(Nullability::END);
        let rev_basis = b
            .strip_trailing_redundant_lookahead(node_fwd_simpl)
            .unwrap_or(node_fwd_simpl);
        let ts_rev_start = b.ts_rev_start(rev_basis)?;
        // ensure_supported_rec(&mut b, ts_rev_start, true, false)?;
        #[cfg(feature = "debug")]
        {
            eprintln!("[fwd]: {:.70}", b.pp(node));
            eprintln!("[ts_rev]: {:.70}", b.pp(ts_rev_start));
        }

        let is_empty_lang = node_fwd_simpl == NodeId::BOT;
        // TODO: make it configurable to actually check and reject empty lang entriely
        let body_after_begin = {
            let mut cur = node_fwd_simpl;
            loop {
                if cur.is_concat(&b) && cur.left(&b) == NodeId::BEGIN {
                    cur = cur.right(&b);
                    continue;
                }
                if cur.is_concat(&b) {
                    let cur_left = cur.left(&b);
                    let cur_right = cur.right(&b);
                    if cur_right == NodeId::BEGIN
                        && b.contains_lookbehind(cur_left)
                        && b.nullability(cur_left) == Nullability::ALWAYS
                    {
                        cur = cur_right;
                        continue;
                    }
                }
                break;
            }
            cur
        };
        let lb_stripped = fwd_start != body_after_begin;
        let fwd_begin_anchored = b.is_begin_anchored(node_fwd_simpl) && !lb_stripped;
        let has_look = b.contains_look(node_fwd_simpl);
        let rev_node = b.reverse(node_fwd_simpl)?;
        let rev_end_anchored = b.is_begin_anchored(rev_node) && !fwd_end_nullable;
        let fixed_length = b.get_fixed_length(node_fwd_simpl);
        let (min_len, max_len) = b.get_min_max_length(node_fwd_simpl);
        let max_length = if max_len != u32::MAX {
            Some(max_len)
        } else {
            None
        };
        let max_cap = opts.max_dfa_capacity.min(u16::MAX as usize);
        let mut opts = opts;
        let has_anchors_pre = b.contains_anchors(node_fwd_simpl);
        let ah = auto_harden(&mut b, fwd_start, has_anchors_pre);
        if ah.full {
            opts.hardened = true;
        }
        let (selected, rev_skip, _fwd_prefix_wins) = if opts.disable_prefixes {
            (None, None, false)
        } else {
            prefix::select_prefix(
                &mut b,
                node_fwd_simpl,
                ts_rev_start,
                has_look,
                min_len,
                max_cap,
                ah.no_fwd_prefix,
                opts.hardened,
                opts.force_convergence,
            )?
        };
        #[cfg(feature = "debug")]
        {
            let kind = match (&selected, &rev_skip) {
                (Some(prefix::PrefixKind::AnchoredFwd(_)), _) => "AnchoredFwd",
                (Some(prefix::PrefixKind::AnchoredFwdLb(_)), _) => "AnchoredFwdLb",
                (Some(prefix::PrefixKind::AnchoredRev), _) => "AnchoredRev",
                (Some(prefix::PrefixKind::PotentialStart), _) => "PotentialStart",
                #[cfg(feature = "convergence_prefix")]
                (Some(prefix::PrefixKind::Convergence), _) => "Convergence",
                (None, Some(_)) => "<none> (rev prefix_skip)",
                (None, None) => "<none>",
            };
            eprintln!("[prefix] selected={kind} rev_skip={}", rev_skip.is_some());
        }
        let has_fwd_prefix = matches!(
            selected,
            Some(prefix::PrefixKind::AnchoredFwd(_) | prefix::PrefixKind::AnchoredFwdLb(_))
        );
        let fwd = ldfa::LDFA::new_fwd(&mut b, fwd_start, max_cap)?;

        let ts_fwd_start = {
            let with_ts = b.mk_concat(NodeId::TS, node);
            // b.simplify_fwd_initial(with_ts)
            with_ts
        };
        // println!("{:?}", b.pp(node));
        // println!("{:?}", b.pp(ts_fwd_start));
        #[allow(unused_mut)]
        let mut ts_fwd = ldfa::LDFA::new_fwd(&mut b, ts_fwd_start, max_cap)?;

        let mut rev_ts = ldfa::LDFA::new_rev(&mut b, ts_rev_start, max_cap)?;
        #[cfg(feature = "convergence_prefix")]
        let mut conv_b: Option<ldfa::LDFA> = None;
        #[cfg(feature = "convergence_prefix")]
        let mut conv_prefix = false;
        if let Some((search, resume_node, b_node)) = rev_skip {
            #[cfg(not(feature = "convergence_prefix"))]
            let _ = b_node;
            #[cfg(feature = "debug")]
            eprintln!("[conv split] resume_node={:?} b_node={:?}", resume_node.map(|n| b.pp(n)), b_node.map(|n| b.pp(n)));
            let resume = match resume_node {
                Some(node) => {
                    let pruned_node = rev_ts.state_nodes[rev_ts.pruned as usize];
                    let union = b.mk_union(node, pruned_node);
                    #[cfg(feature = "debug")]
                    eprintln!("[conv resume build] node={} pruned_node={} union={}", b.pp(node), b.pp(pruned_node), b.pp(union));
                    rev_ts.get_or_register(&mut b, union)
                }
                None => 0,
            };
            #[allow(unused_mut)]
            let mut window = 0u32;
            #[cfg(feature = "convergence_prefix")]
            if resume != 0 {
                let b_node = b_node.expect(
                    "convergence prefix (resume != 0) must carry its right-side `b` node",
                );
                let b_max = b.get_min_max_length(b_node).1;
                let fwd_window = if b_max == u32::MAX { 0 } else { b_max };
                let rev_b_node = b.reverse(b_node)?;
                let rev_b_node = b.normalize_rev(rev_b_node, 0)?;
                let rev_window = b.get_min_max_length(rev_b_node).0;
                window = fwd_window.max(rev_window);
                conv_b = Some(ldfa::LDFA::new_fwd(&mut b, b_node, max_cap)?);
                conv_prefix = true;
            }
            rev_ts.install_prefix(&mut b, search, resume as u32, window)?;
        }
        if !b.starts_with_ts(ts_rev_start) {
            rev_ts.ensure_dead_skip();
        }

        let rev_anchored = if rev_end_anchored {
            let rev_no_ts = b.normalize_rev(rev_node, 0)?;
            Some(ldfa::LDFA::new_rev(&mut b, rev_no_ts, max_cap)?)
        } else {
            None
        };

        #[cfg(feature = "stream")]
        let stream_init = {
            let fwd_pruned = b.prune_begin_eps(ts_fwd_start);
            let rev_pruned = b.prune_begin_eps(ts_rev_start);
            stream::StreamInit {
                start_node: node_fwd_simpl,
                seek_fwd: ts_fwd.get_or_register(&mut b, fwd_pruned).into(),
                seek_rev: rev_ts.get_or_register(&mut b, rev_pruned).into(),
            }
        };

        let (fwd_lb_begin_nullable, fwd_lb_body_nullable, lb_check_bytes) =
            if matches!(selected, Some(prefix::PrefixKind::AnchoredFwdLb(_))) {
                let lb_node = node_fwd_simpl.left(&b);
                let lb_inner = b.get_lookbehind_inner(lb_node);
                let (_, lb_fixed) = prefix::fwd_lb_class(&mut b, lb_node)
                    .ok_or(Error::InternalError("AnchoredFwdLb requires fixed-length lb"))?;
                let begin_nullable = b.nullability(lb_inner).has(Nullability::BEGIN);
                let body_nullable = b.nullability(fwd_start) != Nullability::NEVER;
                (begin_nullable, body_nullable, u8::try_from(lb_fixed).map_err(|_| Error::InternalError("AnchoredFwdLb lb_fixed exceeds u8"))?)
            } else {
                (false, false, 0)
            };

        // lots of conditions when something else is better.. possibly removing it entirely
        let always_nullable = initial_nullability == Nullability::ALWAYS;
        let max_len_limit = if always_nullable { 512 } else { 100 };
        let use_bounded = !opts.disable_prefixes
            && !has_fwd_prefix
            && max_length.is_some()
            && max_len <= max_len_limit
            && !b.contains_lookbehind(node_fwd_simpl)
            && !node_fwd_simpl.contains_lookahead(&b)
            && !b.contains_anchors(node_fwd_simpl)
            && pattern_len <= 150 // a guess..
            && (!empty_nullable || always_nullable);

        let bounded = if use_bounded {
            Some(bdfa::BDFA::new(&mut b, fwd_start)?)
        } else {
            None
        };

        let has_bounded = bounded.is_some();
        let bounded_safe_find_all = if has_bounded {
            if always_nullable {
                true
            } else {
                let inner_match = b.mk_concat(node_fwd_simpl, resharp_algebra::NodeId::TOPPLUS);
                let interior = b.mk_concat(resharp_algebra::NodeId::TOPPLUS, inner_match);
                let overlap = b.mk_inter(node_fwd_simpl, interior);
                b.is_empty_lang(overlap) == Some(true)
            }
        } else {
            false
        };
        let has_anchors = b.contains_anchors(node_fwd_simpl);
        let has_lb = b.contains_lookbehind(node_fwd_simpl);
        let has_la = node_fwd_simpl.contains_lookahead(&b);

        const CLASS_PLUS_FREQ: u64 = 65_535;
        let class_plus = if !has_lb && !has_la && !has_anchors && !has_look && fixed_length.is_none() {
            detect_class_plus(&mut b, fwd_start)
                .filter(|&c| {
                    let class = class_freq_sum(&mut b, c);
                    let compl = total_byte_freq().saturating_sub(class);
                    class >= CLASS_PLUS_FREQ && compl >= CLASS_PLUS_FREQ
                })
                .map(|c| class_membership(&mut b, c))
        } else {
            None
        };

        let neg_lb = if has_lb && matches!(selected, Some(prefix::PrefixKind::AnchoredFwd(_))) {
            prefix::neg_lb_classes(&mut b, node_fwd_simpl)
        } else {
            None
        };

        let need_nn_cycle = (opts.hardened && !has_bounded && fixed_length.is_none() && max_cap >= 64)
            || initial_nullability == Nullability::ALWAYS;
        let nn_cycle = need_nn_cycle && fwd.has_nonnullable_cycle(&mut b, 256);
        let hardened = opts.hardened && !has_bounded && fixed_length.is_none() && max_cap >= 64 && nn_cycle;
        let star_loop =
            initial_nullability == Nullability::ALWAYS && !has_lb && !nn_cycle;

        let fas = if hardened || initial_nullability == Nullability::ALWAYS {
            let ksm = if hardened {
                fwd_start.contains_lookahead(&b) || initial_nullability != Nullability::ALWAYS
            } else {
                true
            };
            let ksm = ksm || initial_nullability == Nullability::ALWAYS;
            Some(fas::FwdDFA::new(&fwd, ksm))
        } else {
            None
        };

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b,
                fwd,
                fwd_ts: ts_fwd,
                rev: rev_anchored,
                rev_ts,
                #[cfg(feature = "stream")]
                stream: stream_init,
                nulls: StartPositions::new(),
                matches: Vec::new(),
                bounded,
                fas,
                #[cfg(feature = "convergence_prefix")]
                conv_b,
            }),
            find_all: compute_find_all(
                is_empty_lang,
                fwd_begin_anchored,
                rev_end_anchored,
                hardened,
                has_bounded,
                class_plus.is_some(),
                &selected,
            ),
            fwd_lb_stripped: lb_stripped,
            class_plus,
            prefix: selected,
            fixed_length,
            empty_nullable,
            always_nullable: initial_nullability == Nullability::ALWAYS,
            star_loop,
            is_empty_lang,
            fwd_begin_anchored,
            rev_end_anchored,
            initial_nullability,
            fwd_end_nullable,
            hardened,
            has_bounded,
            bounded_safe_find_all,
            lb_check_bytes,
            fwd_lb_begin_nullable,
            fwd_lb_body_nullable,
            init_flags: InitialNodeFlags::new(has_anchors, has_lb, has_la),
            #[cfg(feature = "convergence_prefix")]
            conv_prefix,
            neg_lb,
            #[cfg(feature = "stream")]
            stream_cache: Default::default(),
        })
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn node_count(&self) -> u32 {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).b.num_nodes()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn dfa_stats(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (inner.fwd.state_nodes.len(), inner.rev_ts.state_nodes.len())
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn is_hardened(&self) -> bool {
        self.hardened
    }

    #[cfg(feature = "convergence_prefix")]
    #[allow(missing_docs)]
    pub fn uses_convergence_prefix(&self) -> bool {
        self.conv_prefix
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn has_fwd_prefix(&self) -> bool {
        matches!(
            self.prefix,
            Some(prefix::PrefixKind::AnchoredFwd(_) | prefix::PrefixKind::AnchoredFwdLb(_))
        )
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn has_prefix(&self) -> bool {
        self.prefix.is_some()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn is_fwd_begin_anchored(&self) -> bool {
        self.fwd_begin_anchored
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn bdfa_stats(&self) -> Option<(usize, usize, usize)> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .bounded
            .as_ref()
            .map(|b| (b.states.len(), 1usize << b.mt_log, b.prefix_len))
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn find_all_kind_name(&self) -> &'static str {
        match self.find_all {
            FindAll::EmptyLang => "EmptyLang",
            FindAll::Anchored => "Anchored",
            FindAll::EndAnchored => "EndAnchored",
            FindAll::Hardened => "Hardened",
            FindAll::Dfa => "Dfa",
            FindAll::ClassPlus => "ClassPlus",
            FindAll::Bounded => "Bounded",
            FindAll::FwdPrefix => "FwdPrefix",
            FindAll::FwdLbPrefix => "FwdLbPrefix",
        }
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn prefix_kind_name(&self) -> Option<&'static str> {
        match &self.prefix {
            None => None,
            Some(prefix::PrefixKind::AnchoredFwd(_)) => Some("AnchoredFwd"),
            Some(prefix::PrefixKind::AnchoredFwdLb(_)) => Some("AnchoredFwdLb"),
            Some(prefix::PrefixKind::AnchoredRev) => Some("AnchoredRev"),
            Some(prefix::PrefixKind::PotentialStart) => Some("PotentialStart"),
            #[cfg(feature = "convergence_prefix")]
            Some(prefix::PrefixKind::Convergence) => Some("Convergence"),
        }
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn fwd_prefix_kind(&self) -> Option<(&'static str, usize)> {
        match &self.prefix {
            Some(prefix::PrefixKind::AnchoredFwd(fp))
            | Some(prefix::PrefixKind::AnchoredFwdLb(fp)) => Some((fp.variant_name(), fp.len())),
            _ => None,
        }
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn has_accel(&self) -> (bool, bool) {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let fwd = self.prefix.as_ref().is_some_and(|p| p.is_fwd());
        let rev = self.prefix.as_ref().is_some_and(|p| p.is_rev())
            || inner.rev_ts.can_skip();
        (fwd, rev)
    }

    // sole verdict for empty input: a zero-width match at 0 iff the empty
    // string is in the language.
    pub(crate) fn empty_input_match(&self) -> Option<Match> {
        (self.empty_nullable && !self.is_empty_lang).then_some(Match { start: 0, end: 0 })
    }

    /// all leftmost-longest, non-overlapping matches as `[start, end)` byte ranges.
    ///
    /// ```
    /// let re = resharp::Regex::new(r"\d+").unwrap();
    /// let m = re.find_all(b"abc 123 def 456").unwrap();
    /// assert_eq!(m.len(), 2);
    /// assert_eq!((m[0].start, m[0].end), (4, 7));
    /// ```
    pub fn find_all(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if input.is_empty() {
            return Ok(self.empty_input_match().into_iter().collect());
        }

        #[cfg(all(feature = "debug", debug_assertions))]
        eprintln!("[algorithm] {:?} input={:?}", self.find_all, input);

        match self.find_all {
            FindAll::EmptyLang => Ok(vec![]),
            FindAll::ClassPlus => Ok(self.find_all_class_plus(input)),
            FindAll::Anchored => Ok(self.find_anchored(input)?.into_iter().collect()),
            FindAll::EndAnchored => Ok(self.find_end_anchored(input)?.into_iter().collect()),
            FindAll::Hardened | FindAll::Dfa => self.find_all_dfa(input),
            FindAll::Bounded => {
                if self.bounded_safe_find_all {
                    self.find_all_fwd_bounded(input)
                } else {
                    self.find_all_dfa(input)
                }
            }
            FindAll::FwdPrefix => match &self.prefix {
                Some(prefix::PrefixKind::AnchoredFwd(fp)) => {
                    self.find_all_fwd_prefix(fp, self.neg_lb.as_ref(), input)
                }
                _ => Err(Error::InternalError("FwdPrefix without AnchoredFwd prefix")),
            },
            FindAll::FwdLbPrefix => match &self.prefix {
                Some(prefix::PrefixKind::AnchoredFwdLb(fp)) => {
                    self.find_all_fwd_lb_prefix(fp, input)
                }
                _ => Err(Error::InternalError("FwdLbPrefix without AnchoredFwdLb prefix")),
            },
        }
    }
}

fn push_end_zero_width(matches: &mut Vec<Match>, len: usize) {
    if matches.last().map(|m| m.start) != Some(len) {
        matches.push(Match { start: len, end: len });
    }
}

fn single_sat_target(b: &mut RegexBuilder, node: NodeId) -> Option<(NodeId, TSetId)> {
    let der = b.der(node, Nullability::CENTER).ok()?;
    let mut targets: Vec<(NodeId, TSetId)> = Vec::new();
    b.collect_der_targets(der, TSetId::FULL, &mut targets);
    let mut live = targets.into_iter().filter(|(t, _)| *t != NodeId::BOT);
    let first = live.next()?;
    if live.next().is_some() {
        return None;
    }
    Some(first)
}

fn detect_class_plus(b: &mut RegexBuilder, node: NodeId) -> Option<TSetId> {
    if b.nullability(node).has(Nullability::CENTER) {
        return None;
    }
    let (t1, c1) = single_sat_target(b, node)?;
    if !b.nullability(t1).has(Nullability::CENTER) {
        return None;
    }
    let (t2, c2) = single_sat_target(b, t1)?;
    if t2 != t1 || c2 != c1 {
        return None;
    }
    Some(c1)
}

fn class_freq_sum(b: &mut RegexBuilder, set: TSetId) -> u64 {
    b.solver()
        .collect_bytes(set)
        .iter()
        .map(|&c| crate::simd::BYTE_FREQ[c as usize] as u64)
        .sum()
}

fn total_byte_freq() -> u64 {
    crate::simd::BYTE_FREQ.iter().map(|&f| f as u64).sum()
}

fn class_membership(b: &mut RegexBuilder, set: TSetId) -> [u64; 4] {
    let mut table = [0u64; 4];
    for &c in b.solver().collect_bytes(set).iter() {
        table[(c >> 6) as usize] |= 1u64 << (c & 63);
    }
    table
}

fn compute_find_all(
    is_empty_lang: bool,
    fwd_begin_anchored: bool,
    rev_end_anchored: bool,
    hardened: bool,
    has_bounded: bool,
    class_plus: bool,
    prefix: &Option<prefix::PrefixKind>,
) -> FindAll {
    if is_empty_lang {
        return FindAll::EmptyLang;
    }
    if fwd_begin_anchored {
        return FindAll::Anchored;
    }
    if hardened {
        return FindAll::Hardened;
    }
    if class_plus {
        return FindAll::ClassPlus;
    }
    if rev_end_anchored {
        return FindAll::EndAnchored;
    }
    match prefix {
        Some(prefix::PrefixKind::AnchoredFwd(_)) => FindAll::FwdPrefix,
        Some(prefix::PrefixKind::AnchoredFwdLb(_)) => FindAll::FwdLbPrefix,
        Some(prefix::PrefixKind::AnchoredRev | prefix::PrefixKind::PotentialStart) => {
            FindAll::Dfa
        }
        _ => {
            if has_bounded {
                FindAll::Bounded
            } else {
                FindAll::Dfa
            }
        }
    }
}

#[cfg(feature = "convergence_prefix")]
pub(crate) fn find_inner_literal(
    b: &mut resharp_algebra::RegexBuilder,
    search_start: resharp_algebra::NodeId,
) -> Option<(
    resharp_algebra::NodeId,
    Vec<resharp_algebra::solver::TSetId>,
    resharp_algebra::solver::TSetId,
)> {
    use resharp_algebra::NodeId;
    use resharp_algebra::solver::TSetId;

    if search_start == NodeId::BOT || b.get_min_max_length(search_start).0 == 0 {
        return None;
    }

    const MAX_SINK_FREQ: u64 = 25_000;
    fn set_freq(b: &mut resharp_algebra::RegexBuilder, set: TSetId) -> u64 {
        b.solver()
            .collect_bytes(set)
            .iter()
            .map(|&c| crate::simd::BYTE_FREQ[c as usize] as u64)
            .sum()
    }

    let mut spine: Vec<(NodeId, NodeId)> = Vec::new();
    let mut curr = search_start;
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

    let mut best: Option<(usize, TSetId)> = None;
    let mut best_score = u64::MAX;
    let mut needle_union = TSetId::EMPTY;
    let mut quad = false;
    for (i, &(_, head)) in spine.iter().enumerate() {
        if !quad && head.is_pred(b) {
            let l = head.pred_tset(b);
            let freq = set_freq(b, l);
            if freq < MAX_SINK_FREQ && freq < best_score {
                best = Some((i, l));
                best_score = freq;
            }
        }
        if b.get_min_max_length(head).1 == u32::MAX {
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
            if b.solver().is_sat_id(lead, needle_union) {
                quad = true;
            }
        } else if head.is_pred(b) {
            let s = head.pred_tset(b);
            if set_freq(b, s) < MAX_SINK_FREQ {
                needle_union = b.solver().or_id(needle_union, s);
            }
        }
    }
    let (i, _l) = best?;

    let is_byte_lit = |b: &mut RegexBuilder, head: NodeId| {
        if !head.is_pred(b) {
            return false;
        }
        let ts = head.pred_tset(b);
        b.solver().collect_bytes(ts).len() == 1
    };
    let mut i_lo = i;
    let mut i_hi = i;
    if is_byte_lit(b, spine[i].1) {
        while i_lo > 0 && is_byte_lit(b, spine[i_lo - 1].1) {
            i_lo -= 1;
        }
        while i_hi + 1 < spine.len() && is_byte_lit(b, spine[i_hi + 1].1) {
            i_hi += 1;
        }
    }
    let run: Vec<TSetId> = (i_lo..=i_hi).map(|k| spine[k].1.pred_tset(b)).collect();
    let anchor_set = run[0];
    let l_rep = run
        .iter()
        .copied()
        .min_by_key(|&s| set_freq(b, s))
        .unwrap();

    let mut j = i_lo;
    while j > 0 {
        let prev_head = spine[j - 1].1;
        if !prev_head.is_star(b) {
            break;
        }
        let body = prev_head.left(b);
        if !body.is_pred(b) {
            break;
        }
        let body_set = body.pred_tset(b);
        if !b.solver().is_sat_id(body_set, anchor_set) {
            break;
        }
        j -= 1;
    }
    let prefix_result = spine[j].0;
    Some((prefix_result, run, l_rep))
}

#[cfg(feature = "convergence_prefix")]
#[doc(hidden)]
pub fn detect_inner_literal_bytes(pattern: &str) -> Option<Vec<u8>> {
    let mut b = resharp_algebra::RegexBuilder::new();
    let pflags = resharp_parser::PatternFlags::default();
    let node = resharp_parser::parse_ast_with(&mut b, pattern, &pflags).ok()?;
    let node_fwd_simpl = b.simplify_fwd_initial(node);
    let rev_basis = b
        .strip_trailing_redundant_lookahead(node_fwd_simpl)
        .unwrap_or(node_fwd_simpl);
    let ts_rev_start = b.ts_rev_start(rev_basis).ok()?;
    let rev_stripped = crate::prefix::PrefixSets::compute(&mut b, node_fwd_simpl, ts_rev_start)
        .ok()?
        .rev_stripped;
    let (_c, run, _l) = find_inner_literal(&mut b, rev_stripped)?;
    let mut bytes: Vec<u8> = Vec::new();
    for &s in run.iter().rev() {
        bytes.extend(b.solver().collect_bytes(s));
    }
    Some(bytes)
}

impl Regex {
    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn rev_state_dump(&self) -> String {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let rev = &inner.rev_ts;
        let mut out = String::new();
        for (i, &node) in rev.state_nodes.iter().enumerate() {
            let eid = rev.effects_id.get(i).copied().unwrap_or(0);
            let alg_nid = inner.b.get_nulls_id(node);
            let pretty = inner.b.pp(node);
            let pretty = if pretty.len() > 200 {
                format!("{}...", &pretty[..200])
            } else {
                pretty
            };
            out += &format!(
                "  s[{}] eid={} alg_nid={:?} pp={}\n",
                i, eid, alg_nid, pretty
            );
        }
        out
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn effects_debug(&self) -> String {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let rev = &inner.rev_ts;
        let mut out = String::new();
        for (i, &eid) in rev.effects_id.iter().enumerate() {
            if eid != 0 {
                let nulls: Vec<String> = rev.effects[eid as usize]
                    .iter()
                    .map(|n| format!("(mask={},rel={})", n.mask.0, n.rel))
                    .collect();
                out += &format!("  state[{}] eid={} nulls=[{}]\n", i, eid, nulls.join(", "));
            }
        }
        out
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn collect_rev_nulls_debug(&self, input: &[u8]) -> Vec<usize> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.nulls.clear();
        #[cfg(feature = "convergence_prefix")]
        {
            let RegexInner { rev_ts, b, nulls, conv_b, .. } = &mut *inner;
            rev_ts
                .collect_rev(b, input.len() - 1, input, nulls, conv_b.as_mut())
                .unwrap();
        }
        #[cfg(not(feature = "convergence_prefix"))]
        inner
            .rev_ts
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls, None)
            .unwrap();
        inner.nulls.positions_desc().collect()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn scan_fwd_debug(&self, input: &[u8], pos: usize) -> Option<usize> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.fwd.scan_fwd_optional(&mut inner.b, pos, input).unwrap()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn scan_fwd_all_nulls_debug(&self, input: &[u8], pos: usize) -> Vec<usize> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut nulls = resharp_algebra::nulls::StartPositions::new();
        inner
            .fwd
            .scan_fwd_all_nulls_from(&mut inner.b, pos, input, &mut nulls)
            .unwrap();
        nulls.positions_asc().collect()
    }

    /// Walk RTL step by step, printing the rev-DFA state and its nulls
    /// metadata at each position. Returns the trace as a string.
    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn rev_walk_trace(&self, input: &[u8]) -> String {
        use std::fmt::Write;
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let rev = &mut inner.rev_ts;
        let b = &mut inner.b;
        let mut out = String::new();
        if input.is_empty() {
            return out;
        }
        let last = input.len() - 1;
        let mt = rev.mt_lookup[input[last] as usize] as u32;
        let mut sid = rev.begin_table[mt as usize];
        writeln!(
            out,
            "pos={} byte={:?} (BEGIN ctx) -> s[{}]",
            last, input[last] as char, sid
        )
        .unwrap();
        Self::dump_state(&mut out, b, rev, sid);
        for i in (0..last).rev() {
            let mt = rev.mt_lookup[input[i] as usize] as u32;
            sid = rev.lazy_transition(b, sid, mt).unwrap();
            writeln!(
                out,
                "pos={} byte={:?} (CENTER ctx) -> s[{}]",
                i, input[i] as char, sid
            )
            .unwrap();
            Self::dump_state(&mut out, b, rev, sid);
            if sid as u32 <= ldfa::DFA_DEAD as u32 {
                break;
            }
        }
        out
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn fwd_state_dump(&self) -> String {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let fwd = &inner.fwd;
        let mut out = String::new();
        for (i, &node) in fwd.state_nodes.iter().enumerate() {
            let eid = fwd.effects_id.get(i).copied().unwrap_or(0);
            let ceid = fwd.center_effect_id.get(i).copied().unwrap_or(0);
            let pretty = inner.b.pp(node);
            let pretty = if pretty.len() > 400 {
                format!("{}...", &pretty[..400])
            } else {
                pretty
            };
            out += &format!("  s[{}] eid={} ceid={} pp={}\n", i, eid, ceid, pretty);
        }
        out
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn fwd_walk_trace(&self, input: &[u8]) -> String {
        use std::fmt::Write;
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let fwd = &mut inner.fwd;
        let b = &mut inner.b;
        let mut out = String::new();
        if input.is_empty() {
            return out;
        }
        let mt = fwd.mt_lookup[input[0] as usize] as u32;
        let mut sid = fwd.begin_table[mt as usize];
        writeln!(
            out,
            "pos=0 byte={:?} (BEGIN) -> s[{}]",
            input[0] as char, sid
        )
        .unwrap();
        Self::dump_fwd_state(&mut out, b, fwd, sid);
        for i in 1..input.len() {
            let mt = fwd.mt_lookup[input[i] as usize] as u32;
            sid = fwd.lazy_transition(b, sid, mt).unwrap();
            writeln!(out, "pos={} byte={:?} -> s[{}]", i, input[i] as char, sid).unwrap();
            Self::dump_fwd_state(&mut out, b, fwd, sid);
            if sid as u32 <= ldfa::DFA_DEAD as u32 {
                break;
            }
        }
        out
    }

    #[cfg(feature = "diag")]
    fn dump_fwd_state(
        out: &mut String,
        b: &mut resharp_algebra::RegexBuilder,
        fwd: &ldfa::LDFA,
        sid: u16,
    ) {
        use std::fmt::Write;
        if (sid as usize) >= fwd.state_nodes.len() {
            writeln!(out, "  (uninitialized state)").unwrap();
            return;
        }
        let node = fwd.state_nodes[sid as usize];
        let eid = fwd.effects_id.get(sid as usize).copied().unwrap_or(0);
        let ceid = fwd.center_effect_id.get(sid as usize).copied().unwrap_or(0);
        let pp = b.pp(node);
        let pp = if pp.len() > 240 {
            format!("{}...", &pp[..240])
        } else {
            pp
        };
        writeln!(out, "  pp = {}", pp).unwrap();
        writeln!(out, "  eid={} (end), center_eid={}", eid, ceid).unwrap();
        for (label, e) in [("end", eid), ("center", ceid)] {
            if e != 0 && (e as usize) < fwd.effects.len() {
                let entries: Vec<String> = fwd.effects[e as usize]
                    .iter()
                    .map(|n| format!("(mask={:#b},rel={})", n.mask.0, n.rel))
                    .collect();
                writeln!(
                    out,
                    "  effects[{}][{}] = [{}]",
                    label,
                    e,
                    entries.join(", ")
                )
                .unwrap();
            }
        }
    }

    #[cfg(feature = "diag")]
    fn dump_state(
        out: &mut String,
        b: &mut resharp_algebra::RegexBuilder,
        rev: &ldfa::LDFA,
        sid: u16,
    ) {
        use std::fmt::Write;
        if (sid as usize) >= rev.state_nodes.len() {
            writeln!(out, "  (uninitialized state)").unwrap();
            return;
        }
        let node = rev.state_nodes[sid as usize];
        let eid = rev.effects_id.get(sid as usize).copied().unwrap_or(0);
        let alg_nid = b.get_nulls_id(node);
        let pp = b.pp(node);
        let pp = if pp.len() > 240 {
            format!("{}...", &pp[..240])
        } else {
            pp
        };
        writeln!(out, "  pp = {}", pp).unwrap();
        writeln!(out, "  alg_nulls = {:?}", alg_nid).unwrap();
        if eid != 0 {
            let entries: Vec<String> = rev.effects[eid as usize]
                .iter()
                .map(|n| format!("(mask={:#b},rel={})", n.mask.0, n.rel))
                .collect();
            writeln!(
                out,
                "  dfa_effects[eid={}] = [{}] -> EMIT NULL",
                eid,
                entries.join(", ")
            )
            .unwrap();
        } else {
            writeln!(out, "  dfa_effects = (none, eid=0)").unwrap();
        }
    }

    /// streamlined [X]+ for very common regexes like [a-z]+ etc
    fn find_all_class_plus(&self, input: &[u8]) -> Vec<Match> {
        let table = self
            .class_plus
            .expect("FindAll::ClassPlus requires a materialized class membership table");
        let member = |byte: u8| table[(byte >> 6) as usize] & (1u64 << (byte & 63)) != 0;
        let mut matches = Vec::new();
        let n = input.len();
        let mut i = 0usize;
        while i < n {
            if member(input[i]) {
                let start = i;
                i += 1;
                while i < n && member(input[i]) {
                    i += 1;
                }
                matches.push(Match { start, end: i });
            } else {
                i += 1;
            }
        }
        matches
    }

    /// `Y·_*` shape: emit single match at leftmost Y start.
    #[allow(dead_code)]
    fn find_all_trailing_star(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut pos = 0;
        while pos < input.len() {
            if let Some(max_end) = inner.fwd.scan_fwd_optional(&mut inner.b, pos, input)? {
                if max_end > pos {
                    return Ok(vec![Match {
                        start: pos,
                        end: max_end,
                    }]);
                }
            }
            pos += 1;
        }
        Ok(vec![])
    }

    fn find_all_dfa(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if self.fwd_end_nullable {
            self.find_all_dfa_inner::<true>(input)
        } else {
            self.find_all_dfa_inner::<false>(input)
        }
    }

    fn find_all_dfa_inner<const FWD_NULL: bool>(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.nulls.clear();
        inner.matches.clear();

        if self.always_nullable && !self.hardened {
            if self.star_loop {
                let RegexInner {
                    ref mut b,
                    ref mut fwd,
                    ref mut matches,
                    ..
                } = *inner;
                let len = input.len();
                let mut start = 0usize;
                while start < len {
                    let end = fwd.scan_fwd_optional(b, start, input)?.expect(
                        "always-nullable pattern matches (at least empty) at every position",
                    );
                    if end > start {
                        matches.push(Match { start, end });
                        start = end;
                    } else {
                        matches.push(Match { start, end: start });
                        start += 1;
                    }
                }
                push_end_zero_width(matches, len);
                return Ok(matches.clone());
            }
            let RegexInner {
                ref mut b,
                ref mut fwd,
                ref mut matches,
                ref mut fas,
                ..
            } = *inner;
            let fas = fas.as_mut().expect("fas initialized for always_nullable");
            fwd.scan_fwd_active_set::<true>(b, fas, input, &StartPositions::new(), matches)?;
            push_end_zero_width(matches, input.len());
            return Ok(matches.clone());
        }

        if self.initial_nullability.has(Nullability::END) {
            inner.nulls.add(input.len());
        }
        {
            #[cfg(feature = "convergence_prefix")]
            {
                let RegexInner { rev_ts, b, nulls, conv_b, .. } = &mut *inner;
                rev_ts.collect_rev(b, input.len() - 1, input, nulls, conv_b.as_mut())?;
            }
            #[cfg(not(feature = "convergence_prefix"))]
            {
                let RegexInner { rev_ts, b, nulls, .. } = &mut *inner;
                rev_ts.collect_rev(b, input.len() - 1, input, nulls, None)?;
            }
        }

        #[cfg(all(feature = "debug", debug_assertions))]
        eprintln!("[nulls] {:?}", inner.nulls);

        if self.hardened {
            let RegexInner {
                ref mut b,
                ref mut fwd,
                ref mut matches,
                ref mut fas,
                ref nulls,
                ..
            } = *inner;
            let fas = fas.as_mut().unwrap();
            if self.always_nullable {
                fwd.scan_fwd_active_set::<true>(b, fas, input, nulls, matches)?;
                push_end_zero_width(matches, input.len());
            } else {
                fwd.scan_fwd_active_set::<false>(b, fas, input, nulls, matches)?;
            }
            return Ok(matches.clone());
        }

        if let Some(fl) = self.fixed_length {
            let fl = fl as usize;
            let mut last_end = 0;
            for start in inner.nulls.positions_asc() {
                if start >= last_end  {
                    inner.matches.push(Match {
                        start,
                        end: start + fl,
                    });
                    last_end = start + fl;
                }
            }
        } else {
            inner
                .fwd
                .scan_fwd_all(&mut inner.b, &inner.nulls, input, &mut inner.matches)?;
        }

        if self.always_nullable {
            inner.matches.push(Match {
                start: input.len(),
                end: input.len(),
            });
        }

        Ok(inner.matches.clone())
    }

    /// longest match anchored at position 0.
    ///
    /// returns `None` if the pattern does not match at position 0.
    pub fn find_anchored(&self, input: &[u8]) -> Result<Option<Match>, Error> {
        if input.is_empty() {
            return Ok(self.empty_input_match());
        }
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if self.fwd_lb_stripped {
            // ugly scenario for find_anchored, easier to reject it than to special case it
            return Err(Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern))
        }
        Ok(inner.fwd.scan_fwd_optional(&mut inner.b, 0, input)?.map(|end| Match { start: 0, end }))
    }

    /// longest match anchored at the string end (`\z`).
    ///
    /// returns `None` if the pattern does not match ending at the string end.
    pub(crate) fn find_end_anchored(&self, input: &[u8]) -> Result<Option<Match>, Error> {
        debug_assert!(!input.is_empty());
        let len = input.len();
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let RegexInner { b, rev, .. } = &mut *inner;
        let rev_dfa = rev.as_mut().expect(
            "find_end_anchored requires the _*-free reverse DFA, built whenever rev_end_anchored holds (the same condition that selects FindAll::EndAnchored)",
        );
        Ok(rev_dfa
            .scan_rev_from(b, len, 0, input)?
            .map(|start| Match { start, end: len }))
    }

}

