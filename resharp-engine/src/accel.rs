pub use crate::simd::RevTeddySearch;
pub use crate::simd::TSet;

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum Skipper {
    State(MintermSearchValue),
    Prefix(RevTeddySearch),
    Inner {
        search: RevTeddySearch,
        resume: u32,
        pruned: u32,
        window: u32,
    },
}

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SeqOffsetSearch {
    pub seq: RevTeddySearch,
    pub seq_len: usize,
    pub bound: Option<SeqBound>,
}

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum SeqBound {
    Bytes(crate::simd::RevSearchBytes),
    Ranges(crate::simd::RevSearchRanges),
}

impl SeqBound {
    #[inline(always)]
    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        match self {
            SeqBound::Bytes(s) => s.find_rev(haystack),
            SeqBound::Ranges(s) => s.find_rev(haystack),
        }
    }
}

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum MintermSearchValue {
    Exact(crate::simd::RevSearchBytes),
    Range(crate::simd::RevSearchRanges),
    SeqOffset(SeqOffsetSearch),
    All,
}

impl MintermSearchValue {
    #[inline(always)]
    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        match self {
            MintermSearchValue::Exact(s) => s.find_rev(haystack),
            MintermSearchValue::Range(s) => s.find_rev(haystack),
            MintermSearchValue::SeqOffset(s) => {
                let n = haystack.len();
                if n == 0 {
                    return None;
                }
                let ctx = s.seq_len.saturating_sub(1);
                let mut w = 64usize;
                loop {
                    let floor = n.saturating_sub(w);
                    let rb = s
                        .bound
                        .as_ref()
                        .and_then(|x| x.find_rev(&haystack[floor..]))
                        .map(|p| p + floor);
                    let seq_floor = floor.saturating_sub(ctx);
                    let rs = s
                        .seq
                        .find_rev(&haystack[seq_floor..], n - 1 - seq_floor)
                        .map(|p| p + seq_floor)
                        .filter(|&p| p >= floor);
                    let best = match (rs, rb) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        (Some(a), None) => Some(a),
                        (None, b) => b,
                    };
                    if best.is_some() || floor == 0 {
                        return best;
                    }
                    w *= 2;
                }
            }
            MintermSearchValue::All => Some(0),
        }
    }

    #[inline(always)]
    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        match self {
            MintermSearchValue::Exact(s) => s.find_fwd(haystack),
            MintermSearchValue::Range(s) => s.find_fwd(haystack),
            MintermSearchValue::SeqOffset(_) => {
                unreachable!("SeqOffset skip is only built for reverse DFA states")
            }
            MintermSearchValue::All => None,
        }
    }
}

#[cfg_attr(debug_assertions, derive(Debug))]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize, Clone)
)]
pub enum FwdPrefixSearch {
    Literal(crate::simd::FwdLiteralSearch),
    Prefix(crate::simd::FwdPrefixSearch),
    Range(crate::simd::FwdRangeSearch),
}

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
