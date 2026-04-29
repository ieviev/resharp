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
    target_arch = "aarch64",
    all(target_arch = "wasm32", target_feature = "simd128")
)))]
compile_error!(
    "resharp requires a SIMD-capable target: x86_64, aarch64, or wasm32 with target_feature=simd128"
);

pub(crate) mod accel;
pub(crate) mod engine;
pub(crate) mod fas;
pub(crate) mod prefix;

pub(crate) mod simd;

#[doc(hidden)]
pub fn has_simd() -> bool {
    simd::has_simd()
}

#[cfg(feature = "diag")]
pub use prefix::calc_potential_start;
#[cfg(feature = "diag")]
pub use prefix::calc_potential_start_prune;
#[cfg(feature = "diag")]
pub use prefix::calc_prefix_sets;
#[cfg(feature = "diag")]
pub use prefix::PrefixSets;
pub(crate) use resharp_algebra::solver::TSetId;
use resharp_algebra::Kind;
use rustc_hash::FxHashMap;

// bdfa_scan / bdfa_inner const-generic PREFIX modes
const PREFIX_NONE: u8 = 0;
const PREFIX_SEARCH: u8 = 1;
const PREFIX_LITERAL: u8 = 2;

pub use resharp_algebra::nulls::Nullability;
pub use resharp_algebra::NodeId;
pub use resharp_algebra::RegexBuilder;
pub use resharp_algebra::TRegexId;
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
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "parse error: {}", e),
            Error::Algebra(e) => write!(f, "{}", e),
            Error::CapacityExceeded => write!(f, "DFA state capacity exceeded"),
            Error::PatternTooLarge => write!(f, "pattern too large"),
            Error::Serialize(ref s) => write!(f, "serialization error: {}", s),
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
/// Controls which Unicode character tables `\w` and `\d` use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnicodeMode {
    /// `\w` = `[a-zA-Z0-9_]`, `\d` = `[0-9]`. `.` and
    /// bracketed-class negation step byte-by-byte. Fastest
    Ascii,
    /// Default: covers major scripts up through U+07FF (Latin, Greek, Cyrillic,
    /// Hebrew, Arabic, ...). All encoded as 1- or 2-byte UTF-8 sequences.
    #[default]
    Default,
    /// All Unicode word/digit characters, including CJK, historic scripts,
    /// and any code points requiring 3- or 4-byte UTF-8 sequences. `.` and
    /// negated bracket classes match one full UTF-8 codepoint.
    Full,
    /// ASCII `\w`/`\d`/`\s`, but `.`, `[^...]`, `\W`/`\D`/`\S` match one full
    /// UTF-8 codepoint. Matches default JS `RegExp` behavior (no `u` flag).
    Javascript,
}

/// Regex configuration, passed to [`Regex::with_options`].
pub struct RegexOptions {
    /// states to eagerly precompile (0 = fully lazy).
    pub dfa_threshold: usize,
    /// max cached DFA states; clamped to `u16::MAX`.
    pub max_dfa_capacity: usize,
    /// max lookahead context distance (default: 800).
    pub lookahead_context_max: u32,
    /// Unicode coverage for `\w` and `\d` (default: `UnicodeMode::Unicode`).
    pub unicode: UnicodeMode,
    /// global case-insensitive matching (default: false).
    pub case_insensitive: bool,
    /// `.` matches `\n` (default: false). `_` always matches any byte.
    pub dot_matches_new_line: bool,
    /// allow whitespace and `#` comments in the pattern (default: false).
    pub ignore_whitespace: bool,
    /// use O(N·S) hardened forward scan (default: false).
    /// prevents quadratic blowup on adversarial pattern+input combinations.
    pub hardened: bool,
}

impl Default for RegexOptions {
    fn default() -> Self {
        Self {
            dfa_threshold: 0,
            max_dfa_capacity: u16::MAX as usize,
            lookahead_context_max: 800,
            unicode: UnicodeMode::Default,
            case_insensitive: false,
            dot_matches_new_line: false,
            ignore_whitespace: false,
            hardened: false,
        }
    }
}

impl RegexOptions {
    /// set Unicode coverage for `\w` and `\d`.
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
    pub(crate) fwd: engine::LDFA,
    pub(crate) rev_ts: engine::LDFA,
    pub(crate) rev_bare: Option<engine::LDFA>,
    pub(crate) nulls: Vec<usize>,
    pub(crate) matches: Vec<Match>,
    pub(crate) bounded: Option<engine::BDFA>,
    /// experimental forward dfa for hardened mode
    pub(crate) fas: Option<fas::FwdDFA>,
}

/// Lazily compiled regex instance.
/// Uses Mutex for interior mutability.
pub struct Regex {
    pub(crate) inner: Mutex<RegexInner>,
    pub(crate) prefix: Option<prefix::PrefixKind>,
    pub(crate) fixed_length: Option<u32>,
    pub(crate) empty_nullable: bool,
    pub(crate) always_nullable: bool,
    /// node === ⊥
    /// found to be trivially unmatchable, not guaranteed before full expansion
    pub(crate) is_empty_lang: bool,
    pub(crate) fwd_begin_anchored: bool,
    /// rev === _*, skip rev pass entirely
    pub(crate) rev_trivial: bool,
    pub(crate) initial_nullability: Nullability,
    pub(crate) fwd_end_nullable: bool,
    /// `Y·_*` shape: at most one match. skip rev+fwd.
    pub(crate) trailing_star_anchored_left: bool,
    pub(crate) trailing_star_branch_left: bool,
    pub(crate) hardened: bool,
    pub(crate) has_bounded_prefix: bool,
    pub(crate) has_bounded: bool,
    pub(crate) lb_check_bytes: u8,
    pub(crate) fwd_lb_begin_nullable: bool,
    pub(crate) has_anchors: bool,
}

#[inline(never)]
fn bdfa_inner<const PREFIX: u8>(
    table: *const u32,
    ml: *const u8,
    data: *const u8,
    mt_log: u32,
    initial: u16,
    match_end_off: *const u32,
    mut state: u16,
    mut pos: usize,
    len: usize,
    match_buf: *mut Match,
    match_cap: usize,
) -> (u16, usize, usize) {
    let mut mc: usize = 0;
    unsafe {
        while pos < len {
            if PREFIX != PREFIX_NONE && state == initial {
                return (state, pos, mc);
            }
            let mt = *ml.add(*data.add(pos) as usize) as usize;
            let delta = (state as usize) << mt_log | mt;
            let entry = *table.add(delta);
            if entry == 0 {
                return (state, pos, mc);
            }
            let rel = entry >> 16;
            state = (entry & 0xFFFF) as u16;
            if rel > 0 {
                if mc >= match_cap {
                    return (state, pos, mc);
                }
                let end_off = *match_end_off.add(state as usize);
                let end = pos + 1 - end_off as usize;
                *match_buf.add(mc) = Match {
                    start: pos + 1 - rel as usize,
                    end,
                };
                mc += 1;
                state = initial;
                pos = end;
                continue;
            }
            pos += 1;
        }
        (state, pos, mc)
    }
}

// not a security measure. only flags obvious cases where hardening results in better performance
fn auto_harden(b: &mut RegexBuilder, start: NodeId, has_anchors: bool) -> bool {
    const NODE_BUDGET: usize = 128;
    const LARGE_COVER: u32 = 128;
    let opener = opener_class(b, start);
    if opener == TSetId::EMPTY {
        return false;
    }
    let opener_full = b.solver().is_full_id(opener);
    let Some(graph) = build_partial_graph(b, start, NODE_BUDGET) else {
        return false;
    };
    if graph
        .nodes
        .iter()
        .any(|&n| b.get_kind(n) == resharp_algebra::Kind::Compl)
    {
        return false;
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
        return false;
    }
    if !opener_full && b.solver().byte_count(opener) < LARGE_COVER {
        return false;
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
            return true;
        }
        let start_to_i = graph.edges[0]
            .iter()
            .filter(|e| e.dst == i)
            .fold(TSetId::EMPTY, |acc, e| b.solver().or_id(acc, e.set));
        let entry_wide = b.solver().byte_count(start_to_i) >= ENTRY_BYTES;
        if !has_anchors && min_len <= SHORT_PREFIX && entry_wide {
            return true;
        }
    }
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
        if !sticky {
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
        if start_branches || scc_branches {
            return true;
        }
    }
    false
}

struct Edge {
    dst: usize,
    set: TSetId,
}

struct Graph {
    edges: Vec<Vec<Edge>>,
    nodes: Vec<NodeId>,
}

fn build_partial_graph(b: &mut RegexBuilder, start: NodeId, budget: usize) -> Option<Graph> {
    use std::collections::HashMap;
    let mut idx: HashMap<NodeId, usize> = HashMap::from([(start, 0)]);
    let mut edges: Vec<Vec<Edge>> = vec![Vec::new()];
    let mut nodes: Vec<NodeId> = vec![start];
    let mut queue: Vec<(usize, NodeId)> = vec![(0, start)];
    let mut overflow = false;
    while let Some((u, node)) = queue.pop() {
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

/// rejects obviously unsupported before compiling
fn ensure_supported(
    b: &mut RegexBuilder,
    node: NodeId,
) -> Result<(), resharp_algebra::ResharpError> {
    if !node.contains_lookaround(b) {
        return Ok(());
    }
    match b.get_kind(node) {
        Kind::Union => {
            let (l, r) = (node.left(b), node.right(b));
            if l.contains_lookbehind(b) || r.contains_lookbehind(b) {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            Ok(())
        }
        Kind::Concat | Kind::Inter => {
            ensure_supported(b, node.left(b))?;
            ensure_supported(b, node.right(b))
        }
        Kind::Star => {
            if node.left(b).contains_lookaround(b) {
                return Err(resharp_algebra::ResharpError::UnsupportedPattern);
            }
            ensure_supported(b, node.left(b))
        }
        Kind::Counted => ensure_supported(b, node.left(b)),
        Kind::Compl => ensure_supported(b, node.left(b)),
        Kind::Lookbehind | Kind::Lookahead => {
            ensure_supported(b, node.left(b))?;
            ensure_supported(b, node.right(b))
        }
        Kind::Pred => Ok(()),
        Kind::Begin => Ok(()),
        Kind::End => Ok(()),
    }
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
            ignore_whitespace: opts.ignore_whitespace,
        };
        let node = resharp_parser::parse_ast_with(&mut b, pattern, &pflags)?;
        Self::from_node_inner(b, node, opts, pattern.len())
    }

    /// build from a pre-constructed AST node.
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
        const NODE_LIMIT: usize = 200_000;
        if b.tree_size(node, NODE_LIMIT) >= NODE_LIMIT {
            return Err(Error::PatternTooLarge);
        }
        ensure_supported(&mut b, node)?;

        let empty_nullable = b
            .nullability_emptystring(node)
            .has(Nullability::EMPTYSTRING);
        let initial_nullability = b.nullability(node);

        let node = b.simplify_fwd_initial(node);
        let fwd_start = b.strip_lb(node)?;
        let fwd_end_nullable = b.nullability(fwd_start).has(Nullability::END);
        let mut shape_memo: FxHashMap<NodeId, NodeId> = FxHashMap::default();
        let fwd_shape = b.prune_fwd(fwd_start, &mut shape_memo);
        let lb_stripped = fwd_start != node;
        let trailing_star_anchored_left =
            !lb_stripped && b.ends_with_ts(fwd_shape) && !b.starts_with_ts(fwd_shape);
        let trailing_star_branch_left = !lb_stripped
            && !trailing_star_anchored_left
            && b.ends_with_ts_any_branch(fwd_shape)
            && !b.starts_with_ts(fwd_shape);
        let ts_rev_start = b.ts_rev_start(node)?;
        // TODO: make it configurable to actually check and reject empty lang entriely
        let is_empty_lang = node == NodeId::BOT;
        let fwd_begin_anchored = node == NodeId::BEGIN
            || (b.get_kind(node) == resharp_algebra::Kind::Concat
                && node.left(&b) == NodeId::BEGIN);
        #[cfg(feature = "debug")]
        eprintln!(
            "  [rev] ts_rev_start_after_simplify: {:.50}",
            b.pp(ts_rev_start)
        );
        let rev_trivial = b.nullability(ts_rev_start) == Nullability::ALWAYS;
        let fixed_length = b.get_fixed_length(node);
        let (min_len, max_len) = b.get_min_max_length(node);
        let max_length = if max_len != u32::MAX {
            Some(max_len)
        } else {
            None
        };
        let has_look = b.contains_look(node);

        let max_cap = opts.max_dfa_capacity.min(u16::MAX as usize);

        let (selected, rev_skip) =
            prefix::select_prefix(&mut b, node, ts_rev_start, has_look, min_len, max_cap)?;

        let has_fwd_prefix = matches!(
            selected,
            Some(
                prefix::PrefixKind::AnchoredFwd(_)
                    | prefix::PrefixKind::UnanchoredFwd(_)
                    | prefix::PrefixKind::AnchoredFwdLb(_)
            )
        );
        let fwd_prefix_stripped = matches!(selected, Some(prefix::PrefixKind::UnanchoredFwd(_)));
        // default to hardened when the default dispatch would be pathological
        let mut opts = opts;
        let has_anchors_pre = b.contains_anchors(node);
        if !opts.hardened && auto_harden(&mut b, fwd_start, has_anchors_pre) {
            opts.hardened = true;
        }
        let anchored_fwd = matches!(
            selected,
            Some(prefix::PrefixKind::AnchoredFwd(_) | prefix::PrefixKind::AnchoredFwdLb(_))
        );
        let needs_full_fwd = opts.hardened || anchored_fwd;
        let mut fwd = if needs_full_fwd {
            engine::LDFA::new(&mut b, fwd_start, max_cap)?
        } else {
            engine::LDFA::new_fwd(&mut b, fwd_start, max_cap)?
        };

        let mut rev_ts = engine::LDFA::new(&mut b, ts_rev_start, max_cap)?;
        rev_ts.prefix_skip = rev_skip;
        rev_ts.ensure_pruned_skip();

        let (fwd_lb_begin_nullable, lb_check_bytes) =
            if matches!(selected, Some(prefix::PrefixKind::AnchoredFwdLb(_))) {
                let lb_inner = b.get_lookbehind_inner(node.left(&b));
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
                let lb_fixed = b
                    .get_fixed_length(lb_stripped)
                    .expect("AnchoredFwdLb requires fixed-length lb");
                let begin_nullable = b.nullability(lb_inner).has(Nullability::BEGIN);
                (begin_nullable, lb_fixed as u8)
            } else {
                (false, 0)
            };

        if opts.dfa_threshold > 0 {
            fwd.precompile(&mut b, opts.dfa_threshold);
            if !has_fwd_prefix {
                rev_ts.precompile(&mut b, opts.dfa_threshold);
            }
        }

        let rev_bare = if fwd_prefix_stripped {
            Some(engine::LDFA::new(&mut b, ts_rev_start, max_cap)?)
        } else {
            None
        };

        // lots of conditions when something else is better.. possibly removing it entirely
        let use_bounded = !has_fwd_prefix
            && max_length.is_some()
            && max_len <= 100
            && fixed_length.is_none()
            && !has_look
            && !b.contains_anchors(node)
            && pattern_len <= 150
            && !empty_nullable;

        let bounded = if use_bounded {
            Some(engine::BDFA::new(&mut b, fwd_start)?)
        } else {
            None
        };

        let has_bounded = bounded.is_some();
        let has_bounded_prefix = bounded
            .as_ref()
            .is_some_and(|bd: &crate::engine::BDFA| bd.prefix.is_some());

        let has_anchors = b.contains_anchors(node);

        let hardened = if opts.hardened && !has_bounded && fixed_length.is_none() && max_cap >= 64 {
            fwd.has_nonnullable_cycle(&mut b, 256)
        } else {
            false
        };

        if cfg!(feature = "debug") {
            eprintln!("  [fwd] {:.50}", b.pp(fwd_start));
            eprintln!("  [rev] {:.50}", b.pp(ts_rev_start));
            eprintln!("  [hardened] {:.50}", hardened);
            eprintln!("  [pre] {:.50}", 1);
        }

        let fas = if hardened {
            // operate on fwd_start: the fwd DFA never sees the leading lookbehind
            // (strip_lb), and FAS lives on top of that DFA.
            let x = fas::FwdDFA::new(&fwd, fwd_start.contains_lookahead(&b));
            Some(x)
        } else {
            None
        };

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b,
                fwd,
                rev_ts,
                rev_bare,
                nulls: Vec::new(),
                matches: Vec::new(),
                bounded,
                fas,
            }),
            prefix: selected,
            fixed_length,
            empty_nullable,
            always_nullable: initial_nullability == Nullability::ALWAYS,
            is_empty_lang,
            fwd_begin_anchored,
            rev_trivial,
            initial_nullability,
            fwd_end_nullable,
            trailing_star_anchored_left,
            trailing_star_branch_left,
            hardened,
            has_bounded_prefix,
            has_bounded,
            lb_check_bytes,
            fwd_lb_begin_nullable,
            has_anchors,
        })
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn node_count(&self) -> u32 {
        self.inner.lock().unwrap().b.num_nodes()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn dfa_stats(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap();
        (inner.fwd.state_nodes.len(), inner.rev_ts.state_nodes.len())
    }

    /// whether hardened linear-scan mode is enabled
    pub fn is_hardened(&self) -> bool {
        self.hardened
    }

    /// whether the pattern is forward-begin-anchored (`^`/`\A`)
    pub fn is_fwd_begin_anchored(&self) -> bool {
        self.fwd_begin_anchored
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn bdfa_stats(&self) -> Option<(usize, usize, usize)> {
        let inner = self.inner.lock().unwrap();
        inner
            .bounded
            .as_ref()
            .map(|b| (b.states.len(), 1usize << b.mt_log, b.prefix_len))
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn prefix_kind_name(&self) -> Option<&'static str> {
        match &self.prefix {
            None => None,
            Some(prefix::PrefixKind::AnchoredFwd(_)) => Some("AnchoredFwd"),
            Some(prefix::PrefixKind::UnanchoredFwd(_)) => Some("UnanchoredFwd"),
            Some(prefix::PrefixKind::AnchoredFwdLb(_)) => Some("AnchoredFwdLb"),
            Some(prefix::PrefixKind::AnchoredRev) => Some("AnchoredRev"),
            Some(prefix::PrefixKind::PotentialStart) => Some("PotentialStart"),
        }
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn fwd_prefix_kind(&self) -> Option<(&'static str, usize)> {
        match &self.prefix {
            Some(prefix::PrefixKind::AnchoredFwd(fp))
            | Some(prefix::PrefixKind::UnanchoredFwd(fp))
            | Some(prefix::PrefixKind::AnchoredFwdLb(fp)) => Some((fp.variant_name(), fp.len())),
            _ => None,
        }
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn has_accel(&self) -> (bool, bool) {
        let inner = self.inner.lock().unwrap();
        let fwd = self.prefix.as_ref().is_some_and(|p| p.is_fwd());
        let rev = self.prefix.as_ref().is_some_and(|p| p.is_rev())
            || inner.rev_ts.prefix_skip.is_some()
            || inner.rev_ts.can_skip();
        (fwd, rev)
    }

    /// all non-overlapping leftmost-first matches as `[start, end)` byte ranges.
    ///
    /// ```
    /// let re = resharp::Regex::new(r"\d+").unwrap();
    /// let m = re.find_all(b"abc 123 def 456").unwrap();
    /// assert_eq!(m.len(), 2);
    /// assert_eq!((m[0].start, m[0].end), (4, 7));
    /// ```
    pub fn find_all(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if self.is_empty_lang {
            return Ok(vec![]);
        }
        if input.is_empty() {
            return if self.empty_nullable {
                Ok(vec![Match { start: 0, end: 0 }])
            } else {
                Ok(vec![])
            };
        }
        if self.fwd_begin_anchored {
            return Ok(self.find_anchored(input)?.into_iter().collect());
        }
        if self.hardened {
            if self.has_bounded_prefix || self.has_bounded {
                return self.find_all_fwd_bounded(input);
            }
            return self.find_all_dfa(input);
        }
        // `Y·_*` shape: single match `(leftmost Y start, input.len())`.
        // Skip trailing-star path if Y has a usable fwd prefix (SIMD wins).
        let has_fwd_prefix = matches!(
            &self.prefix,
            Some(
                prefix::PrefixKind::AnchoredFwd(_)
                    | prefix::PrefixKind::UnanchoredFwd(_)
                    | prefix::PrefixKind::AnchoredFwdLb(_)
            )
        );
        if self.trailing_star_anchored_left && !has_fwd_prefix {
            return self.find_all_trailing_star(input);
        }
        // Some-branch trailing `_*`: probe leftmost match, fall back if not saturating.
        if self.trailing_star_branch_left && !has_fwd_prefix {
            if let Some(out) = self.find_all_trailing_star_probe(input)? {
                return Ok(out);
            }
        }
        #[cfg(all(feature = "debug", debug_assertions))]
        {
            let pre_kind = match &self.prefix {
                None => "None",
                Some(prefix::PrefixKind::AnchoredRev) => "AnchoredRev",
                Some(prefix::PrefixKind::AnchoredFwd(_)) => "AnchoredFwd",
                Some(prefix::PrefixKind::UnanchoredFwd(_)) => "UnanchoredFwd",
                Some(prefix::PrefixKind::AnchoredFwdLb(_)) => "AnchoredFwdLb",
                Some(prefix::PrefixKind::PotentialStart) => "PotentialStart",
            };
            eprintln!(
                "[algorithm] pre={}, bound={}, end-null={}",
                pre_kind, self.has_bounded, self.fwd_end_nullable
            );
        }
        // Prefix selection already chose; honour it. Bounded BDFA only wins with no prefix.
        match &self.prefix {
            Some(prefix::PrefixKind::UnanchoredFwd(_)) => {
                return self.find_all_fwd_prefix_stripped(input);
            }
            Some(prefix::PrefixKind::AnchoredFwd(_)) => {
                return self.find_all_fwd_prefix(input);
            }
            Some(prefix::PrefixKind::AnchoredFwdLb(_)) => {
                return self.find_all_fwd_lb_prefix(input);
            }
            Some(prefix::PrefixKind::AnchoredRev | prefix::PrefixKind::PotentialStart) => {
                return self.find_all_dfa(input);
            }
            None => {}
        }
        if self.has_bounded {
            return self.find_all_fwd_bounded(input);
        }
        self.find_all_dfa(input)
    }
}

#[cfg(feature = "convergence_prefix")]
pub(crate) fn find_strict_convergence_node(
    b: &mut resharp_algebra::RegexBuilder,
    ts: &mut engine::LDFA,
    rev_start: resharp_algebra::NodeId,
    max_depth: u32,
) -> Option<(resharp_algebra::NodeId, u32)> {
    use resharp_algebra::{Kind, NodeId};
    use std::collections::HashSet;

    // strip leading `_*` skip + `\A`
    let stripped = b.nonbegins(rev_start);
    let stripped = b.strip_prefix_safe(stripped);
    if stripped == NodeId::BOT {
        return None;
    }
    let (min_len, _) = b.get_min_max_length(stripped);
    if min_len == 0 {
        return None;
    }
    let stripped_sid = ts.get_or_register(b, stripped);
    if stripped_sid <= engine::DFA_DEAD {
        return None;
    }
    ts.ensure_capacity(stripped_sid);
    if ts.create_state(b, stripped_sid).is_err() {
        return None;
    }
    let num_mt = ts.minterms.len() as u32;
    let mut frontier: HashSet<u16> = HashSet::new();
    frontier.insert(stripped_sid);

    /// Flatten `n` into `Concat(Pred, TAIL)` leaves.
    fn collect_pred_leaves(
        b: &mut resharp_algebra::RegexBuilder,
        n: NodeId,
        out: &mut Vec<(NodeId, NodeId)>,
    ) -> bool {
        let n = b.nonbegins(n);
        if n == NodeId::BOT {
            return true;
        }
        match b.get_kind(n) {
            Kind::Union => {
                collect_pred_leaves(b, n.left(b), out) && collect_pred_leaves(b, n.right(b), out)
            }
            Kind::Pred => {
                out.push((n, NodeId::EPS));
                true
            }
            Kind::Concat => {
                let head = n.left(b);
                let tail = n.right(b);
                match b.get_kind(head) {
                    Kind::Pred => {
                        out.push((head, tail));
                        true
                    }
                    Kind::Star | Kind::Compl => collect_pred_leaves(b, tail, out),
                    Kind::Union => {
                        let l = b.mk_concat(head.left(b), tail);
                        let r = b.mk_concat(head.right(b), tail);
                        collect_pred_leaves(b, l, out) && collect_pred_leaves(b, r, out)
                    }
                    Kind::Concat => {
                        let inner_l = head.left(b);
                        let inner_r = head.right(b);
                        let new_tail = b.mk_concat(inner_r, tail);
                        let flat = b.mk_concat(inner_l, new_tail);
                        collect_pred_leaves(b, flat, out)
                    }
                    _ if b.any_nonbegin_nullable(head) => collect_pred_leaves(b, tail, out),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    let max_depth = max_depth.min(min_len.saturating_sub(1));
    for depth in 0..=max_depth {
        let mut common_tail: Option<NodeId> = None;
        let mut pred_union: Option<NodeId> = None;
        let mut ok = true;
        'state_loop: for &s in &frontier {
            let node = ts.state_nodes[s as usize];
            let mut leaves: Vec<(NodeId, NodeId)> = Vec::new();
            if !collect_pred_leaves(b, node, &mut leaves) || leaves.is_empty() {
                ok = false;
                break 'state_loop;
            }
            for (head, tail) in leaves {
                match common_tail {
                    None => common_tail = Some(tail),
                    Some(t) if t == tail => {}
                    _ => {
                        ok = false;
                        break 'state_loop;
                    }
                }
                pred_union = Some(match pred_union {
                    None => head,
                    Some(p) => b.mk_union(p, head),
                });
            }
        }
        if ok {
            if let (Some(head), Some(tail)) = (pred_union, common_tail) {
                let synth = b.mk_concat(head, tail);
                return Some((synth, depth));
            }
        }
        // Advance BFS one step.
        if depth == max_depth {
            break;
        }
        let mut next: HashSet<u16> = HashSet::new();
        for &s in &frontier {
            for mt in 0..num_mt {
                let ns = ts.lazy_transition(b, s, mt).unwrap_or(engine::DFA_DEAD);
                if ns > engine::DFA_DEAD {
                    next.insert(ns);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        frontier = next;
    }
    None
}

impl Regex {
    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn rev_state_dump(&self) -> String {
        let inner = &mut *self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
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
        let inner = &mut *self.inner.lock().unwrap();
        inner.nulls.clear();
        inner
            .rev_ts
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls)
            .unwrap();
        inner.nulls.clone()
    }

    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn scan_fwd_debug(&self, input: &[u8], pos: usize) -> usize {
        let inner = &mut *self.inner.lock().unwrap();
        inner.fwd.scan_fwd_slow(&mut inner.b, pos, input).unwrap()
    }

    /// Walk RTL step by step, printing the rev-DFA state and its nulls
    /// metadata at each position. Returns the trace as a string.
    #[cfg(feature = "diag")]
    #[allow(missing_docs)]
    pub fn rev_walk_trace(&self, input: &[u8]) -> String {
        use std::fmt::Write;
        let inner = &mut *self.inner.lock().unwrap();
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
            if sid as u32 <= engine::DFA_DEAD as u32 {
                break;
            }
        }
        out
    }

    #[cfg(feature = "diag")]
    fn dump_state(
        out: &mut String,
        b: &mut resharp_algebra::RegexBuilder,
        rev: &engine::LDFA,
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

    /// `Y·_*` shape: emit single match at leftmost Y start.
    fn find_all_trailing_star(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        let mut pos = 0;
        while pos < input.len() {
            let max_end = inner.fwd.scan_fwd_slow(&mut inner.b, pos, input)?;
            if max_end != engine::NO_MATCH && max_end > pos {
                return Ok(vec![Match {
                    start: pos,
                    end: max_end,
                }]);
            }
            pos += 1;
        }
        Ok(vec![])
    }

    fn find_all_trailing_star_probe(&self, input: &[u8]) -> Result<Option<Vec<Match>>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        let mut pos = 0;
        while pos < input.len() {
            let max_end = inner.fwd.scan_fwd_slow(&mut inner.b, pos, input)?;
            if max_end != engine::NO_MATCH && max_end > pos {
                if max_end == input.len() {
                    return Ok(Some(vec![Match {
                        start: pos,
                        end: max_end,
                    }]));
                }
                return Ok(None);
            }
            pos += 1;
        }
        Ok(Some(vec![]))
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
        let inner = &mut *self.inner.lock().unwrap();
        inner.nulls.clear();
        inner.matches.clear();

        if (self.always_nullable || self.rev_trivial) && !self.hardened {
            Self::find_all_nullable_slow(&mut inner.fwd, &mut inner.b, input, &mut inner.matches)?;
            return Ok(inner.matches.clone());
        }

        if self.initial_nullability.has(Nullability::END) {
            inner.nulls.push(input.len());
        }
        inner
            .rev_ts
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls)?;

        #[cfg(feature = "debug")]
        {
            eprintln!("nulls_after_rev={}", &inner.nulls.len());
        }

        // FAS fast path: only spawn matches at begin positions discovered by rev pass
        // (unless the pattern is always-nullable, in which case every position is valid).
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
                // FAS now backfills empty matches at uncovered always-nullable
                // positions including data_end; only push if not already there.
                if matches.last().map(|m| m.start) != Some(input.len()) {
                    matches.push(Match {
                        start: input.len(),
                        end: input.len(),
                    });
                }
            } else {
                fwd.scan_fwd_active_set::<false>(b, fas, input, nulls, matches)?;
            }
            return Ok(matches.clone());
        }

        if let Some(fl) = self.fixed_length {
            let fl = fl as usize;
            let mut last_end = 0;
            for &start in inner.nulls.iter().rev() {
                if start >= last_end && start + fl <= input.len() {
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

    fn find_all_nullable_slow(
        fwd: &mut engine::LDFA,
        b: &mut RegexBuilder,
        input: &[u8],
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let mut pos = 0;
        while pos < input.len() {
            let max_end = fwd.scan_fwd_slow(b, pos, input)?;
            if max_end != engine::NO_MATCH && max_end > pos {
                matches.push(Match {
                    start: pos,
                    end: max_end,
                });
                pos = max_end;
            } else if max_end != engine::NO_MATCH {
                matches.push(Match {
                    start: pos,
                    end: pos,
                });
                pos += 1;
            } else {
                pos += 1;
            }
        }
        // trailing empty at end-of-input
        let end_null = engine::has_any_null(
            &fwd.effects_id,
            &fwd.effects,
            engine::DFA_INITIAL as u32,
            Nullability::END,
        );
        if end_null {
            matches.push(Match {
                start: input.len(),
                end: input.len(),
            });
        }
        Ok(())
    }

    fn find_all_fwd_bounded(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let RegexInner {
            b,
            bounded,
            matches: matches_buf,
            ..
        } = &mut *self.inner.lock().unwrap();
        let bounded = bounded.as_mut().unwrap();
        matches_buf.clear();
        match &bounded.prefix {
            Some(p) if p.is_literal() => {
                Self::bdfa_scan::<{ PREFIX_LITERAL }, false>(bounded, b, input, matches_buf)?;
            }
            Some(_) => {
                Self::bdfa_scan::<{ PREFIX_SEARCH }, false>(bounded, b, input, matches_buf)?;
            }
            None => {
                Self::bdfa_scan::<{ PREFIX_NONE }, false>(bounded, b, input, matches_buf)?;
            }
        }
        Ok(matches_buf.clone())
    }

    fn is_match_fwd_bounded(&self, input: &[u8]) -> Result<bool, Error> {
        let RegexInner {
            b,
            bounded,
            matches: matches_buf,
            ..
        } = &mut *self.inner.lock().unwrap();
        let bounded = bounded.as_mut().unwrap();
        matches_buf.clear();
        let found = match &bounded.prefix {
            Some(p) if p.is_literal() => {
                Self::bdfa_scan::<{ PREFIX_LITERAL }, true>(bounded, b, input, matches_buf)?
            }
            Some(_) => Self::bdfa_scan::<{ PREFIX_SEARCH }, true>(bounded, b, input, matches_buf)?,
            None => Self::bdfa_scan::<{ PREFIX_NONE }, true>(bounded, b, input, matches_buf)?,
        };
        Ok(found)
    }

    fn bdfa_scan<const PREFIX: u8, const ISMATCH: bool>(
        bounded: &mut engine::BDFA,
        b: &mut RegexBuilder,
        input: &[u8],
        matches: &mut Vec<Match>,
    ) -> Result<bool, Error> {
        let initial = bounded.initial;
        let mt_log = bounded.mt_log;
        let ml = bounded.minterms_lookup;
        let len = input.len();
        let mut state = initial;
        let mut pos: usize = 0;

        if PREFIX == PREFIX_NONE {
            let data = input.as_ptr();
            if !ISMATCH {
                matches.reserve(2048);
            }
            loop {
                if !ISMATCH && matches.len() == matches.capacity() {
                    matches.reserve(matches.capacity().max(256));
                }
                let spare = if ISMATCH {
                    1
                } else {
                    matches.capacity() - matches.len()
                };
                let buf_ptr = unsafe { matches.as_mut_ptr().add(matches.len()) };
                let table = bounded.table.as_ptr();
                let meo = bounded.match_end_off.as_ptr();
                let (s, p, mc) = bdfa_inner::<{ PREFIX_NONE }>(
                    table,
                    ml.as_ptr(),
                    data,
                    mt_log,
                    initial,
                    meo,
                    state,
                    pos,
                    len,
                    buf_ptr,
                    spare,
                );
                state = s;
                pos = p;
                if ISMATCH && mc > 0 {
                    return Ok(true);
                }
                unsafe { matches.set_len(matches.len() + mc) };
                if pos >= len {
                    break;
                }
                let mt = ml[input[pos] as usize] as usize;
                let entry = bounded.transition(b, state, mt)?;
                state = (entry & 0xFFFF) as u16;
                let rel = entry >> 16;
                if rel > 0 {
                    if ISMATCH {
                        return Ok(true);
                    }
                    let end_off = bounded.match_end_off[state as usize];
                    matches.push(Match {
                        start: pos + 1 - rel as usize,
                        end: pos + 1 - end_off as usize,
                    });
                    state = initial;
                } else {
                    pos += 1;
                }
            }
        } else {
            // PREFIX_SEARCH / PREFIX_LITERAL
            'main: loop {
                if pos >= len {
                    break;
                }

                if state == initial {
                    let found = bounded.prefix.as_ref().unwrap().find_fwd(input, pos);
                    match found {
                        Some(p) => {
                            if PREFIX == PREFIX_LITERAL {
                                pos = p + bounded.prefix_len;
                                state = bounded.after_prefix;
                            } else {
                                pos = p;
                                for _ in 0..bounded.prefix_len {
                                    if pos >= len {
                                        break;
                                    }
                                    let mt = ml[input[pos] as usize] as usize;
                                    let delta = (state as usize) << mt_log | mt;
                                    let entry = bounded.table[delta];
                                    let entry = if entry != 0 {
                                        entry
                                    } else {
                                        bounded.transition(b, state, mt)?
                                    };
                                    state = (entry & 0xFFFF) as u16;
                                    if state == initial {
                                        break;
                                    }
                                    pos += 1;
                                }
                            }
                            let rel = bounded.match_rel[state as usize];
                            if rel > 0 {
                                if ISMATCH {
                                    return Ok(true);
                                }
                                let end_off = bounded.match_end_off[state as usize];
                                matches.push(Match {
                                    start: pos - rel as usize + 1,
                                    end: pos - end_off as usize + 1,
                                });
                                state = initial;
                            }
                            continue 'main;
                        }
                        None => break 'main,
                    }
                }

                unsafe {
                    let table = bounded.table.as_ptr();
                    let data = input.as_ptr();
                    let ml_ptr = ml.as_ptr();
                    let meo = bounded.match_end_off.as_ptr();

                    while pos < len {
                        let mt = *ml_ptr.add(*data.add(pos) as usize) as usize;
                        let delta = (state as usize) << mt_log | mt;
                        let entry = *table.add(delta);
                        if entry == 0 {
                            break;
                        }
                        let rel = entry >> 16;
                        state = (entry & 0xFFFF) as u16;
                        if state == initial {
                            continue 'main;
                        }
                        if rel > 0 {
                            if ISMATCH {
                                return Ok(true);
                            }
                            let end_off = *meo.add(state as usize);
                            matches.push(Match {
                                start: pos + 1 - rel as usize,
                                end: pos + 1 - end_off as usize,
                            });
                            state = initial;
                            continue 'main;
                        }
                        pos += 1;
                    }
                }

                if pos >= len {
                    break;
                }
                let mt = ml[input[pos] as usize] as usize;
                let entry = bounded.transition(b, state, mt)?;
                state = (entry & 0xFFFF) as u16;
                let rel = entry >> 16;
                if rel > 0 {
                    if ISMATCH {
                        return Ok(true);
                    }
                    let end_off = bounded.match_end_off[state as usize];
                    let end = pos + 1 - end_off as usize;
                    matches.push(Match {
                        start: pos + 1 - rel as usize,
                        end,
                    });
                    state = initial;
                    pos = end;
                } else {
                    pos += 1;
                }
            }
        }

        if state != initial {
            let node = bounded.states[state as usize];
            if node != NodeId::MISSING {
                // walk chain: find best match among all chain nodes
                let mut best_val = 0u32;
                let mut best_step = 0u32;
                let mut cur = node;
                while cur.0 > NodeId::BOT.0 {
                    let packed = b.get_extra(cur);
                    let step = packed & 0xFFFF;
                    let best = packed >> 16;
                    if best > best_val {
                        best_val = best;
                        best_step = step;
                    }
                    cur = cur.right(b);
                }
                if best_val > 0 {
                    if ISMATCH {
                        return Ok(true);
                    }
                    matches.push(Match {
                        start: len - best_step as usize,
                        end: len - best_step as usize + best_val as usize,
                    });
                }
            }
        }

        Ok(false)
    }

    fn find_all_fwd_prefix(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.prefix.as_ref().and_then(|p| p.fwd_search()).unwrap();
        let inner = &mut *self.inner.lock().unwrap();
        let matches = &mut inner.matches;
        matches.clear();
        let mut search_start = 0;

        if self.fixed_length == Some(fwd_prefix.len() as u32)
            && !self.has_anchors
            && fwd_prefix.find_all_literal(input, matches)
        {
        } else {
            {
                // special case for pos 0 with \A anchors
                let mt = inner.fwd.mt_lookup[input[0] as usize];
                let state = inner.fwd.begin_table[mt as usize] as u32;
                if state != inner.fwd.pruned as u32 {
                    let max_end = inner.fwd.scan_fwd_from(&mut inner.b, state, 1, input)?;
                    if max_end != engine::NO_MATCH && max_end > 0 {
                        matches.push(Match {
                            start: 0,
                            end: max_end,
                        });
                        search_start = max_end;
                    }
                }
            }
            let prefix_len = fwd_prefix.len();
            while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
                let state = inner
                    .fwd
                    .walk_input(&mut inner.b, candidate, prefix_len, input)?;
                if state != 0 {
                    let max_end = inner.fwd.scan_fwd_from(
                        &mut inner.b,
                        state,
                        candidate + prefix_len,
                        input,
                    )?;
                    if max_end != engine::NO_MATCH && max_end > candidate {
                        matches.push(Match {
                            start: candidate,
                            end: max_end,
                        });
                        search_start = max_end;
                        continue;
                    }
                }
                search_start = candidate + 1;
            }
        }

        Ok(matches.clone())
    }

    fn find_all_fwd_prefix_stripped(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.prefix.as_ref().and_then(|p| p.fwd_search()).unwrap();
        let inner = &mut *self.inner.lock().unwrap();
        let prefix_len = fwd_prefix.len();
        inner.fwd.create_state(&mut inner.b, engine::DFA_INITIAL)?;
        inner.matches.clear();
        let mut search_start = 0;

        while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
            // 1. confirm match end via fwd DFA
            let mut state = engine::DFA_INITIAL;
            for i in 0..prefix_len {
                let mt = inner.fwd.mt_lookup[input[candidate + i] as usize] as u32;
                state = inner.fwd.lazy_transition(&mut inner.b, state, mt)?;
                if state == engine::DFA_DEAD {
                    break;
                }
            }
            if state == engine::DFA_DEAD {
                search_start = candidate + 1;
                continue;
            }
            let max_end = inner.fwd.scan_fwd_from(
                &mut inner.b,
                state as u32,
                candidate + prefix_len,
                input,
            )?;
            if max_end == engine::NO_MATCH {
                search_start = candidate + 1;
                continue;
            }

            // 2. find match start via bare rev DFA from confirmed end
            let rev_bare = inner.rev_bare.as_mut().unwrap();
            let match_start = rev_bare.scan_rev_from(&mut inner.b, max_end, search_start, input)?;
            if match_start != engine::NO_MATCH {
                inner.matches.push(Match {
                    start: match_start,
                    end: max_end,
                });
                search_start = max_end;
            } else {
                search_start = candidate + 1;
            }
        }
        Ok(inner.matches.clone())
    }

    fn find_all_fwd_lb_prefix(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.prefix.as_ref().and_then(|p| p.fwd_search()).unwrap();
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        let lb_len = self.lb_check_bytes as usize;
        let mut search_start = 0usize;

        if self.fwd_lb_begin_nullable && !input.is_empty() {
            let max_end = inner.fwd.scan_fwd_slow(&mut inner.b, 0, input)?;
            if max_end != engine::NO_MATCH {
                inner.matches.push(Match {
                    start: 0,
                    end: max_end,
                });
                search_start = if max_end == 0 { 1 } else { max_end };
            }
        }

        while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
            let body_start = candidate + lb_len;
            let max_end = inner.fwd.scan_fwd_from(
                &mut inner.b,
                engine::DFA_INITIAL as u32,
                body_start,
                input,
            )?;
            if max_end != engine::NO_MATCH {
                inner.matches.push(Match {
                    start: body_start,
                    end: max_end,
                });
                search_start = max_end;
            } else {
                search_start = body_start;
            }
        }

        Ok(inner.matches.clone())
    }

    /// longest match anchored at position 0.
    ///
    /// returns `None` if the pattern does not match at position 0.
    pub fn find_anchored(&self, input: &[u8]) -> Result<Option<Match>, Error> {
        if self.is_empty_lang {
            return Ok(None);
        }
        if input.is_empty() {
            return if self.empty_nullable {
                Ok(Some(Match { start: 0, end: 0 }))
            } else {
                Ok(None)
            };
        }
        let inner = &mut *self.inner.lock().unwrap();
        let max_end = inner.fwd.scan_fwd_slow(&mut inner.b, 0, input)?;
        if max_end != engine::NO_MATCH {
            Ok(Some(Match {
                start: 0,
                end: max_end,
            }))
        } else {
            Ok(None)
        }
    }

    /// whether the pattern matches anywhere in the input.
    ///
    /// faster than `find_all` when you only need a yes/no answer.
    pub fn is_match(&self, input: &[u8]) -> Result<bool, Error> {
        if self.is_empty_lang {
            return Ok(false);
        }
        if input.is_empty() {
            return Ok(self.empty_nullable);
        }
        if self.fwd_begin_anchored {
            return Ok(self.find_anchored(input)?.is_some());
        }
        if self.has_bounded {
            return self.is_match_fwd_bounded(input);
        }
        if let Some(fwd_prefix) = self.prefix.as_ref().and_then(|p| p.fwd_search()) {
            let inner = &mut *self.inner.lock().unwrap();
            let prefix_len = fwd_prefix.len();
            let mut search_start = 0;
            while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
                let state = inner
                    .fwd
                    .walk_input(&mut inner.b, candidate, prefix_len, input)?;
                if state != 0 {
                    let max_end = inner.fwd.scan_fwd_from(
                        &mut inner.b,
                        state,
                        candidate + prefix_len,
                        input,
                    )?;
                    if max_end != engine::NO_MATCH && max_end > candidate {
                        return Ok(true);
                    }
                }
                search_start = candidate + 1;
            }
            return Ok(false);
        }
        let inner = &mut *self.inner.lock().unwrap();
        if inner.rev_ts.effects_id[engine::DFA_INITIAL as usize] != 0 {
            return Ok(true);
        }
        inner.nulls.clear();
        inner
            .rev_ts
            .collect_rev_first(&mut inner.b, input.len() - 1, input, &mut inner.nulls)?;
        Ok(!inner.nulls.is_empty())
    }
}
