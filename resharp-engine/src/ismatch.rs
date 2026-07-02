use crate::{Error, FindAll, Nullability, Regex};

impl Regex {
    // reverse-only part llmatch
    pub(crate) fn is_match_dfa(&self, input: &[u8]) -> Result<bool, Error> {
        if self.initial_nullability.has(Nullability::BEGIN)
            || self.initial_nullability.has(Nullability::END)
        {
            return Ok(true);
        }
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.nulls.clear();
        #[cfg(feature = "convergence_prefix")]
        if self.conv_prefix {
            let crate::RegexInner { rev_ts, b, nulls, conv_b, .. } = &mut *inner;
            rev_ts.collect_rev_first(b, input.len() - 1, input, nulls, conv_b.as_mut())?;
            return Ok(!nulls.is_empty());
        }
        inner
            .rev_ts
            .collect_rev_first(&mut inner.b, input.len() - 1, input, &mut inner.nulls, None)?;
        Ok(!inner.nulls.is_empty())
    }

    pub(crate) fn is_match_fwd_ts(&self, input: &[u8]) -> Result<bool, Error> {
        let inner = &mut *self.inner.lock().unwrap_or_else(|e| e.into_inner());
        Ok(inner
            .fwd_ts
            .scan_fwd_optional(&mut inner.b, 0, input)?
            .is_some())
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
        match self.find_all {
            FindAll::EmptyLang => Ok(false),
            FindAll::Anchored => Ok(self.find_anchored(input)?.is_some()),
            FindAll::EndAnchored => Ok(self.find_end_anchored(input)?.is_some()),
            FindAll::Hardened | FindAll::Dfa => self.is_match_dfa(input),
            FindAll::Bounded | FindAll::FwdPrefix | FindAll::FwdLbPrefix => {
                self.is_match_fwd_ts(input)
            }
        }
    }
}
