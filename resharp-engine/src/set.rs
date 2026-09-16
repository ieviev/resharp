use resharp_algebra::nulls::{Nullability, NO_TAG};
use resharp_algebra::{NodeId, RegexBuilder};

use crate::ldfa::{context_at, DFA_DEAD, DFA_INITIAL, LDFA};
use crate::{pattern_flags, Error, Match, Regex, RegexInner, RegexOptions};

/// a span of a [`RegexSet`] match plus the index of the member that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaggedMatch {
    /// inclusive start.
    pub start: usize,
    /// exclusive end.
    pub end: usize,
    /// member index. the lowest one when several members match the same span.
    pub tag: usize,
}

/// several patterns compiled into one automaton.
///
/// ```
/// use resharp::{RegexSet, TaggedMatch};
///
/// let set = RegexSet::new([r"[0-9]+", r"[a-z]+"]).unwrap();
/// assert_eq!(
///     set.categorize_all(b"ab 12").unwrap(),
///     [TaggedMatch { start: 0, end: 2, tag: 1 }, TaggedMatch { start: 3, end: 5, tag: 0 }]
/// );
/// assert_eq!(set.matched(b"ab").unwrap(), [1]);
/// ```
pub struct RegexSet {
    re: Regex,
    len: usize,
}

pub(crate) struct SetState {
    any: LDFA,
}

impl RegexSet {
    /// compile a set with default options.
    pub fn new<I, S>(patterns: I) -> Result<RegexSet, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::with_options(patterns, RegexOptions::default())
    }

    /// compile a set with custom [`RegexOptions`]. members may not contain capture
    /// groups or lookbehinds, including a leading `\b` or `^` (use `\A`). the span
    /// would still be right but could not be attributed to a member.
    pub fn with_options<I, S>(patterns: I, opts: RegexOptions) -> Result<RegexSet, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut b = RegexBuilder::new();
        b.lookahead_context_max = opts.lookahead_context_max;
        let pflags = pattern_flags(&opts);
        let mut union = NodeId::BOT;
        let mut any = NodeId::BOT;
        let mut len = 0usize;
        for (i, p) in patterns.into_iter().enumerate() {
            let node = resharp_parser::parse_ast_with(&mut b, p.as_ref(), &pflags)?;
            if node.contains_tags(&b) {
                return Err(Error::SetMember(i, "capture groups are not supported in a set"));
            }
            if b.contains_lookbehind(node) {
                return Err(Error::SetMember(
                    i,
                    "lookbehind (including leading `\\b` or `^`) is not supported in a set",
                ));
            }
            let Some(elim) = b.try_elim_lookarounds(node) else {
                return Err(Error::SetMember(i, "lookaround cannot be eliminated"));
            };
            let tag = b.mk_tag(i as u32);
            let floating = b.mk_concats([NodeId::TS, elim, NodeId::TS, tag].into_iter());
            any = b.mk_union(any, floating);
            let tagged = b.push_tag_trailing(node, i as u32);
            union = b.mk_union(union, tagged);
            len = i + 1;
        }
        let max_cap = opts.max_dfa_capacity.min(u16::MAX as usize);
        let re = Regex::from_set_node(b, union, opts)?;
        {
            let inner = &mut *re.inner.lock().unwrap_or_else(|e| e.into_inner());
            let any = LDFA::new_fwd(&mut inner.b, any, max_cap)?;
            inner.set = Some(SetState { any });
        }
        Ok(RegexSet { re, len })
    }

    /// number of members.
    pub fn len(&self) -> usize {
        self.len
    }

    /// true if the set has no members.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// the union regex. it carries tag state the `dump`/`load` format has no room
    /// for, so `dump()` on it returns `Err`.
    pub fn regex(&self) -> &Regex {
        &self.re
    }

    /// true if any member matches somewhere in `input`.
    pub fn is_match(&self, input: &[u8]) -> Result<bool, Error> {
        self.re.is_match(input)
    }

    /// sorted indices of the members that match somewhere in `input`. a member whose
    /// only matches lie inside a longer sibling's span is still reported.
    pub fn matched(&self, input: &[u8]) -> Result<Vec<usize>, Error> {
        let inner = &mut *self.re.inner.lock().unwrap_or_else(|e| e.into_inner());
        let RegexInner { b, set, .. } = inner;
        let SetState { any } = set_state(set)?;
        let state = if input.is_empty() {
            DFA_INITIAL as u32
        } else {
            any.walk_input(b, 0, input.len(), input)?
        };
        if state <= DFA_DEAD as u32 {
            return Ok(Vec::new());
        }
        let mask = context_at(input.len(), input.len());
        let mut out: Vec<usize> = tags_at(any, state, mask, 0).map(|t| t as usize).collect();
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }

    /// leftmost-longest non-overlapping spans of the union of all members.
    pub fn find_all(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        self.re.find_all(input)
    }

    /// like [`find_all`](Self::find_all), each span labelled with the member that produced it.
    pub fn categorize_all(&self, input: &[u8]) -> Result<Vec<TaggedMatch>, Error> {
        let matches = self.re.find_all(input)?;
        let inner = &mut *self.re.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::with_capacity(matches.len());
        for m in matches {
            let tag = tag_of(inner, input, m)?;
            out.push(TaggedMatch { start: m.start, end: m.end, tag });
        }
        Ok(out)
    }
}

fn set_state(set: &mut Option<SetState>) -> Result<&mut SetState, Error> {
    set.as_mut().ok_or(Error::InternalError("regex was not built as a set"))
}

/// member tags nullable in `state` at context `mask` and `rel` bytes past the match end.
fn tags_at(dfa: &LDFA, state: u32, mask: Nullability, rel: u32) -> impl Iterator<Item = u32> + '_ {
    dfa.effects[dfa.effects_id[state as usize] as usize]
        .iter()
        .filter(move |n| n.tag != NO_TAG && n.rel == rel && n.mask.has(mask))
        .map(|n| n.tag)
}

fn tag_of(inner: &mut RegexInner, input: &[u8], m: Match) -> Result<usize, Error> {
    let RegexInner { b, fwd, .. } = inner;
    let len = input.len();
    let ctx_max = b.lookahead_context_max as usize;
    let mut pos = m.start;
    let mut state: u16 = if pos == 0 { DFA_INITIAL } else { fwd.pruned };
    let mut best: Option<u32> = None;
    while state > DFA_DEAD {
        if pos >= m.end {
            let rel = (pos - m.end) as u32;
            if let Some(t) = tags_at(fwd, state as u32, context_at(pos, len), rel).min() {
                best = Some(best.map_or(t, |bt| bt.min(t)));
            }
            if !fwd.state_nodes[state as usize].contains_lookahead(b) || pos - m.end >= ctx_max {
                break;
            }
        }
        if pos >= len {
            break;
        }
        let mt = fwd.mt_lookup[input[pos] as usize] as u32;
        state = if pos == 0 {
            fwd.begin_table[mt as usize]
        } else {
            fwd.lazy_transition(b, state, mt)?
        };
        pos += 1;
    }
    best.map(|t| t as usize).ok_or(Error::InternalError("set match has no tag"))
}
