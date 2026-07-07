use crate::{accel::FwdPrefixSearch, ldfa, Error, Match, Regex, RegexBuilder};

fn fwd_prefix_impl(
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

    let lang_is_prefix_literal = fwd_prefix.is_literal()
        && fixed_length == Some(prefix_len as u32)
        && !has_anchors
        && !has_la
        && neg_lb.is_none();
    if lang_is_prefix_literal {
        fwd_prefix.find_all_literal(input, matches);
        return Ok(());
    }

    let mut search_start = 0;

    {
        let mt = fwd.mt_lookup[input[0] as usize];
        let state = fwd.begin_table[mt as usize] as u32;
        if state != fwd.pruned as u32 {
            if let Some(max_end) = fwd.scan_fwd_from(b, state, 1, input)? {
                if max_end > 0 {
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
                    search_start = max_end;
                    continue;
                }
            }
        }
        search_start = candidate + 1;
    }

    Ok(())
}

fn try_emit_zero_width(
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
        matches.push(Match { start: at, end: at });
        return Ok(true);
    }
    Ok(false)
}

fn fwd_lb_prefix_impl(
    fwd: &mut ldfa::LDFA,
    b: &mut RegexBuilder,
    lb_len: usize,
    fwd_lb_begin_nullable: bool,
    body_nullable: bool,
    fwd_prefix: &FwdPrefixSearch,
    input: &[u8],
    matches: &mut Vec<Match>,
) -> Result<(), Error> {
    let mut search_start = 0;

    if fwd_lb_begin_nullable {
        if let Some(max_end) = fwd.scan_fwd_optional(b, 0, input)? {
            matches.push(Match {
                start: 0,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > 0 && body_nullable {
                if try_emit_zero_width(fwd, b, lb_len, fwd_prefix, input, max_end, matches)? {
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
            matches.push(Match {
                start: body_start,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > body_start && body_nullable {
                if try_emit_zero_width(fwd, b, lb_len, fwd_prefix, input, max_end, matches)? {
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
                candidate + 1
            };
        } else {
            search_start = candidate + 1;
        }
    }

    Ok(())
}

impl Regex {
    pub(crate) fn find_all_fwd_prefix(
        &self,
        fwd_prefix: &FwdPrefixSearch,
        neg_lb: Option<&crate::prefix::NegLb>,
        input: &[u8],
    ) -> Result<Vec<Match>, Error> {
        debug_assert!(!input.is_empty());
        let inner = &mut *self.inner.lock().unwrap();
        inner.matches.clear();
        fwd_prefix_impl(
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
        fwd_lb_prefix_impl(
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
