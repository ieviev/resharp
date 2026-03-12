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
pub use resharp_algebra::solver::TSetId;

pub use resharp_algebra::nulls::Nullability;
pub use resharp_algebra::NodeId;
pub use resharp_algebra::RegexBuilder;

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

/// lazy DFA engine options.
pub struct EngineOptions {
    /// states to eagerly precompile (0 = fully lazy).
    pub dfa_threshold: usize,
    /// max cached DFA states; clamped to `u16::MAX`.
    pub max_dfa_capacity: usize,
    /// max lookahead context distance (default: 800).
    pub lookahead_context_max: u32,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            dfa_threshold: 0,
            max_dfa_capacity: u16::MAX as usize,
            lookahead_context_max: 800,
        }
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

struct RegexInner {
    b: RegexBuilder,
    fwd: engine::LazyDFA,
    rev: engine::LazyDFA,
    nulls_buf: Vec<usize>,
}

/// compiled regex backed by a lazy DFA.
///
/// uses a `Mutex` for mutable DFA state; clone for per-thread matching.
pub struct Regex {
    inner: Mutex<RegexInner>,
    fwd_prefix: Option<accel::FwdPrefixSearch>,
    fixed_length: Option<u32>,
    #[allow(dead_code)]
    max_length: Option<u32>,
    empty_nullable: bool,
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
        let node = resharp_parser::parse_ast(&mut b, pattern)?;
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
        let rev_start = b.reverse(node)?;
        let ts_rev_start = b.mk_concat(NodeId::TS, rev_start);

        let fixed_length = b.get_fixed_length(node);
        let (min_len, max_len) = b.get_min_max_length(node);
        let max_length = if max_len != u32::MAX {
            Some(max_len)
        } else {
            None
        };
        let can_match_fwd = !b.is_infinite(node) && !b.contains_look(node);

        let max_cap = opts.max_dfa_capacity.min(u16::MAX as usize);
        let mut fwd = engine::LazyDFA::new(&mut b, fwd_start, max_cap)?;
        let mut rev = engine::LazyDFA::new(&mut b, ts_rev_start, max_cap)?;

        if opts.dfa_threshold > 0 {
            fwd.precompile(&mut b, opts.dfa_threshold);
            rev.precompile(&mut b, opts.dfa_threshold);
        }

        let fwd_prefix = if min_len > 0 && can_match_fwd {
            engine::build_fwd_prefix(&mut b, node)?
        } else {
            None
        };

        rev.compute_skip(&mut b, rev_start)?;

        Ok(Regex {
            inner: Mutex::new(RegexInner {
                b,
                fwd,
                rev,
                nulls_buf: Vec::new(),
            }),
            fwd_prefix,
            fixed_length,
            max_length,
            empty_nullable,
        })
    }

    /// (fwd_states, rev_states) count.
    pub fn dfa_stats(&self) -> (usize, usize) {
        let inner = self.inner.lock().unwrap();
        (inner.fwd.state_nodes.len(), inner.rev.state_nodes.len())
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
        if self.fwd_prefix.is_some() {
            return self.find_all_fwd_prefix(input);
        }
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
        let inner = &mut *self.inner.lock().unwrap();

        inner.nulls_buf.clear();

        inner
            .rev
            .collect_rev(&mut inner.b, input.len() - 1, input, &mut inner.nulls_buf)?;

        let mut matches = Vec::new();
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
                .scan_fwd_all(&mut inner.b, &inner.nulls_buf, input, &mut matches)?;
        }

        if inner.rev.effects_id[inner.rev.initial as usize] != 0 {
            matches.push(Match {
                start: input.len(),
                end: input.len(),
            });
        }

        Ok(matches)
    }

    fn find_all_fwd_prefix(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let fwd_prefix = self.fwd_prefix.as_ref().unwrap();
        let mut matches = Vec::new();
        let mut search_start = 0;

        if self.fixed_length.is_some() && fwd_prefix.find_all_literal(input, &mut matches) {
            // done
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
            let inner = &mut *self.inner.lock().unwrap();
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

        Ok(matches)
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
        let inner = &mut *self.inner.lock().unwrap();
        if inner.rev.effects_id[inner.rev.initial as usize] != 0 {
            return Ok(true);
        }
        inner
            .rev
            .any_nullable_rev(&mut inner.b, input.len() - 1, input)
    }
}
