pub use crate::simd::RevPrefixSearch;
pub use crate::simd::RevSearchBytes as MintermSearchValue;
pub use crate::simd::TSet;

pub enum FwdPrefixSearch {
    Literal(crate::simd::FwdLiteralSearch),
    Prefix(crate::simd::FwdPrefixSearch),
}

impl FwdPrefixSearch {
    pub fn len(&self) -> usize {
        match self {
            FwdPrefixSearch::Literal(s) => s.len(),
            FwdPrefixSearch::Prefix(s) => s.len(),
        }
    }

    #[inline(always)]
    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        match self {
            FwdPrefixSearch::Literal(s) => s.find_fwd(&haystack[start..]).map(|i| i + start),
            FwdPrefixSearch::Prefix(s) => s.find_fwd(haystack, start),
        }
    }

    /// bulk collect all fixed-length literal matches. returns true if this is a literal.
    pub fn find_all_literal(&self, haystack: &[u8], matches: &mut Vec<crate::Match>) -> bool {
        match self {
            FwdPrefixSearch::Literal(s) => {
                // Safety: Match is #[repr(C)] with fields (start: usize, end: usize),
                // identical layout to (usize, usize).
                let raw = unsafe {
                    &mut *(matches as *mut Vec<crate::Match> as *mut Vec<(usize, usize)>)
                };
                s.find_all_fixed(haystack, raw);
                true
            }
            _ => false,
        }
    }
}
