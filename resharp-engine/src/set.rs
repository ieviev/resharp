use resharp_algebra::nulls::{Nullability, NO_TAG};
use resharp_algebra::{NodeId, RegexBuilder};

use crate::ldfa::{context_at, Sid, DFA_DEAD, DFA_INITIAL, LDFA};
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

/// [`RegexSet::label_all`] result: each span with the id of its member set.
pub struct Labels {
    spans: Vec<(Match, usize)>,
    sets: Vec<Vec<usize>>,
}

impl Labels {
    /// number of spans.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// true if no span matched.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// each span with the sorted member indices that match exactly that span.
    pub fn iter(&self) -> impl Iterator<Item = (Match, &[usize])> + '_ {
        self.spans
            .iter()
            .map(move |&(m, id)| (m, self.sets[id].as_slice()))
    }
}

/// several patterns compiled into one automaton.
///
/// ```
/// use resharp::{RegexSet, TaggedMatch};
///
/// let set = RegexSet::new([r"[0-9]+", r"[a-z]+"]).unwrap();
/// assert_eq!(
///     set.classify_all(b"ab 12").unwrap(),
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
    /// groups or lookbehinds (including leading `\b` or `^`; use `\A`), unless
    /// [`RegexOptions::regexset_enforce_disjoint`] admits a disjoint lookbehind member.
    pub fn with_options<I, S>(patterns: I, opts: RegexOptions) -> Result<RegexSet, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut b = RegexBuilder::new();
        b.lookahead_context_max = opts.lookahead_context_max;
        let enforce_disjoint = opts.regexset_enforce_disjoint;
        let pflags = pattern_flags(&opts);
        let mut union = NodeId::BOT;
        let mut any = NodeId::BOT;
        let mut len = 0usize;
        let mut bodies: Vec<(NodeId, bool)> = Vec::new();
        for (i, p) in patterns.into_iter().enumerate() {
            let node = resharp_parser::parse_ast_with(&mut b, p.as_ref(), &pflags)?;
            if node.contains_tags(&b) {
                return Err(Error::SetMember(
                    i,
                    "capture groups are not supported in a set",
                ));
            }
            let lookbehind = b.contains_lookbehind(node);
            if lookbehind && !enforce_disjoint {
                return Err(Error::SetMember(
                    i,
                    "lookbehind (including leading `\\b` or `^`) is not supported in a set; set RegexOptions::regexset_enforce_disjoint to allow it (may be expensive to compile!)",
                ));
            }
            let body = if lookbehind { b.strip_lb(node)? } else { node };
            bodies.push((body, lookbehind));
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
        if enforce_disjoint {
            check_lookbehind_disjoint(&mut b, &bodies)?;
        }
        let max_cap = crate::ldfa::checked_capacity(opts.max_dfa_capacity)?;
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

    /// bulk ismatch, returns indices of all patterns matched.
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

    /// like find_all, but a match can have a tag index (multiclass classification).
    pub fn classify_all(&self, input: &[u8]) -> Result<Vec<TaggedMatch>, Error> {
        let matches = self.re.find_all(input)?;
        let inner = &mut *self.re.inner.lock().unwrap_or_else(|e| e.into_inner());
        let len = input.len();
        let mut by_effect: Vec<Option<usize>> = Vec::new();
        let mut out = Vec::with_capacity(matches.len());
        let mut buf = Vec::new();
        for m in matches {
            let end = span_end(inner, input, m)?;
            let eid = cached_eid(inner, m, end, len);
            let tag = match cache_get(&by_effect, eid) {
                Some(tag) => tag,
                None => {
                    let tag = span_min_tag(inner, input, m, end, &mut buf)?;
                    if let Some(eid) = eid {
                        cache_put(&mut by_effect, eid, tag);
                    }
                    tag
                }
            };
            out.push(TaggedMatch {
                start: m.start,
                end: m.end,
                tag,
            });
        }
        Ok(out)
    }

    /// like find_all, but a match can have N tag indices (multilabel classification).
    pub fn label_all(&self, input: &[u8]) -> Result<Labels, Error> {
        let (spans, sets) = self.span_sets(input)?;
        Ok(Labels { spans, sets })
    }

    fn span_sets(&self, input: &[u8]) -> Result<(Vec<(Match, usize)>, Vec<Vec<usize>>), Error> {
        let matches = self.re.find_all(input)?;
        let inner = &mut *self.re.inner.lock().unwrap_or_else(|e| e.into_inner());
        let len = input.len();
        let mut by_effect: Vec<Option<usize>> = Vec::new();
        let mut sets: Vec<Vec<usize>> = Vec::new();
        let mut spans = Vec::with_capacity(matches.len());
        let mut buf = Vec::new();
        for m in matches {
            buf.clear();
            let end = span_end(inner, input, m)?;
            let eid = cached_eid(inner, m, end, len);
            let id = match cache_get(&by_effect, eid) {
                Some(id) => id,
                None => {
                    span_tail_tags(inner, input, m, end, &mut buf)?;
                    sets.push(std::mem::take(&mut buf));
                    let id = sets.len() - 1;
                    if let Some(eid) = eid {
                        cache_put(&mut by_effect, eid, id);
                    }
                    id
                }
            };
            spans.push((m, id));
        }
        Ok((spans, sets))
    }
}

fn check_lookbehind_disjoint(b: &mut RegexBuilder, bodies: &[(NodeId, bool)]) -> Result<(), Error> {
    for (j, &(bj, lbj)) in bodies.iter().enumerate() {
        for &(bi, lbi) in &bodies[..j] {
            if !(lbi || lbj) {
                continue;
            }
            let overlap = b.mk_inter(bi, bj);
            if b.is_empty_lang(overlap) != Some(true) {
                return Err(Error::SetMember(
                    j,
                    "lookbehind member must be disjoint from every other member",
                ));
            }
        }
    }
    Ok(())
}

fn set_state(set: &mut Option<SetState>) -> Result<&mut SetState, Error> {
    set.as_mut()
        .ok_or(Error::InternalError("regex was not built as a set"))
}

/// member tags nullable in `state` at context `mask` and `rel` bytes past the match end.
fn tags_at(dfa: &LDFA, state: u32, mask: Nullability, rel: u32) -> impl Iterator<Item = u32> + '_ {
    dfa.effects[dfa.effects_id[state as usize] as usize]
        .iter()
        .filter(move |n| n.tag != NO_TAG && n.rel == rel && n.mask.has(mask))
        .map(|n| n.tag)
}

fn insert_sorted(sorted: &mut Vec<usize>, t: usize) {
    if let Err(at) = sorted.binary_search(&t) {
        sorted.insert(at, t);
    }
}

fn span_end(inner: &mut RegexInner, input: &[u8], m: Match) -> Result<Sid, Error> {
    let RegexInner { b, fwd, .. } = inner;
    let mut pos = m.start;
    let mut state: Sid = if pos == 0 { DFA_INITIAL } else { fwd.pruned };
    while pos < m.end && state > DFA_DEAD {
        let mt = fwd.mt_lookup[input[pos] as usize] as u32;
        state = if pos == 0 {
            fwd.begin_table[mt as usize]
        } else {
            fwd.lazy_transition(b, state, mt)?
        };
        pos += 1;
    }
    Ok(state)
}

fn span_tail_tags(
    inner: &mut RegexInner,
    input: &[u8],
    m: Match,
    mut state: Sid,
    out: &mut Vec<usize>,
) -> Result<(), Error> {
    let RegexInner { b, fwd, .. } = inner;
    let len = input.len();
    let ctx_max = b.lookahead_context_max as usize;
    let mut pos = m.end;
    while state > DFA_DEAD {
        let rel = (pos - m.end) as u32;
        for t in tags_at(fwd, state as u32, context_at(pos, len), rel) {
            insert_sorted(out, t as usize);
        }
        if !fwd.state_nodes[state as usize].contains_lookahead(b)
            || pos - m.end >= ctx_max
            || pos >= len
        {
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
    if out.is_empty() {
        return Err(Error::InternalError("set match has no tag"));
    }
    Ok(())
}

fn span_min_tag(
    inner: &mut RegexInner,
    input: &[u8],
    m: Match,
    end: Sid,
    buf: &mut Vec<usize>,
) -> Result<usize, Error> {
    buf.clear();
    span_tail_tags(inner, input, m, end, buf)?;
    buf.first()
        .copied()
        .ok_or(Error::InternalError("set match has no tag"))
}

fn cache_get(cache: &[Option<usize>], eid: Option<usize>) -> Option<usize> {
    eid.and_then(|eid| cache.get(eid).copied().flatten())
}

fn cache_put(cache: &mut Vec<Option<usize>>, eid: usize, v: usize) {
    if eid >= cache.len() {
        cache.resize(eid + 1, None);
    }
    cache[eid] = Some(v);
}

fn cached_eid(inner: &RegexInner, m: Match, end: Sid, len: usize) -> Option<usize> {
    let center_end = m.end > 0 && m.end < len;
    (center_end
        && end > DFA_DEAD
        && !inner.fwd.state_nodes[end as usize].contains_lookahead(&inner.b))
    .then(|| inner.fwd.effects_id[end as usize] as usize)
}
