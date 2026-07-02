/// REMOVED FROM PUBLIC API TILL SEMANTICS FINISHED.
// //! All stream/seek methods return shortest matches (left-to-right, earliest end).

use crate::{accel, ldfa, prefix, Error, Match, Nullability, Regex};
use resharp_algebra::nulls::{EID_ALWAYS0, EID_BEGIN0, EID_CENTER0, EID_END0, EID_NONE};

fn collect_valid_ends(dfa: &ldfa::LDFA, state: u32, pos: usize, len: usize, out: &mut Vec<usize>) {
    out.clear();
    let eid = dfa.effects_id[state as usize] as u32;
    match eid {
        EID_NONE => {}
        EID_CENTER0 => {
            if pos < len {
                out.push(pos);
            }
        }
        EID_ALWAYS0 => out.push(pos),
        EID_BEGIN0 => {
            if pos == 0 {
                out.push(pos);
            }
        }
        EID_END0 => {
            if pos == len {
                out.push(pos);
            }
        }
        _ => {
            for n in &dfa.effects[eid as usize] {
                let Some(e) = pos.checked_sub(n.rel as usize) else { continue };
                let ctx = if e == len { Nullability::END } else { Nullability::CENTER };
                if n.mask.has(ctx) {
                    out.push(e);
                }
            }
            out.sort_unstable();
            out.dedup();
        }
    }
}

fn begin_search_start(re: &Regex, input: &[u8]) -> Result<usize, Error> {
    let inner = &mut *re.inner.lock().unwrap();
    let mt = inner.fwd.mt_lookup[input[0] as usize];
    let st0 = inner.fwd.begin_table[mt as usize] as u32;
    if st0 == inner.fwd.pruned as u32 || st0 <= ldfa::DFA_DEAD as u32 {
        return Ok(0);
    }
    let end = input.len();
    let (st, p, hit) = inner
        .fwd
        .scan_fwd_first_null_from(&mut inner.b, st0, 1, input)?;
    Ok(resolve_emit(&inner.fwd, st, p, end, hit).map_or(0, |e| if e == 0 { 1 } else { e }))
}

fn stream_anchored_fwd<const STOP: bool, F: FnMut(usize, usize)>(
    re: &Regex,
    fwd_prefix: &accel::FwdPrefixSearch,
    lb_len: usize,
    search_start: usize,
    input: &[u8],
    mut emit: F,
) -> Result<(), Error> {
    let inner = &mut *re.inner.lock().unwrap();
    let end = input.len();
    let prefix_len = fwd_prefix.len();
    let mut search_start = search_start;

    while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
        let (body_state, body_pos) = if lb_len > 0 {
            (ldfa::DFA_INITIAL as u32, candidate + lb_len)
        } else {
            let st = inner
                .fwd
                .walk_input(&mut inner.b, candidate, prefix_len, input)?;
            (st, candidate + prefix_len)
        };
        if body_state == 0 {
            search_start = candidate + 1;
            continue;
        }
        let mut state = body_state;
        let mut pos = body_pos;
        let emitted = loop {
            let (st, p, hit) =
                inner
                    .fwd
                    .scan_fwd_first_null_from(&mut inner.b, state, pos, input)?;
            if let Some(end_pos) = resolve_emit(&inner.fwd, st, p, end, hit) {
                break Some(end_pos);
            }
            if hit && p < end {
                let mt = inner.fwd.mt_lookup[input[p] as usize] as u32;
                let nxt = inner.fwd.lazy_transition(&mut inner.b, st as u16, mt)? as u32;
                if nxt <= ldfa::DFA_DEAD as u32 {
                    break None;
                }
                state = nxt;
                pos = p + 1;
                continue;
            }
            break None;
        };
        if let Some(end_pos) = emitted {
            let m_start = candidate + lb_len;
            emit(m_start, end_pos);
            if STOP {
                return Ok(());
            }
            search_start = if end_pos == m_start {
                m_start + 1
            } else {
                end_pos
            };
        } else {
            search_start = candidate + 1;
        }
    }
    Ok(())
}

fn resolve_emit(
    fwd: &ldfa::LDFA,
    state: u32,
    pos: usize,
    end: usize,
    hit_null: bool,
) -> Option<usize> {
    if state <= ldfa::DFA_DEAD as u32 {
        return None;
    }
    let mask = if pos == end {
        Nullability::END
    } else if hit_null {
        Nullability::CENTER
    } else {
        return None;
    };
    if !ldfa::has_any_null(&fwd.effects_id, &fwd.effects, state, mask) {
        return None;
    }
    let mut match_end = 0usize;
    ldfa::collect_max_fwd(
        &fwd.effects_id,
        &fwd.effects,
        state,
        pos,
        mask,
        &mut match_end,
    );
    Some(if match_end == 0 { pos } else { match_end })
}

impl Regex {
    pub(crate) fn init_stream_fwd_only(&self) -> Result<(), Error> {
        if self.stream_cache.fwd_prefix.get().is_some() {
            return Ok(());
        }
        let mut inner = self.inner.lock().unwrap();
        let start_node = inner.stream.start_node;
        let p: Option<accel::FwdPrefixSearch> = prefix::build_fwd_prefix(&mut inner.b, start_node)?;
        let _ = self.stream_cache.fwd_prefix.set(p);
        Ok(())
    }

    pub(crate) fn init_stream(&self) -> Result<(), Error> {
        if self.stream_cache.fwd_prefix.get().is_some()
            && self.stream_cache.rev_inited.get().is_some()
        {
            return Ok(());
        }
        let mut inner = self.inner.lock().unwrap();
        let start_node = inner.stream.start_node;
        if self.stream_cache.fwd_prefix.get().is_none() {
            let p: Option<accel::FwdPrefixSearch> =
                prefix::build_fwd_prefix(&mut inner.b, start_node)?;
            let _ = self.stream_cache.fwd_prefix.set(p);
        }
        if self.stream_cache.rev_inited.get().is_none() {
            let rev = inner.b.reverse(start_node).map_err(Error::Algebra)?;
            let rev = inner.b.strip_lb(rev).map_err(Error::Algebra)?;
            let rev = inner.b.normalize_rev(rev, 0).map_err(Error::Algebra)?;
            let max_cap = inner.fwd.max_capacity;
            inner.rev = Some(ldfa::LDFA::new_rev(&mut inner.b, rev, max_cap)?);
            let _ = self.stream_cache.rev_inited.set(());
        }
        Ok(())
    }

    /// Shortest matches, left-to-right. State resets after each match.
    pub fn stream(&self, input: &[u8]) -> Result<Vec<Match>, Error> {
        let mut out = Vec::new();
        self.stream_with(input, |m| out.push(m))?;
        Ok(out)
    }

    /// Shortest matches; callback variant of [`Regex::stream`].
    pub fn stream_with<F: FnMut(Match)>(&self, input: &[u8], on_match: F) -> Result<(), Error> {
        self.stream_with_inner::<false, _>(input, on_match)
    }

    /// Shortest match starting earliest in `input`, or `None`.
    pub fn stream_first(&self, input: &[u8]) -> Result<Option<Match>, Error> {
        let mut out = None;
        self.stream_with_inner::<true, _>(input, |m| {
            if out.is_none() {
                out = Some(m);
            }
        })?;
        Ok(out)
    }

    fn stream_with_inner<const STOP: bool, F: FnMut(Match)>(
        &self,
        input: &[u8],
        mut on_match: F,
    ) -> Result<(), Error> {
        if input.is_empty() {
            if self.empty_nullable {
                on_match(Match { start: 0, end: 0 });
            }
            return Ok(());
        }
        self.init_stream()?;
        match &self.prefix {
            Some(prefix::PrefixKind::AnchoredFwd(fp)) => {
                let search_start = begin_search_start(self, input)?;
                if search_start > 0 {
                    on_match(Match {
                        start: 0,
                        end: search_start,
                    });
                    if STOP {
                        return Ok(());
                    }
                }
                return stream_anchored_fwd::<STOP, _>(self, fp, 0, search_start, input, |s, e| {
                    on_match(Match { start: s, end: e })
                });
            }
            Some(prefix::PrefixKind::AnchoredFwdLb(fp)) => {
                let lb_len = self.lb_check_bytes as usize;
                if !self.fwd_lb_begin_nullable {
                    return stream_anchored_fwd::<STOP, _>(self, fp, lb_len, 0, input, |s, e| {
                        on_match(Match { start: s, end: e })
                    });
                }
            }
            _ => {}
        }
        let begin_nullable = self.initial_nullability.has(Nullability::BEGIN);
        let fwd_prefix = self.stream_cache.fwd_prefix.get().unwrap().as_ref();
        let inner = &mut *self.inner.lock().unwrap();
        stream_general::<true, STOP, _>(inner, fwd_prefix, begin_nullable, input, |s, e| {
            on_match(Match { start: s, end: e })
        })
    }
}

fn try_emit_step<const REV: bool, F: FnMut(usize, usize)>(
    inner: &mut crate::RegexInner,
    input: &[u8],
    pos: usize,
    skip_until: &mut usize,
    state: u32,
    emit: &mut F,
) -> Result<Option<usize>, Error> {
    let len = input.len();
    let mut ends: Vec<usize> = Vec::new();
    collect_valid_ends(&inner.fwd_ts, state, pos, len, &mut ends);
    let mut resume: Option<usize> = None;
    for &match_end in ends.iter() {
        if match_end < *skip_until {
            continue;
        }
        let m_start = if REV {
            let rev = inner.rev.as_mut().unwrap();
            let s = rev.scan_rev_from(&mut inner.b, match_end, *skip_until, input)?;
            s.unwrap_or(match_end)
        } else {
            match_end
        };
        if m_start < *skip_until {
            continue;
        }
        emit(m_start, match_end);
        *skip_until = if match_end > m_start { match_end } else { match_end + 1 };
        resume = Some(match_end);
    }
    Ok(resume)
}

fn stream_general<const REV: bool, const STOP: bool, F: FnMut(usize, usize)>(
    inner: &mut crate::RegexInner,
    fwd_prefix: Option<&accel::FwdPrefixSearch>,
    begin_nullable: bool,
    input: &[u8],
    mut emit: F,
) -> Result<(), Error> {
    let mut skip_until = 0usize;

    if begin_nullable {
        emit(0, 0);
        skip_until = 1;
        if STOP {
            return Ok(());
        }
        stream_feed_loop::<REV, true, STOP, _>(
            inner,
            fwd_prefix,
            input,
            0,
            ldfa::DFA_INITIAL as u32,
            &mut skip_until,
            &mut emit,
        )?;
        return Ok(());
    }

    let mt0 = inner.fwd_ts.mt_lookup[input[0] as usize] as u32;
    let first = inner.fwd_ts.begin_table[mt0 as usize] as u32;
    let resume = try_emit_step::<REV, _>(inner, input, 1, &mut skip_until, first, &mut emit)?;
    if STOP && resume.is_some() {
        return Ok(());
    }
    let (pos, state) = match resume {
        Some(e) => (e, ldfa::DFA_INITIAL as u32),
        None => (1usize, first),
    };
    stream_feed_loop::<REV, true, STOP, _>(
        inner,
        fwd_prefix,
        input,
        pos,
        state,
        &mut skip_until,
        &mut emit,
    )?;
    Ok(())
}

fn stream_feed_loop<
    const REV: bool,
    const PREFIX: bool,
    const STOP: bool,
    F: FnMut(usize, usize),
>(
    inner: &mut crate::RegexInner,
    fwd_prefix: Option<&accel::FwdPrefixSearch>,
    input: &[u8],
    mut pos: usize,
    init_state: u32,
    skip_until: &mut usize,
    emit: &mut F,
) -> Result<u32, Error> {
    let end = input.len();
    let mut state = init_state;
    while pos < end {
        if PREFIX && state == ldfa::DFA_INITIAL as u32 {
            if let Some(fp) = fwd_prefix {
                match fp.find_fwd(input, pos) {
                    Some(cand) => pos = cand,
                    None => break,
                }
            }
        }
        if !PREFIX {
            let sid = inner
                .fwd_ts
                .skip_ids
                .get(state as usize)
                .copied()
                .unwrap_or(0);
            if sid != 0 {
                let searcher = crate::scan::state_searcher(&inner.fwd_ts.skip_searchers, sid);
                match searcher.find_fwd(&input[pos..end]) {
                    Some(off) => pos += off,
                    None => break,
                }
                if pos >= end {
                    break;
                }
            }
        }
        let mt = inner.fwd_ts.mt_lookup[input[pos] as usize] as u32;
        let next = inner
            .fwd_ts
            .lazy_transition(&mut inner.b, state as u16, mt)? as u32;
        pos += 1;
        if next == ldfa::DFA_DEAD as u32 {
            state = ldfa::DFA_INITIAL as u32;
            continue;
        }
        let resume = try_emit_step::<REV, _>(inner, input, pos, skip_until, next, emit)?;
        if STOP && resume.is_some() {
            return Ok(ldfa::DFA_INITIAL as u32);
        }
        if let Some(e) = resume {
            pos = e;
        }
        state = if resume.is_some() {
            ldfa::DFA_INITIAL as u32
        } else {
            next
        };
    }
    Ok(state)
}

impl Regex {
    /// Shortest match ends only; skips the rev pass.
    pub fn stream_ends(&self, input: &[u8]) -> Result<Vec<usize>, Error> {
        let mut out = Vec::new();
        self.stream_ends_with(input, |e| out.push(e))?;
        Ok(out)
    }

    /// Shortest match ends; callback variant of [`Regex::stream_ends`].
    pub fn stream_ends_with<F: FnMut(usize)>(
        &self,
        input: &[u8],
        mut on_match: F,
    ) -> Result<(), Error> {
        if input.is_empty() {
            if self.empty_nullable {
                on_match(0);
            }
            return Ok(());
        }
        self.init_stream_fwd_only()?;
        match &self.prefix {
            Some(prefix::PrefixKind::AnchoredFwd(fp)) => {
                let search_start = begin_search_start(self, input)?;
                if search_start > 0 {
                    on_match(search_start);
                }
                return stream_anchored_fwd::<false, _>(
                    self,
                    fp,
                    0,
                    search_start,
                    input,
                    |_, e| on_match(e),
                );
            }
            Some(prefix::PrefixKind::AnchoredFwdLb(fp)) => {
                let lb_len = self.lb_check_bytes as usize;
                if !self.fwd_lb_begin_nullable {
                    return stream_anchored_fwd::<false, _>(self, fp, lb_len, 0, input, |_, e| {
                        on_match(e)
                    });
                }
            }
            _ => {}
        }
        let begin_nullable = self.initial_nullability.has(Nullability::BEGIN);
        let fwd_prefix = self.stream_cache.fwd_prefix.get().unwrap().as_ref();
        let inner = &mut *self.inner.lock().unwrap();
        stream_general::<false, false, _>(inner, fwd_prefix, begin_nullable, input, |_, e| on_match(e))
    }
}

/// Opaque DFA state for [`Regex::stream_chunk`].
#[derive(Clone, Copy)]
pub struct StreamState(pub(crate) u32, pub(crate) usize);

impl StreamState {
    /// Initial state for the first chunk.
    pub fn new() -> Self {
        Self(ldfa::DFA_INITIAL as u32, 0)
    }
    /// Initial state starting at absolute byte offset `pos` (for resuming mid-stream).
    pub fn at(pos: usize) -> Self {
        Self(ldfa::DFA_INITIAL as u32, pos)
    }
    /// Absolute byte offset consumed so far.
    pub fn pos(&self) -> usize {
        self.1
    }
    #[doc(hidden)]
    pub fn state(&self) -> u32 {
        self.0
    }
    #[doc(hidden)]
    pub fn from_raw(state: u32, pos: usize) -> Self {
        Self(state, pos)
    }
}

impl Default for StreamState {
    fn default() -> Self {
        Self::new()
    }
}

/// Initial DFA state ids for [`Regex::seek_fwd`] / [`Regex::seek_rev`].
/// Both states have leading `\A` / `\z` epsilons stripped via `prune_begin_eps`,
/// so seeking can start from any byte offset.
/// Eagerly precomputed state ids and node refs used by streaming/seeking methods.
pub(crate) struct StreamInit {
    pub start_node: resharp_algebra::NodeId,
    pub seek_fwd: u32,
    pub seek_rev: u32,
}

/// Lazily populated caches used by streaming/seeking methods.
#[derive(Default)]
pub(crate) struct StreamCache {
    pub fwd_prefix: std::sync::OnceLock<Option<accel::FwdPrefixSearch>>,
    pub rev_inited: std::sync::OnceLock<()>,
}

impl Regex {
    /// Feed one chunk, resuming from `state`. Emits shortest match ends in absolute offsets.
    /// Pass `StreamState::new()` for the first chunk.
    pub fn stream_chunk<F: FnMut(usize)>(
        &self,
        chunk: &[u8],
        state: StreamState,
        mut on_match: F,
    ) -> Result<StreamState, Error> {
        self.init_stream_fwd_only()?;
        let fwd_prefix = self.stream_cache.fwd_prefix.get().unwrap().as_ref();
        let inner = &mut *self.inner.lock().unwrap();
        let offset = state.1;
        let next = stream_feed_loop::<false, false, false, _>(
            inner,
            fwd_prefix,
            chunk,
            0,
            state.0,
            &mut 0,
            &mut |_, e| on_match(offset + e),
        )?;
        Ok(StreamState(next, offset + chunk.len()))
    }

    /// Initial state for [`Regex::seek_fwd`] / [`Regex::seek_rev`] cursors.
    pub const SEEK_INITIAL: u32 = 0;

    /// Forward cursor scan; returns the next shortest match end as `(resume_state, end)`.
    /// First call: `state = SEEK_INITIAL`, `pos = 0`. Subsequent calls: pass back the returned values.
    pub fn seek_fwd(
        &self,
        input: &[u8],
        mut state: u32,
        mut pos: usize,
    ) -> Result<Option<(u32, usize)>, Error> {
        self.init_stream_fwd_only()?;
        let inner = &mut *self.inner.lock().unwrap();
        let fwd_initial_state = inner.stream.seek_fwd;
        if state == Self::SEEK_INITIAL {
            state = fwd_initial_state;
        }
        let end = input.len();
        let mut transitioned = false;
        while pos < end {
            let sid = inner
                .fwd_ts
                .skip_ids
                .get(state as usize)
                .copied()
                .unwrap_or(0);
            if sid != 0 {
                let accel::Skipper::State(s) = &inner.fwd_ts.skip_searchers[sid as usize - 1] else { unreachable!() };
                match s.find_fwd(&input[pos..end]) {
                    Some(off) => pos += off,
                    None => return Ok(None),
                }
                if pos >= end {
                    break;
                }
            }
            let mt = inner.fwd_ts.mt_lookup[input[pos] as usize] as u32;
            let next = inner
                .fwd_ts
                .lazy_transition(&mut inner.b, state as u16, mt)? as u32;
            pos += 1;
            if next == ldfa::DFA_DEAD as u32 {
                state = fwd_initial_state;
                transitioned = false;
                continue;
            }
            state = next;
            transitioned = true;
            if pos == end {
                break;
            }
            let dfa = &inner.fwd_ts;
            if ldfa::has_any_null(&dfa.effects_id, &dfa.effects, state, Nullability::CENTER) {
                let mut match_end = 0usize;
                ldfa::collect_max_fwd(
                    &dfa.effects_id,
                    &dfa.effects,
                    state,
                    pos,
                    Nullability::CENTER,
                    &mut match_end,
                );
                let match_end = if match_end == 0 { pos } else { match_end };
                return Ok(Some((fwd_initial_state, match_end)));
            }
        }
        if transitioned && pos == end {
            let dfa = &inner.fwd_ts;
            if ldfa::has_any_null(&dfa.effects_id, &dfa.effects, state, Nullability::END) {
                let mut match_end = 0usize;
                ldfa::collect_max_fwd(
                    &dfa.effects_id,
                    &dfa.effects,
                    state,
                    end,
                    Nullability::END,
                    &mut match_end,
                );
                let match_end = if match_end == 0 { end } else { match_end };
                return Ok(Some((fwd_initial_state, match_end)));
            }
        }
        Ok(None)
    }

    /// Reverse cursor scan over `input[..pos]` using rev LDFA on `_*·rev(node)`.
    /// Returns the next shortest match start (rightmost-first) as `(resume_state, start)`.
    /// First call: `state = SEEK_INITIAL`, `pos = input.len()`.
    pub fn seek_rev(
        &self,
        input: &[u8],
        mut state: u32,
        mut pos: usize,
    ) -> Result<Option<(u32, usize)>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        let rev_initial_state = inner.stream.seek_rev;
        if state == Self::SEEK_INITIAL {
            state = rev_initial_state;
        }
        let mut transitioned = false;
        while pos > 0 {
            let sid = inner
                .rev_ts
                .skip_ids
                .get(state as usize)
                .copied()
                .unwrap_or(0);
            if sid != 0 {
                let accel::Skipper::State(s) = &inner.rev_ts.skip_searchers[sid as usize - 1] else { unreachable!() };
                match s.find_rev(&input[..pos]) {
                    Some(idx) => pos = idx + 1,
                    None => return Ok(None),
                }
            }
            pos -= 1;
            let mt = inner.rev_ts.mt_lookup[input[pos] as usize] as u32;
            let next = inner
                .rev_ts
                .lazy_transition(&mut inner.b, state as u16, mt)? as u32;
            if next == ldfa::DFA_DEAD as u32 {
                state = rev_initial_state;
                transitioned = false;
                continue;
            }
            state = next;
            transitioned = true;
            if pos == 0 {
                break;
            }
            let dfa = &inner.rev_ts;
            if ldfa::has_any_null(&dfa.effects_id, &dfa.effects, state, Nullability::CENTER) {
                let mut match_start = usize::MAX;
                ldfa::collect_max_rev(
                    &dfa.effects_id,
                    &dfa.effects,
                    state,
                    pos,
                    Nullability::CENTER,
                    &mut match_start,
                );
                let match_start = if match_start == usize::MAX {
                    pos
                } else {
                    match_start
                };
                return Ok(Some((rev_initial_state, match_start)));
            }
        }
        if transitioned && pos == 0 {
            let dfa = &inner.rev_ts;
            if ldfa::has_any_null(&dfa.effects_id, &dfa.effects, state, Nullability::END) {
                let mut match_start = usize::MAX;
                ldfa::collect_max_rev(
                    &dfa.effects_id,
                    &dfa.effects,
                    state,
                    0,
                    Nullability::END,
                    &mut match_start,
                );
                let match_start = if match_start == usize::MAX {
                    0
                } else {
                    match_start
                };
                return Ok(Some((rev_initial_state, match_start)));
            }
        }
        Ok(None)
    }
}
