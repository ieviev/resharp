pub use resharp_algebra::solver::TSet;

pub static BYTE_FREQ: [u8; 256] = {
    let mut t = [255u8; 256];
    t[b'e' as usize] = 200;
    t[b't' as usize] = 190;
    t[b'a' as usize] = 180;
    t[b'o' as usize] = 175;
    t[b'i' as usize] = 170;
    t[b'n' as usize] = 165;
    t[b's' as usize] = 160;
    t[b'h' as usize] = 155;
    t[b'r' as usize] = 150;
    t[b'd' as usize] = 140;
    t[b'l' as usize] = 135;
    t[b'c' as usize] = 130;
    t[b'u' as usize] = 125;
    t[b'm' as usize] = 120;
    t[b'w' as usize] = 115;
    t[b'f' as usize] = 110;
    t[b'g' as usize] = 105;
    t[b'y' as usize] = 100;
    t[b'p' as usize] = 95;
    t[b'b' as usize] = 90;
    t[b'v' as usize] = 85;
    t[b'k' as usize] = 80;
    t[b'j' as usize] = 50;
    t[b'x' as usize] = 45;
    t[b'q' as usize] = 40;
    t[b'z' as usize] = 35;

    t[b'E' as usize] = 30;
    t[b'T' as usize] = 29;
    t[b'A' as usize] = 28;
    t[b'O' as usize] = 27;
    t[b'I' as usize] = 26;
    t[b'N' as usize] = 25;
    t[b'S' as usize] = 24;
    t[b'H' as usize] = 23;
    t[b'R' as usize] = 22;
    t[b'D' as usize] = 21;
    t[b'L' as usize] = 20;
    t[b'C' as usize] = 19;
    t[b'U' as usize] = 18;
    t[b'M' as usize] = 17;
    t[b'W' as usize] = 16;
    t[b'F' as usize] = 15;
    t[b'G' as usize] = 14;
    t[b'Y' as usize] = 13;
    t[b'P' as usize] = 12;
    t[b'B' as usize] = 11;
    t[b'V' as usize] = 10;
    t[b'K' as usize] = 9;
    t[b'J' as usize] = 8;
    t[b'X' as usize] = 7;
    t[b'Q' as usize] = 6;
    t[b'Z' as usize] = 5;

    t[b' ' as usize] = 210;
    t[b'\n' as usize] = 205;
    t[b'\r' as usize] = 195;
    t[b'.' as usize] = 70;
    t[b',' as usize] = 65;
    t[b'\'' as usize] = 55;
    t[b'"' as usize] = 50;
    t[b'-' as usize] = 48;

    t[b'0' as usize] = 60;
    t[b'1' as usize] = 58;
    t[b'2' as usize] = 56;
    t[b'3' as usize] = 54;
    t[b'4' as usize] = 52;
    t[b'5' as usize] = 50;
    t[b'6' as usize] = 48;
    t[b'7' as usize] = 46;
    t[b'8' as usize] = 44;
    t[b'9' as usize] = 42;
    t
};

#[inline]
pub fn has_simd() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

// ---- x86_64 AVX2 implementations ----

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

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

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.find_rev_avx2(haystack) }
    }

    #[target_feature(enable = "avx2")]
    unsafe fn find_rev_avx2(&self, haystack: &[u8]) -> Option<usize> {
        let len = haystack.len();
        if len == 0 {
            return None;
        }
        let ptr = haystack.as_ptr();
        let v0 = _mm256_set1_epi8(self.bytes[0] as i8);
        let n = self.bytes.len();

        if len >= 32 {
            let mut pos = len - 32;
            loop {
                let chunk = _mm256_loadu_si256(ptr.add(pos) as *const __m256i);
                let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v0)) as u32;
                if n >= 2 {
                    let v1 = _mm256_set1_epi8(self.bytes[1] as i8);
                    mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v1)) as u32;
                }
                if n >= 3 {
                    let v2 = _mm256_set1_epi8(self.bytes[2] as i8);
                    mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v2)) as u32;
                }
                if mask != 0 {
                    return Some(pos + 31 - mask.leading_zeros() as usize);
                }
                if pos < 32 {
                    break;
                }
                pos -= 32;
            }
        }
        // tail: overlapping load from position 0, mask to only check uncovered bytes
        if len < 32 {
            let mut buf = [0u8; 32];
            buf[..len].copy_from_slice(&haystack[..len]);
            let chunk = _mm256_loadu_si256(buf.as_ptr() as *const __m256i);
            let mut mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v0)) as u32;
            if n >= 2 {
                let v1 = _mm256_set1_epi8(self.bytes[1] as i8);
                mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v1)) as u32;
            }
            if n >= 3 {
                let v2 = _mm256_set1_epi8(self.bytes[2] as i8);
                mask |= _mm256_movemask_epi8(_mm256_cmpeq_epi8(chunk, v2)) as u32;
            }
            mask &= (1u32 << len) - 1; // mask off padding bytes
            if mask != 0 {
                return Some(31 - mask.leading_zeros() as usize);
            }
        }
        None
    }
}

#[cfg(target_arch = "x86_64")]
pub struct FwdLiteralSearch {
    needle: Vec<u8>,
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
    sets: Vec<TSet>,
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
pub struct FwdPrefixSearch {
    len: usize,
    num_simd: usize,
    masks: Box<TeddyMasks>,
    sets: Vec<TSet>,
}

#[cfg(target_arch = "x86_64")]
#[repr(align(32))]
struct TeddyMasks {
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
        _freq_order: &[usize],
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

        Self {
            len,
            num_simd,
            masks,
            sets: all_sets,
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
                if let Some(m) = Self::verify_inline(ptr, pos, mask, sets_ptr, len) {
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
                if let Some(m) = Self::verify_inline(ptr, pos, mask, sets_ptr, len) {
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
                    if let Some(m) = Self::verify_inline(ptr, pos, mask_a, sets_ptr, len) {
                        return Some(m);
                    }
                }
                if mask_b != 0 {
                    if let Some(m) = Self::verify_inline(ptr, pos + 32, mask_b, sets_ptr, len) {
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
                if let Some(m) = Self::verify_inline(ptr, pos, mask, sets_ptr, len) {
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
    ) -> Option<usize> {
        while bits != 0 {
            let bit = bits.trailing_zeros() as usize;
            let candidate = pos + bit;
            let base = ptr.add(candidate);
            let mut ok = true;
            let mut j = 0;
            while j < len {
                if !(*sets_ptr.add(j)).contains_byte(*base.add(j)) {
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

// ---- non-x86_64 stubs (never constructed, only needed for type-checking) ----

#[cfg(not(target_arch = "x86_64"))]
pub struct RevSearchBytes {
    _private: (),
}

#[cfg(not(target_arch = "x86_64"))]
impl RevSearchBytes {
    pub fn bytes(&self) -> &[u8] {
        unreachable!()
    }

    pub fn find_rev(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub struct FwdLiteralSearch {
    _private: (),
}

#[cfg(not(target_arch = "x86_64"))]
impl FwdLiteralSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8]) -> Option<usize> {
        unreachable!()
    }

    pub fn find_all_fixed(&self, _haystack: &[u8], _matches: &mut Vec<(usize, usize)>) {
        unreachable!()
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub struct RevPrefixSearch {
    _private: (),
}

#[cfg(not(target_arch = "x86_64"))]
impl RevPrefixSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn find_rev(&self, _haystack: &[u8], _end: usize) -> Option<usize> {
        unreachable!()
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub struct FwdPrefixSearch {
    _private: (),
}

#[cfg(not(target_arch = "x86_64"))]
impl FwdPrefixSearch {
    pub fn len(&self) -> usize {
        unreachable!()
    }

    pub fn find_fwd(&self, _haystack: &[u8], _start: usize) -> Option<usize> {
        unreachable!()
    }
}
