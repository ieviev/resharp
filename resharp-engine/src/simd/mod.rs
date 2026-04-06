pub use resharp_algebra::solver::TSet;

mod byte_freq;
pub use byte_freq::BYTE_FREQ;

#[inline]
pub fn has_simd() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(target_arch = "aarch64")]
    {
        true
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        false
    }
}

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
mod neon;
#[cfg(target_arch = "aarch64")]
pub use neon::*;

#[cfg(target_arch = "x86_64")]
pub struct RevSearchBytes {
    bytes: Vec<u8>,
}

#[cfg(target_arch = "x86_64")]
impl RevSearchBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        debug_assert!(!bytes.is_empty() && bytes.len() <= 3);
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_avx2::<true>(haystack) }
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_avx2::<false>(haystack) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn search_avx2<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let len = haystack.len();
        if len == 0 {
            return None;
        }
        let ptr = haystack.as_ptr();
        let v0 = _mm256_set1_epi8(self.bytes[0] as i8);
        let n = self.bytes.len();

        macro_rules! compute_mask {
            ($chunk:expr) => {{
                let chunk = $chunk;
                let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v0)) as u32;
                if n >= 2 {
                    let v1 = _mm256_set1_epi8(self.bytes[1] as i8);
                    mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v1)) as u32;
                }
                if n >= 3 {
                    let v2 = _mm256_set1_epi8(self.bytes[2] as i8);
                    mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v2)) as u32;
                }
                mask
            }};
        }

        if FWD {
            let mut pos = 0;
            while pos + 32 <= len {
                let mask = compute_mask!(_mm256_loadu_si256(ptr.add(pos) as *const __m256i));
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
                pos += 32;
            }
            if pos < len {
                let mut buf = [0u8; 32];
                buf[..len - pos].copy_from_slice(&haystack[pos..]);
                let mut mask =
                    compute_mask!(_mm256_loadu_si256(buf.as_ptr() as *const __m256i));
                mask &= (1u32 << (len - pos)) - 1;
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
            }
        } else {
            if len >= 32 {
                let mut pos = len - 32;
                loop {
                    let mask =
                        compute_mask!(_mm256_loadu_si256(ptr.add(pos) as *const __m256i));
                    if mask != 0 {
                        return Some(pos + 31 - mask.leading_zeros() as usize);
                    }
                    if pos < 32 {
                        break;
                    }
                    pos -= 32;
                }
            }
            let gap = if len >= 32 { len % 32 } else { len };
            if gap > 0 {
                let mut buf = [0u8; 32];
                buf[..gap].copy_from_slice(&haystack[..gap]);
                let mut mask =
                    compute_mask!(_mm256_loadu_si256(buf.as_ptr() as *const __m256i));
                mask &= (1u32 << gap) - 1;
                if mask != 0 {
                    return Some(31 - mask.leading_zeros() as usize);
                }
            }
        }
        None
    }
}

#[cfg(target_arch = "x86_64")]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FwdLiteralSearch {
    pub(crate) needle: Vec<u8>,
    chunks: Vec<u64>,
    rare_idx: usize,
    rare_byte: u8,
    confirm: (usize, u8),
}

#[cfg(target_arch = "x86_64")]
impl FwdLiteralSearch {
    pub fn len(&self) -> usize {
        self.needle.len()
    }

    pub fn rare_byte(&self) -> u8 {
        self.rare_byte
    }

    pub fn new(needle: &[u8]) -> Self {
        debug_assert!(!needle.is_empty());
        let mut rare_idx = 0;
        let mut rare_freq = BYTE_FREQ[needle[0] as usize];
        for (i, &b) in needle.iter().enumerate().skip(1) {
            let f = BYTE_FREQ[b as usize];
            if f < rare_freq {
                rare_freq = f;
                rare_idx = i;
            }
        }
        let confirm_idx = if needle.len() > 1 {
            let mut ci = if rare_idx == 0 { 1 } else { 0 };
            let mut cf = BYTE_FREQ[needle[ci] as usize];
            for (i, &b) in needle.iter().enumerate() {
                if i == rare_idx {
                    continue;
                }
                let f = BYTE_FREQ[b as usize];
                if f < cf {
                    cf = f;
                    ci = i;
                }
            }
            ci
        } else {
            0
        };
        let mut chunks = Vec::with_capacity((needle.len() + 7) / 8);
        let mut i = 0;
        while i + 8 <= needle.len() {
            let mut v = [0u8; 8];
            v.copy_from_slice(&needle[i..i + 8]);
            chunks.push(u64::from_ne_bytes(v));
            i += 8;
        }
        if i < needle.len() {
            let mut v = [0u8; 8];
            v[..needle.len() - i].copy_from_slice(&needle[i..]);
            chunks.push(u64::from_ne_bytes(v));
        }
        Self {
            rare_idx,
            rare_byte: needle[rare_idx],
            confirm: (confirm_idx, needle[confirm_idx]),
            needle: needle.to_vec(),
            chunks,
        }
    }

    #[inline]
    fn verify(&self, haystack: &[u8], start: usize) -> bool {
        let n = self.needle.len();
        unsafe {
            let hp = haystack.as_ptr().add(start);
            let mut ci = 0;
            let mut off = 0;
            while off + 8 <= n {
                let h = (hp.add(off) as *const u64).read_unaligned();
                if h != self.chunks[ci] {
                    return false;
                }
                ci += 1;
                off += 8;
            }
            if off < n {
                let h = (hp.add(off) as *const u64).read_unaligned();
                let mask = (1u64 << ((n - off) * 8)) - 1;
                if (h ^ self.chunks[ci]) & mask != 0 {
                    return false;
                }
            }
        }
        true
    }

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.find_fwd_avx2(haystack) }
    }

    pub fn find_all_fixed(&self, haystack: &[u8], matches: &mut Vec<(usize, usize)>) {
        unsafe { self.find_all_fixed_avx2(haystack, matches) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_all_fixed_avx2(&self, haystack: &[u8], matches: &mut Vec<(usize, usize)>) {
        let nlen = self.needle.len();
        if haystack.len() < nlen {
            return;
        }
        let ptr = haystack.as_ptr();
        let rare_idx = self.rare_idx;
        let rare_byte = self.rare_byte;
        let confirm_idx = self.confirm.0;
        let confirm_byte = self.confirm.1;
        let end = haystack.len() - nlen + rare_idx;
        let vrare = _mm256_set1_epi8(rare_byte as i8);
        let mut last_end: usize = 0;

        let mut pos = rare_idx;
        while pos + 32 <= end + 1 {
            let chunk = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, vrare)) as u32;
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if start >= last_end
                    && *ptr.add(start + confirm_idx) == confirm_byte
                    && self.verify(haystack, start)
                {
                    let m_end = start + nlen;
                    matches.push((start, m_end));
                    last_end = m_end;
                }
                mask &= mask - 1;
            }
            pos += 32;
        }
        // scalar tail
        while pos <= end {
            let start = pos - rare_idx;
            if start >= last_end
                && *ptr.add(pos) == rare_byte
                && *ptr.add(start + confirm_idx) == confirm_byte
                && self.verify(haystack, start)
            {
                let m_end = start + nlen;
                matches.push((start, m_end));
                last_end = m_end;
            }
            pos += 1;
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_fwd_avx2(&self, haystack: &[u8]) -> Option<usize> {
        let nlen = self.needle.len();
        if haystack.len() < nlen {
            return None;
        }
        let ptr = haystack.as_ptr();
        let rare_idx = self.rare_idx;
        let rare_byte = self.rare_byte;
        let confirm_idx = self.confirm.0;
        let confirm_byte = self.confirm.1;
        let end = haystack.len() - nlen + rare_idx;
        let vrare = _mm256_set1_epi8(rare_byte as i8);

        let mut pos = rare_idx;
        while pos + 32 <= end + 1 {
            let chunk = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, vrare)) as u32;
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if *ptr.add(start + confirm_idx) == confirm_byte && self.verify(haystack, start) {
                    return Some(start);
                }
                mask &= mask - 1;
            }
            pos += 32;
        }
        // scalar tail
        while pos <= end {
            if *ptr.add(pos) == rare_byte {
                let start = pos - rare_idx;
                if *ptr.add(start + confirm_idx) == confirm_byte && self.verify(haystack, start) {
                    return Some(start);
                }
            }
            pos += 1;
        }
        None
    }
}

#[cfg(target_arch = "x86_64")]
pub struct RevPrefixSearch {
    len: usize,
    num_simd: usize,
    masks: Box<TeddyMasks>,
    pub(crate) sets: Vec<TSet>,
}

#[cfg(target_arch = "x86_64")]
impl RevPrefixSearch {
    pub fn new(len: usize, byte_sets_raw: &[Vec<u8>], all_sets: Vec<TSet>) -> Self {
        debug_assert_eq!(all_sets.len(), len);
        debug_assert_eq!(byte_sets_raw.len(), len);

        let num_simd = len.min(3);
        let mut masks = Box::new(TeddyMasks {
            lo: [[0u8; 32]; 3],
            hi: [[0u8; 32]; 3],
        });

        for i in 0..num_simd {
            let mut lo = [0u8; 16];
            let mut hi = [0u8; 16];
            for &b in &byte_sets_raw[i] {
                lo[(b & 0xF) as usize] |= 0x80;
                hi[(b >> 4) as usize] |= 0x80;
            }
            masks.lo[i][..16].copy_from_slice(&lo);
            masks.lo[i][16..].copy_from_slice(&lo);
            masks.hi[i][..16].copy_from_slice(&hi);
            masks.hi[i][16..].copy_from_slice(&hi);
        }

        Self {
            len,
            num_simd,
            masks,
            sets: all_sets,
        }
    }

    // TBD: cleanup
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn find_rev(&self, haystack: &[u8], end: usize) -> Option<usize> {
        unsafe { self.find_rev_avx2(haystack, end) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_rev_avx2(&self, haystack: &[u8], end: usize) -> Option<usize> {
        match self.num_simd {
            1 => self.teddy_rev_1(haystack, end),
            2 => self.teddy_rev_2(haystack, end),
            _ => self.teddy_rev_3(haystack, end),
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_rev_1(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 31 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 31;

        loop {
            let c0 = _mm256_loadu_si256(ptr.add(chunk_pos) as *const __m256i);
            let r0 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
            );
            let mask = _mm256_movemask_epi8(r0) as u32;
            if mask != 0 {
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 32 + min_pos {
                break;
            }
            chunk_pos -= 32;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_rev_2(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let vlo1 = _mm256_load_si256(self.masks.lo[1].as_ptr() as *const __m256i);
        let vhi1 = _mm256_load_si256(self.masks.hi[1].as_ptr() as *const __m256i);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 31 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 31;

        loop {
            let c0 = _mm256_loadu_si256(ptr.add(chunk_pos) as *const __m256i);
            let c1 = _mm256_loadu_si256(ptr.add(chunk_pos - 1) as *const __m256i);
            let r0 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
            );
            let r1 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1, nib)),
                _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1, 4), nib)),
            );
            let combined = _mm256_and_si256(r0, r1);
            let mask = _mm256_movemask_epi8(combined) as u32;
            if mask != 0 {
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 32 + min_pos {
                break;
            }
            chunk_pos -= 32;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_rev_3(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let vlo1 = _mm256_load_si256(self.masks.lo[1].as_ptr() as *const __m256i);
        let vhi1 = _mm256_load_si256(self.masks.hi[1].as_ptr() as *const __m256i);
        let vlo2 = _mm256_load_si256(self.masks.lo[2].as_ptr() as *const __m256i);
        let vhi2 = _mm256_load_si256(self.masks.hi[2].as_ptr() as *const __m256i);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 31 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 31;

        while chunk_pos >= 64 + min_pos {
            let c0a = _mm256_loadu_si256(ptr.add(chunk_pos) as *const __m256i);
            let c1a = _mm256_loadu_si256(ptr.add(chunk_pos - 1) as *const __m256i);
            let c2a = _mm256_loadu_si256(ptr.add(chunk_pos - 2) as *const __m256i);
            let ra = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0a, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0a, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1a, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1a, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2a, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2a, 4), nib)),
                ),
            );

            let c0b = _mm256_loadu_si256(ptr.add(chunk_pos - 32) as *const __m256i);
            let c1b = _mm256_loadu_si256(ptr.add(chunk_pos - 33) as *const __m256i);
            let c2b = _mm256_loadu_si256(ptr.add(chunk_pos - 34) as *const __m256i);
            let rb = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0b, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0b, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1b, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1b, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2b, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2b, 4), nib)),
                ),
            );

            let mask_a = _mm256_movemask_epi8(ra) as u32;
            let mask_b = _mm256_movemask_epi8(rb) as u32;
            if (mask_a | mask_b) != 0 {
                if mask_a != 0 {
                    if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask_a, sets_ptr, len)
                    {
                        return Some(m);
                    }
                }
                if mask_b != 0 {
                    if let Some(m) =
                        Self::verify_rev_inline(ptr, chunk_pos - 32, mask_b, sets_ptr, len)
                    {
                        return Some(m);
                    }
                }
            }
            chunk_pos -= 64;
        }

        loop {
            let c0 = _mm256_loadu_si256(ptr.add(chunk_pos) as *const __m256i);
            let c1 = _mm256_loadu_si256(ptr.add(chunk_pos - 1) as *const __m256i);
            let c2 = _mm256_loadu_si256(ptr.add(chunk_pos - 2) as *const __m256i);
            let combined = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2, 4), nib)),
                ),
            );
            let mask = _mm256_movemask_epi8(combined) as u32;
            if mask != 0 {
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 32 + min_pos {
                break;
            }
            chunk_pos -= 32;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

    /// brute-force check for positions <= end (handles < 32 byte tails)
    fn verify_tail(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let min_pos = self.len - 1;
        let mut pos = end;
        'outer: loop {
            if pos < min_pos {
                return None;
            }
            for i in 0..self.len {
                if !self.sets[i].contains_byte(haystack[pos - i]) {
                    if pos == min_pos {
                        return None;
                    }
                    pos -= 1;
                    continue 'outer;
                }
            }
            return Some(pos);
        }
    }

    #[inline(always)]
    unsafe fn verify_rev_inline(
        ptr: *const u8,
        chunk_pos: usize,
        mut bits: u32,
        sets_ptr: *const TSet,
        len: usize,
    ) -> Option<usize> {
        while bits != 0 {
            let bit = 31 - bits.leading_zeros() as usize;
            let candidate = chunk_pos + bit;
            if candidate + 1 < len {
                bits &= !(1u32 << bit);
                continue;
            }
            let mut ok = true;
            let mut j = 0;
            while j < len {
                if !(*sets_ptr.add(j)).contains_byte(*ptr.add(candidate - j)) {
                    ok = false;
                    break;
                }
                j += 1;
            }
            if ok {
                return Some(candidate);
            }
            bits &= !(1u32 << bit);
        }
        None
    }
}

#[cfg(target_arch = "x86_64")]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FwdPrefixSearch {
    len: usize,
    num_simd: usize,
    masks: Box<TeddyMasks>,
    pub(crate) sets: Vec<TSet>,
    verify_order: [u8; 16],
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[repr(align(32))]
#[cfg_attr(debug_assertions, derive(Debug))]
pub(crate) struct TeddyMasks {
    lo: [[u8; 32]; 3],
    hi: [[u8; 32]; 3],
}

#[cfg(target_arch = "x86_64")]
impl FwdPrefixSearch {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn new(
        len: usize,
        freq_order: &[usize],
        byte_sets_raw: &[Vec<u8>],
        all_sets: Vec<TSet>,
    ) -> Self {
        debug_assert_eq!(all_sets.len(), len);
        debug_assert_eq!(byte_sets_raw.len(), len);

        let num_simd = len.min(3);
        let mut masks = Box::new(TeddyMasks {
            lo: [[0u8; 32]; 3],
            hi: [[0u8; 32]; 3],
        });

        for i in 0..num_simd {
            let mut lo = [0u8; 16];
            let mut hi = [0u8; 16];
            for &b in &byte_sets_raw[i] {
                lo[(b & 0xF) as usize] |= 0x80;
                hi[(b >> 4) as usize] |= 0x80;
            }
            masks.lo[i][..16].copy_from_slice(&lo);
            masks.lo[i][16..].copy_from_slice(&lo);
            masks.hi[i][..16].copy_from_slice(&hi);
            masks.hi[i][16..].copy_from_slice(&hi);
        }

        // build verify order: non-SIMD positions first (rarest first),
        // then SIMD positions last (already pre-filtered by SIMD)
        let mut verify_order = [0u8; 16];
        let mut vi = 0;
        // non-SIMD positions in frequency order (rarest first)
        for &pos in freq_order {
            if pos >= num_simd && pos < len {
                verify_order[vi] = pos as u8;
                vi += 1;
            }
        }
        // SIMD positions last (in frequency order)
        for &pos in freq_order {
            if pos < num_simd {
                verify_order[vi] = pos as u8;
                vi += 1;
            }
        }

        Self {
            len,
            num_simd,
            masks,
            sets: all_sets,
            verify_order,
        }
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe { self.find_fwd_avx2(haystack, start) }
    }

    /// brute-force check for remaining positions (handles < 32 byte tails)
    fn verify_tail_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        if haystack.len() < self.len {
            return None;
        }
        let end = haystack.len() - self.len;
        let mut pos = start;
        'outer: while pos <= end {
            for i in 0..self.len {
                if !self.sets[i].contains_byte(haystack[pos + i]) {
                    pos += 1;
                    continue 'outer;
                }
            }
            return Some(pos);
        }
        None
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_fwd_avx2(&self, haystack: &[u8], start: usize) -> Option<usize> {
        match self.num_simd {
            1 => self.teddy_1(haystack, start),
            2 => self.teddy_2(haystack, start),
            _ => self.teddy_3(haystack, start),
        }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_1(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;

        let simd_end = haystack.len().saturating_sub(31 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let c0 = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let r0 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
            );
            let mask = _mm256_movemask_epi8(r0) as u32;
            if mask != 0 {
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 32;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_2(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let vlo1 = _mm256_load_si256(self.masks.lo[1].as_ptr() as *const __m256i);
        let vhi1 = _mm256_load_si256(self.masks.hi[1].as_ptr() as *const __m256i);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;

        let simd_end = haystack.len().saturating_sub(31 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let c0 = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let c1 = _mm256_loadu_si256(ptr.add(pos + 1) as *const __m256i);
            let r0 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
            );
            let r1 = _mm256_and_si256(
                _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1, nib)),
                _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1, 4), nib)),
            );
            let combined = _mm256_and_si256(r0, r1);
            let mask = _mm256_movemask_epi8(combined) as u32;
            if mask != 0 {
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 32;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    #[target_feature(enable = "avx2")]
    unsafe fn teddy_3(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = _mm256_set1_epi8(0x0F);
        let vlo0 = _mm256_load_si256(self.masks.lo[0].as_ptr() as *const __m256i);
        let vhi0 = _mm256_load_si256(self.masks.hi[0].as_ptr() as *const __m256i);
        let vlo1 = _mm256_load_si256(self.masks.lo[1].as_ptr() as *const __m256i);
        let vhi1 = _mm256_load_si256(self.masks.hi[1].as_ptr() as *const __m256i);
        let vlo2 = _mm256_load_si256(self.masks.lo[2].as_ptr() as *const __m256i);
        let vhi2 = _mm256_load_si256(self.masks.hi[2].as_ptr() as *const __m256i);

        let simd_end = haystack.len().saturating_sub(31 + self.len - 1);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let mut pos = start;

        while pos + 32 < simd_end {
            let c0a = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let c1a = _mm256_loadu_si256(ptr.add(pos + 1) as *const __m256i);
            let c2a = _mm256_loadu_si256(ptr.add(pos + 2) as *const __m256i);
            let ra = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0a, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0a, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1a, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1a, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2a, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2a, 4), nib)),
                ),
            );

            let c0b = _mm256_loadu_si256(ptr.add(pos + 32) as *const __m256i);
            let c1b = _mm256_loadu_si256(ptr.add(pos + 33) as *const __m256i);
            let c2b = _mm256_loadu_si256(ptr.add(pos + 34) as *const __m256i);
            let rb = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0b, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0b, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1b, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1b, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2b, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2b, 4), nib)),
                ),
            );

            let mask_a = _mm256_movemask_epi8(ra) as u32;
            let mask_b = _mm256_movemask_epi8(rb) as u32;
            if (mask_a | mask_b) != 0 {
                if mask_a != 0 {
                    if let Some(m) = Self::verify_inline(
                        ptr,
                        pos,
                        mask_a,
                        sets_ptr,
                        len,
                        self.verify_order.as_ptr(),
                    ) {
                        return Some(m);
                    }
                }
                if mask_b != 0 {
                    if let Some(m) = Self::verify_inline(
                        ptr,
                        pos + 32,
                        mask_b,
                        sets_ptr,
                        len,
                        self.verify_order.as_ptr(),
                    ) {
                        return Some(m);
                    }
                }
            }
            pos += 64;
        }

        while pos < simd_end {
            let c0 = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
            let c1 = _mm256_loadu_si256(ptr.add(pos + 1) as *const __m256i);
            let c2 = _mm256_loadu_si256(ptr.add(pos + 2) as *const __m256i);
            let combined = _mm256_and_si256(
                _mm256_and_si256(
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo0, _mm256_and_si256(c0, nib)),
                        _mm256_shuffle_epi8(vhi0, _mm256_and_si256(_mm256_srli_epi16(c0, 4), nib)),
                    ),
                    _mm256_and_si256(
                        _mm256_shuffle_epi8(vlo1, _mm256_and_si256(c1, nib)),
                        _mm256_shuffle_epi8(vhi1, _mm256_and_si256(_mm256_srli_epi16(c1, 4), nib)),
                    ),
                ),
                _mm256_and_si256(
                    _mm256_shuffle_epi8(vlo2, _mm256_and_si256(c2, nib)),
                    _mm256_shuffle_epi8(vhi2, _mm256_and_si256(_mm256_srli_epi16(c2, 4), nib)),
                ),
            );
            let mask = _mm256_movemask_epi8(combined) as u32;
            if mask != 0 {
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 32;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    #[inline(always)]
    unsafe fn verify_inline(
        ptr: *const u8,
        pos: usize,
        mut bits: u32,
        sets_ptr: *const TSet,
        len: usize,
        verify_order: *const u8,
    ) -> Option<usize> {
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let candidate = pos + bit;
            let base = ptr.add(candidate);
            let mut ok = true;
            let mut j = 0;
            while j < len {
                let idx = *verify_order.add(j) as usize;
                if !(*sets_ptr.add(idx)).contains_byte(*base.add(idx)) {
                    ok = false;
                    break;
                }
                j += 1;
            }
            if ok {
                return Some(candidate);
            }
            bits &= bits - 1;
        }
        None
    }
}

#[cfg(target_arch = "x86_64")]
#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FwdRangeSearch {
    len: usize,
    pub(crate) anchor_pos: usize,
    pub(crate) ranges: Vec<(u8, u8)>,
    pub(crate) sets: Vec<TSet>,
}

#[cfg(target_arch = "x86_64")]
impl FwdRangeSearch {
    pub fn new(len: usize, anchor_pos: usize, ranges: Vec<(u8, u8)>, sets: Vec<TSet>) -> Self {
        debug_assert!(!ranges.is_empty() && ranges.len() <= 3);
        debug_assert!(anchor_pos < len);
        Self {
            len,
            anchor_pos,
            ranges,
            sets,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe { self.find_fwd_avx2(haystack, start) }
    }

    fn verify_tail_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        if haystack.len() < self.len {
            return None;
        }
        let end = haystack.len() - self.len;
        let mut pos = start;
        'outer: while pos <= end {
            for i in 0..self.len {
                if !self.sets[i].contains_byte(haystack[pos + i]) {
                    pos += 1;
                    continue 'outer;
                }
            }
            return Some(pos);
        }
        None
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_fwd_avx2(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let n = self.ranges.len();
        let anchor = self.anchor_pos;
        let lo0 = _mm256_set1_epi8(self.ranges[0].0 as i8);
        let hi0 = _mm256_set1_epi8(self.ranges[0].1 as i8);

        let simd_end = haystack.len().saturating_sub(31 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let chunk = _mm256_loadu_si256(ptr.add(pos + anchor) as *const __m256i);
            let ge0 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo0), chunk);
            let le0 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi0), chunk);
            let mut mask = _mm256_movemask_epi8(_mm256_and_si256(ge0, le0)) as u32;
            if n >= 2 {
                let lo1 = _mm256_set1_epi8(self.ranges[1].0 as i8);
                let hi1 = _mm256_set1_epi8(self.ranges[1].1 as i8);
                let ge1 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo1), chunk);
                let le1 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi1), chunk);
                mask |= _mm256_movemask_epi8(_mm256_and_si256(ge1, le1)) as u32;
            }
            if n >= 3 {
                let lo2 = _mm256_set1_epi8(self.ranges[2].0 as i8);
                let hi2 = _mm256_set1_epi8(self.ranges[2].1 as i8);
                let ge2 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo2), chunk);
                let le2 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi2), chunk);
                mask |= _mm256_movemask_epi8(_mm256_and_si256(ge2, le2)) as u32;
            }
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let candidate = pos + bit;
                let mut ok = true;
                for i in 0..self.len {
                    if !self.sets[i].contains_byte(*ptr.add(candidate + i)) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return Some(candidate);
                }
                mask &= mask - 1;
            }
            pos += 32;
        }
        self.verify_tail_fwd(haystack, pos)
    }
}

#[cfg(target_arch = "aarch64")]
pub struct FwdRangeSearch {
    len: usize,
    pub(crate) anchor_pos: usize,
    pub(crate) ranges: Vec<(u8, u8)>,
    pub(crate) sets: Vec<TSet>,
}

#[cfg(target_arch = "aarch64")]
impl FwdRangeSearch {
    pub fn new(len: usize, anchor_pos: usize, ranges: Vec<(u8, u8)>, sets: Vec<TSet>) -> Self {
        debug_assert!(!ranges.is_empty() && ranges.len() <= 3);
        debug_assert!(anchor_pos < len);
        Self {
            len,
            anchor_pos,
            ranges,
            sets,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe { self.find_fwd_neon(haystack, start) }
    }

    fn verify_tail_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        if haystack.len() < self.len {
            return None;
        }
        let end = haystack.len() - self.len;
        let mut pos = start;
        'outer: while pos <= end {
            for i in 0..self.len {
                if !self.sets[i].contains_byte(haystack[pos + i]) {
                    pos += 1;
                    continue 'outer;
                }
            }
            return Some(pos);
        }
        None
    }

    unsafe fn find_fwd_neon(&self, haystack: &[u8], start: usize) -> Option<usize> {
        use neon::neon_movemask;
        use std::arch::aarch64::*;

        let ptr = haystack.as_ptr();
        let n = self.ranges.len();
        let anchor = self.anchor_pos;
        let lo0 = vdupq_n_u8(self.ranges[0].0);
        let hi0 = vdupq_n_u8(self.ranges[0].1);

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let chunk = vld1q_u8(ptr.add(pos + anchor));
            let in0 = vandq_u8(vcgeq_u8(chunk, lo0), vcleq_u8(chunk, hi0));
            let combined = if n >= 3 {
                let lo1 = vdupq_n_u8(self.ranges[1].0);
                let hi1 = vdupq_n_u8(self.ranges[1].1);
                let lo2 = vdupq_n_u8(self.ranges[2].0);
                let hi2 = vdupq_n_u8(self.ranges[2].1);
                let in1 = vandq_u8(vcgeq_u8(chunk, lo1), vcleq_u8(chunk, hi1));
                let in2 = vandq_u8(vcgeq_u8(chunk, lo2), vcleq_u8(chunk, hi2));
                vorrq_u8(in0, vorrq_u8(in1, in2))
            } else if n >= 2 {
                let lo1 = vdupq_n_u8(self.ranges[1].0);
                let hi1 = vdupq_n_u8(self.ranges[1].1);
                let in1 = vandq_u8(vcgeq_u8(chunk, lo1), vcleq_u8(chunk, hi1));
                vorrq_u8(in0, in1)
            } else {
                in0
            };
            let mut mask = neon_movemask(combined) as u32;
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let candidate = pos + bit;
                let mut ok = true;
                for i in 0..self.len {
                    if !self.sets[i].contains_byte(*ptr.add(candidate + i)) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    return Some(candidate);
                }
                mask &= mask - 1;
            }
            pos += 16;
        }
        self.verify_tail_fwd(haystack, pos)
    }
}

#[cfg(target_arch = "x86_64")]
pub struct RevSearchRanges {
    ranges: Vec<(u8, u8)>,
}

#[cfg(target_arch = "x86_64")]
impl RevSearchRanges {
    pub fn new(ranges: Vec<(u8, u8)>) -> Self {
        debug_assert!(!ranges.is_empty() && ranges.len() <= 3);
        Self { ranges }
    }

    pub fn ranges(&self) -> &[(u8, u8)] {
        &self.ranges
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_avx2::<false>(haystack) }
    }

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_avx2::<true>(haystack) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn search_avx2<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let len = haystack.len();
        if len == 0 {
            return None;
        }
        let ptr = haystack.as_ptr();
        let n = self.ranges.len();
        let lo0 = _mm256_set1_epi8(self.ranges[0].0 as i8);
        let hi0 = _mm256_set1_epi8(self.ranges[0].1 as i8);

        macro_rules! compute_mask {
            ($chunk:expr) => {{
                let chunk = $chunk;
                let ge0 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo0), chunk);
                let le0 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi0), chunk);
                let mut mask = _mm256_movemask_epi8(_mm256_and_si256(ge0, le0)) as u32;
                if n >= 2 {
                    let lo1 = _mm256_set1_epi8(self.ranges[1].0 as i8);
                    let hi1 = _mm256_set1_epi8(self.ranges[1].1 as i8);
                    let ge1 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo1), chunk);
                    let le1 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi1), chunk);
                    mask |= _mm256_movemask_epi8(_mm256_and_si256(ge1, le1)) as u32;
                }
                if n >= 3 {
                    let lo2 = _mm256_set1_epi8(self.ranges[2].0 as i8);
                    let hi2 = _mm256_set1_epi8(self.ranges[2].1 as i8);
                    let ge2 = _mm256_cmpeq_epi8(_mm256_max_epu8(chunk, lo2), chunk);
                    let le2 = _mm256_cmpeq_epi8(_mm256_min_epu8(chunk, hi2), chunk);
                    mask |= _mm256_movemask_epi8(_mm256_and_si256(ge2, le2)) as u32;
                }
                mask
            }};
        }

        if FWD {
            let mut pos = 0;
            while pos + 32 <= len {
                let mask = compute_mask!(_mm256_loadu_si256(ptr.add(pos) as *const __m256i));
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
                pos += 32;
            }
            if pos < len {
                let mut buf = [0u8; 32];
                buf[..len - pos].copy_from_slice(&haystack[pos..]);
                let mut mask =
                    compute_mask!(_mm256_loadu_si256(buf.as_ptr() as *const __m256i));
                mask &= (1u32 << (len - pos)) - 1;
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
            }
        } else {
            if len >= 32 {
                let mut pos = len - 32;
                loop {
                    let mask =
                        compute_mask!(_mm256_loadu_si256(ptr.add(pos) as *const __m256i));
                    if mask != 0 {
                        return Some(pos + 31 - mask.leading_zeros() as usize);
                    }
                    if pos < 32 {
                        break;
                    }
                    pos -= 32;
                }
            }
            let gap = if len >= 32 { len % 32 } else { len };
            if gap > 0 {
                let mut buf = [0u8; 32];
                buf[..gap].copy_from_slice(&haystack[..gap]);
                let mut mask =
                    compute_mask!(_mm256_loadu_si256(buf.as_ptr() as *const __m256i));
                mask &= (1u32 << gap) - 1;
                if mask != 0 {
                    return Some(31 - mask.leading_zeros() as usize);
                }
            }
        }
        None
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct RevSearchBytes {
    _private: (),
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl RevSearchBytes {
    pub fn bytes(&self) -> &[u8] {
        unreachable!()
    }

    pub fn find_rev(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct FwdLiteralSearch {
    _private: (),
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl FwdLiteralSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn rare_byte(&self) -> u8 {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }

    pub fn find_all_fixed(&self, _haystack: &[u8], _matches: &mut Vec<(usize, usize)>) {
        unreachable!()
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct RevPrefixSearch {
    _private: (),
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl RevPrefixSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn find_rev(&self, _haystack: &[u8], _end: usize) -> Option<usize> {
        unreachable!()
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct FwdPrefixSearch {
    _private: (),
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl FwdPrefixSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8], _start: usize) -> Option<usize> {
        unreachable!()
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub struct RevSearchRanges {
    _private: (),
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
impl RevSearchRanges {
    pub fn ranges(&self) -> &[(u8, u8)] {
        unreachable!()
    }

    pub fn find_rev(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }
}
