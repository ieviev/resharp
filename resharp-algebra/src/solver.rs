#[derive(Clone, Copy, PartialEq, Hash, Eq, Debug, PartialOrd, Ord)]
pub struct TSet(pub [u64; 4]);

impl TSet {
    #[inline]
    pub const fn splat(v: u64) -> Self {
        TSet([v, v, v, v])
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut bits = [0u64; 4];
        for &b in bytes {
            bits[b as usize / 64] |= 1u64 << (b as usize % 64);
        }
        Self(bits)
    }

    #[inline(always)]
    pub fn contains_byte(&self, b: u8) -> bool {
        self.0[b as usize / 64] & (1u64 << (b as usize % 64)) != 0
    }
}

impl std::ops::Index<usize> for TSet {
    type Output = u64;
    #[inline]
    fn index(&self, i: usize) -> &u64 {
        &self.0[i]
    }
}

impl std::ops::IndexMut<usize> for TSet {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut u64 {
        &mut self.0[i]
    }
}

impl std::ops::BitAnd for TSet {
    type Output = TSet;
    #[inline]
    fn bitand(self, rhs: TSet) -> TSet {
        TSet([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl std::ops::BitAnd for &TSet {
    type Output = TSet;
    #[inline]
    fn bitand(self, rhs: &TSet) -> TSet {
        TSet([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl std::ops::BitOr for TSet {
    type Output = TSet;
    #[inline]
    fn bitor(self, rhs: TSet) -> TSet {
        TSet([
            self.0[0] | rhs.0[0],
            self.0[1] | rhs.0[1],
            self.0[2] | rhs.0[2],
            self.0[3] | rhs.0[3],
        ])
    }
}

impl std::ops::Not for TSet {
    type Output = TSet;
    #[inline]
    fn not(self) -> TSet {
        TSet([!self.0[0], !self.0[1], !self.0[2], !self.0[3]])
    }
}

// &TSet ops used by Solver helper methods
impl std::ops::BitAnd<TSet> for &TSet {
    type Output = TSet;
    #[inline]
    fn bitand(self, rhs: TSet) -> TSet {
        TSet([
            self.0[0] & rhs.0[0],
            self.0[1] & rhs.0[1],
            self.0[2] & rhs.0[2],
            self.0[3] & rhs.0[3],
        ])
    }
}

impl std::ops::BitOr<TSet> for &TSet {
    type Output = TSet;
    #[inline]
    fn bitor(self, rhs: TSet) -> TSet {
        TSet([
            self.0[0] | rhs.0[0],
            self.0[1] | rhs.0[1],
            self.0[2] | rhs.0[2],
            self.0[3] | rhs.0[3],
        ])
    }
}

const EMPTY: TSet = TSet::splat(u64::MIN);
const FULL: TSet = TSet::splat(u64::MAX);

#[derive(Clone, Copy, PartialEq, Hash, Eq, Debug, PartialOrd, Ord)]
pub struct TSetId(pub u32);
impl TSetId {
    pub const EMPTY: TSetId = TSetId(0);
    pub const FULL: TSetId = TSetId(1);
}

use std::collections::{BTreeMap, BTreeSet};

pub struct Solver {
    cache: BTreeMap<TSet, TSetId>,
    pub array: Vec<TSet>,
}

impl Solver {
    pub fn new() -> Solver {
        let mut inst = Self {
            cache: BTreeMap::new(),
            array: Vec::new(),
        };
        let _ = inst.init(Solver::empty()); // 0
        let _ = inst.init(Solver::full()); // 1
        inst
    }

    fn init(&mut self, inst: TSet) -> TSetId {
        let new_id = TSetId(self.cache.len() as u32);
        self.cache.insert(inst, new_id);
        self.array.push(inst);
        new_id
    }

    pub fn get_set(&self, set_id: TSetId) -> TSet {
        self.array[set_id.0 as usize]
    }

    pub fn get_set_ref(&self, set_id: TSetId) -> &TSet {
        &self.array[set_id.0 as usize]
    }

    pub fn get_id(&mut self, inst: TSet) -> TSetId {
        match self.cache.get(&inst) {
            Some(&id) => id,
            None => self.init(inst),
        }
    }

    pub fn has_bit_set(&mut self, set_id: TSetId, idx: usize, bit: u64) -> bool {
        self.array[set_id.0 as usize][idx] & bit != 0
    }

    pub fn pp_collect_ranges(tset: &TSet) -> BTreeSet<(u8, u8)> {
        let mut ranges: BTreeSet<(u8, u8)> = BTreeSet::new();
        let mut rangestart: Option<u8> = None;
        let mut prevchar: Option<u8> = None;
        for i in 0..4 {
            for j in 0..64 {
                let nthbit = 1u64 << j;
                if tset[i] & nthbit != 0 {
                    let cc = (i * 64 + j) as u8;
                    if rangestart.is_none() {
                        rangestart = Some(cc);
                        prevchar = Some(cc);
                        continue;
                    }

                    if let Some(currstart) = rangestart {
                        if let Some(currprev) = prevchar {
                            if currprev as u8 == cc as u8 - 1 {
                                prevchar = Some(cc);
                                continue;
                            } else {
                                if currstart == currprev {
                                    ranges.insert((currstart, currstart));
                                } else {
                                    ranges.insert((currstart, currprev));
                                }
                                rangestart = Some(cc);
                                prevchar = Some(cc);
                            }
                        } else {
                        }
                    } else {
                    }
                }
            }
        }
        if let Some(start) = rangestart {
            if let Some(prevchar) = prevchar {
                if prevchar as u8 == start as u8 {
                    ranges.insert((start, start));
                } else {
                    ranges.insert((start, prevchar));
                }
            } else {
                // single char
                ranges.insert((start, start));
            }
        }
        ranges
    }

    fn pp_byte(b: u8) -> String {
        if cfg!(feature = "graphviz") {
            match b as char {
                // graphviz doesnt like \n so we use \ṅ
                '\n' => return r"\ṅ".to_owned(),
                '"' => return r"\u{201c}".to_owned(),
                '\r' => return r"\r".to_owned(),
                '\t' => return r"\t".to_owned(),
                _ => {}
            }
        }
        match b as char {
            '\n' => r"\n".to_owned(),
            '\r' => r"\r".to_owned(),
            '\t' => r"\t".to_owned(),
            ' ' => r" ".to_owned(),
            '_' | '.' | '+' | '-' | '\\' | '&' | '|' | '~' | '{' | '}' | '[' | ']' | '(' | ')'
            | '*' | '?' | '^' | '$' => r"\".to_owned() + &(b as char).to_string(),
            c if c.is_ascii_punctuation() || c.is_ascii_alphanumeric() => c.to_string(),
            _ => format!("\\x{:02X}", b),
        }
    }

    fn pp_content(ranges: &BTreeSet<(u8, u8)>) -> String {
        let display_range = |c, c2| {
            if c == c2 {
                Self::pp_byte(c)
            } else if c.abs_diff(c2) == 1 {
                format!("{}{}", Self::pp_byte(c), Self::pp_byte(c2))
            } else {
                format!("{}-{}", Self::pp_byte(c), Self::pp_byte(c2))
            }
        };

        if ranges.len() == 0 {
            return "\u{22a5}".to_owned();
        }
        if ranges.len() == 1 {
            let (s, e) = ranges.iter().next().unwrap();
            if s == e {
                return Self::pp_byte(*s);
            } else {
                return format!(
                    "{}",
                    ranges
                        .iter()
                        .map(|(s, e)| display_range(*s, *e))
                        .collect::<Vec<_>>()
                        .join("")
                );
            }
        }
        if ranges.len() > 20 {
            return "\u{03c6}".to_owned();
        }
        return format!(
            "{}",
            ranges
                .iter()
                .map(|(s, e)| display_range(*s, *e))
                .collect::<Vec<_>>()
                .join("")
        );
    }

    pub fn pp_first(&self, tset: &TSet) -> char {
        let tryn1 = |i: usize| {
            for j in 0..32 {
                let nthbit = 1u64 << j;
                if tset[i] & nthbit != 0 {
                    let cc = (i * 64 + j) as u8 as char;
                    return Some(cc);
                }
            }
            None
        };
        let tryn2 = |i: usize| {
            for j in 33..64 {
                let nthbit = 1u64 << j;
                if tset[i] & nthbit != 0 {
                    let cc = (i * 64 + j) as u8 as char;
                    return Some(cc);
                }
            }
            None
        };
        // readable ones first
        tryn2(0)
            .or_else(|| tryn2(1))
            .or_else(|| tryn1(1))
            .or_else(|| tryn1(2))
            .or_else(|| tryn2(2))
            .or_else(|| tryn1(3))
            .or_else(|| tryn2(3))
            .or_else(|| tryn1(0))
            .unwrap_or('\u{22a5}')
    }

    pub fn byte_ranges(&self, tset: TSetId) -> Vec<(u8, u8)> {
        let tset = self.get_set(tset);
        Self::pp_collect_ranges(&tset).into_iter().collect()
    }

    #[allow(unused)]
    fn first_byte(tset: &TSet) -> u8 {
        for i in 0..4 {
            for j in 0..64 {
                let nthbit = 1u64 << j;
                if tset[i] & nthbit != 0 {
                    let cc = (i * 64 + j) as u8;
                    return cc;
                }
            }
        }
        return 0;
    }

    pub fn pp(&self, tset: TSetId) -> String {
        if tset == TSetId::FULL {
            return "_".to_owned();
        }
        if tset == TSetId::EMPTY {
            return "\u{22a5}".to_owned();
        }
        let tset = self.get_set(tset);
        let ranges: BTreeSet<(u8, u8)> = Self::pp_collect_ranges(&tset);
        let rstart = ranges.first().unwrap().0;
        let rend = ranges.last().unwrap().1;
        if ranges.len() >= 2 && rstart == 0 && rend == 255 {
            let not_id = Self::not(&tset);
            let not_ranges = Self::pp_collect_ranges(&not_id);
            if not_ranges.len() == 1 && not_ranges.iter().next() == Some(&(10, 10)) {
                return r".".to_owned();
            }
            let content = Self::pp_content(&not_ranges);
            return format!("[^{}]", content);
        }
        if ranges.len() == 0 {
            return "\u{22a5}".to_owned();
        }
        if ranges.len() == 1 {
            let (s, e) = ranges.iter().next().unwrap();
            if s == e {
                return Self::pp_byte(*s);
            } else {
                let content = Self::pp_content(&ranges);
                return format!("[{}]", content);
            }
        }
        let content = Self::pp_content(&ranges);
        return format!("[{}]", content);
    }
}

impl Solver {
    #[inline]
    pub fn full() -> TSet {
        FULL
    }

    #[inline]
    pub fn empty() -> TSet {
        EMPTY
    }

    #[inline]
    pub fn or_id(&mut self, set1: TSetId, set2: TSetId) -> TSetId {
        self.get_id(self.get_set(set1) | self.get_set(set2))
    }

    #[inline]
    pub fn and_id(&mut self, set1: TSetId, set2: TSetId) -> TSetId {
        self.get_id(self.get_set(set1) & self.get_set(set2))
    }

    #[inline]
    pub fn not_id(&mut self, set_id: TSetId) -> TSetId {
        self.get_id(!self.get_set(set_id))
    }

    #[inline]
    pub fn is_sat_id(&mut self, set1: TSetId, set2: TSetId) -> bool {
        self.and_id(set1, set2) != TSetId::EMPTY
    }
    #[inline]
    pub fn unsat_id(&mut self, set1: TSetId, set2: TSetId) -> bool {
        self.and_id(set1, set2) == TSetId::EMPTY
    }

    pub fn byte_count(&self, set_id: TSetId) -> u32 {
        let tset = self.get_set(set_id);
        (0..4).map(|i| tset[i].count_ones()).sum()
    }

    pub fn collect_bytes(&self, set_id: TSetId) -> Vec<u8> {
        let tset = self.get_set(set_id);
        let mut bytes = Vec::new();
        for i in 0..4 {
            let mut bits = tset[i];
            while bits != 0 {
                let j = bits.trailing_zeros() as usize;
                bytes.push((i * 64 + j) as u8);
                bits &= bits - 1;
            }
        }
        bytes
    }

    pub fn single_byte(&self, set_id: TSetId) -> Option<u8> {
        let tset = self.get_set(set_id);
        let total: u32 = (0..4).map(|i| tset[i].count_ones()).sum();
        if total != 1 {
            return None;
        }
        for i in 0..4 {
            if tset[i] != 0 {
                return Some((i * 64 + tset[i].trailing_zeros() as usize) as u8);
            }
        }
        None
    }

    #[inline]
    pub fn is_empty_id(&self, set1: TSetId) -> bool {
        set1 == TSetId::EMPTY
    }

    #[inline]
    pub fn is_full_id(&self, set1: TSetId) -> bool {
        set1 == TSetId::FULL
    }

    #[inline]
    pub fn contains_id(&mut self, large_id: TSetId, small_id: TSetId) -> bool {
        let not_large = self.not_id(large_id);
        self.and_id(small_id, not_large) == TSetId::EMPTY
    }

    pub fn u8_to_set_id(&mut self, byte: u8) -> TSetId {
        let mut result = TSet::splat(u64::MIN);
        let nthbit = 1u64 << byte % 64;
        match byte {
            0..=63 => {
                result[0] = nthbit;
            }
            64..=127 => {
                result[1] = nthbit;
            }
            128..=191 => {
                result[2] = nthbit;
            }
            192..=255 => {
                result[3] = nthbit;
            }
        }
        self.get_id(result)
    }

    pub fn range_to_set_id(&mut self, start: u8, end: u8) -> TSetId {
        let mut result = TSet::splat(u64::MIN);
        for byte in start..=end {
            let nthbit = 1u64 << byte % 64;
            match byte {
                0..=63 => {
                    result[0] |= nthbit;
                }
                64..=127 => {
                    result[1] |= nthbit;
                }
                128..=191 => {
                    result[2] |= nthbit;
                }
                192..=255 => {
                    result[3] |= nthbit;
                }
            }
        }
        self.get_id(result)
    }

    #[inline]
    pub fn and(set1: &TSet, set2: &TSet) -> TSet {
        *set1 & *set2
    }

    #[inline]
    pub fn is_sat(set1: &TSet, set2: &TSet) -> bool {
        *set1 & *set2 != Solver::empty()
    }

    #[inline]
    pub fn or(set1: &TSet, set2: &TSet) -> TSet {
        *set1 | *set2
    }

    #[inline]
    pub fn not(set: &TSet) -> TSet {
        !*set
    }

    #[inline]
    pub fn is_full(set: &TSet) -> bool {
        *set == Self::full()
    }

    #[inline]
    pub fn is_empty(set: &TSet) -> bool {
        *set == Solver::empty()
    }

    #[inline]
    pub fn contains(large: &TSet, small: &TSet) -> bool {
        Solver::empty() == (*small & !*large)
    }

    pub fn u8_to_set(byte: u8) -> TSet {
        let mut result = TSet::splat(u64::MIN);
        let nthbit = 1u64 << byte % 64;
        match byte {
            0..=63 => {
                result[0] = nthbit;
            }
            64..=127 => {
                result[1] = nthbit;
            }
            128..=191 => {
                result[2] = nthbit;
            }
            192..=255 => {
                result[3] = nthbit;
            }
        }
        result
    }

    pub fn range_to_set(start: u8, end: u8) -> TSet {
        let mut result = TSet::splat(u64::MIN);
        for byte in start..=end {
            let nthbit = 1u64 << byte % 64;
            match byte {
                0..=63 => {
                    result[0] |= nthbit;
                }
                64..=127 => {
                    result[1] |= nthbit;
                }
                128..=191 => {
                    result[2] |= nthbit;
                }
                192..=255 => {
                    result[3] |= nthbit;
                }
            }
        }
        result
    }
}
