pub use crate::simd::RevPrefixSearch;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128")))]
pub use crate::simd::TSet;

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128")))]
pub enum MintermSearchValue {
    Exact(crate::simd::RevSearchBytes),
    Range(crate::simd::RevSearchRanges),
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128")))]
impl MintermSearchValue {
    #[inline(always)]
    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        match self {
            MintermSearchValue::Exact(s) => s.find_rev(haystack),
            MintermSearchValue::Range(s) => s.find_rev(haystack),
        }
    }

    #[inline(always)]
    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        match self {
            MintermSearchValue::Exact(s) => s.find_fwd(haystack),
            MintermSearchValue::Range(s) => s.find_fwd(haystack),
        }
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128"))))]
pub enum MintermSearchValue {}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128"))))]
impl MintermSearchValue {
    pub fn find_rev(&self, _haystack: &[u8]) -> Option<usize> {
        match *self {}
    }

    pub fn find_fwd(&self, _haystack: &[u8]) -> Option<usize> {
        match *self {}
    }
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128")))]
#[cfg_attr(debug_assertions, derive(Debug))]
pub enum FwdPrefixSearch {
    Literal(crate::simd::FwdLiteralSearch),
    Prefix(crate::simd::FwdPrefixSearch),
    Range(crate::simd::FwdRangeSearch),
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128")))]
#[allow(dead_code)]
impl FwdPrefixSearch {
    pub fn is_literal(&self) -> bool {
        matches!(self, FwdPrefixSearch::Literal(_))
    }

    pub fn len(&self) -> usize {
        match self {
            FwdPrefixSearch::Literal(s) => s.len(),
            FwdPrefixSearch::Prefix(s) => s.len(),
            FwdPrefixSearch::Range(s) => s.len(),
        }
    }

    #[inline(always)]
    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        match self {
            FwdPrefixSearch::Literal(s) => s.find_fwd(&haystack[start..]).map(|i| i + start),
            FwdPrefixSearch::Prefix(s) => s.find_fwd(haystack, start),
            FwdPrefixSearch::Range(s) => s.find_fwd(haystack, start),
        }
    }

    /// bulk collect all fixed-length literal matches. returns true if this is a literal.
    pub fn variant_name(&self) -> &'static str {
        match self {
            FwdPrefixSearch::Literal(_) => "Literal",
            FwdPrefixSearch::Prefix(_) => "Teddy",
            FwdPrefixSearch::Range(_) => "Range",
        }
    }

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

// stub for non-x86_64: uninhabited enum, methods are unreachable
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128"))))]
pub enum FwdPrefixSearch {}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", all(target_arch = "wasm32", target_feature = "simd128"))))]
impl FwdPrefixSearch {
    pub fn is_literal(&self) -> bool {
        match *self {}
    }

    pub fn len(&self) -> usize {
        match *self {}
    }

    pub fn find_fwd(&self, _haystack: &[u8], _start: usize) -> Option<usize> {
        match *self {}
    }

    pub fn find_all_literal(&self, _haystack: &[u8], _matches: &mut Vec<crate::Match>) -> bool {
        match *self {}
    }
}
