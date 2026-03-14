//! resharp - a regex engine with all boolean operations and lookarounds,
//! powered by symbolic derivatives and lazy DFA construction.

#![deny(missing_docs)]

pub(crate) mod accel;
pub(crate) mod engine;
pub(crate) mod simd;

#[doc(hidden)]
pub use engine::calc_potential_start;
#[doc(hidden)]
pub use engine::calc_prefix_sets;
#[doc(hidden)]
pub use engine::BDFA;
#[doc(hidden)]
pub use resharp_algebra::solver::TSetId;

pub use resharp_algebra::nulls::Nullability;
pub use resharp_algebra::NodeId;
pub use resharp_algebra::RegexBuilder;
pub use resharp_parser::escape;
pub use resharp_parser::escape_into;

use std::sync::Mutex;

/// error from compiling or matching a regex.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// parse failure.
    Parse(resharp_parser::ResharpError),
    /// algebra error (unsupported pattern, anchor limit).
    Algebra(resharp_algebra::AlgebraError),
    /// DFA state cache exceeded `max_dfa_capacity`.
    CapacityExceeded,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "parse error: {}", e),
            Error::Algebra(e) => write!(f, "{}", e),
            Error::CapacityExceeded => write!(f, "DFA state capacity exceeded"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Parse(e) => Some(e),
            Error::Algebra(e) => Some(e),
            Error::CapacityExceeded => None,
        }
    }
}

impl From<resharp_parser::ResharpError> for Error {
    fn from(e: resharp_parser::ResharpError) -> Self {
        Error::Parse(e)
    }
}

impl From<resharp_algebra::AlgebraError> for Error {
    fn from(e: resharp_algebra::AlgebraError) -> Self {
        Error::Algebra(e)
    }
}

/// combined pattern + engine options.
pub struct EngineOptions {
    /// states to eagerly precompile (0 = fully lazy).
    pub dfa_threshold: usize,
    /// max cached DFA states; clamped to `u16::MAX`.
    pub max_dfa_capacity: usize,
    /// max lookahead context distance (default: 800).
    pub lookahead_context_max: u32,
    /// `\w`/`\d`/`\s` match full Unicode (true) or ASCII only (false).
    pub unicode: bool,
    /// global case-insensitive matching.
    pub case_insensitive: bool,
    /// `.` matches `\n` (behaves like `_`).
    pub dot_matches_new_line: bool,
    /// allow whitespace and `#` comments in the pattern.
    pub ignore_whitespace: bool,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            dfa_threshold: 0,
            max_dfa_capacity: u16::MAX as usize,
            lookahead_context_max: 800,
            unicode: true,
            case_insensitive: false,
            dot_matches_new_line: false,
            ignore_whitespace: false,
        }
    }
}

impl EngineOptions {
    /// set unicode mode.
    pub fn unicode(mut self, yes: bool) -> Self { self.unicode = yes; self }
    /// set case-insensitive mode.
    pub fn case_insensitive(mut self, yes: bool) -> Self { self.case_insensitive = yes; self }
    /// set dot-matches-newline mode.
    pub fn dot_matches_new_line(mut self, yes: bool) -> Self { self.dot_matches_new_line = yes; self }
    /// set ignore-whitespace (verbose) mode.
    pub fn ignore_whitespace(mut self, yes: bool) -> Self { self.ignore_whitespace = yes; self }
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

struct RegexInner {
    b: RegexBuilder,
    fwd: engine::LDFA,
    rev: engine::LDFA,
    nulls_buf: Vec<usize>,
    matches_buf: Vec<Match>,
    bounded: Option<engine::BDFA>,
}

/// compiled regex backed by a lazy DFA.
///
/// uses a `Mutex` for mutable DFA state; clone for per-thread matching.
pub struct Regex {
    inner: Mutex<RegexInner>,
    fwd_prefix: Option<accel::FwdPrefixSearch>,
    fwd_prefix_stripped: bool,
    fixed_length: Option<u32>,
    max_length: Option<u32>,
    empty_nullable: bool,
    fwd_end_nullable: bool,
}

#[inline(never)]
fn bdfa_inner<const PREFIX: u8>(
    table: *const u32,
    ml: *const u8,
    data: *const u8,
    mt_log: u32,
    initial: u16,
    mut state: u16,
    mut pos: usize,
    len: usize,
    match_buf: *mut Match,
    match_cap: usize,
) -> (u16, usize, usize) {
    let mut mc: usize = 0;
    unsafe {
        while pos < len {
            if PREFIX > 0 && state == initial {
                return (state, pos, mc);
            }
            let mt = *ml.add(*data.add(pos) as usize) as usize;
            let delta = (state as usize) << mt_log | mt;
            let entry = *table.add(delta);
            if entry == 0 {
                return (state, pos, mc); // cache miss
            }
            let rel = entry >> 16;
            state = (entry & 0xFFFF) as u16;
            if rel > 0 {
                if mc >= match_cap {
                    return (state, pos, mc);
                }
                *match_buf.add(mc) = Match {
                    start: pos - rel as usize,
                    end: pos,
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
    /// compile with default options.
    pub fn new(pattern: &str) -> Result<Regex, Error> {
        Self::with_options(pattern, EngineOptions::default())
    }

    /// compile with custom options.
    pub fn with_options(pattern: &str, opts: EngineOptions) -> Result<Regex, Error> {
        let mut b = RegexBuilder::new();
        b.lookahead_context_max = opts.lookahead_context_max;
        let pflags = resharp_parser::PatternFlags {
            unicode: opts.unicode,
            case_insensitive: opts.case_insensitive,
            dot_matches_new_line: opts.dot_matches_new_line,
            ignore_whitespace: opts.ignore_whitespace,
        };
        let node = resharp_parser::parse_ast_with(&mut b, pattern, &pflags)?;
        Self::from_node(b, node, opts)
    }

    /// build from a pre-constructed AST node.
    pub fn from_node(
        mut b: RegexBuilder,
        node: NodeId,
        opts: EngineOptions,
    ) -> Result<Regex, Error> {
        let empty_nullable = b
            .nullability_emptystring(node)
            .has(Nullability::EMPTYSTRING);

        let fwd_start = b.strip_lb(node)?;
        let fwd_end_nullable = b.nullability(fwd_start).has(Nullability::END);
        let rev_start = b.reverse(node)?;
        let ts_rev_start = b.mk_concat(NodeId::TS, rev_start);

        let fixed_length = b.get_fixed_length(node);
        let (min_len, max_len) = b.get_min_max_length(node);
        let max_length = if max_len != u32::MAX {
            Some(max_len)
        } else {
            None
        };
        let has_look = b.contains_look(node);

        let max_cap = opts.max_dfa_capacity.min(u16::MAX as usize);
        let mut fwd = engine::LDFA::new(&mut b, fwd_start, max_cap)?;
        let mut rev = engine::LDFA::new(&mut b, ts_rev_start, max_cap)?;

        if opts.dfa_threshold > 0 {
            fwd.precompile(&mut b, opts.dfa_threshold);
            rev.precompile(&mut b, opts.dfa_threshold);
        }

        let (fwd_prefix, fwd_prefix_stripped) = if min_len > 0 && !has_look {
            let (fp, stripped) = engine::build_fwd_prefix(&mut b, node)?;
            let is_literal = matches!(&fp, Some(crate::accel::FwdPrefixSearch::Literal(_)));
            if !stripped && !is_literal && b.is_infinite(node) {
                let strict = engine::build_strict_literal_prefix(&mut b, node)?;
                (strict, false)
            } else {
                (fp, stripped)
            }
        } else {
            (None, false)
        };

        rev.compute_skip(&mut b, rev_start)?;

        if fwd_prefix_stripped {
            fwd.compute_fwd_skip(&mut b);
        }

        let use_bounded = max_length.is_some()
            && fixed_length.is_none()
            && !has_look
            && !b.contains_anchors(node) // TBD: handle anchors in BDFA
            && b.num_nodes() < 5000; // rev+fwd is better for large regexes

        if cfg!(feature = "debug-nulls") {
            eprintln!(
                "  [bounded-check] max_length={:?} fixed_length={:?} has_look={} anchors={} fwd_prefix={} -> use={}",
                max_length, fixed_length, has_look, b.contains_anchors(node), fwd_prefix.is_some(), use_bounded
            );
        }

        let bounded = if use_bounded {
            Some(engine::BDFA::new(&mut b, fwd_start)?)
        } else {
            None
        };

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b,
                fwd,
                rev,
                nulls_buf: Vec::new(),
                matches_buf: Vec::new(),
                bounded,
            }),
            fwd_prefix,
            fwd_prefix_stripped,
            fixed_length,
            max_length,
            empty_nullable,
            fwd_end_nullable,
        })
    }

    /// number of algebra nodes created during compilation.
    pub fn node_count(&self) -> u32 {
        self.inner.lock().unwrap().b.num_nodes()
    }

    /// (fwd_states, rev_states) count.
    pub fn dfa_stats(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap();
        (inner.fwd.state_nodes.len(), inner.rev.state_nodes.len())
    }

    /// BDFA stats: (states, minterms, prefix_len) if BDFA is active.
    pub fn bdfa_stats(&self) -> Option<(usize, usize, usize)> {
        let inner = self.inner.lock().unwrap();
        inner.bounded.as_ref().map(|b| (b.states.len(), b.num_mt, b.prefix_len))
    }

    /// whether forward prefix or reverse skip acceleration is active.
    pub fn has_accel(&self) -> (bool, bool) {
        let inner = self.inner.lock().unwrap();
        let fwd = self.fwd_prefix.is_some();
        let rev = inner.rev.prefix_skip.is_some() || inner.rev.can_skip();
        (fwd, rev)
    }

    /// all non-overlapping matches, left-to-right.
    pub fn find_all(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if input.is_empty() {
            return if self.empty_nullable {
                Ok(vec![Match { start: 0, end: 0 }])
            } else {
                Ok(vec![])
            };
        }
        // 1. bounded + fwd prefix → BDFA with prefix skip
        {
            let inner = self.inner.lock().unwrap();
            if let Some(ref bd) = inner.bounded {
                if bd.prefix.is_some() {
                    drop(inner);
                    return self.find_all_fwd_bounded(input);
                }
            }
        }
        // 2. rare literal fwd prefix → left-right
        if self.fwd_prefix.is_some() {
            if self.fwd_prefix_stripped {
                return self.find_all_fwd_prefix_stripped(input);
            }
            return self.find_all_fwd_prefix(input);
        }
        // 3. rev prefix exists → right-left (standard DFA)
        {
            let inner = self.inner.lock().unwrap();
            let has_rev_accel = inner.rev.prefix_skip.is_some() || inner.rev.can_skip();
            if has_rev_accel {
                drop(inner);
                return self.find_all_dfa(input);
            }
        }
        // 4. bounded, no prefixes → BDFA
        {
            let inner = self.inner.lock().unwrap();
            if inner.bounded.is_some() {
                drop(inner);
                return self.find_all_fwd_bounded(input);
            }
        }
        // 5. fallback → right-left
        self.find_all_dfa(input)
    }

    /// debug: dump rev DFA effects_id and effects.
    pub fn effects_debug(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let rev = &inner.rev;
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

    /// debug: run only the reverse DFA, return null positions.
    pub fn collect_rev_nulls_debug(&self, input: &[u8]) -> Vec<usize> {
        let inner = &mut *self.inner.lock().unwrap();
        inner.nulls_buf.clear();
        inner
            .rev
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls_buf)
            .unwrap();
        inner.nulls_buf.clone()
    }

    fn find_all_dfa(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        if self.fwd_end_nullable {
            self.find_all_dfa_inner::<true>(input)
        } else {
            self.find_all_dfa_inner::<false>(input)
        }
    }

    fn find_all_dfa_inner<const FWD_NULL: bool>(
        &self,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap();

        let rev_initial_nullable = inner.rev.effects_id[inner.rev.initial as usize] != 0;

        if rev_initial_nullable {
            inner.matches_buf.clear();
            Self::find_all_nullable_slow(&mut inner.fwd, &mut inner.b, input, &mut inner.matches_buf)?;
            return Ok(inner.matches_buf.clone());
        }

        inner.nulls_buf.clear();

        inner
            .rev
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls_buf)?;

        inner.matches_buf.clear();
        let matches = &mut inner.matches_buf;
        if let Some(fl) = self.fixed_length {
            let fl = fl as usize;
            let mut last_end = 0;
            for &start in inner.nulls_buf.iter().rev() {
                if start >= last_end && start + fl <= input.len() {
                    matches.push(Match {
                        start,
                        end: start + fl,
                    });
                    last_end = start + fl;
                }
            }
        } else {
            inner
                .fwd
                .scan_fwd_all(&mut inner.b, &inner.nulls_buf, input, self.max_length, matches)?;
        }

        if FWD_NULL
            && inner.nulls_buf.first() == Some(&input.len())
            && matches.last().map_or(true, |m| m.end <= input.len())
        {
            matches.push(Match {
                start: input.len(),
                end: input.len(),
            });
        }

        Ok(inner.matches_buf.clone())
    }

    fn find_all_nullable_slow(
        fwd: &mut engine::LDFA,
        b: &mut RegexBuilder,
        input: &[u8],
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let mut pos = 0;
        while pos < input.len() {
            let max_end = fwd.scan_fwd(b, pos, input)?;
            if max_end != engine::NO_MATCH && max_end > pos {
                matches.push(Match {
                    start: pos,
                    end: max_end,
                });
                pos = max_end;
            } else {
                matches.push(Match { start: pos, end: pos });
                pos += 1;
            }
        }
        matches.push(Match {
            start: input.len(),
            end: input.len(),
        });
        Ok(())
    }

    fn find_all_fwd_bounded(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let RegexInner { b, bounded, matches_buf, .. } = &mut *self.inner.lock().unwrap();
        let bounded = bounded.as_mut().unwrap();
        matches_buf.clear();
        match bounded.prefix {
            Some(accel::FwdPrefixSearch::Literal(_)) => {
                Self::bdfa_scan::<2, false>(bounded, b, input, matches_buf)?;
            }
            Some(accel::FwdPrefixSearch::Prefix(_)) => {
                Self::bdfa_scan::<1, false>(bounded, b, input, matches_buf)?;
            }
            None => {
                Self::bdfa_scan::<0, false>(bounded, b, input, matches_buf)?;
            }
        }
        Ok(matches_buf.clone())
    }

    fn is_match_fwd_bounded(&self, input: &[u8]) -> Result<bool, Error> {
        let RegexInner { b, bounded, matches_buf, .. } = &mut *self.inner.lock().unwrap();
        let bounded = bounded.as_mut().unwrap();
        matches_buf.clear();
        let found = match bounded.prefix {
            Some(accel::FwdPrefixSearch::Literal(_)) => {
                Self::bdfa_scan::<2, true>(bounded, b, input, matches_buf)?
            }
            Some(accel::FwdPrefixSearch::Prefix(_)) => {
                Self::bdfa_scan::<1, true>(bounded, b, input, matches_buf)?
            }
            None => {
                Self::bdfa_scan::<0, true>(bounded, b, input, matches_buf)?
            }
        };
        Ok(found)
    }

    fn bdfa_scan<const PREFIX: u8, const ISMATCH: bool>(
        bounded: &mut BDFA,
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

        if PREFIX == 0 {
            let data = input.as_ptr();
            if !ISMATCH {
                matches.reserve(2048);
            }
            loop {
                if !ISMATCH {
                    if matches.len() == matches.capacity() {
                        matches.reserve(matches.capacity().max(256));
                    }
                }
                let spare = if ISMATCH { 1 } else { matches.capacity() - matches.len() };
                let buf_ptr = unsafe { matches.as_mut_ptr().add(matches.len()) };
                let table = bounded.table.as_ptr();
                let (s, p, mc) = bdfa_inner::<0>(
                    table, ml.as_ptr(), data, mt_log, initial, state, pos, len,
                    buf_ptr, spare,
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
                    if ISMATCH { return Ok(true); }
                    matches.push(Match {
                        start: pos - rel as usize,
                        end: pos,
                    });
                    state = initial;
                } else {
                    pos += 1;
                }
            }
        } else {
            // PREFIX 1/2: unified loop with Teddy prefix inlined
            'main: loop {
                if pos >= len {
                    break;
                }

                if state == initial {
                    let found = bounded.prefix.as_ref().unwrap().find_fwd(input, pos);
                    match found {
                        Some(p) => {
                            if PREFIX == 2 {
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
                                if ISMATCH { return Ok(true); }
                                matches.push(Match {
                                    start: pos - rel as usize,
                                    end: pos,
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
                            if ISMATCH { return Ok(true); }
                            matches.push(Match {
                                start: pos - rel as usize,
                                end: pos,
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
                    if ISMATCH { return Ok(true); }
                    matches.push(Match {
                        start: pos - rel as usize,
                        end: pos,
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
                let best = BDFA::counted_best(node, b);
                if best > 0 {
                    if ISMATCH { return Ok(true); }
                    matches.push(Match {
                        start: len - best as usize,
                        end: len,
                    });
                }
            }
        }

        Ok(false)
    }

    fn find_all_fwd_prefix(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.fwd_prefix.as_ref().unwrap();
        let inner = &mut *self.inner.lock().unwrap();
        let matches = &mut inner.matches_buf;
        matches.clear();
        let mut search_start = 0;

        if self.fixed_length == Some(fwd_prefix.len() as u32)
            && fwd_prefix.find_all_literal(input, matches)
        {
        } else if let Some(fl) = self.fixed_length {
            while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
                let end = candidate + fl as usize;
                if end <= input.len() {
                    matches.push(Match {
                        start: candidate,
                        end,
                    });
                    search_start = end;
                } else {
                    break;
                }
            }
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
        }

        Ok(matches.clone())
    }

    fn find_all_fwd_prefix_stripped(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.fwd_prefix.as_ref().unwrap();
        let inner = &mut *self.inner.lock().unwrap();
        let prefix_len = fwd_prefix.len();
        let initial = inner.fwd.initial;
        inner.fwd.precompile_state(&mut inner.b, initial)?;
        inner.matches_buf.clear();
        let mut search_start = 0;

        while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
            let mut state = initial;
            for i in 0..prefix_len {
                let mt = inner.fwd.minterms_lookup[input[candidate + i] as usize] as u32;
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
            let mut match_start = candidate;
            while match_start > search_start {
                let b = input[match_start - 1];
                let mt = inner.fwd.minterms_lookup[b as usize] as usize;
                let delta = (initial as usize) << inner.fwd.mt_log | mt;
                let in_bounds = delta < inner.fwd.center_table.len();
                let ct_val = if in_bounds { inner.fwd.center_table[delta] } else { 0 };
                if in_bounds && ct_val > engine::DFA_DEAD {
                    match_start -= 1;
                } else {
                    break;
                }
            }
            if max_end > match_start {
                inner.matches_buf.push(Match {
                    start: match_start,
                    end: max_end,
                });
                search_start = max_end;
            } else {
                search_start = candidate + 1;
            }
        }

        Ok(inner.matches_buf.clone())
    }

    /// longest match anchored at position 0, forward DFA only.
    pub fn find_anchored(&self, input: &[u8]) -> Result<Option<Match>, Error> {
        if input.is_empty() {
            return if self.empty_nullable {
                Ok(Some(Match { start: 0, end: 0 }))
            } else {
                Ok(None)
            };
        }
        let inner = &mut *self.inner.lock().unwrap();
        let max_end = inner.fwd.scan_fwd(&mut inner.b, 0, input)?;
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
    pub fn is_match(&self, input: &[u8]) -> Result<bool, Error> {
        if input.is_empty() {
            return Ok(self.empty_nullable);
        }
        // 1. bounded → BDFA is_match
        {
            let inner = self.inner.lock().unwrap();
            if inner.bounded.is_some() {
                drop(inner);
                return self.is_match_fwd_bounded(input);
            }
        }
        // 2. fwd prefix → find first candidate
        if let Some(ref fwd_prefix) = self.fwd_prefix {
            if let Some(fl) = self.fixed_length {
                // prefix hit + fixed length = confirmed match
                if let Some(candidate) = fwd_prefix.find_fwd(input, 0) {
                    return Ok(candidate + fl as usize <= input.len());
                }
                return Ok(false);
            }
            // variable length: prefix hit + fwd DFA confirm
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
        // 3. fallback
        Ok(!self.find_all(input)?.is_empty())
    }
}
