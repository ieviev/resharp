use crate::{accel::FwdPrefixSearch, ldfa::{self, Sid}, Error, Match, Regex, RegexBuilder};
use resharp_algebra::nulls::Nullability;

fn verify_lb_literal(
    lb_verify: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    pos: usize,
    lb_len: usize,
    input: &[u8],
) -> Result<bool, Error> {
    let mut state = ldfa::DFA_INITIAL;
    for i in 0..lb_len {
        let mt = lb_verify.mt_lookup[input[pos + i] as usize] as u32;
        state = lb_verify.lazy_transition(b, state, mt)?;
        if state <= ldfa::DFA_DEAD {
            return Ok(false);
        }
    }
    Ok(ldfa::has_any_null(
        &lb_verify.effects_id,
        &lb_verify.effects,
        state as u32,
        Nullability::CENTER,
    ))
}

fn is_exact_literal(
    fwd_prefix: &FwdPrefixSearch,
    fixed_length: Option<u32>,
    has_anchors: bool,
    has_la: bool,
    neg_lb: Option<&crate::prefix::NegLb>,
) -> bool {
    fwd_prefix.is_literal()
        && fixed_length == Some(fwd_prefix.len() as u32)
        && !has_anchors
        && !has_la
        && neg_lb.is_none()
}

fn fwd_prefix_impl<const FIRST: bool>(
    fwd: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    fixed_length: Option<u32>,
    has_anchors: bool,
    has_la: bool,
    neg_lb: Option<&crate::prefix::NegLb>,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    matches: &mut Vec<Match>,
) -> Result<(), Error> {
    let prefix_len = fwd_prefix.len();

    if is_exact_literal(fwd_prefix, fixed_length, has_anchors, has_la, neg_lb) {
        if FIRST {
            if let Some(p) = fwd_prefix.find_fwd(input, 0) {
                matches.push(Match { start: p, end: p + prefix_len });
            }
            return Ok(());
        }
        fwd_prefix.find_all_literal(input, matches);
        return Ok(());
    }

    let mut search_start = 0;

    {
        let mt = fwd.mt_lookup[input[0] as usize];
        let state = fwd.begin_table[mt as usize] as u32;
        if let Some(max_end) = fwd.scan_fwd_from(b, state, 1, input)? {
            if max_end > 0 {
                matches.push(Match {
                    start: 0,
                    end: max_end,
                });
                if FIRST {
                    return Ok(());
                }
                search_start = max_end;
            }
        }
    }

    while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
        if let Some(neg) = neg_lb {
            if neg.rejects(input, candidate) {
                search_start = candidate + 1;
                continue;
            }
        }
        let state = fwd.walk_input(b, candidate, prefix_len, input)?;
        if state != 0 {
            if let Some(max_end) = fwd.scan_fwd_from(b, state, candidate + prefix_len, input)? {
                if max_end > candidate {
                    matches.push(Match {
                        start: candidate,
                        end: max_end,
                    });
                    if FIRST {
                        return Ok(());
                    }
                    search_start = max_end;
                    continue;
                }
            }
        }
        search_start = candidate + 1;
    }

    Ok(())
}

fn accepts_at(fwd: &ldfa::LDFA, state: u32, fpos: usize, hit: bool, end: usize) -> bool {
    if fpos < end {
        return hit;
    }
    state > ldfa::DFA_DEAD as u32
        && ldfa::has_any_null(&fwd.effects_id, &fwd.effects, state, Nullability::END)
}

fn fwd_prefix_first_budget(
    fwd: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    neg_lb: Option<&crate::prefix::NegLb>,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
) -> Result<Option<bool>, Error> {
    let end = input.len();
    let prefix_len = fwd_prefix.len();
    let mut budget = end.saturating_mul(4).saturating_add(512);
    {
        let mt = fwd.mt_lookup[input[0] as usize];
        let state = fwd.begin_table[mt as usize] as u32;
        if state > ldfa::DFA_DEAD as u32 {
            let (st, fpos, hit) = fwd.scan_fwd_first_null_from(b, state, 1, input)?;
            if accepts_at(fwd, st, fpos, hit, end) {
                return Ok(Some(true));
            }
            budget = budget.saturating_sub(fpos);
        }
    }
    let mut search_start = 0;
    while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
        search_start = candidate + 1;
        if neg_lb.is_some_and(|neg| neg.rejects(input, candidate)) {
            continue;
        }
        let state = fwd.walk_input(b, candidate, prefix_len, input)?;
        if state == 0 {
            continue;
        }
        let next = candidate + prefix_len;
        if next < end && !ldfa::has_any_null(&fwd.effects_id, &fwd.effects, state, Nullability::CENTER) {
            let mt = fwd.mt_lookup[input[next] as usize] as u32;
            let st = fwd.lazy_transition(b, state as Sid, mt)?;
            if st <= ldfa::DFA_DEAD {
                continue;
            }
        }
        let (st, fpos, hit) = fwd.scan_fwd_first_null_from(b, state, candidate + prefix_len, input)?;
        if accepts_at(fwd, st, fpos, hit, end) {
            return Ok(Some(true));
        }
        let used = fpos.saturating_sub(candidate);
        if used > budget {
            return Ok(None);
        }
        budget -= used;
    }
    Ok(Some(false))
}

fn try_emit_zero_width(
    fwd: &mut ldfa::LDFA,
    lb_verify: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    lb_len: usize,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    at: usize,
    matches: &mut Vec<Match>,
) -> Result<bool, Error> {
    if at < lb_len {
        return Ok(false);
    }
    let lb_pos = at - lb_len;
    if fwd_prefix.find_fwd(input, lb_pos) != Some(lb_pos) {
        return Ok(false);
    }
    if !verify_lb_literal(lb_verify, b, lb_pos, lb_len, input)? {
        return Ok(false);
    }
    if fwd.scan_fwd_from(b, ldfa::DFA_INITIAL as u32, at, input)? == Some(at) {
        matches.push(Match { start: at, end: at });
        return Ok(true);
    }
    Ok(false)
}

fn begin_path_matches(begin_classes: &[crate::accel::TSet], input: &[u8]) -> bool {
    begin_classes.len() <= input.len()
        && begin_classes
            .iter()
            .enumerate()
            .all(|(i, class)| class.contains_byte(input[i]))
}

fn fwd_lb_prefix_impl<const FIRST: bool>(
    fwd: &mut ldfa::LDFA,
    lb_verify: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    lb_len: usize,
    fwd_lb_begin_nullable: bool,
    fwd_lb_begin_len: usize,
    fwd_lb_begin_classes: &[crate::accel::TSet],
    body_nullable: bool,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    mut search_start: usize,
    matches: &mut Vec<Match>,
) -> Result<bool, Error> {
    if fwd_lb_begin_nullable && begin_path_matches(fwd_lb_begin_classes, input) {
        let body_start = fwd_lb_begin_len;
        let max_end = if FIRST {
            if fwd.matches_from(b, body_start, input)? {
                return Ok(true);
            }
            None
        } else {
            fwd.scan_fwd_optional(b, body_start, input)?
        };
        if let Some(max_end) = max_end {
            matches.push(Match {
                start: body_start,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > body_start && body_nullable
                && try_emit_zero_width(fwd, lb_verify, b, lb_len, fwd_prefix, input, max_end, matches)? {
                    emitted_zw = true;
                }
            // back up lb_len so it can be re-checked for next match
            search_start = if emitted_zw {
                (max_end + 1).saturating_sub(lb_len)
            } else {
                max_end.saturating_sub(lb_len)
            };
        }
    }

    while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
        let body_start = candidate + lb_len;
        if body_start > input.len() {
            search_start = candidate + 1;
            continue;
        }
        if !verify_lb_literal(lb_verify, b, candidate, lb_len, input)? {
            search_start = candidate + 1;
            continue;
        }
        if FIRST {
            if fwd.matches_from(b, body_start, input)? {
                return Ok(true);
            }
            search_start = candidate + 1;
            continue;
        }
        if let Some(max_end) = fwd.scan_fwd_from(
            b,
            ldfa::DFA_INITIAL as u32,
            body_start,
            input,
        )? {
            matches.push(Match {
                start: body_start,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > body_start && body_nullable
                && try_emit_zero_width(fwd, lb_verify, b, lb_len, fwd_prefix, input, max_end, matches)? {
                    emitted_zw = true;
                }
            search_start = if max_end > body_start {
                if emitted_zw {
                    (max_end + 1).saturating_sub(lb_len)
                } else {
                    max_end - lb_len
                }
            } else {
                candidate + 1
            };
        } else {
            search_start = candidate + 1;
        }
    }

    Ok(false)
}

impl Regex {
    pub(crate) fn is_match_fwd_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        neg_lb: Option<&crate::prefix::NegLb>,
        input: &[u8],
    ) -> Result<bool, Error> {
        debug_assert!(!input.is_empty());
        if is_exact_literal(
            fwd_prefix,
            self.fixed_length,
            self.init_flags.has_anchors(),
            self.init_flags.has_la(),
            neg_lb,
        ) {
            return Ok(fwd_prefix.find_fwd(input, 0).is_some());
        }
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.matches.clear();
        fwd_prefix_impl::<true>(
            &mut inner.fwd,
            &mut inner.b,
            self.fixed_length,
            self.init_flags.has_anchors(),
            self.init_flags.has_la(),
            neg_lb,
            fwd_prefix,
            input,
            &mut inner.matches,
        )?;
        Ok(!inner.matches.is_empty())
    }

    pub(crate) fn is_match_elim_prefix(&self, fwd_prefix: &FwdPrefixSearch, input: &[u8]) -> Result<bool, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let elim = Self::eliminated_root(inner)
            .ok_or(Error::InternalError("elim prefix without eliminated root"))?;
        if inner.ism_elim_fwd.is_none() {
            inner.ism_elim_fwd = Some(ldfa::LDFA::new_fwd(&mut inner.b, elim, inner.max_cap)?);
        }
        let crate::RegexInner { b, ism_elim_fwd, matches, .. } = inner;
        let fwd = ism_elim_fwd
            .as_mut()
            .ok_or(Error::InternalError("elim forward DFA missing"))?;
        matches.clear();
        fwd_prefix_impl::<true>(fwd, b, None, false, false, None, fwd_prefix, input, matches)?;
        Ok(!matches.is_empty())
    }

    pub(crate) fn is_match_fwd_budget(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        neg_lb: Option<&crate::prefix::NegLb>,
        input: &[u8],
    ) -> Result<Option<bool>, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        fwd_prefix_first_budget(&mut inner.fwd, &mut inner.b, neg_lb, fwd_prefix, input)
    }

    pub(crate) fn is_match_fwd_lb_prefix_from(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
        search_start: usize,
    ) -> Result<bool, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.matches.clear();
        let lb_verify = inner
            .lb_verify
            .as_mut()
            .ok_or(Error::InternalError("FwdLbPrefix without lb_verify automaton"))?;
        fwd_lb_prefix_impl::<true>(
            &mut inner.fwd,
            lb_verify,
            &mut inner.b,
            self.lb_check_bytes as usize,
            self.fwd_lb_begin_nullable,
            self.fwd_lb_begin_len as usize,
            &self.fwd_lb_begin_classes,
            self.fwd_lb_body_nullable,
            fwd_prefix,
            input,
            search_start,
            &mut inner.matches,
        )
    }

    pub(crate) fn find_all_fwd_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        neg_lb: Option<&crate::prefix::NegLb>,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_prefix_impl::<false>(
            &mut inner.fwd,
            &mut inner.b,
            self.fixed_length,
            self.init_flags.has_anchors(),
            self.init_flags.has_la(),
            neg_lb,
            fwd_prefix,
            input,
            &mut inner.matches,
        )?;
        Ok(inner.matches.clone())
    }

    pub(crate) fn find_all_fwd_lb_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        let lb_verify = inner
            .lb_verify
            .as_mut()
            .ok_or(Error::InternalError("FwdLbPrefix without lb_verify automaton"))?;
        fwd_lb_prefix_impl::<false>(
            &mut inner.fwd,
            lb_verify,
            &mut inner.b,
            self.lb_check_bytes as usize,
            self.fwd_lb_begin_nullable,
            self.fwd_lb_begin_len as usize,
            &self.fwd_lb_begin_classes,
            self.fwd_lb_body_nullable,
            fwd_prefix,
            input,
            0,
            &mut inner.matches,
        )?;
        Ok(inner.matches.clone())
    }
}
