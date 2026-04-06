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
#[cfg(feature = "diag")]
pub use resharp_algebra::solver::TSetId;
use resharp_algebra::Kind;

// bdfa_scan / bdfa_inner const-generic PREFIX modes
const PREFIX_NONE: u8 = 0;
const PREFIX_SEARCH: u8 = 1;
const PREFIX_LITERAL: u8 = 2;

pub use resharp_algebra::nulls::Nullability;
pub use resharp_algebra::NodeId;
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
    Parse(Box<resharp_parser::ResharpError>),
    /// algebra error (unsupported pattern, anchor limit).
    Algebra(resharp_algebra::AlgebraError),
    /// DFA state cache exceeded `max_dfa_capacity`.
    CapacityExceeded,
    /// serialization or deserialization failure.
    Serialize(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "parse error: {}", e),
            Error::Algebra(e) => write!(f, "{}", e),
            Error::CapacityExceeded => write!(f, "DFA state capacity exceeded"),
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
    /// ASCII only: `\w` = `[a-zA-Z0-9_]`, `\d` = `[0-9]`.
    Ascii,
    /// Default: covers major scripts up through U+07FF (Latin, Greek, Cyrillic,
    /// Hebrew, Arabic, ...). All encoded as 1- or 2-byte UTF-8 sequences.
    #[default]
    Unicode,
    /// All Unicode word/digit characters, including CJK, historic scripts,
    /// and any code points requiring 3- or 4-byte UTF-8 sequences.
    Full,
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
            unicode: UnicodeMode::Unicode,
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
    pub(crate) fwd_end_nullable: bool,
    pub(crate) hardened: bool,
    pub(crate) has_bounded_prefix: bool,
    pub(crate) has_bounded: bool,
    /// Number of lb bytes baked into the AnchoredFwdLb SIMD prefix.
    /// match.start = simd_candidate + lb_check_bytes.
    pub(crate) lb_check_bytes: u8,
    /// True when the lb contains \A: a match at position 0 is possible
    /// and must be tried via begin_table before the SIMD scan.
    pub(crate) fwd_lb_begin_nullable: bool,
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
                *match_buf.add(mc) = Match {
                    start: pos + 1 - rel as usize,
                    end: pos + 1 - end_off as usize,
                };
                mc += 1;
                state = initial;
                continue;
            }
            pos += 1;
        }
        (state, pos, mc)
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
        let empty_nullable = b
            .nullability_emptystring(node)
            .has(Nullability::EMPTYSTRING);

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

        let mut fwd = engine::LDFA::new(&mut b, fwd_start, max_cap)?;
        let mut rev = engine::LDFA::new(&mut b, ts_rev_start, max_cap)?;
        rev.prefix_skip = rev_skip;

        let (fwd_lb_begin_nullable, lb_check_bytes) =
            if matches!(selected, Some(prefix::PrefixKind::AnchoredFwdLb(_))) {
                let lb = node.left(&b);
                let lb_inner = b.get_lookbehind_inner(lb);
                let lb_nonbegin = b.nonbegins(lb_inner);
                let lb_stripped = b.strip_prefix_safe(lb_nonbegin);
                let (_, lb_max) = b.get_min_max_length(lb_stripped);
                let begin_nullable = b.nullability(lb_inner).has(Nullability::BEGIN);
                (begin_nullable, lb_max.min(4) as u8)
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
        if fwd_prefix_stripped {
            fwd.compute_fwd_skip(&mut b);
        } else if !opts.hardened && pattern_len <= 150 {
            fwd.compute_fwd_skip_inner(&mut b, 10);
        }

        let use_bounded = !has_fwd_prefix
            && max_length.is_some()
            && max_len <= 100
            && fixed_length.is_none()
            && !has_look
            && !b.contains_anchors(node)
            && pattern_len <= 150;

        if cfg!(feature = "debug-nulls") {
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

        let hardened = if opts.hardened && !has_bounded && fixed_length.is_none() && max_cap >= 64 {
            fwd.has_nonnullable_cycle(&mut b, 256)
        } else {
            false
        };

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
            fwd_end_nullable,
            hardened,
            has_bounded_prefix,
            has_bounded,
            lb_check_bytes,
            fwd_lb_begin_nullable,
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
    pub fn dump_fwd_dfa(&self) {
        let inner = self.inner.lock().unwrap();
        let fwd = &inner.fwd;
        let stride = 1usize << fwd.mt_log;
        eprintln!(
            "fwd: minterms={} states={}",
            fwd.mt_num,
            fwd.state_nodes.len()
        );
        eprintln!(
            "  mt['A']={} mt['a']={} mt['\\n']={} mt[0]={}",
            fwd.mt_lookup[b'A' as usize],
            fwd.mt_lookup[b'a' as usize],
            fwd.mt_lookup[b'\n' as usize],
            fwd.mt_lookup[0]
        );
        for sid in 2..fwd.state_nodes.len() {
            let base = sid * stride;
            if base + stride > fwd.center_table.len() {
                continue;
            }
            let row: Vec<u16> = (0..fwd.mt_num as usize)
                .map(|mt| fwd.center_table[base + mt])
                .collect();
            let nullable = sid < fwd.effects_id.len() && fwd.effects_id[sid] != 0;
            let node = inner.b.pp(fwd.state_nodes[sid]);
            let skip = fwd.skip_ids.get(sid).copied().unwrap_or(0);
            eprintln!(
                "  s{}: null={} skip={} node={} tr={:?}",
                sid, nullable, skip, node, row
            );
        }
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
        #[cfg(all(feature = "debug-nulls", debug_assertions))]
        {
            // eprintln!("prefix: '{:?}'", &self.prefix);
        }

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

    fn find_all_dfa(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if self.fwd_end_nullable {
            self.find_all_dfa_inner::<true>(input)
        } else {
            self.find_all_dfa_inner::<false>(input)
        }
    }

    fn find_all_dfa_inner<const FWD_NULL: bool>(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        #[cfg(feature = "debug-nulls")]
        {
            eprintln!("find_all_dfa_inner:");
            eprintln!(
                "rev0: {}",
                inner
                    .b
                    .pp(inner.rev.state_nodes[engine::DFA_INITIAL as usize])
            );
        }

        let rev_initial_nullable = inner.rev_ts.effects_id[engine::DFA_INITIAL as usize] != 0;
        if rev_initial_nullable {
            inner.matches.clear();
            Self::find_all_nullable_slow(&mut inner.fwd, &mut inner.b, input, &mut inner.matches)?;
            return Ok(inner.matches.clone());
        }

        inner.nulls.clear();

        if rev_initial_nullable {
            inner.nulls.push(input.len());
        }
        inner
            .rev_ts
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls)?;

        inner.matches.clear();
        let matches = &mut inner.matches;
        #[cfg(feature = "debug-nulls")]
        {
            eprintln!("nulls_buf={:?}", inner.nulls);
        }

        if let Some(fl) = self.fixed_length {
            let fl = fl as usize;
            let mut last_end = 0;
            for &start in inner.nulls.iter().rev() {
                if start >= last_end && start + fl <= input.len() {
                    matches.push(Match {
                        start,
                        end: start + fl,
                    });
                    last_end = start + fl;
                }
            }
        } else if self.hardened {
            if cfg!(feature = "debug-nulls") {
                eprintln!("  [dispatch] scan_fwd_ordered");
            }
            inner.fwd.scan_fwd_ordered(
                &mut inner.b,
                &inner.nulls,
                input,
                self.max_length,
                matches,
            )?;
        } else {
            inner
                .fwd
                .scan_fwd_all(&mut inner.b, &inner.nulls, input, self.max_length, matches)?;
        }

        if FWD_NULL
            && inner.nulls.first() == Some(&input.len())
            && matches.last().map_or(true, |m| m.end <= input.len())
        {
            matches.push(Match {
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
        fwd.skip_built = true;
        fwd.build_skip_all(b);
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
                    matches.push(Match {
                        start: pos + 1 - rel as usize,
                        end: pos + 1 - end_off as usize,
                    });
                    state = initial;
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
            && fwd_prefix.find_all_literal(input, matches)
        {
        } else {
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
            #[cfg(feature = "debug-nulls")]
            eprintln!(
                "  [debug-nulls] fwd_prefix candidates={} confirmed={} false_positives={}",
                n_candidates,
                n_confirmed,
                n_candidates.saturating_sub(n_confirmed)
            );
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
        // TODO: very critical of this, likely unnecessary special case, need to measure if it makes any difference
        if let Some(fwd_prefix) = self.prefix.as_ref().and_then(|p| p.fwd_search()) {
            if let Some(fl) = self.fixed_length {
                if fl as usize == fwd_prefix.len() {
                    if let Some(candidate) = fwd_prefix.find_fwd(input, 0) {
                        return Ok(candidate + fl as usize <= input.len());
                    }
                    return Ok(false);
                }
            }
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
