#![allow(dead_code)]
use crate::{accel::FwdPrefixSearch, ldfa, Error, Match, Regex, RegexBuilder};

fn fwd_prefix_impl<const IS_MATCH: bool>(
    fwd: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    fixed_length: Option<u32>,
    has_anchors: bool,
    has_la: bool,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    matches: &mut Vec<Match>,
) -> Result<bool, Error> {
    let prefix_len = fwd_prefix.len();

    let lang_is_prefix_literal = fwd_prefix.is_literal()
        && fixed_length == Some(prefix_len as u32)
        && !has_anchors
        && !has_la;
    if lang_is_prefix_literal {
        if IS_MATCH {
            return Ok(fwd_prefix.find_fwd(input, 0).is_some());
        }
        fwd_prefix.find_all_literal(input, matches);
        return Ok(false);
    }

    let mut search_start = 0;

    {
        let mt = fwd.mt_lookup[input[0] as usize];
        let state = fwd.begin_table[mt as usize] as u32;
        if state != fwd.pruned as u32 {
            if let Some(max_end) = fwd.scan_fwd_from(b, state, 1, input)? {
                if max_end > 0 {
                    if IS_MATCH {
                        return Ok(true);
                    }
                    matches.push(Match {
                        start: 0,
                        end: max_end,
                    });
                    search_start = max_end;
                }
            }
        }
    }

    while let Some(candidate) = fwd_prefix.find_fwd(input, search_start) {
        let state = fwd.walk_input(b, candidate, prefix_len, input)?;
        if state != 0 {
            if let Some(max_end) = fwd.scan_fwd_from(b, state, candidate + prefix_len, input)? {
                if max_end > candidate {
                    if IS_MATCH {
                        return Ok(true);
                    }
                    matches.push(Match {
                        start: candidate,
                        end: max_end,
                    });
                    search_start = max_end;
                    continue;
                }
            }
        }
        search_start = candidate + 1;
    }

    Ok(false)
}

fn try_emit_zero_width<const IS_MATCH: bool>(
    fwd: &mut ldfa::LDFA,
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
    if fwd.scan_fwd_from(b, ldfa::DFA_INITIAL as u32, at, input)? == Some(at) {
        if IS_MATCH {
            return Ok(true);
        }
        matches.push(Match { start: at, end: at });
        return Ok(true);
    }
    Ok(false)
}

fn fwd_lb_prefix_impl<const IS_MATCH: bool>(
    fwd: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    lb_len: usize,
    fwd_lb_begin_nullable: bool,
    body_nullable: bool,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    matches: &mut Vec<Match>,
) -> Result<bool, Error> {
    let mut search_start = 0;

    if fwd_lb_begin_nullable {
        if let Some(max_end) = fwd.scan_fwd_slow(b, 0, input)? {
            if IS_MATCH {
                return Ok(true);
            }
            matches.push(Match {
                start: 0,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > 0 && body_nullable {
                if try_emit_zero_width::<IS_MATCH>(fwd, b, lb_len, fwd_prefix, input, max_end, matches)? {
                    if IS_MATCH {
                        return Ok(true);
                    }
                    emitted_zw = true;
                }
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
        if let Some(max_end) = fwd.scan_fwd_from(
            b,
            ldfa::DFA_INITIAL as u32,
            body_start,
            input,
        )? {
            if IS_MATCH {
                return Ok(true);
            }
            matches.push(Match {
                start: body_start,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > body_start && body_nullable {
                if try_emit_zero_width::<IS_MATCH>(fwd, b, lb_len, fwd_prefix, input, max_end, matches)? {
                    if IS_MATCH {
                        return Ok(true);
                    }
                    emitted_zw = true;
                }
            }
            search_start = if max_end > body_start {
                if emitted_zw {
                    (max_end + 1).saturating_sub(lb_len)
                } else {
                    max_end - lb_len
                }
            } else {
                body_start
            };
        } else {
            search_start = body_start;
        }
    }

    Ok(false)
}

impl Regex {
    pub(crate) fn find_all_fwd_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_prefix_impl::<false>(
            &mut inner.fwd,
            &mut inner.b,
            self.fixed_length,
            self.has_anchors,
            self.has_la,
            fwd_prefix,
            input,
            &mut inner.matches,
        )?;
        Ok(inner.matches.clone())
    }

    pub(crate) fn is_match_fwd_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
    ) -> Result<bool, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_prefix_impl::<true>(
            &mut inner.fwd,
            &mut inner.b,
            self.fixed_length,
            self.has_anchors,
            self.has_la,
            fwd_prefix,
            input,
            &mut inner.matches,
        )
    }

    pub(crate) fn is_match_fwd_lb_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
    ) -> Result<bool, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_lb_prefix_impl::<true>(
            &mut inner.fwd,
            &mut inner.b,
            self.lb_check_bytes as usize,
            self.fwd_lb_begin_nullable,
            self.fwd_lb_body_nullable,
            fwd_prefix,
            input,
            &mut inner.matches,
        )
    }

    pub(crate) fn find_all_fwd_lb_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_lb_prefix_impl::<false>(
            &mut inner.fwd,
            &mut inner.b,
            self.lb_check_bytes as usize,
            self.fwd_lb_begin_nullable,
            self.fwd_lb_body_nullable,
            fwd_prefix,
            input,
            &mut inner.matches,
        )?;
        Ok(inner.matches.clone())
    }
}
