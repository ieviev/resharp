use std::arch::aarch64::*;

use super::{TSet, TeddyMasks, BYTE_FREQ};

#[inline(always)]
pub(crate) unsafe fn neon_movemask(v: uint8x16_t) -> u16 {
    let signs = vreinterpretq_u8_s8(vshrq_n_s8(vreinterpretq_s8_u8(v), 7));
    const MASK_BITS: [u8; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
    let mask = vld1_u8(MASK_BITS.as_ptr());
    let lo = vand_u8(vget_low_u8(signs), mask);
    let hi = vand_u8(vget_high_u8(signs), mask);
    (vaddv_u8(lo) as u16) | ((vaddv_u8(hi) as u16) << 8)
}

pub struct RevSearchBytes {
    bytes: Vec<u8>,
}

impl RevSearchBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        debug_assert!(!bytes.is_empty() && bytes.len() <= 3);
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_neon::<true>(haystack) }
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_neon::<false>(haystack) }
    }

    unsafe fn search_neon<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let len = haystack.len();
        if len == 0 {
            return None;
        }
        let ptr = haystack.as_ptr();
        let v0 = vdupq_n_u8(self.bytes[0]);
        let n = self.bytes.len();
        let v1 = if n >= 2 {
            vdupq_n_u8(self.bytes[1])
        } else {
            v0
        };
        let v2 = if n >= 3 {
            vdupq_n_u8(self.bytes[2])
        } else {
            v0
        };

        macro_rules! compute_combined {
            ($chunk:expr) => {{
                let chunk = $chunk;
                let cmp0 = vceqq_u8(chunk, v0);
                if n >= 3 {
                    vorrq_u8(cmp0, vorrq_u8(vceqq_u8(chunk, v1), vceqq_u8(chunk, v2)))
                } else if n >= 2 {
                    vorrq_u8(cmp0, vceqq_u8(chunk, v1))
                } else {
                    cmp0
                }
            }};
        }

        if FWD {
            let mut pos = 0;
            while pos + 16 <= len {
                let combined = compute_combined!(vld1q_u8(ptr.add(pos)));
                if vmaxvq_u8(combined) != 0 {
                    let mask = neon_movemask(combined);
                    return Some(pos + mask.trailing_zeros() as usize);
                }
                pos += 16;
            }
            if pos < len {
                let mut buf = [0u8; 16];
                buf[..len - pos].copy_from_slice(&haystack[pos..]);
                let combined = compute_combined!(vld1q_u8(buf.as_ptr()));
                let mut mask = neon_movemask(combined);
                mask &= (1u16 << (len - pos)) - 1;
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
            }
        } else {
            if len >= 16 {
                let mut pos = len - 16;
                loop {
                    let combined = compute_combined!(vld1q_u8(ptr.add(pos)));
                    if vmaxvq_u8(combined) != 0 {
                        let mask = neon_movemask(combined);
                        return Some(pos + 15 - mask.leading_zeros() as usize);
                    }
                    if pos < 16 {
                        break;
                    }
                    pos -= 16;
                }
            }
            let gap = if len >= 16 { len % 16 } else { len };
            if gap > 0 {
                let mut buf = [0u8; 16];
                buf[..gap].copy_from_slice(&haystack[..gap]);
                let combined = compute_combined!(vld1q_u8(buf.as_ptr()));
                let mut mask = neon_movemask(combined);
                mask &= (1u16 << gap) - 1;
                if mask != 0 {
                    return Some(15 - mask.leading_zeros() as usize);
                }
            }
        }
        None
    }
}

pub struct RevSearchRanges {
    ranges: Vec<(u8, u8)>,
}

impl RevSearchRanges {
    pub fn new(ranges: Vec<(u8, u8)>) -> Self {
        debug_assert!(!ranges.is_empty() && ranges.len() <= 3);
        Self { ranges }
    }

    pub fn ranges(&self) -> &[(u8, u8)] {
        &self.ranges
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_neon::<false>(haystack) }
    }

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search_neon::<true>(haystack) }
    }

    unsafe fn search_neon<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let len = haystack.len();
        if len == 0 {
            return None;
        }
        let ptr = haystack.as_ptr();
        let n = self.ranges.len();
        let lo0 = vdupq_n_u8(self.ranges[0].0);
        let hi0 = vdupq_n_u8(self.ranges[0].1);
        let lo1 = if n >= 2 {
            vdupq_n_u8(self.ranges[1].0)
        } else {
            lo0
        };
        let hi1 = if n >= 2 {
            vdupq_n_u8(self.ranges[1].1)
        } else {
            hi0
        };
        let lo2 = if n >= 3 {
            vdupq_n_u8(self.ranges[2].0)
        } else {
            lo0
        };
        let hi2 = if n >= 3 {
            vdupq_n_u8(self.ranges[2].1)
        } else {
            hi0
        };

        macro_rules! compute_combined {
            ($chunk:expr) => {{
                let chunk = $chunk;
                let in0 = vandq_u8(vcgeq_u8(chunk, lo0), vcleq_u8(chunk, hi0));
                if n >= 3 {
                    let in1 = vandq_u8(vcgeq_u8(chunk, lo1), vcleq_u8(chunk, hi1));
                    let in2 = vandq_u8(vcgeq_u8(chunk, lo2), vcleq_u8(chunk, hi2));
                    vorrq_u8(in0, vorrq_u8(in1, in2))
                } else if n >= 2 {
                    let in1 = vandq_u8(vcgeq_u8(chunk, lo1), vcleq_u8(chunk, hi1));
                    vorrq_u8(in0, in1)
                } else {
                    in0
                }
            }};
        }

        if FWD {
            let mut pos = 0;
            while pos + 16 <= len {
                let combined = compute_combined!(vld1q_u8(ptr.add(pos)));
                if vmaxvq_u8(combined) != 0 {
                    let mask = neon_movemask(combined);
                    return Some(pos + mask.trailing_zeros() as usize);
                }
                pos += 16;
            }
            if pos < len {
                let mut buf = [0u8; 16];
                buf[..len - pos].copy_from_slice(&haystack[pos..]);
                let combined = compute_combined!(vld1q_u8(buf.as_ptr()));
                let mut mask = neon_movemask(combined);
                mask &= (1u16 << (len - pos)) - 1;
                if mask != 0 {
                    return Some(pos + mask.trailing_zeros() as usize);
                }
            }
        } else {
            if len >= 16 {
                let mut pos = len - 16;
                loop {
                    let combined = compute_combined!(vld1q_u8(ptr.add(pos)));
                    if vmaxvq_u8(combined) != 0 {
                        let mask = neon_movemask(combined);
                        return Some(pos + 15 - mask.leading_zeros() as usize);
                    }
                    if pos < 16 {
                        break;
                    }
                    pos -= 16;
                }
            }
            let gap = if len >= 16 { len % 16 } else { len };
            if gap > 0 {
                let mut buf = [0u8; 16];
                buf[..gap].copy_from_slice(&haystack[..gap]);
                let combined = compute_combined!(vld1q_u8(buf.as_ptr()));
                let mut mask = neon_movemask(combined);
                mask &= (1u16 << gap) - 1;
                if mask != 0 {
                    return Some(15 - mask.leading_zeros() as usize);
                }
            }
        }
        None
    }
}

pub struct FwdLiteralSearch {
    pub(crate) needle: Vec<u8>,
    chunks: Vec<u64>,
    rare_idx: usize,
    rare_byte: u8,
    confirm: (usize, u8),
    confirm_offset: isize,
}

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
            confirm_offset: confirm_idx as isize - rare_idx as isize,
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
        unsafe { self.find_fwd_neon(haystack) }
    }

    pub fn find_all_fixed(&self, haystack: &[u8], matches: &mut Vec<(usize, usize)>) {
        unsafe { self.find_all_fixed_neon(haystack, matches) }
    }

    unsafe fn find_all_fixed_neon(&self, haystack: &[u8], matches: &mut Vec<(usize, usize)>) {
        let nlen = self.needle.len();
        if haystack.len() < nlen {
            return;
        }
        let ptr = haystack.as_ptr();
        let rare_idx = self.rare_idx;
        let rare_byte = self.rare_byte;
        let confirm_byte = self.confirm.1;
        let confirm_offset = self.confirm_offset;
        let end = haystack.len() - nlen + rare_idx;
        let vrare = vdupq_n_u8(rare_byte);
        let vconfirm = vdupq_n_u8(confirm_byte);
        let mut last_end: usize = 0;

        let mut pos = rare_idx;
        while pos + 32 <= end + 1 {
            let r0 = vceqq_u8(vld1q_u8(ptr.add(pos)), vrare);
            let r1 = vceqq_u8(vld1q_u8(ptr.add(pos + 16)), vrare);
            let c0 = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + confirm_offset)),
                vconfirm,
            );
            let c1 = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + 16 + confirm_offset)),
                vconfirm,
            );
            let and0 = vandq_u8(r0, c0);
            let and1 = vandq_u8(r1, c1);
            if vmaxvq_u8(vorrq_u8(and0, and1)) == 0 {
                pos += 32;
                continue;
            }
            let mut mask = neon_movemask(and0);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if start >= last_end && self.verify(haystack, start) {
                    let m_end = start + nlen;
                    matches.push((start, m_end));
                    last_end = m_end;
                }
                mask &= mask - 1;
            }
            let mut mask = neon_movemask(and1);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + 16 + bit - rare_idx;
                if start >= last_end && self.verify(haystack, start) {
                    let m_end = start + nlen;
                    matches.push((start, m_end));
                    last_end = m_end;
                }
                mask &= mask - 1;
            }
            pos += 32;
        }
        while pos + 16 <= end + 1 {
            let r = vceqq_u8(vld1q_u8(ptr.add(pos)), vrare);
            let c = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + confirm_offset)),
                vconfirm,
            );
            let and = vandq_u8(r, c);
            if vmaxvq_u8(and) == 0 {
                pos += 16;
                continue;
            }
            let mut mask = neon_movemask(and);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if start >= last_end && self.verify(haystack, start) {
                    let m_end = start + nlen;
                    matches.push((start, m_end));
                    last_end = m_end;
                }
                mask &= mask - 1;
            }
            pos += 16;
        }
        while pos <= end {
            let start = pos - rare_idx;
            if start >= last_end
                && *ptr.add(pos) == rare_byte
                && *ptr.offset(pos as isize + confirm_offset) == confirm_byte
                && self.verify(haystack, start)
            {
                let m_end = start + nlen;
                matches.push((start, m_end));
                last_end = m_end;
            }
            pos += 1;
        }
    }

    unsafe fn find_fwd_neon(&self, haystack: &[u8]) -> Option<usize> {
        let nlen = self.needle.len();
        if haystack.len() < nlen {
            return None;
        }
        let ptr = haystack.as_ptr();
        let rare_idx = self.rare_idx;
        let rare_byte = self.rare_byte;
        let confirm_byte = self.confirm.1;
        let confirm_offset = self.confirm_offset;
        let end = haystack.len() - nlen + rare_idx;
        let vrare = vdupq_n_u8(rare_byte);
        let vconfirm = vdupq_n_u8(confirm_byte);

        let mut pos = rare_idx;
        while pos + 32 <= end + 1 {
            let r0 = vceqq_u8(vld1q_u8(ptr.add(pos)), vrare);
            let r1 = vceqq_u8(vld1q_u8(ptr.add(pos + 16)), vrare);
            let c0 = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + confirm_offset)),
                vconfirm,
            );
            let c1 = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + 16 + confirm_offset)),
                vconfirm,
            );
            let and0 = vandq_u8(r0, c0);
            let and1 = vandq_u8(r1, c1);
            if vmaxvq_u8(vorrq_u8(and0, and1)) == 0 {
                pos += 32;
                continue;
            }
            let mut mask = neon_movemask(and0);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if self.verify(haystack, start) {
                    return Some(start);
                }
                mask &= mask - 1;
            }
            let mut mask = neon_movemask(and1);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + 16 + bit - rare_idx;
                if self.verify(haystack, start) {
                    return Some(start);
                }
                mask &= mask - 1;
            }
            pos += 32;
        }
        while pos + 16 <= end + 1 {
            let r = vceqq_u8(vld1q_u8(ptr.add(pos)), vrare);
            let c = vceqq_u8(
                vld1q_u8(ptr.offset(pos as isize + confirm_offset)),
                vconfirm,
            );
            let and = vandq_u8(r, c);
            if vmaxvq_u8(and) == 0 {
                pos += 16;
                continue;
            }
            let mut mask = neon_movemask(and);
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if self.verify(haystack, start) {
                    return Some(start);
                }
                mask &= mask - 1;
            }
            pos += 16;
        }
        while pos <= end {
            if *ptr.add(pos) == rare_byte
                && *ptr.offset(pos as isize + confirm_offset) == confirm_byte
            {
                let start = pos - rare_idx;
                if self.verify(haystack, start) {
                    return Some(start);
                }
            }
            pos += 1;
        }
        None
    }
}

pub struct RevPrefixSearch {
    len: usize,
    num_simd: usize,
    masks: Box<TeddyMasks>,
    pub(crate) sets: Vec<TSet>,
}

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
        unsafe {
            match self.num_simd {
                1 => self.teddy_rev_1(haystack, end),
                2 => self.teddy_rev_2(haystack, end),
                _ => self.teddy_rev_3(haystack, end),
            }
        }
    }

    unsafe fn teddy_rev_1(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 15 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 15;

        loop {
            let c0 = vld1q_u8(ptr.add(chunk_pos));
            let r0 = vandq_u8(
                vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
            );
            if vmaxvq_u8(r0) != 0 {
                let mask = neon_movemask(r0);
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 16 + min_pos {
                break;
            }
            chunk_pos -= 16;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

    unsafe fn teddy_rev_2(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let vlo1 = vld1q_u8(self.masks.lo[1].as_ptr());
        let vhi1 = vld1q_u8(self.masks.hi[1].as_ptr());
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 15 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 15;

        loop {
            let c0 = vld1q_u8(ptr.add(chunk_pos));
            let c1 = vld1q_u8(ptr.add(chunk_pos - 1));
            let r0 = vandq_u8(
                vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
            );
            let r1 = vandq_u8(
                vqtbl1q_u8(vlo1, vandq_u8(c1, nib)),
                vqtbl1q_u8(vhi1, vshrq_n_u8(c1, 4)),
            );
            let combined = vandq_u8(r0, r1);
            if vmaxvq_u8(combined) != 0 {
                let mask = neon_movemask(combined);
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 16 + min_pos {
                break;
            }
            chunk_pos -= 16;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

    unsafe fn teddy_rev_3(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let vlo1 = vld1q_u8(self.masks.lo[1].as_ptr());
        let vhi1 = vld1q_u8(self.masks.hi[1].as_ptr());
        let vlo2 = vld1q_u8(self.masks.lo[2].as_ptr());
        let vhi2 = vld1q_u8(self.masks.hi[2].as_ptr());
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 15 + min_pos {
            return self.verify_tail(haystack, end);
        }

        let mut chunk_pos = end - 15;

        while chunk_pos >= 32 + min_pos {
            let c0a = vld1q_u8(ptr.add(chunk_pos));
            let c1a = vld1q_u8(ptr.add(chunk_pos - 1));
            let c2a = vld1q_u8(ptr.add(chunk_pos - 2));
            let ra = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0a, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0a, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1a, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1a, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2a, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2a, 4)),
                ),
            );

            let c0b = vld1q_u8(ptr.add(chunk_pos - 16));
            let c1b = vld1q_u8(ptr.add(chunk_pos - 17));
            let c2b = vld1q_u8(ptr.add(chunk_pos - 18));
            let rb = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0b, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0b, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1b, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1b, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2b, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2b, 4)),
                ),
            );

            if vmaxvq_u8(vorrq_u8(ra, rb)) != 0 {
                if vmaxvq_u8(ra) != 0 {
                    let mask_a = neon_movemask(ra);
                    if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask_a, sets_ptr, len)
                    {
                        return Some(m);
                    }
                }
                if vmaxvq_u8(rb) != 0 {
                    let mask_b = neon_movemask(rb);
                    if let Some(m) =
                        Self::verify_rev_inline(ptr, chunk_pos - 16, mask_b, sets_ptr, len)
                    {
                        return Some(m);
                    }
                }
            }
            chunk_pos -= 32;
        }

        loop {
            let c0 = vld1q_u8(ptr.add(chunk_pos));
            let c1 = vld1q_u8(ptr.add(chunk_pos - 1));
            let c2 = vld1q_u8(ptr.add(chunk_pos - 2));
            let combined = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2, 4)),
                ),
            );
            if vmaxvq_u8(combined) != 0 {
                let mask = neon_movemask(combined);
                if let Some(m) = Self::verify_rev_inline(ptr, chunk_pos, mask, sets_ptr, len) {
                    return Some(m);
                }
            }
            if chunk_pos < 16 + min_pos {
                break;
            }
            chunk_pos -= 16;
        }
        self.verify_tail(haystack, chunk_pos.saturating_sub(1).min(end))
    }

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
        mut bits: u16,
        sets_ptr: *const TSet,
        len: usize,
    ) -> Option<usize> {
        while bits != 0 {
            let bit = 15 - bits.leading_zeros() as usize;
            let candidate = chunk_pos + bit;
            if candidate + 1 < len {
                bits &= !(1u16 << bit);
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
            bits &= !(1u16 << bit);
        }
        None
    }
}

pub struct FwdPrefixSearch {
    len: usize,
    num_simd: usize,
    simd_offsets: [usize; 3],
    masks: Box<TeddyMasks>,
    pub(crate) sets: Vec<TSet>,
    verify_order: [u8; 16],
}

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
        let mut simd_offsets = [0usize; 3];
        let mut masks = Box::new(TeddyMasks {
            lo: [[0u8; 32]; 3],
            hi: [[0u8; 32]; 3],
        });

        for i in 0..num_simd {
            let pos = freq_order[i];
            simd_offsets[i] = pos;
            let mut lo = [0u8; 16];
            let mut hi = [0u8; 16];
            for &b in &byte_sets_raw[pos] {
                lo[(b & 0xF) as usize] |= 0x80;
                hi[(b >> 4) as usize] |= 0x80;
            }
            masks.lo[i][..16].copy_from_slice(&lo);
            masks.lo[i][16..].copy_from_slice(&lo);
            masks.hi[i][..16].copy_from_slice(&hi);
            masks.hi[i][16..].copy_from_slice(&hi);
        }

        let mut verify_order = [0u8; 16];
        let mut vi = 0;
        for &pos in freq_order {
            if pos >= num_simd && pos < len {
                verify_order[vi] = pos as u8;
                vi += 1;
            }
        }
        for &pos in freq_order {
            if pos < num_simd {
                verify_order[vi] = pos as u8;
                vi += 1;
            }
        }

        Self {
            len,
            num_simd,
            simd_offsets,
            masks,
            sets: all_sets,
            verify_order,
        }
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe {
            match self.num_simd {
                1 => self.teddy_1(haystack, start),
                2 => self.teddy_2(haystack, start),
                _ => self.teddy_3(haystack, start),
            }
        }
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

    unsafe fn teddy_1(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let off0 = self.simd_offsets[0];
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let c0 = vld1q_u8(ptr.add(pos + off0));
            let r0 = vandq_u8(
                vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
            );
            if vmaxvq_u8(r0) != 0 {
                let mask = neon_movemask(r0);
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 16;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    unsafe fn teddy_2(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let vlo1 = vld1q_u8(self.masks.lo[1].as_ptr());
        let vhi1 = vld1q_u8(self.masks.hi[1].as_ptr());
        let off0 = self.simd_offsets[0];
        let off1 = self.simd_offsets[1];
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let c0 = vld1q_u8(ptr.add(pos + off0));
            let c1 = vld1q_u8(ptr.add(pos + off1));
            let r0 = vandq_u8(
                vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
            );
            let r1 = vandq_u8(
                vqtbl1q_u8(vlo1, vandq_u8(c1, nib)),
                vqtbl1q_u8(vhi1, vshrq_n_u8(c1, 4)),
            );
            let combined = vandq_u8(r0, r1);
            if vmaxvq_u8(combined) != 0 {
                let mask = neon_movemask(combined);
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 16;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    unsafe fn teddy_3(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = vdupq_n_u8(0x0F);
        let vlo0 = vld1q_u8(self.masks.lo[0].as_ptr());
        let vhi0 = vld1q_u8(self.masks.hi[0].as_ptr());
        let vlo1 = vld1q_u8(self.masks.lo[1].as_ptr());
        let vhi1 = vld1q_u8(self.masks.hi[1].as_ptr());
        let vlo2 = vld1q_u8(self.masks.lo[2].as_ptr());
        let vhi2 = vld1q_u8(self.masks.hi[2].as_ptr());
        let off0 = self.simd_offsets[0];
        let off1 = self.simd_offsets[1];
        let off2 = self.simd_offsets[2];

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let mut pos = start;

        while pos + 16 < simd_end {
            let c0a = vld1q_u8(ptr.add(pos + off0));
            let c1a = vld1q_u8(ptr.add(pos + off1));
            let c2a = vld1q_u8(ptr.add(pos + off2));
            let ra = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0a, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0a, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1a, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1a, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2a, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2a, 4)),
                ),
            );

            let c0b = vld1q_u8(ptr.add(pos + 16 + off0));
            let c1b = vld1q_u8(ptr.add(pos + 16 + off1));
            let c2b = vld1q_u8(ptr.add(pos + 16 + off2));
            let rb = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0b, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0b, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1b, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1b, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2b, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2b, 4)),
                ),
            );

            if vmaxvq_u8(vorrq_u8(ra, rb)) != 0 {
                if vmaxvq_u8(ra) != 0 {
                    let mask_a = neon_movemask(ra);
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
                if vmaxvq_u8(rb) != 0 {
                    let mask_b = neon_movemask(rb);
                    if let Some(m) = Self::verify_inline(
                        ptr,
                        pos + 16,
                        mask_b,
                        sets_ptr,
                        len,
                        self.verify_order.as_ptr(),
                    ) {
                        return Some(m);
                    }
                }
            }
            pos += 32;
        }

        while pos < simd_end {
            let c0 = vld1q_u8(ptr.add(pos + off0));
            let c1 = vld1q_u8(ptr.add(pos + off1));
            let c2 = vld1q_u8(ptr.add(pos + off2));
            let combined = vandq_u8(
                vandq_u8(
                    vandq_u8(
                        vqtbl1q_u8(vlo0, vandq_u8(c0, nib)),
                        vqtbl1q_u8(vhi0, vshrq_n_u8(c0, 4)),
                    ),
                    vandq_u8(
                        vqtbl1q_u8(vlo1, vandq_u8(c1, nib)),
                        vqtbl1q_u8(vhi1, vshrq_n_u8(c1, 4)),
                    ),
                ),
                vandq_u8(
                    vqtbl1q_u8(vlo2, vandq_u8(c2, nib)),
                    vqtbl1q_u8(vhi2, vshrq_n_u8(c2, 4)),
                ),
            );
            if vmaxvq_u8(combined) != 0 {
                let mask = neon_movemask(combined);
                if let Some(m) =
                    Self::verify_inline(ptr, pos, mask, sets_ptr, len, self.verify_order.as_ptr())
                {
                    return Some(m);
                }
            }
            pos += 16;
        }
        self.verify_tail_fwd(haystack, pos)
    }

    #[inline(always)]
    unsafe fn verify_inline(
        ptr: *const u8,
        pos: usize,
        mut bits: u16,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movemask_all_zero() {
        unsafe {
            let v = vdupq_n_u8(0);
            assert_eq!(neon_movemask(v), 0u16);
        }
    }

    #[test]
    fn movemask_all_ones() {
        unsafe {
            let v = vdupq_n_u8(0xFF);
            assert_eq!(neon_movemask(v), 0xFFFFu16);
        }
    }

    #[test]
    fn movemask_single_bits() {
        unsafe {
            for i in 0..16u8 {
                let mut arr = [0u8; 16];
                arr[i as usize] = 0xFF;
                let v = vld1q_u8(arr.as_ptr());
                let m = neon_movemask(v);
                assert_eq!(m, 1u16 << i, "bit {} should be set", i);
            }
        }
    }

    #[test]
    fn movemask_0x80() {
        // teddy produces 0x80 bytes, not 0xFF
        unsafe {
            let mut arr = [0u8; 16];
            arr[6] = 0x80;
            arr[14] = 0x80;
            let v = vld1q_u8(arr.as_ptr());
            let m = neon_movemask(v);
            assert_eq!(m, (1u16 << 6) | (1u16 << 14));
        }
    }

    #[test]
    fn movemask_compare() {
        unsafe {
            let hay = b"hello world12345";
            let v = vld1q_u8(hay.as_ptr());
            let target = vdupq_n_u8(b'l');
            let cmp = vceqq_u8(v, target);
            let m = neon_movemask(cmp);
            // 'l' at positions 2, 3, 9
            assert_eq!(m, (1 << 2) | (1 << 3) | (1 << 9));
        }
    }

    #[test]
    fn rev_search_bytes_short() {
        let s = RevSearchBytes::new(vec![b'c']);
        assert_eq!(s.find_rev(b""), None);
        assert_eq!(s.find_rev(b"c"), Some(0));
        assert_eq!(s.find_rev(b"abc"), Some(2));
        assert_eq!(s.find_rev(b"xxxxx"), None);
        assert_eq!(s.find_rev(b"cxxc"), Some(3));
    }

    #[test]
    fn rev_search_bytes_16() {
        let s = RevSearchBytes::new(vec![b'Z']);
        // exactly 16 bytes - one SIMD chunk
        assert_eq!(s.find_rev(b"0123456789abcdeZ"), Some(15));
        assert_eq!(s.find_rev(b"Z123456789abcdef"), Some(0));
        assert_eq!(s.find_rev(b"01234Z6789abcdef"), Some(5));
    }

    #[test]
    fn rev_search_bytes_32() {
        let s = RevSearchBytes::new(vec![b'Z']);
        let hay = b"0123456789abcdef0123456789abcdeZ";
        assert_eq!(hay.len(), 32);
        assert_eq!(s.find_rev(hay), Some(31));
        let hay2 = b"Z1234567890123456789012345678901";
        assert_eq!(s.find_rev(hay2), Some(0));
    }

    #[test]
    fn rev_search_bytes_multi() {
        let s = RevSearchBytes::new(vec![b'a', b'b', b'c']);
        assert_eq!(s.find_rev(b"xxxxxxxxxxxxxc"), Some(13));
        assert_eq!(s.find_rev(b"xxxxxxxxxxxxxxx"), None);
    }

    #[test]
    fn fwd_literal_basic() {
        let s = FwdLiteralSearch::new(b"abc");
        assert_eq!(s.find_fwd(b"xxxabcxxx"), Some(3));
        assert_eq!(s.find_fwd(b"abc"), Some(0));
        assert_eq!(s.find_fwd(b"ab"), None);
        assert_eq!(s.find_fwd(b""), None);
    }

    #[test]
    fn fwd_literal_long() {
        let s = FwdLiteralSearch::new(b"XY");
        let mut hay = vec![b'.'; 100];
        hay[50] = b'X';
        hay[51] = b'Y';
        assert_eq!(s.find_fwd(&hay), Some(50));
    }

    #[test]
    fn fwd_literal_all_fixed() {
        let s = FwdLiteralSearch::new(b"ab");
        let mut m = Vec::new();
        s.find_all_fixed(b"xabxabxab", &mut m);
        assert_eq!(m, vec![(1, 3), (4, 6), (7, 9)]);
    }

    #[test]
    fn fwd_prefix_teddy1() {
        // single position: match byte 'a'
        let sets_raw = vec![vec![b'a']];
        let all_sets = vec![TSet::from_bytes(&[b'a'])];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        assert_eq!(s.find_fwd(b"xxaxx", 0), Some(2));
        assert_eq!(s.find_fwd(b"xxxxx", 0), None);
        // long haystack
        let mut hay = vec![b'.'; 50];
        hay[30] = b'a';
        assert_eq!(s.find_fwd(&hay, 0), Some(30));
    }

    #[test]
    fn fwd_prefix_teddy2() {
        // two positions: match 'a' then 'b'
        let sets_raw = vec![vec![b'a'], vec![b'b']];
        let all_sets = vec![TSet::from_bytes(&[b'a']), TSet::from_bytes(&[b'b'])];
        let s = FwdPrefixSearch::new(2, &[0, 1], &sets_raw, all_sets);
        assert_eq!(s.find_fwd(b"xxabxx", 0), Some(2));
        assert_eq!(s.find_fwd(b"xxbaxx", 0), None);
        // long haystack
        let mut hay = vec![b'.'; 50];
        hay[30] = b'a';
        hay[31] = b'b';
        assert_eq!(s.find_fwd(&hay, 0), Some(30));
    }

    #[test]
    fn rev_prefix_teddy1() {
        // single position: match byte 'c'
        let sets_raw = vec![vec![b'c']];
        let all_sets = vec![TSet::from_bytes(&[b'c'])];
        let s = RevPrefixSearch::new(1, &sets_raw, all_sets);
        assert_eq!(s.find_rev(b"xxcxx", 4), Some(2));
        assert_eq!(s.find_rev(b"xxxxx", 4), None);
        // long haystack
        let mut hay = vec![b'.'; 50];
        hay[30] = b'c';
        assert_eq!(s.find_rev(&hay, 49), Some(30));
    }

    #[test]
    fn rev_prefix_teddy2() {
        let sets_raw = vec![vec![b'c'], vec![b'b']];
        let all_sets = vec![TSet::from_bytes(&[b'c']), TSet::from_bytes(&[b'b'])];
        let s = RevPrefixSearch::new(2, &sets_raw, all_sets);
        assert_eq!(s.find_rev(b"xxbcxx", 5), Some(3));
        assert_eq!(s.find_rev(b"xxxcxx", 5), None);
        let mut hay = vec![b'.'; 50];
        hay[29] = b'b';
        hay[30] = b'c';
        assert_eq!(s.find_rev(&hay, 49), Some(30));
    }

    #[test]
    fn fwd_prefix_teddy3() {
        let sets_raw = vec![vec![b'a'], vec![b'b'], vec![b'c']];
        let all_sets = vec![
            TSet::from_bytes(&[b'a']),
            TSet::from_bytes(&[b'b']),
            TSet::from_bytes(&[b'c']),
        ];
        let s = FwdPrefixSearch::new(3, &[0, 1, 2], &sets_raw, all_sets);
        // short (scalar tail)
        assert_eq!(s.find_fwd(b"xxabcxx", 0), Some(2));
        assert_eq!(s.find_fwd(b"xxacbxx", 0), None);
        // 50 bytes - exercises SIMD loop
        let mut hay = vec![b'.'; 50];
        hay[30] = b'a';
        hay[31] = b'b';
        hay[32] = b'c';
        assert_eq!(s.find_fwd(&hay, 0), Some(30));
        // 100 bytes - exercises double-pump loop
        let mut hay = vec![b'.'; 100];
        hay[70] = b'a';
        hay[71] = b'b';
        hay[72] = b'c';
        assert_eq!(s.find_fwd(&hay, 0), Some(70));
    }

    #[test]
    fn rev_prefix_teddy3() {
        let sets_raw = vec![vec![b'c'], vec![b'b'], vec![b'a']];
        let all_sets = vec![
            TSet::from_bytes(&[b'c']),
            TSet::from_bytes(&[b'b']),
            TSet::from_bytes(&[b'a']),
        ];
        let s = RevPrefixSearch::new(3, &sets_raw, all_sets);
        // short (scalar tail)
        assert_eq!(s.find_rev(b"xxabcxx", 6), Some(4));
        // 50 bytes - SIMD loop
        let mut hay = vec![b'.'; 50];
        hay[28] = b'a';
        hay[29] = b'b';
        hay[30] = b'c';
        assert_eq!(s.find_rev(&hay, 49), Some(30));
        // 100 bytes - double-pump loop
        let mut hay = vec![b'.'; 100];
        hay[68] = b'a';
        hay[69] = b'b';
        hay[70] = b'c';
        assert_eq!(s.find_rev(&hay, 99), Some(70));
    }

    #[test]
    fn fwd_prefix_char_class() {
        // position 0: any digit [0-9]
        let digits: Vec<u8> = (b'0'..=b'9').collect();
        let sets_raw = vec![digits.clone()];
        let all_sets = vec![TSet::from_bytes(&digits)];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        hay[25] = b'5';
        assert_eq!(s.find_fwd(&hay, 0), Some(25));
        // every digit should match
        for d in b'0'..=b'9' {
            hay[25] = d;
            assert_eq!(s.find_fwd(&hay, 0), Some(25), "digit {} failed", d);
        }
    }

    #[test]
    fn rev_prefix_char_class() {
        let vowels: Vec<u8> = vec![b'a', b'e', b'i', b'o', b'u'];
        let sets_raw = vec![vowels.clone()];
        let all_sets = vec![TSet::from_bytes(&vowels)];
        let s = RevPrefixSearch::new(1, &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        hay[35] = b'o';
        assert_eq!(s.find_rev(&hay, 49), Some(35));
        for &v in &vowels {
            hay[35] = v;
            assert_eq!(s.find_rev(&hay, 49), Some(35), "vowel {} failed", v as char);
        }
    }

    #[test]
    fn fwd_prefix_at_chunk_boundaries() {
        let sets_raw = vec![vec![b'X']];
        let all_sets = vec![TSet::from_bytes(&[b'X'])];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        // match at position 15 (last byte of first 16-byte chunk)
        let mut hay = vec![b'.'; 50];
        hay[15] = b'X';
        assert_eq!(s.find_fwd(&hay, 0), Some(15));
        // match at position 16 (first byte of second chunk)
        hay[15] = b'.';
        hay[16] = b'X';
        assert_eq!(s.find_fwd(&hay, 0), Some(16));
        // match at position 31 (last byte of second chunk)
        hay[16] = b'.';
        hay[31] = b'X';
        assert_eq!(s.find_fwd(&hay, 0), Some(31));
        // match at position 32
        hay[31] = b'.';
        hay[32] = b'X';
        assert_eq!(s.find_fwd(&hay, 0), Some(32));
    }

    #[test]
    fn rev_search_at_chunk_boundaries() {
        let s = RevSearchBytes::new(vec![b'Z']);
        // match at position 15 in 32-byte haystack
        let mut hay = vec![b'.'; 32];
        hay[15] = b'Z';
        assert_eq!(s.find_rev(&hay), Some(15));
        // match at position 16
        hay[15] = b'.';
        hay[16] = b'Z';
        assert_eq!(s.find_rev(&hay), Some(16));
        // two matches - should return the last one
        hay[5] = b'Z';
        assert_eq!(s.find_rev(&hay), Some(16));
    }

    #[test]
    fn rev_prefix_at_chunk_boundaries() {
        let sets_raw = vec![vec![b'X']];
        let all_sets = vec![TSet::from_bytes(&[b'X'])];
        let s = RevPrefixSearch::new(1, &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        // match at position 15
        hay[15] = b'X';
        assert_eq!(s.find_rev(&hay, 49), Some(15));
        // match at position 16
        hay[15] = b'.';
        hay[16] = b'X';
        assert_eq!(s.find_rev(&hay, 49), Some(16));
        // match at position 31
        hay[16] = b'.';
        hay[31] = b'X';
        assert_eq!(s.find_rev(&hay, 49), Some(31));
    }

    #[test]
    fn fwd_prefix_size_sweep() {
        let sets_raw = vec![vec![b'a'], vec![b'b']];
        let all_sets = vec![TSet::from_bytes(&[b'a']), TSet::from_bytes(&[b'b'])];
        let s = FwdPrefixSearch::new(2, &[0, 1], &sets_raw, all_sets);
        // test every size from 3 to 80, match near the end
        for size in 3..=80 {
            let mut hay = vec![b'.'; size];
            hay[size - 3] = b'a';
            hay[size - 2] = b'b';
            assert_eq!(
                s.find_fwd(&hay, 0),
                Some(size - 3),
                "failed for size {}",
                size
            );
        }
    }

    #[test]
    fn rev_prefix_size_sweep() {
        let sets_raw = vec![vec![b'c'], vec![b'b']];
        let all_sets = vec![TSet::from_bytes(&[b'c']), TSet::from_bytes(&[b'b'])];
        let s = RevPrefixSearch::new(2, &sets_raw, all_sets);
        for size in 3..=80 {
            let mut hay = vec![b'.'; size];
            hay[1] = b'b';
            hay[2] = b'c';
            assert_eq!(
                s.find_rev(&hay, size - 1),
                Some(2),
                "failed for size {}",
                size
            );
        }
    }

    #[test]
    fn rev_search_size_sweep() {
        let s = RevSearchBytes::new(vec![b'Z']);
        for size in 1..=80 {
            let mut hay = vec![b'.'; size];
            hay[0] = b'Z';
            assert_eq!(s.find_rev(&hay), Some(0), "failed for size {}", size);
            hay[0] = b'.';
            hay[size - 1] = b'Z';
            assert_eq!(
                s.find_rev(&hay),
                Some(size - 1),
                "failed for size {} (end)",
                size
            );
        }
    }

    #[test]
    fn fwd_literal_size_sweep() {
        let s = FwdLiteralSearch::new(b"XY");
        for size in 2..=80 {
            let mut hay = vec![b'.'; size];
            hay[size - 2] = b'X';
            hay[size - 1] = b'Y';
            assert_eq!(s.find_fwd(&hay), Some(size - 2), "failed for size {}", size);
        }
    }

    #[test]
    fn fwd_literal_all_fixed_long() {
        let s = FwdLiteralSearch::new(b"ab");
        // place "ab" at positions 14, 30, 46 (across chunk boundaries)
        let mut hay = vec![b'.'; 60];
        hay[14] = b'a';
        hay[15] = b'b';
        hay[30] = b'a';
        hay[31] = b'b';
        hay[46] = b'a';
        hay[47] = b'b';
        let mut m = Vec::new();
        s.find_all_fixed(&hay, &mut m);
        assert_eq!(m, vec![(14, 16), (30, 32), (46, 48)]);
    }

    #[test]
    fn fwd_prefix_with_start_offset() {
        let sets_raw = vec![vec![b'a']];
        let all_sets = vec![TSet::from_bytes(&[b'a'])];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        hay[10] = b'a';
        hay[30] = b'a';
        assert_eq!(s.find_fwd(&hay, 0), Some(10));
        assert_eq!(s.find_fwd(&hay, 11), Some(30));
        assert_eq!(s.find_fwd(&hay, 31), None);
    }

    #[test]
    fn fwd_prefix_no_nibble_collision() {
        // 'a' = 0x61, 'q' = 0x71 - same low nibble (1), different high nibble (6 vs 7)
        // '1' = 0x31 - same low nibble (1), different high nibble (3)
        let sets_raw = vec![vec![b'a']];
        let all_sets = vec![TSet::from_bytes(&[b'a'])];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        // haystack full of 'q' and '1' - should NOT match
        let hay = vec![b'q'; 50];
        assert_eq!(s.find_fwd(&hay, 0), None);
        let hay2 = vec![b'1'; 50];
        assert_eq!(s.find_fwd(&hay2, 0), None);
        // 'A' = 0x41 - same low nibble (1), different high (4)
        let hay3 = vec![b'A'; 50];
        assert_eq!(s.find_fwd(&hay3, 0), None);
    }

    #[test]
    fn rev_prefix_no_nibble_collision() {
        let sets_raw = vec![vec![b'c']];
        let all_sets = vec![TSet::from_bytes(&[b'c'])];
        let s = RevPrefixSearch::new(1, &sets_raw, all_sets);
        // 'c' = 0x63, 's' = 0x73 - same low nibble
        let hay = vec![b's'; 50];
        assert_eq!(s.find_rev(&hay, 49), None);
    }

    #[test]
    fn rev_prefix_finds_last() {
        let sets_raw = vec![vec![b'X']];
        let all_sets = vec![TSet::from_bytes(&[b'X'])];
        let s = RevPrefixSearch::new(1, &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        hay[10] = b'X';
        hay[20] = b'X';
        hay[40] = b'X';
        // searching from end=49, should find the LAST occurrence (40)
        assert_eq!(s.find_rev(&hay, 49), Some(40));
        // searching from end=39, should find 20
        assert_eq!(s.find_rev(&hay, 39), Some(20));
        // searching from end=19, should find 10
        assert_eq!(s.find_rev(&hay, 19), Some(10));
    }

    #[test]
    fn fwd_prefix_finds_first() {
        let sets_raw = vec![vec![b'X']];
        let all_sets = vec![TSet::from_bytes(&[b'X'])];
        let s = FwdPrefixSearch::new(1, &[0], &sets_raw, all_sets);
        let mut hay = vec![b'.'; 50];
        hay[10] = b'X';
        hay[20] = b'X';
        hay[40] = b'X';
        assert_eq!(s.find_fwd(&hay, 0), Some(10));
        assert_eq!(s.find_fwd(&hay, 11), Some(20));
        assert_eq!(s.find_fwd(&hay, 21), Some(40));
        assert_eq!(s.find_fwd(&hay, 41), None);
    }

    #[test]
    fn fwd_prefix_teddy3_second_chunk() {
        let sets_raw = vec![vec![b'a'], vec![b'b'], vec![b'c']];
        let all_sets = vec![
            TSet::from_bytes(&[b'a']),
            TSet::from_bytes(&[b'b']),
            TSet::from_bytes(&[b'c']),
        ];
        let s = FwdPrefixSearch::new(3, &[0, 1, 2], &sets_raw, all_sets);
        // 48 bytes: match in the second 16-byte chunk of the double-pump
        let mut hay = vec![b'.'; 48];
        hay[20] = b'a';
        hay[21] = b'b';
        hay[22] = b'c';
        assert_eq!(s.find_fwd(&hay, 0), Some(20));
    }

    #[test]
    fn rev_prefix_teddy3_second_chunk() {
        let sets_raw = vec![vec![b'c'], vec![b'b'], vec![b'a']];
        let all_sets = vec![
            TSet::from_bytes(&[b'c']),
            TSet::from_bytes(&[b'b']),
            TSet::from_bytes(&[b'a']),
        ];
        let s = RevPrefixSearch::new(3, &sets_raw, all_sets);
        // 80 bytes: match early, found during second chunk of double-pump
        let mut hay = vec![b'.'; 80];
        hay[20] = b'a';
        hay[21] = b'b';
        hay[22] = b'c';
        assert_eq!(s.find_rev(&hay, 79), Some(22));
    }

    #[test]
    fn movemask_mixed_values() {
        unsafe {
            // various values with bit 7 set
            let arr: [u8; 16] = [
                0x80, 0, 0xFF, 0, 0x80, 0, 0, 0xFE, 0, 0, 0x80, 0, 0, 0, 0xC0, 0,
            ];
            let v = vld1q_u8(arr.as_ptr());
            let m = neon_movemask(v);
            // bits 0, 2, 4, 7, 10, 14 should be set
            assert_eq!(
                m,
                (1 << 0) | (1 << 2) | (1 << 4) | (1 << 7) | (1 << 10) | (1 << 14)
            );
        }
    }

    #[test]
    fn rev_search_2bytes_long() {
        let s = RevSearchBytes::new(vec![b'X', b'Y']);
        let mut hay = vec![b'.'; 64];
        hay[50] = b'Y';
        assert_eq!(s.find_rev(&hay), Some(50));
        hay[50] = b'.';
        hay[10] = b'X';
        assert_eq!(s.find_rev(&hay), Some(10));
    }

    #[test]
    fn rev_search_3bytes_long() {
        let s = RevSearchBytes::new(vec![b'X', b'Y', b'Z']);
        let mut hay = vec![b'.'; 64];
        hay[60] = b'Z';
        assert_eq!(s.find_rev(&hay), Some(60));
        // check all 3 bytes are found
        hay[60] = b'.';
        hay[5] = b'X';
        assert_eq!(s.find_rev(&hay), Some(5));
        hay[5] = b'.';
        hay[30] = b'Y';
        assert_eq!(s.find_rev(&hay), Some(30));
    }
}
