use crate::{Error, FindAll, Nullability, Regex};
use resharp_algebra::NodeId;

pub(crate) enum IsMatchDfa {
    Pending(NodeId),
    Fwd(crate::ldfa::LDFA),
    Rev(crate::ldfa::LDFA),
    Split(crate::ldfa::LDFA, crate::ldfa::LDFA),
    #[cfg(feature = "serialize")]
    Unneeded,
}

impl IsMatchDfa {
    #[cfg(feature = "serialize")]
    pub(crate) fn from_dump(dfa: Option<crate::ldfa::LDFA>, begin: Option<crate::ldfa::LDFA>) -> Self {
        match dfa {
            Some(rest) if begin.is_some() => IsMatchDfa::Split(begin.unwrap(), rest),
            Some(d) if d.is_forward => IsMatchDfa::Fwd(d),
            Some(d) => IsMatchDfa::Rev(d),
            None => IsMatchDfa::Unneeded,
        }
    }
}

impl Regex {
    pub(crate) fn eliminated_root(inner: &mut crate::RegexInner) -> Option<NodeId> {
        if inner.ism_elim.is_none() {
            let elim = match inner.is_match_dfa {
                IsMatchDfa::Pending(basis) if basis.contains_lookaround(&inner.b) => inner.b.try_elim_lookarounds(basis),
                _ => None,
            };
            inner.ism_elim = Some(elim);
        }
        inner.ism_elim.flatten()
    }

    fn resolve_is_match(inner: &mut crate::RegexInner) -> Result<(), Error> {
        let elim = Self::eliminated_root(inner);
        let crate::RegexInner { is_match_dfa, max_cap, b, .. } = inner;
        let IsMatchDfa::Pending(basis) = *is_match_dfa else {
            return Ok(());
        };
        *is_match_dfa = if let Some(elim) = elim {
            let with_ts = b.mk_concat(NodeId::TS, elim);
            let contains = b.mk_concat(with_ts, NodeId::TS);
            IsMatchDfa::Fwd(crate::ldfa::LDFA::new_fwd(b, contains, *max_cap)?)
        } else {
            let (begin, rest) = Self::split_begin_arms(b, basis);
            let rev = b.ts_rev_start(rest)?;
            let contains = b.mk_concat(rev, NodeId::TS);
            let rev = crate::ldfa::LDFA::new_rev(b, contains, *max_cap)?;
            match begin {
                Some(begin) => IsMatchDfa::Split(crate::ldfa::LDFA::new_fwd(b, begin, *max_cap)?, rev),
                None => IsMatchDfa::Rev(rev),
            }
        };
        Ok(())
    }

    fn split_begin_arms(b: &mut resharp_algebra::RegexBuilder, basis: NodeId) -> (Option<NodeId>, NodeId) {
        if !basis.is_union(b) || b.contains_look(basis) {
            return (None, basis);
        }
        let mut arms = Vec::new();
        basis.iter_union_while(b, &mut |_, arm| {
            arms.push(arm);
            true
        });
        let (begin, rest): (Vec<NodeId>, Vec<NodeId>) = arms.into_iter().partition(|&arm| b.is_begin_anchored(arm));
        if begin.is_empty() || rest.is_empty() {
            return (None, basis);
        }
        (Some(b.mk_unions(begin.into_iter())), b.mk_unions(rest.into_iter()))
    }

    #[cfg(feature = "serialize")]
    pub(crate) fn prepare_is_match_for_dump(&self, inner: &mut crate::RegexInner) -> Result<(), Error> {
        match self.find_all {
            FindAll::EmptyLang
            | FindAll::Anchored
            | FindAll::EndAnchored
            | FindAll::ClassPlus
            | FindAll::FwdPrefix => Ok(()),
            FindAll::FwdLbPrefix if self.is_match_prefix.is_none() => Ok(()),
            FindAll::Bounded => Ok(()),
            _ => {
                Self::resolve_is_match(inner)?;
                if matches!(inner.is_match_dfa, IsMatchDfa::Unneeded) {
                    return Err(Error::InternalError("missing is_match DFA"));
                }
                Ok(())
            }
        }
    }

    pub(crate) fn is_match_dfa(&self, input: &[u8]) -> Result<bool, Error> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.nulls.clear();
        Self::resolve_is_match(inner)?;
        let crate::RegexInner { is_match_dfa, b, nulls, .. } = inner;
        let rev = match is_match_dfa {
            IsMatchDfa::Fwd(dfa) => return dfa.matches_from(b, 0, input),
            IsMatchDfa::Rev(dfa) => dfa,
            IsMatchDfa::Split(begin, rest) => {
                if begin.matches_from(b, 0, input)? {
                    return Ok(true);
                }
                rest
            }
            #[cfg(feature = "serialize")]
            IsMatchDfa::Unneeded => return Err(Error::InternalError("missing is_match DFA")),
            IsMatchDfa::Pending(_) => unreachable!("resolve_is_match leaves no pending state"),
        };
        rev.collect_rev_first(b, input.len() - 1, input, nulls, None)?;
        Ok(!nulls.is_empty())
    }

    fn is_match_class_plus(&self, input: &[u8]) -> bool {
        let table = self
            .class_plus
            .expect("FindAll::ClassPlus requires a materialized class membership table");
        input
            .iter()
            .any(|&b| table[(b >> 6) as usize] & (1u64 << (b & 63)) != 0)
    }

    fn is_match_anchored(&self, input: &[u8]) -> Result<bool, Error> {
        if self.fwd_lb_stripped {
            return Err(Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern));
        }
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.fwd.matches_from(&mut inner.b, 0, input)
    }

    fn is_match_end_anchored(&self, input: &[u8]) -> Result<bool, Error> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let crate::RegexInner { b, rev, .. } = inner;
        let rev = rev.as_mut().expect("EndAnchored requires a reverse DFA");
        rev.matches_rev_from(b, input.len(), 0, input)
    }

    pub(crate) fn required_literal_absent(&self, input: &[u8]) -> bool {
        self.is_match_prefix.as_ref().is_some_and(|p| p.find_fwd(input, 0).is_none())
            || self.is_match_extra.iter().any(|p| p.find_fwd(input, 0).is_none())
    }

    pub(crate) fn forward_rejects(&self, input: &[u8]) -> Result<bool, Error> {
        match &self.is_match_fwd {
            Some(fp) if !input.is_empty() => {
                Ok(self.is_match_fwd_budget(fp, self.neg_lb.as_ref(), input)? == Some(false))
            }
            _ => Ok(false),
        }
    }

    /// whether the pattern matches anywhere in the input.
    ///
    /// faster than `find_all` when you only need a yes/no answer.
    pub fn is_match(&self, input: &[u8]) -> Result<bool, Error> {
        if input.is_empty() {
            #[cfg(feature = "debug")]
            eprintln!(
                "[is_match] path=empty_input empty_nullable={}",
                self.empty_nullable
            );
            return Ok(self.empty_input_match().is_some());
        }
        #[cfg(all(feature = "debug", debug_assertions))]
        eprintln!("[is_match] path={:?}", self.find_all);
        if self.initial_nullability.has(Nullability::BEGIN | Nullability::END) {
            return Ok(true);
        }
        if input.len() < self.min_length as usize {
            return Ok(false);
        }
        if self.fwd_begin_anchored
            && self.rev_end_anchored
            && !self.init_flags.has_lb()
            && !self.init_flags.has_la()
            && self.fixed_length.is_some_and(|len| input.len() != len as usize)
        {
            return Ok(false);
        }
        match self.find_all {
            FindAll::EmptyLang => Ok(false),
            FindAll::Anchored => self.is_match_anchored(input),
            FindAll::EndAnchored => self.is_match_end_anchored(input),
            FindAll::ClassPlus => Ok(self.is_match_class_plus(input)),
            FindAll::FwdPrefix => {
                let Some(crate::prefix::PrefixKind::AnchoredFwd(fp)) = &self.prefix else {
                    return Err(Error::InternalError("FwdPrefix without AnchoredFwd prefix"));
                };
                if self.required_literal_absent(input) {
                    return Ok(false);
                }
                let eliminated = self.init_flags.has_la() && {
                    let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    Self::eliminated_root(inner).is_some()
                };
                if eliminated {
                    return self.is_match_elim_prefix(fp, input);
                }
                self.is_match_fwd_prefix(fp, self.neg_lb.as_ref(), input)
            }
            FindAll::FwdLbPrefix => {
                let search_start = if let Some(prefix) = &self.is_match_prefix {
                    match prefix.find_fwd(input, 0) {
                        None => return Ok(false),
                        Some(_) if input.len() <= 256 => return self.is_match_dfa(input),
                        Some(pos) => pos.saturating_sub(self.lb_check_bytes as usize),
                    }
                } else {
                    0
                };
                let Some(crate::prefix::PrefixKind::AnchoredFwdLb(fp)) = &self.prefix else {
                    return Err(Error::InternalError("FwdLbPrefix without AnchoredFwdLb prefix"));
                };
                self.is_match_fwd_lb_prefix_from(fp, input, search_start)
            }
            FindAll::Bounded => self.is_match_bounded(input),
            _ => {
                if self.required_literal_absent(input) {
                    return Ok(false);
                }
                if let Some(fp) = &self.is_match_fwd {
                    if let Some(found) = self.is_match_fwd_budget(fp, self.neg_lb.as_ref(), input)? {
                        return Ok(found);
                    }
                }
                if let Some(prefix) = &self.is_match_prefix {
                    if self.fixed_length == Some(prefix.len() as u32)
                        && !self.init_flags.has_anchors()
                        && !self.init_flags.has_la()
                        && !self.init_flags.has_lb()
                        && self.neg_lb.is_none()
                    {
                        return Ok(true);
                    }
                }
                self.is_match_dfa(input)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_input_does_not_build_match_states() {
        let re = Regex::new(r"[0-9a-f]{64}\.json").unwrap();
        let before = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert!(!re.is_match(&[b'f'; 63]).unwrap());
        let after = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert_eq!(before, after);
        let mut input = vec![b'f'; 64];
        input.extend_from_slice(b".json");
        assert!(re.is_match(&input).unwrap());
    }

    #[test]
    fn short_input_does_not_build_find_states() {
        let re = Regex::new(r"[0-9a-f]{64}\.json").unwrap();
        let before = {
            let inner = re.inner.lock().unwrap();
            (inner.fwd.state_nodes.len(), inner.rev_ts.as_ref().map_or(0, |d| d.state_nodes.len()))
        };
        assert!(re.find_all(&[b'f'; 63]).unwrap().is_empty());
        assert_eq!(re.find_anchored(&[b'f'; 63]).unwrap(), None);
        let after = {
            let inner = re.inner.lock().unwrap();
            (inner.fwd.state_nodes.len(), inner.rev_ts.as_ref().map_or(0, |d| d.state_nodes.len()))
        };
        assert_eq!(before, after);
        let mut input = vec![b'f'; 64];
        input.extend_from_slice(b".json");
        assert_eq!(re.find_all(&input).unwrap(), vec![crate::Match { start: 0, end: 69 }]);
        assert_eq!(re.find_anchored(&input).unwrap(), Some(crate::Match { start: 0, end: 69 }));
    }

    #[test]
    fn short_input_preserves_unsupported_anchored_error() {
        let re = Regex::new(r"(?<=a)bc").unwrap();
        assert!(matches!(
            re.find_anchored(b"b"),
            Err(Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern))
        ));
    }

    #[test]
    fn fixed_length_fully_anchored_rejects_long_input_before_building_states() {
        let re = Regex::with_options(r"^true$", crate::RegexOptions::default().multiline(false)).unwrap();
        assert!(re.fwd_begin_anchored && re.rev_end_anchored);
        assert_eq!(re.fixed_length, Some(4));
        let before = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert!(!re.is_match(b"truex").unwrap());
        let after = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert_eq!(before, after);
        assert!(re.is_match(b"true").unwrap());
    }

    #[test]
    fn simd_literal_tails_find_every_position() {
        for (pattern, needle) in [("//", &b"//"[..]), ("q", b"q"), ("xy[0-9]", b"xy1"), ("[ab]/", b"a/"), ("needle", b"needle"), ("abcdefghijklmnopqrs", b"abcdefghijklmnopqrs")] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            for len in needle.len()..110 {
                assert!(!re.is_match(&vec![b'.'; len]).unwrap(), "{pattern} len={len}");
                for at in 0..=len - needle.len() {
                    let mut input = vec![b'.'; len];
                    input[at..at + needle.len()].copy_from_slice(needle);
                    assert!(re.is_match(&input).unwrap(), "{pattern} len={len} at={at}");
                    let found = re.find_all(&input).unwrap();
                    assert_eq!(found.first().map(|m| m.start), Some(at), "{pattern} len={len} at={at}");
                }
            }
        }
    }

    #[test]
    fn rev_literal_tails_find_every_position() {
        for pattern in [r"[0-9a-f]{2,}\.json", r"\w+\.json", r"[a-z]+@q", r"[0-9a-f]+xyzzy1"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            let needle = if pattern.ends_with("json") { &b"ab.json"[..] } else if pattern.ends_with('q') { &b"ab@q"[..] } else { &b"abxyzzy1"[..] };
            for len in 0usize..140 {
                for at in 0..=len.saturating_sub(needle.len()) {
                    let mut input = vec![b'.'; len];
                    if len >= needle.len() {
                        input[at..at + needle.len()].copy_from_slice(needle);
                    }
                    let expected: Vec<(usize, usize)> = oracle.find_iter(&input).map(|m| (m.start(), m.end())).collect();
                    let found: Vec<(usize, usize)> = if input.is_empty() { vec![] } else { re.find_all(&input).unwrap().iter().map(|m| (m.start, m.end)).collect() };
                    assert_eq!(found, expected, "{pattern} len={len} at={at}");
                }
            }
        }
    }

    #[test]
    fn find_all_required_literal_reject_agrees_with_oracle() {
        for pattern in [r"\w+\.json", r"[0-9a-f]{6,64}\.json", r"[a-z]+@[a-z]+\.com", r"\w+://\w+", r"[0-9]+px", r"x[a-z]*function", r"\$([$&]|[0-9]+)", ">", "<a>", "a>b"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["", "plain text", "a.json", "abcdef.json x.json", "ab@cd.com ef@gh.co", "http://x ftp://y", "12px 3p", "xfunction xabfunction", "$& $12 $", "json .json", "://", "px", "<a>", "a>b", "x>", ">"] {
                let expected: Vec<(usize, usize)> = oracle.find_iter(input.as_bytes()).map(|m| (m.start(), m.end())).collect();
                let found: Vec<(usize, usize)> = re.find_all(input.as_bytes()).unwrap().iter().map(|m| (m.start, m.end)).collect();
                assert_eq!(found, expected, "{pattern} {input:?}");
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), !expected.is_empty(), "is_match {pattern} {input:?}");
            }
        }
    }

    #[test]
    fn begin_anchored_arm_split_agrees_with_oracle() {
        for pattern in [r"[ \t]+$|^[ \t]+", r"^a|b$", r"^a|b", r"^ab+|cd", r"^[a-c]+x|y[a-c]*$|zz", r"(^a|b)c"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["a", "b", " ", "x ", " x", "x y", "ab", "abb", "xab", "xb", "cd", "xcdx", "abcx", "cx", "yab", "ya", "zz", "xzzx", "bc", "ac", "xbc", "z"] {
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{pattern} {input:?}");
            }
        }
        let re = Regex::with_options(r"[ \t]+$|^[ \t]+", crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
        assert!(re.is_match(b" x").unwrap());
        assert!(matches!(re.inner.lock().unwrap().is_match_dfa, IsMatchDfa::Split(..)));
    }

    #[test]
    fn multiple_required_literals_agree_with_oracle() {
        for pattern in [r"!\[[^]]*\]\([^)]{3,}\)", r"[a-z]+@[a-z]+\.com/path", r"x\w+needle\w+tail", r"\w+://host[a-z]*/end"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            assert!(!re.is_match_extra.is_empty(), "{pattern}");
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["", "![a](xyz)", "![a]", "](xyz)", "![a](xy)", "vec![a, b]; f(x)", "a@b.com/path", "a@b.com", "/path", "xanyneedleabtail", "xanyneedle", "needletail", "http://host/end", "http://hostab/end", "://host", "/end"] {
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{pattern} {input:?}");
                assert_eq!(re.find_all(input.as_bytes()).unwrap().len(), oracle.find_iter(input.as_bytes()).count(), "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn potential_start_prefilter_agrees_with_oracle() {
        for pattern in [r"\\u[0-9A-Fa-f]{4}|(\\\\)+", r"(ab|cd)[0-9]+x*"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            assert!(!re.is_match_extra.is_empty(), "{pattern}");
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["", "a", "\\", "\\n", "\\u00e9", "\\u12", "x\\\\y", "\\\\\\", "ab", "ab1", "cd22x", "xz", "yz9", "qqr", "r", "a\\u00E9"] {
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{pattern} {input:?}");
                assert_eq!(re.find_all(input.as_bytes()).unwrap().len(), oracle.find_iter(input.as_bytes()).count(), "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn begin_anchored_arm_is_not_dropped_by_potential_start_prefilter() {
        for pattern in [r"(a|^)[ ]*<", r"(\r|\r?\n|^)[ \t]*<[^\r]+", r"([^\\]|^)(/\*.*\*/|//[^\r]*)"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["<x", " <x", "a<x", "//x", "/*x*/", "x//y", "\\//y", "\r\n <q", "b", ""] {
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn is_match_forward_direction_agrees_with_oracle() {
        let pattern = r"(className|data-[0-9A-Z_a-z]+|style)[ \t]*=[ \t]*\{[^}]{0,100}\{[^}]{0,100}\}[^}]{0,100}\}";
        let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
        assert!(re.is_match_fwd.is_some());
        let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
        for input in ["", "className", "className={a{b}c}", "style = { {x} y }", "data-id={{}}}", "data-x-y={a{b}c}", "xdata-={ { } }", "style={a}", "className=}", "style={{}}", "a style={{}} b }", "data-1 = {q{r}s}"] {
            assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{input:?}");
            assert_eq!(re.find_all(input.as_bytes()).unwrap().len(), oracle.find_iter(input.as_bytes()).count(), "{input:?}");
        }
    }

    #[test]
    fn is_match_forward_budget_agrees_with_oracle() {
        let long = "foo".repeat(20_000);
        let long_hit = format!("{long}!");
        for pattern in [r"function[ \t]+[0-9A-Z_a-z]+[ \t]*\([^)]*\)[ \t]*\{", r"foo[a-z]+!"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            for input in ["", "function", "function f() {", "function  f ( a ) {x}", "function f(", "functionfunction f(){", "x function\tg(\n) {", "function f() ", "foo!", "fooa!", "foofoo", &long, &long_hit, &long_hit[1..]] {
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), oracle.is_match(input.as_bytes()), "{pattern} {:?}", &input[..input.len().min(40)]);
                let got: Vec<(usize, usize)> = re.find_all(input.as_bytes()).unwrap().iter().map(|m| (m.start, m.end)).collect();
                let want: Vec<(usize, usize)> = oracle.find_iter(input.as_bytes()).map(|m| (m.start(), m.end())).collect();
                assert_eq!(got, want, "find_all {pattern} {:?}", &input[..input.len().min(40)]);
            }
        }
        let re = Regex::with_options(r"function[ \t]+[0-9A-Z_a-z]+[ \t]*\([^)]*\)[ \t]*\{", crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
        assert!(re.is_match_fwd.is_some());
    }

    #[test]
    fn teddy_bucket_masks_find_every_position() {
        for pattern in [r#"["'`][a-c]{3}[x-z]"#, r"[<>&][a-z]{2}[0-9]", r"[\x80-\xff.][a-c]{2}[x-z]"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let oracle = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
            let hit: &[u8] = match pattern.as_bytes()[1] {
                b'"' => b"'abcx",
                b'<' => b">ab1",
                _ => b".abx",
            };
            for len in 0..140usize {
                for at in (0..len.saturating_sub(hit.len() - 1)).step_by(1) {
                    let mut input = b" bg`g\"b".iter().copied().cycle().take(len).collect::<Vec<u8>>();
                    input[at..at + hit.len()].copy_from_slice(hit);
                    let got: Vec<(usize, usize)> = re.find_all(&input).unwrap().iter().map(|m| (m.start, m.end)).collect();
                    let want: Vec<(usize, usize)> = oracle.find_iter(&input).map(|m| (m.start(), m.end())).collect();
                    assert_eq!(got, want, "{pattern} len={len} at={at}");
                    assert_eq!(re.is_match(&input).unwrap(), !want.is_empty(), "{pattern} len={len} at={at}");
                }
            }
        }
    }

    #[test]
    fn inner_required_literal_prefilter_agrees_with_find_all() {
        for pattern in [r"[0-9a-f]{4}\.json", r"[a-z]+@[a-z]+\.com", r"\w+://\w+", r"[0-9]+px", r"(a|b)c+needle[0-9]"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            assert!(re.is_match_prefix.is_some(), "{pattern}");
            for input in ["abcd.json", "abcd.jso", ".json", "a@b.com", "a@b.co", "@.com", "http://x", "://x", "ab://", "12px", "px", "1p", "acneedle1", "acneedle", "needle1", "xxneedle"] {
                let expected = !re.find_all(input.as_bytes()).unwrap().is_empty();
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), expected, "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn required_literal_prefilter_agrees_with_find_all() {
        for pattern in [r"\$([$&]|[0-9]+)", r"Version/[.0-9_]+[^\r]*Safari", r"<([0-9A-Z_a-z]*:)?dimension"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            assert!(re.is_match_prefix.is_some(), "{pattern}");
            for input in ["$", "$1", "x$&", "Version/1.2 Safari", "Version/ Safari", "V", "<dimension", "<a:dimension", "<a:dimensio", "abcd", "abcdx", "xabbd", "nothing"] {
                let expected = !re.find_all(input.as_bytes()).unwrap().is_empty();
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), expected, "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn lookbehind_first_acceptance_matches_longest_scan() {
        let mut tested = 0;
        for pattern in [r"\bxml\b", r"\bxml(?:a|ab{1,80})", r"(?<=ab)xml", r"(?<!a)xml", r"\bxml(?=x)", r"(?<=a)b*", r"(?<=a)b?$"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap_or_else(|e| panic!("{pattern}: {e:?}"));
            let Some(crate::prefix::PrefixKind::AnchoredFwdLb(fp)) = &re.prefix else { continue };
            tested += 1;
            for input in ["x", "xml", "!xml!", "axml xmlx", "abxml", "abxmlx", "xml2 xml", "!xml2", "abxml!xml", "a", "aa", "ab", "a!", "foo"] {
                let expected = !re.find_all(input.as_bytes()).unwrap().is_empty();
                assert_eq!(re.is_match_fwd_lb_prefix_from(fp, input.as_bytes(), 0).unwrap(), expected, "{pattern} {input:?}");
            }
        }
        assert!(tested >= 5);
    }

    #[test]
    fn lookbehind_is_match_stops_after_first_accepting_end() {
        let re = Regex::with_options(r"\bxml(?:a|ab{1,80})", crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
        assert_eq!(re.find_all_kind_name(), "FwdLbPrefix");
        let input = format!("{} xmla{}", "!".repeat(512), "b".repeat(80));
        let before = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert!(re.is_match(input.as_bytes()).unwrap());
        let after = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert!(after - before < 20, "created {} states", after - before);
        assert_eq!(re.find_all(input.as_bytes()).unwrap()[0].end, input.len());
    }

    #[test]
    fn lookbehind_prefix_starts_from_first_body_literal() {
        for pattern in [r"\bxml\b", r"(?<=ab)xml", r"(?<!a)xml", r"\bxml(?=x)"] {
            let re = Regex::with_options(pattern, crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
            let Some(crate::prefix::PrefixKind::AnchoredFwdLb(fp)) = &re.prefix else { panic!("{pattern}") };
            let body = re.is_match_prefix.as_ref().unwrap();
            let inputs = [
                "xml".to_string(), "!xml!".into(), "axml xmlx".into(), "abxml".into(),
                "abxmlx".into(), "xml2 xml".into(), "!xml2".into(), "abxml!xml".into(),
                format!("{} abxmlx", "a".repeat(512)),
                format!("xml2{} abxmlx", "a".repeat(512)),
            ];
            for input in inputs {
                let Some(pos) = body.find_fwd(input.as_bytes(), 0) else { continue };
                let start = pos.saturating_sub(re.lb_check_bytes as usize);
                let expected = !re.find_all(input.as_bytes()).unwrap().is_empty();
                assert_eq!(re.is_match_fwd_lb_prefix_from(fp, input.as_bytes(), start).unwrap(), expected, "{pattern} {input:?}");
                assert_eq!(re.is_match(input.as_bytes()).unwrap(), expected, "{pattern} {input:?}");
            }
        }
    }

    #[test]
    fn word_boundary_elimination_uses_literal_prefilter() {
        let re = Regex::with_options(r"\bxml\b", crate::RegexOptions::default().unicode(crate::UnicodeMode::Ascii).multiline(false)).unwrap();
        assert!(!re.is_match(b"no match here").unwrap());
        assert!(matches!(re.inner.lock().unwrap().is_match_dfa, IsMatchDfa::Pending(_)));
        assert!(re.is_match(format!("{} xml", "a".repeat(256)).as_bytes()).unwrap());
        assert!(matches!(re.inner.lock().unwrap().is_match_dfa, IsMatchDfa::Pending(_)));
        for (input, expected) in [("xml", true), ("!xml!", true), ("éxmlé", true), ("axml", false), ("xml2", false)] {
            assert_eq!(re.is_match(input.as_bytes()).unwrap(), expected, "{input:?}");
        }
        assert!(matches!(re.inner.lock().unwrap().is_match_dfa, IsMatchDfa::Fwd(_)));
    }

    #[test]
    fn anchored_is_match_stops_at_first_accepting_end() {
        let re = Regex::with_options(r"^a+b?", crate::RegexOptions::default().multiline(false)).unwrap();
        assert_eq!(re.find_all_kind_name(), "Anchored");
        assert!(re.is_match(b"a").unwrap());
        let before = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert!(re.is_match(b"ab").unwrap());
        let after = re.inner.lock().unwrap().fwd.state_nodes.len();
        assert_eq!(before, after);
    }

    #[test]
    fn end_anchored_is_match_stops_at_first_accepting_start() {
        let re = Regex::with_options(r"a?b+$", crate::RegexOptions::default().multiline(false)).unwrap();
        assert_eq!(re.find_all_kind_name(), "EndAnchored");
        assert!(re.is_match(b"b").unwrap());
        let before = re.inner.lock().unwrap().rev.as_ref().unwrap().state_nodes.len();
        assert!(re.is_match(b"aaab").unwrap());
        let after = re.inner.lock().unwrap().rev.as_ref().unwrap().state_nodes.len();
        assert_eq!(before, after);
    }

    #[test]
    fn contains_dfas_have_both_stars() {
        for (pattern, forward) in [(r"^a", true), (r"[^:]//+", false)] {
            let re = Regex::new(pattern).unwrap();
            let inner = &mut *re.inner.lock().unwrap();
            let IsMatchDfa::Pending(basis) = inner.is_match_dfa else { panic!("expected pending DFA") };
            let expected = if forward {
                let elim = inner.b.try_elim_lookarounds(basis).unwrap();
                inner.b.mk_concat(NodeId::TS, elim)
            } else {
                inner.b.ts_rev_start(basis).unwrap()
            };
            let expected = inner.b.mk_concat(expected, NodeId::TS);
            Regex::resolve_is_match(inner).unwrap();
            let dfa = match (&inner.is_match_dfa, forward) {
                (IsMatchDfa::Fwd(dfa), true) | (IsMatchDfa::Rev(dfa), false) => dfa,
                _ => panic!("unexpected DFA direction for {pattern}"),
            };
            assert_eq!(dfa.state_nodes[crate::ldfa::DFA_INITIAL as usize], expected);
        }
    }
}
