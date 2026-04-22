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
//! use [`EngineOptions`] with [`Regex::with_options`] for non-default settings:
//!
//! ```
//! use resharp::{Regex, EngineOptions};
//!
//! let re = Regex::with_options(
//!     r"hello world",
//!     EngineOptions::default()
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

pub(crate) mod accel;
pub(crate) mod engine;
pub(crate) mod prefix;

pub(crate) mod simd;

#[doc(hidden)]
pub fn has_simd() -> bool {
    simd::has_simd()
}

#[cfg(feature = "diag")]
pub use engine::BDFA;
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
    Parse(Box<resharp_parser::ResharpError>),
    /// algebra error (unsupported pattern, anchor limit).
    Algebra(resharp_algebra::AlgebraError),
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

impl From<resharp_parser::ResharpError> for Error {
    fn from(e: resharp_parser::ResharpError) -> Self {
        Error::Parse(Box::new(e))
    }
}

impl From<resharp_algebra::AlgebraError> for Error {
    fn from(e: resharp_algebra::AlgebraError) -> Self {
        Error::Algebra(e)
    }
}

/// configuration for pattern compilation and engine behavior.
///
/// all options have sensible defaults via [`Default`]. use the builder
/// methods to override:
///
/// ```
/// use resharp::EngineOptions;
///
/// let opts = EngineOptions::default()
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
    /// and any code points requiring 3- or 4-byte UTF-8 sequences.
    Full,
    /// ASCII `\w`/`\d`/`\s`, but `.`, `[^...]`, `\W`/`\D`/`\S` match one full
    /// UTF-8 codepoint. Matches default JS `RegExp` behavior (no `u` flag).
    Javascript,
}

/// Engine configuration, passed to [`Regex::with_options`].
pub struct EngineOptions {
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

impl Default for EngineOptions {
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

impl EngineOptions {
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
}

/// Lazily compiled regex instance.
/// Uses Mutex for interior mutability.
pub struct Regex {
    pub(crate) inner: Mutex<RegexInner>,
    pub(crate) prefix: Option<prefix::PrefixKind>,
    pub(crate) fixed_length: Option<u32>,
    pub(crate) max_length: Option<u32>,
    pub(crate) empty_nullable: bool,
    pub(crate) always_nullable: bool,
    /// rev === _*, skip rev pass entirely
    pub(crate) rev_trivial: bool,
    pub(crate) initial_nullability: Nullability,
    pub(crate) fwd_end_nullable: bool,
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
    let opener = opener_class(b, start);
    if opener == TSetId::EMPTY || !b.solver().is_full_id(opener) {
        return false;
    }
    let Some(graph) = build_partial_graph(b, start, NODE_BUDGET) else { return false };
    let always = resharp_algebra::nulls::Nullability::ALWAYS;
    for (i, &n) in graph.nodes.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if n.nullability(b) == resharp_algebra::nulls::Nullability::NEVER {
            continue;
        }
        let mut total = TSetId::EMPTY;
        let mut ok = true;
        for e in &graph.edges[i] {
            total = b.solver().or_id(total, e.set);
            if e.dst != i && graph.nodes[e.dst].nullability(b) != always {
                ok = false;
                break;
            }
        }
        if ok && b.solver().is_full_id(total) {
            return false;
        }
    }
    if !has_anchors
        && graph.edges[0].len() == 1
        && b.solver().is_full_id(graph.edges[0][0].set)
    {
        return false;
    }
    for scc in tarjan_sccs(&graph) {
        let non_trivial =
            scc.len() > 1 || graph.edges[scc[0]].iter().any(|e| e.dst == scc[0]);
        if !non_trivial {
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

fn tarjan_sccs(graph: &Graph) -> Vec<Vec<usize>> {
    let n = graph.edges.len();
    let mut index = vec![u32::MAX; n];
    let mut lowlink = vec![0u32; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut sccs: Vec<Vec<usize>> = Vec::new();
    let mut next_index: u32 = 0;
    let mut dfs: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if index[root] != u32::MAX {
            continue;
        }
        index[root] = next_index;
        lowlink[root] = next_index;
        next_index += 1;
        stack.push(root);
        on_stack[root] = true;
        dfs.push((root, 0));
        while let Some(&(u, ei)) = dfs.last() {
            if let Some(edge) = graph.edges[u].get(ei) {
                dfs.last_mut().unwrap().1 += 1;
                let v = edge.dst;
                if index[v] == u32::MAX {
                    index[v] = next_index;
                    lowlink[v] = next_index;
                    next_index += 1;
                    stack.push(v);
                    on_stack[v] = true;
                    dfs.push((v, 0));
                } else if on_stack[v] {
                    lowlink[u] = lowlink[u].min(index[v]);
                }
            } else {
                dfs.pop();
                if let Some(&(p, _)) = dfs.last() {
                    lowlink[p] = lowlink[p].min(lowlink[u]);
                }
                if lowlink[u] == index[u] {
                    let mut comp = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        comp.push(w);
                        if w == u {
                            break;
                        }
                    }
                    sccs.push(comp);
                }
            }
        }
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
) -> Result<(), resharp_algebra::AlgebraError> {
    if !node.contains_lookaround(b) {
        return Ok(());
    }
    match b.get_kind(node) {
        Kind::Union => {
            let (l, r) = (node.left(b), node.right(b));
            if l.contains_lookbehind(b) || r.contains_lookbehind(b) {
                return Err(resharp_algebra::AlgebraError::UnsupportedPattern);
            }
            Ok(())
        }
        Kind::Concat | Kind::Inter => {
            ensure_supported(b, node.left(b))?;
            ensure_supported(b, node.right(b))
        }
        Kind::Star => {
            if node.left(b).contains_lookaround(b) {
                return Err(resharp_algebra::AlgebraError::UnsupportedPattern);
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
        Self::with_options(pattern, EngineOptions::default())
    }

    /// compile a pattern with custom [`EngineOptions`].
    ///
    /// ```
    /// use resharp::{Regex, EngineOptions};
    ///
    /// let re = Regex::with_options(
    ///     r"hello",
    ///     EngineOptions::default().case_insensitive(true),
    /// ).unwrap();
    /// assert!(re.is_match(b"HELLO").unwrap());
    /// ```
    pub fn with_options(pattern: &str, opts: EngineOptions) -> Result<Regex, Error> {
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
    pub fn from_node(b: RegexBuilder, node: NodeId, opts: EngineOptions) -> Result<Regex, Error> {
        Self::from_node_inner(b, node, opts, 0)
    }

    fn from_node_inner(
        mut b: RegexBuilder,
        node: NodeId,
        opts: EngineOptions,
        pattern_len: usize,
    ) -> Result<Regex, Error> {
        // Guard against pathological AST sizes (deep recursion in reverse /
        // der / normalize would otherwise stack-overflow on debug builds,
        // which default to a 2 MiB thread stack under `cargo test`).
        const NODE_LIMIT: usize = 200_000;
        if b.tree_size(node, NODE_LIMIT) >= NODE_LIMIT {
            return Err(Error::PatternTooLarge);
        }
        ensure_supported(&mut b, node)?;

        let empty_nullable = b
            .nullability_emptystring(node)
            .has(Nullability::EMPTYSTRING);
        let always_nullable = b.nullability(node) == Nullability::ALWAYS;
        let initial_nullability = b.nullability(node);

        let fwd_start = b.strip_lb(node)?;
        let fwd_end_nullable = b.nullability(fwd_start).has(Nullability::END);
        let rev_start = b.reverse(node)?;
        let rev_start = b.normalize_rev(rev_start)?;
        let ts_rev_start =
            if b.get_kind(rev_start) == Kind::Concat && rev_start.left(&b) == NodeId::BEGIN {
                rev_start
            } else {
                b.mk_concat(NodeId::TS, rev_start)
            };
        let ts_rev_start = b.simplify_rev_initial(ts_rev_start);
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
            prefix::select_prefix(&mut b, node, rev_start, has_look, min_len)?;

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

        let mut rev = engine::LDFA::new(&mut b, ts_rev_start, max_cap)?;
        rev.prefix_skip = rev_skip;

        let (fwd_lb_begin_nullable, lb_check_bytes) =
            if matches!(selected, Some(prefix::PrefixKind::AnchoredFwdLb(_))) {
                let lb = node.left(&b);
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
                rev.precompile(&mut b, opts.dfa_threshold);
            }
        }

        let rev_bare = if fwd_prefix_stripped {
            Some(engine::LDFA::new(&mut b, rev_start, max_cap)?)
        } else {
            None
        };
        let use_bounded = !has_fwd_prefix
            && max_length.is_some()
            && max_len <= 100
            && fixed_length.is_none()
            && !has_look
            && !b.contains_anchors(node)
            && pattern_len <= 150
            && !empty_nullable;

        if cfg!(feature = "debug") {
            eprintln!(
                "  [bounded-check] max_length={:?} fixed_length={:?} has_look={} anchors={} fwd_prefix={} -> use={}",
                max_length, fixed_length, has_look, b.contains_anchors(node), selected.is_some(), use_bounded
            );
        }

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
        }

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b,
                fwd,
                rev_ts: rev,
                rev_bare,
                nulls: Vec::new(),
                matches: Vec::new(),
                bounded,
            }),
            prefix: selected,
            fixed_length,
            max_length,
            empty_nullable,
            always_nullable,
            rev_trivial,
            initial_nullability,
            fwd_end_nullable,
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
    pub fn max_length(&self) -> Option<u32> {
        self.max_length
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
        if input.is_empty() {
            return if self.empty_nullable {
                Ok(vec![Match { start: 0, end: 0 }])
            } else {
                Ok(vec![])
            };
        }
        if self.hardened {
            if self.has_bounded_prefix || self.has_bounded {
                return self.find_all_fwd_bounded(input);
            }
            return self.find_all_dfa(input);
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
        if self.has_bounded_prefix {
            return self.find_all_fwd_bounded(input);
        }
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
            _ => {}
        }
        if self.has_bounded {
            return self.find_all_fwd_bounded(input);
        }
        self.find_all_dfa(input)
    }

    #[allow(missing_docs)]
    pub fn bfs_tails_dump(&self, max_depth: u32) -> String {
        use resharp_algebra::{Kind, NodeId};
        use std::collections::HashSet;
        let inner = &mut *self.inner.lock().unwrap();
        let b = &mut inner.b;
        let ts = &mut inner.rev_ts;
        let num_mt = ts.minterms.len() as u32;
        let mut seed: HashSet<u16> = HashSet::new();
        for mt in 0..num_mt {
            let s = ts.begin_table[mt as usize];
            if s > engine::DFA_DEAD {
                ts.create_state(b, s).ok();
                seed.insert(s);
            }
        }
        let mut seen: HashSet<u16> = seed.clone();
        let mut states_at_depth: Vec<Vec<u16>> = vec![seed.into_iter().collect()];
        for _ in 1..=max_depth {
            let mut next = Vec::new();
            for &s in states_at_depth.last().unwrap() {
                for mt in 0..num_mt {
                    let ns = ts.lazy_transition(b, s, mt).unwrap_or(engine::DFA_DEAD);
                    if ns > engine::DFA_DEAD && seen.insert(ns) {
                        next.push(ns);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            states_at_depth.push(next);
        }
        fn flatten_union(b: &resharp_algebra::RegexBuilder, n: NodeId, out: &mut Vec<NodeId>) {
            if b.get_kind(n) == Kind::Union {
                flatten_union(b, n.left(b), out);
                flatten_union(b, n.right(b), out);
            } else {
                out.push(n);
            }
        }
        fn strip_star(b: &resharp_algebra::RegexBuilder, mut n: NodeId) -> NodeId {
            loop {
                if b.get_kind(n) != Kind::Concat {
                    return n;
                }
                let lk = b.get_kind(n.left(b));
                if lk == Kind::Star || lk == Kind::Compl {
                    n = n.right(b);
                    continue;
                }
                return n;
            }
        }
        let mut out = String::new();
        for (d, states) in states_at_depth.iter().enumerate() {
            for &s in states {
                let node = ts.state_nodes[s as usize];
                let mut branches = Vec::new();
                flatten_union(b, node, &mut branches);
                out.push_str(&format!(
                    "  depth={} state={} {} branches:\n",
                    d,
                    s,
                    branches.len()
                ));
                for br in branches {
                    let t = strip_star(b, br);
                    out.push_str(&format!("    tail: {}\n", b.pp(t)));
                }
            }
        }
        out
    }

    #[allow(missing_docs)]
    pub fn find_convergence_node(&self, max_depth: u32) -> Option<(String, u32)> {
        let (node, depth) = self.find_convergence_node_id(max_depth)?;
        let inner = &*self.inner.lock().unwrap();
        Some((inner.b.pp(node), depth))
    }

    #[allow(missing_docs)]
    pub fn find_convergence_node_id(
        &self,
        max_depth: u32,
    ) -> Option<(resharp_algebra::NodeId, u32)> {
        use resharp_algebra::{Kind, NodeId};
        use std::collections::{HashMap, HashSet};
        let inner = &mut *self.inner.lock().unwrap();
        let b = &mut inner.b;
        let ts = &mut inner.rev_ts;
        let num_mt = ts.minterms.len() as u32;
        let mut seed: HashSet<u16> = HashSet::new();
        for mt in 0..num_mt {
            let s = ts.begin_table[mt as usize];
            if s > engine::DFA_DEAD {
                ts.create_state(b, s).ok();
                seed.insert(s);
            }
        }
        if seed.is_empty() {
            return None;
        }
        let mut seen: HashSet<u16> = seed.clone();
        let mut states_at_depth: Vec<Vec<u16>> = vec![seed.into_iter().collect()];
        for _ in 1..=max_depth {
            let mut next = Vec::new();
            for &s in states_at_depth.last().unwrap() {
                for mt in 0..num_mt {
                    let ns = ts.lazy_transition(b, s, mt).unwrap_or(engine::DFA_DEAD);
                    if ns > engine::DFA_DEAD && seen.insert(ns) {
                        next.push(ns);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            states_at_depth.push(next);
        }

        fn flatten_union(b: &resharp_algebra::RegexBuilder, n: NodeId, out: &mut Vec<NodeId>) {
            if b.get_kind(n) == Kind::Union {
                flatten_union(b, n.left(b), out);
                flatten_union(b, n.right(b), out);
            } else {
                out.push(n);
            }
        }
        fn strip_star_prefix(b: &resharp_algebra::RegexBuilder, mut n: NodeId) -> NodeId {
            loop {
                if b.get_kind(n) != Kind::Concat {
                    return n;
                }
                let lk = b.get_kind(n.left(b));
                if lk == Kind::Star || lk == Kind::Compl {
                    n = n.right(b);
                    continue;
                }
                return n;
            }
        }

        let mut tails_per_state: HashMap<u16, HashSet<NodeId>> = HashMap::new();
        for states in &states_at_depth {
            for &s in states {
                let node = ts.state_nodes[s as usize];
                let mut branches = Vec::new();
                flatten_union(b, node, &mut branches);
                let mut tails: HashSet<NodeId> = HashSet::new();
                for br in branches {
                    tails.insert(strip_star_prefix(b, br));
                }
                tails_per_state.insert(s, tails);
            }
        }

        fn has_leading_lb(b: &resharp_algebra::RegexBuilder, n: NodeId) -> bool {
            if b.get_kind(n) != Kind::Concat {
                return false;
            }
            matches!(b.get_kind(n.left(b)), Kind::Lookbehind | Kind::Compl)
        }

        let mut all_tails: HashSet<NodeId> = HashSet::new();
        for ts_set in tails_per_state.values() {
            for &t in ts_set {
                if t == NodeId::BOT || t == NodeId::MISSING || t == NodeId::EPS {
                    continue;
                }
                all_tails.insert(t);
            }
        }

        let max_d = states_at_depth.len() - 1;
        let mut best: Option<(bool, u32, usize, NodeId)> = None;
        for t in all_tails {
            let mut peel: Option<u32> = None;
            for p in 0..=max_d {
                let ok = (p..=max_d).all(|d| {
                    states_at_depth[d]
                        .iter()
                        .all(|s| tails_per_state[&s].contains(&t))
                });
                if ok {
                    peel = Some(p as u32);
                    break;
                }
            }
            let Some(p) = peel else { continue };
            let clean = !has_leading_lb(b, t);
            let len = b.pp(t).len();
            let key = (!clean, p, len, t);
            if best.as_ref().map_or(true, |cur| {
                (cur.0, cur.1, cur.2, cur.3) > (key.0, key.1, key.2, key.3)
            }) {
                best = Some(key);
            }
        }
        let (_dirty, p, _len, node) = best?;
        Some((node, p))
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

    fn find_all_dfa(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if self.fwd_end_nullable {
            self.find_all_dfa_inner::<true>(input)
        } else {
            self.find_all_dfa_inner::<false>(input)
        }
    }

    fn find_all_dfa_inner<const FWD_NULL: bool>(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
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
            eprintln!("nulls_buf={:?}", &inner.nulls[..inner.nulls.len().min(10)]);
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
        } else if self.hardened {
            if cfg!(feature = "debug") {
                eprintln!("  [dispatch] scan_fwd_ordered");
            }
            inner.fwd.scan_fwd_ordered(
                &mut inner.b,
                &inner.nulls,
                input,
                self.max_length,
                &mut inner.matches,
            )?;
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
            // Try position 0 via the begins table, which handles \A anchors
            // that the SIMD prefix search may skip.
            {
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

        // \A: lb contains \A so a match is possible at position 0 without any preceding lb
        // byte. Handle this explicitly via begin_table before the SIMD scan.
        if self.fwd_lb_begin_nullable && !input.is_empty() {
            let state = inner.fwd.walk_input(&mut inner.b, 0, 1, input)?;
            if state != 0 {
                let max_end = inner.fwd.scan_fwd_from(&mut inner.b, state, 1, input)?;
                if max_end != engine::NO_MATCH {
                    inner.matches.push(Match {
                        start: 0,
                        end: max_end,
                    });
                    search_start = max_end;
                }
            }
        }

        // SIMD scan for lb bytes. Only valid when in the initial/pruned DFA state,
        // which is always the case here (scan_fwd_from fully processes each candidate).
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
        if input.is_empty() {
            return Ok(self.empty_nullable);
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
