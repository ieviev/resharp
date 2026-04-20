//! WebAssembly SIMD (simd128) backend.

use core::arch::wasm32::*;

use super::{TSet, TeddyMasks, BYTE_FREQ};

#[inline(always)]
unsafe fn teddy_chunk<const N: usize>(
    ptr: *const u8,
    pos: usize,
    offsets: &[usize; 3],
    masks_lo: &[v128; 3],
    masks_hi: &[v128; 3],
    nib: v128,
) -> v128 {
    let load = |i: usize| v128_load(ptr.add(pos + offsets[i]) as *const v128);
    let lookup = |i: usize, c: v128| {
        v128_and(
            u8x16_swizzle(masks_lo[i], v128_and(c, nib)),
            u8x16_swizzle(masks_hi[i], u8x16_shr(c, 4)),
        )
    };
    let mut r = lookup(0, load(0));
    if N >= 2 {
        r = v128_and(r, lookup(1, load(1)));
    }
    if N >= 3 {
        r = v128_and(r, lookup(2, load(2)));
    }
    r
}

#[inline(always)]
unsafe fn teddy_chunk_rev<const N: usize>(
    ptr: *const u8,
    chunk_pos: usize,
    masks_lo: &[v128; 3],
    masks_hi: &[v128; 3],
    nib: v128,
) -> v128 {
    let load = |off: usize| v128_load(ptr.add(chunk_pos - off) as *const v128);
    let lookup = |i: usize, c: v128| {
        v128_and(
            u8x16_swizzle(masks_lo[i], v128_and(c, nib)),
            u8x16_swizzle(masks_hi[i], u8x16_shr(c, 4)),
        )
    };
    let mut r = lookup(0, load(0));
    if N >= 2 {
        r = v128_and(r, lookup(1, load(1)));
    }
    if N >= 3 {
        r = v128_and(r, lookup(2, load(2)));
    }
    r
}

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FwdLiteralSearch {
    pub(crate) needle: Vec<u8>,
    chunks: Vec<u64>,
    rare_idx: usize,
    rare_byte: u8,
    confirm: (usize, u8),
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
        let mut sink: Vec<(usize, usize)> = Vec::new();
        unsafe { self.scan::<false>(haystack, &mut sink) }
    }

    pub fn find_all_fixed(&self, haystack: &[u8], matches: &mut Vec<(usize, usize)>) {
        unsafe {
            self.scan::<true>(haystack, matches);
        }
    }

    // COLLECT_ALL=false: return first match start (sink unused).
    // COLLECT_ALL=true:  push every non-overlapping match into `matches`, return None.
    #[inline(always)]
    unsafe fn scan<const COLLECT_ALL: bool>(
        &self,
        haystack: &[u8],
        matches: &mut Vec<(usize, usize)>,
    ) -> Option<usize> {
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
        let vrare = u8x16_splat(rare_byte);
        let mut last_end: usize = 0;

        let mut handle = |this: &Self, start: usize| -> Option<usize> {
            if COLLECT_ALL && start < last_end {
                return None;
            }
            if *ptr.add(start + confirm_idx) != confirm_byte || !this.verify(haystack, start) {
                return None;
            }
            if COLLECT_ALL {
                let m_end = start + nlen;
                matches.push((start, m_end));
                last_end = m_end;
                None
            } else {
                Some(start)
            }
        };

        let mut pos = rare_idx;
        while pos + 16 <= end + 1 {
            let chunk = v128_load(ptr.add(pos) as *const v128);
            let mut mask = u8x16_bitmask(u8x16_eq(chunk, vrare));
            while mask != 0 {
                let bit = mask.trailing_zeros() as usize;
                let start = pos + bit - rare_idx;
                if let Some(s) = handle(self, start) {
                    return Some(s);
                }
                mask &= mask - 1;
            }
            pos += 16;
        }
        while pos <= end {
            if *ptr.add(pos) == rare_byte {
                let start = pos - rare_idx;
                if let Some(s) = handle(self, start) {
                    return Some(s);
                }
            }
            pos += 1;
        }
        None
    }
}

#[inline(always)]
unsafe fn scan_chunk_bytes<const N: usize>(chunk: v128, v: &[v128; 3]) -> v128 {
    let c0 = u8x16_eq(chunk, v[0]);
    if N >= 3 {
        v128_or(c0, v128_or(u8x16_eq(chunk, v[1]), u8x16_eq(chunk, v[2])))
    } else if N >= 2 {
        v128_or(c0, u8x16_eq(chunk, v[1]))
    } else {
        c0
    }
}

#[inline(always)]
unsafe fn scan_chunk_ranges<const N: usize>(
    chunk: v128,
    lo: &[v128; 3],
    hi: &[v128; 3],
) -> v128 {
    let in0 = v128_and(u8x16_ge(chunk, lo[0]), u8x16_le(chunk, hi[0]));
    if N >= 3 {
        let in1 = v128_and(u8x16_ge(chunk, lo[1]), u8x16_le(chunk, hi[1]));
        let in2 = v128_and(u8x16_ge(chunk, lo[2]), u8x16_le(chunk, hi[2]));
        v128_or(in0, v128_or(in1, in2))
    } else if N >= 2 {
        let in1 = v128_and(u8x16_ge(chunk, lo[1]), u8x16_le(chunk, hi[1]));
        v128_or(in0, in1)
    } else {
        in0
    }
}

#[inline(always)]
unsafe fn linear_scan<const FWD: bool>(
    haystack: &[u8],
    mut compute: impl FnMut(v128) -> v128,
) -> Option<usize> {
    let len = haystack.len();
    if len == 0 {
        return None;
    }
    let ptr = haystack.as_ptr();
    if FWD {
        let mut pos = 0;
        while pos + 16 <= len {
            let combined = compute(v128_load(ptr.add(pos) as *const v128));
            if v128_any_true(combined) {
                let mask = u8x16_bitmask(combined) as u16;
                return Some(pos + mask.trailing_zeros() as usize);
            }
            pos += 16;
        }
        if pos < len {
            let mut buf = [0u8; 16];
            buf[..len - pos].copy_from_slice(&haystack[pos..]);
            let combined = compute(v128_load(buf.as_ptr() as *const v128));
            let mut mask = u8x16_bitmask(combined) as u16;
            mask &= (1u16 << (len - pos)) - 1;
            if mask != 0 {
                return Some(pos + mask.trailing_zeros() as usize);
            }
        }
    } else {
        if len >= 16 {
            let mut pos = len - 16;
            loop {
                let combined = compute(v128_load(ptr.add(pos) as *const v128));
                if v128_any_true(combined) {
                    let mask = u8x16_bitmask(combined) as u16;
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
            let combined = compute(v128_load(buf.as_ptr() as *const v128));
            let mut mask = u8x16_bitmask(combined) as u16;
            mask &= (1u16 << gap) - 1;
            if mask != 0 {
                return Some(15 - mask.leading_zeros() as usize);
            }
        }
    }
    None
}

#[cfg_attr(debug_assertions, derive(Debug))]
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
        unsafe { self.search::<true>(haystack) }
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search::<false>(haystack) }
    }

    unsafe fn search<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let n = self.bytes.len();
        let v = [
            u8x16_splat(self.bytes[0]),
            u8x16_splat(self.bytes[if n >= 2 { 1 } else { 0 }]),
            u8x16_splat(self.bytes[if n >= 3 { 2 } else { 0 }]),
        ];
        match n {
            1 => linear_scan::<FWD>(haystack, |c| scan_chunk_bytes::<1>(c, &v)),
            2 => linear_scan::<FWD>(haystack, |c| scan_chunk_bytes::<2>(c, &v)),
            _ => linear_scan::<FWD>(haystack, |c| scan_chunk_bytes::<3>(c, &v)),
        }
    }
}

#[cfg_attr(debug_assertions, derive(Debug))]
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

    pub fn find_fwd(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search::<true>(haystack) }
    }

    pub fn find_rev(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.search::<false>(haystack) }
    }

    unsafe fn search<const FWD: bool>(&self, haystack: &[u8]) -> Option<usize> {
        let n = self.ranges.len();
        let lo = [
            u8x16_splat(self.ranges[0].0),
            u8x16_splat(self.ranges[if n >= 2 { 1 } else { 0 }].0),
            u8x16_splat(self.ranges[if n >= 3 { 2 } else { 0 }].0),
        ];
        let hi = [
            u8x16_splat(self.ranges[0].1),
            u8x16_splat(self.ranges[if n >= 2 { 1 } else { 0 }].1),
            u8x16_splat(self.ranges[if n >= 3 { 2 } else { 0 }].1),
        ];
        match n {
            1 => linear_scan::<FWD>(haystack, |c| scan_chunk_ranges::<1>(c, &lo, &hi)),
            2 => linear_scan::<FWD>(haystack, |c| scan_chunk_ranges::<2>(c, &lo, &hi)),
            _ => linear_scan::<FWD>(haystack, |c| scan_chunk_ranges::<3>(c, &lo, &hi)),
        }
    }
}

#[cfg_attr(debug_assertions, derive(Debug))]
pub struct FwdRangeSearch {
    len: usize,
    pub(crate) anchor_pos: usize,
    pub(crate) ranges: Vec<(u8, u8)>,
    pub(crate) sets: Vec<TSet>,
}

impl FwdRangeSearch {
    pub fn new(len: usize, anchor_pos: usize, ranges: Vec<(u8, u8)>, sets: Vec<TSet>) -> Self {
        debug_assert!(!ranges.is_empty() && ranges.len() <= 3);
        debug_assert!(anchor_pos < len);
        Self { len, anchor_pos, ranges, sets }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe { self.find_fwd_simd(haystack, start) }
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

    unsafe fn find_fwd_simd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let n = self.ranges.len();
        let anchor = self.anchor_pos;
        let lo = [
            u8x16_splat(self.ranges[0].0),
            u8x16_splat(self.ranges[if n >= 2 { 1 } else { 0 }].0),
            u8x16_splat(self.ranges[if n >= 3 { 2 } else { 0 }].0),
        ];
        let hi = [
            u8x16_splat(self.ranges[0].1),
            u8x16_splat(self.ranges[if n >= 2 { 1 } else { 0 }].1),
            u8x16_splat(self.ranges[if n >= 3 { 2 } else { 0 }].1),
        ];

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let mut pos = start;
        while pos < simd_end {
            let chunk = v128_load(ptr.add(pos + anchor) as *const v128);
            let combined = match n {
                1 => scan_chunk_ranges::<1>(chunk, &lo, &hi),
                2 => scan_chunk_ranges::<2>(chunk, &lo, &hi),
                _ => scan_chunk_ranges::<3>(chunk, &lo, &hi),
            };
            let mut mask = u8x16_bitmask(combined) as u32;
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

#[cfg_attr(debug_assertions, derive(Debug))]
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
        let mut masks = Box::new(TeddyMasks { lo: [[0u8; 32]; 3], hi: [[0u8; 32]; 3] });
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
        Self { len, num_simd, masks, sets: all_sets }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn find_rev(&self, haystack: &[u8], end: usize) -> Option<usize> {
        unsafe {
            match self.num_simd {
                1 => self.teddy_rev::<1>(haystack, end),
                2 => self.teddy_rev::<2>(haystack, end),
                _ => self.teddy_rev::<3>(haystack, end),
            }
        }
    }

    unsafe fn teddy_rev<const N: usize>(&self, haystack: &[u8], end: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = u8x16_splat(0x0F);
        let masks_lo = [
            v128_load(self.masks.lo[0].as_ptr() as *const v128),
            v128_load(self.masks.lo[1].as_ptr() as *const v128),
            v128_load(self.masks.lo[2].as_ptr() as *const v128),
        ];
        let masks_hi = [
            v128_load(self.masks.hi[0].as_ptr() as *const v128),
            v128_load(self.masks.hi[1].as_ptr() as *const v128),
            v128_load(self.masks.hi[2].as_ptr() as *const v128),
        ];
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;
        let min_pos = len - 1;

        if end < 15 + min_pos {
            return self.verify_tail(haystack, end);
        }
        let mut chunk_pos = end - 15;

        loop {
            let r = teddy_chunk_rev::<N>(ptr, chunk_pos, &masks_lo, &masks_hi, nib);
            let mask = u8x16_bitmask(r);
            if mask != 0 {
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

#[cfg_attr(debug_assertions, derive(Debug))]
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
        let mut masks = Box::new(TeddyMasks { lo: [[0u8; 32]; 3], hi: [[0u8; 32]; 3] });
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
        Self { len, num_simd, simd_offsets, masks, sets: all_sets, verify_order }
    }

    pub fn find_fwd(&self, haystack: &[u8], start: usize) -> Option<usize> {
        unsafe {
            match self.num_simd {
                1 => self.teddy_fwd::<1>(haystack, start),
                2 => self.teddy_fwd::<2>(haystack, start),
                _ => self.teddy_fwd::<3>(haystack, start),
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

    unsafe fn teddy_fwd<const N: usize>(&self, haystack: &[u8], start: usize) -> Option<usize> {
        let ptr = haystack.as_ptr();
        let nib = u8x16_splat(0x0F);
        let masks_lo = [
            v128_load(self.masks.lo[0].as_ptr() as *const v128),
            v128_load(self.masks.lo[1].as_ptr() as *const v128),
            v128_load(self.masks.lo[2].as_ptr() as *const v128),
        ];
        let masks_hi = [
            v128_load(self.masks.hi[0].as_ptr() as *const v128),
            v128_load(self.masks.hi[1].as_ptr() as *const v128),
            v128_load(self.masks.hi[2].as_ptr() as *const v128),
        ];
        let sets_ptr = self.sets.as_ptr();
        let len = self.len;

        let simd_end = haystack.len().saturating_sub(15 + self.len - 1);
        let mut pos = start;

        while pos < simd_end {
            let r = teddy_chunk::<N>(ptr, pos, &self.simd_offsets, &masks_lo, &masks_hi, nib);
            let mask = u8x16_bitmask(r);
            if mask != 0 {
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
