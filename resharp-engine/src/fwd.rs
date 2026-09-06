use crate::{accel::FwdPrefixSearch, ldfa, Error, Match, Regex, RegexBuilder};
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
    Ok(b
        .nullability(lb_verify.state_nodes[state as usize])
        .has(Nullability::CENTER))
}

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

fn fwd_lb_prefix_impl(
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
    matches: &mut Vec<Match>,
) -> Result<(), Error> {
    let mut search_start = 0;

    if fwd_lb_begin_nullable && begin_path_matches(fwd_lb_begin_classes, input) {
        let body_start = fwd_lb_begin_len;
        if let Some(max_end) = fwd.scan_fwd_optional(b, body_start, input)? {
            matches.push(Match {
                start: body_start,
                end: max_end,
            });
            let mut emitted_zw = false;
            if max_end > body_start && body_nullable {
                if try_emit_zero_width(fwd, lb_verify, b, lb_len, fwd_prefix, input, max_end, matches)? {
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
        if body_start > input.len() {
            search_start = candidate + 1;
            continue;
        }
        if !verify_lb_literal(lb_verify, b, candidate, lb_len, input)? {
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
            if max_end > body_start && body_nullable {
                if try_emit_zero_width(fwd, lb_verify, b, lb_len, fwd_prefix, input, max_end, matches)? {
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
        let lb_verify = inner
            .lb_verify
            .as_mut()
            .ok_or(Error::InternalError("FwdLbPrefix without lb_verify automaton"))?;
        fwd_lb_prefix_impl(
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
            &mut inner.matches,
        )?;
        Ok(inner.matches.clone())
    }
}
