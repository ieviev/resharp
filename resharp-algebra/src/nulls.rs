use rustc_hash::FxHashMap;
use std::fmt::Debug;

#[derive(Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Nullability(pub u8);

impl Debug for Nullability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = &self.0;
        f.write_str(format!("{num}").as_str())
    }
}
impl Nullability {
    pub const NEVER: Nullability = Nullability(0b000);
    pub const CENTER: Nullability = Nullability(0b001);
    pub const ALWAYS: Nullability = Nullability(0b111);
    pub const BEGIN: Nullability = Nullability(0b010);
    pub const END: Nullability = Nullability(0b100);
    pub const NONBEGIN: Nullability = Nullability(0b011);
    pub const EMPTYSTRING: Nullability = Nullability(0b110);
    #[inline]
    pub fn has(self, flag: Nullability) -> bool {
        self.0 & flag.0 != 0
    }
    #[inline]
    pub fn and(self, other: Nullability) -> Nullability {
        Nullability(self.0 & other.0)
    }
    #[inline]
    pub fn or(self, other: Nullability) -> Nullability {
        Nullability(self.0 | other.0)
    }
    #[inline]
    pub fn not(self) -> Nullability {
        Nullability(!self.0)
    }
}

#[derive(PartialEq, Eq, Clone, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct NullState {
    pub mask: Nullability,
    pub rel: u32,
}
impl NullState {
    pub fn new(mask: Nullability, rel: u32) -> NullState {
        NullState { mask, rel }
    }
    pub fn new0(mask: Nullability) -> NullState {
        NullState { mask, rel: 0 }
    }

    pub fn is_center_nullable(&self) -> bool {
        self.mask.and(Nullability::CENTER) != Nullability::NEVER
    }
    pub fn is_mask_nullable(&self, mask: Nullability) -> bool {
        self.mask.and(mask) != Nullability::NEVER
    }
}
impl Ord for NullState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .rel
            .cmp(&self.rel)
            .then_with(|| self.mask.cmp(&other.mask))
    }
}
impl PartialOrd for NullState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Debug for NullState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entry(&self.mask).entry(&self.rel).finish()
    }
}

type Nulls = BTreeSet<NullState>;

#[derive(Clone, Copy, PartialEq, Hash, Eq, Debug, PartialOrd, Ord)]
pub struct NullsId(pub u32);
impl NullsId {
    pub const EMPTY: NullsId = NullsId(0);
    pub const CENTER0: NullsId = NullsId(1);
    pub const ALWAYS0: NullsId = NullsId(2);
    pub const BEGIN0: NullsId = NullsId(3);
    pub const END0: NullsId = NullsId(4);
}

pub const EID_NONE: u32 = NullsId::EMPTY.0;
pub const EID_CENTER0: u32 = NullsId::CENTER0.0;
pub const EID_ALWAYS0: u32 = NullsId::ALWAYS0.0;
pub const EID_BEGIN0: u32 = NullsId::BEGIN0.0;
pub const EID_END0: u32 = NullsId::END0.0;

pub fn has_any_null(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    mask: Nullability,
) -> bool {
    let eid = effects_id[state as usize] as u32;
    if eid == 0 {
        return false;
    }
    if eid == EID_ALWAYS0 {
        return mask.has(Nullability::ALWAYS);
    }
    if eid == EID_CENTER0 {
        return mask.has(Nullability::CENTER);
    }
    effects[eid as usize].iter().any(|n| n.mask.has(mask))
}

/// Descending, non-overlapping `[lo, hi)` match-start runs (`runs[0]` highest).
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct StartPositions {
    runs: Vec<(usize, usize)>,
}

impl StartPositions {
    #[inline]
    pub fn new() -> Self {
        StartPositions { runs: Vec::new() }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.runs.clear();
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.runs.iter().map(|&(lo, hi)| hi - lo).sum()
    }

    #[inline]
    pub fn min_pos(&self) -> Option<usize> {
        self.runs.last().map(|&(lo, _)| lo)
    }

    #[inline]
    pub fn runs(&self) -> &[(usize, usize)] {
        &self.runs
    }

    /// Insert one position, preserving the descending non-overlapping invariant.
    #[inline]
    pub fn add(&mut self, pos: usize) {
        match self.runs.last_mut() {
            None => self.runs.push((pos, pos + 1)),
            Some(last) => {
                if pos < last.0 {
                    if pos + 1 == last.0 {
                        last.0 = pos;
                    } else {
                        self.runs.push((pos, pos + 1));
                    }
                } else if pos < last.1 {
                } else {
                    self.insert_general(pos);
                }
            }
        }
    }

    /// Append a descending run `[lo, hi)` at or below the current minimum.
    #[inline]
    pub fn add_range(&mut self, lo: usize, hi: usize) {
        if lo >= hi {
            return;
        }
        match self.runs.last_mut() {
            Some(last) if hi == last.0 => last.0 = lo,
            Some(last) if hi < last.0 => self.runs.push((lo, hi)),
            None => self.runs.push((lo, hi)),
            Some(_) => {
                for p in (lo..hi).rev() {
                    self.add(p);
                }
            }
        }
    }

    #[cold]
    fn insert_general(&mut self, pos: usize) {
        let mut i = 0;
        while i < self.runs.len() && self.runs[i].0 > pos {
            i += 1;
        }
        if i < self.runs.len() {
            let (lo, hi) = self.runs[i];
            debug_assert!(lo <= pos);
            if pos < hi {
                return;
            }
        }
        self.runs.insert(i, (pos, pos + 1));
    }

    #[inline]
    pub fn positions_desc(&self) -> impl Iterator<Item = usize> + '_ {
        self.runs.iter().flat_map(|&(lo, hi)| (lo..hi).rev())
    }

    #[inline]
    pub fn positions_asc(&self) -> impl Iterator<Item = usize> + '_ {
        self.runs.iter().rev().flat_map(|&(lo, hi)| lo..hi)
    }
}

#[inline]
pub fn push_null_desc(nulls: &mut StartPositions, v: usize) {
    nulls.add(v);
}

#[inline(always)]
pub fn collect_nulls(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    nulls: &mut StartPositions,
) {
    let eid = effects_id[state as usize] as u32;
    if eid != 0 {
        match eid {
            EID_ALWAYS0 => {
                if mask.has(Nullability::ALWAYS) {
                    nulls.add(pos);
                }
            }
            EID_CENTER0 => {
                if mask.has(Nullability::CENTER) {
                    nulls.add(pos);
                }
            }
            EID_BEGIN0 => {
                if mask.has(Nullability::BEGIN) {
                    nulls.add(pos);
                }
            }
            EID_END0 => {
                if mask.has(Nullability::END) {
                    nulls.add(pos);
                }
            }
            _ => {
                for n in &effects[eid as usize] {
                    if n.mask.has(mask) {
                        let resolved = pos + n.rel as usize;
                        nulls.add(resolved);
                    }
                }
            }
        }
    }
}

use std::{collections::BTreeSet, hash::Hash};

#[repr(u8)]
#[derive(Hash, PartialEq, Eq)]
enum Operation {
    Or,
    Inter,
}

#[derive(Hash, PartialEq, Eq)]
struct Key {
    op: Operation,
    left: NullsId,
    right: NullsId,
}

pub struct NullsBuilder {
    cache: FxHashMap<Nulls, NullsId>,
    created: FxHashMap<Key, NullsId>,
    add_rel_cache: FxHashMap<(NullsId, u32), NullsId>,
    pub array: Vec<Nulls>,
}

impl Default for NullsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NullsBuilder {
    pub fn new() -> NullsBuilder {
        let mut inst = Self {
            cache: FxHashMap::default(),
            array: Vec::new(),
            created: FxHashMap::default(),
            add_rel_cache: FxHashMap::default(),
        };
        let _ = inst.init(BTreeSet::new());
        let _center = inst.init1(NullState::new0(Nullability::CENTER));
        let _always = inst.init1(NullState::new0(Nullability::ALWAYS));
        let _begin = inst.init1(NullState::new0(Nullability::BEGIN));
        let _end = inst.init1(NullState::new0(Nullability::END));
        debug_assert!(_center == NullsId::CENTER0);
        debug_assert!(_always == NullsId::ALWAYS0);
        debug_assert!(_begin == NullsId::BEGIN0);
        debug_assert!(_end == NullsId::END0);
        inst
    }

    fn init(&mut self, inst: Nulls) -> NullsId {
        let new_id = NullsId(self.cache.len() as u32);
        self.cache.insert(inst.clone(), new_id);
        self.array.push(inst);
        new_id
    }

    fn init1(&mut self, inst: NullState) -> NullsId {
        let mut b = BTreeSet::new();
        b.insert(inst);
        let new_id = NullsId(self.cache.len() as u32);
        self.cache.insert(b.clone(), new_id);
        self.array.push(b);
        new_id
    }

    pub fn get_set_ref(&self, set_id: NullsId) -> &Nulls {
        &self.array[set_id.0 as usize]
    }

    pub fn get_id(&mut self, inst: Nulls) -> NullsId {
        match self.cache.get(&inst) {
            Some(&id) => id,
            None => self.init(inst),
        }
    }
}

impl NullsBuilder {
    #[inline]
    fn is_created(&self, inst: &Key) -> Option<&NullsId> {
        self.created.get(inst)
    }

    #[inline]
    pub fn or_id(&mut self, set1: NullsId, set2: NullsId) -> NullsId {
        if set1 > set2 {
            return self.or_id(set2, set1);
        }
        let key = Key {
            op: Operation::Or,
            left: set1,
            right: set2,
        };
        if let Some(v) = self.is_created(&key) {
            return *v;
        }
        if set1 == set2 {
            return set1;
        }
        if set1 == NullsId::ALWAYS0 && set2 == NullsId::END0 {
            return NullsId::ALWAYS0;
        }

        let all = self.get_set_ref(set1) | self.get_set_ref(set2);
        let mut result: BTreeSet<&NullState> = BTreeSet::new();
        for m in all.iter().rev() {
            let dominated = result
                .iter()
                .any(|v| v.rel == m.rel && (v.mask.0 & m.mask.0) == m.mask.0);
            if !dominated {
                result.insert(m);
            }
        }

        let result = result.into_iter().cloned().collect::<BTreeSet<_>>();

        let new_id = self.get_id(result);
        self.created.insert(key, new_id);
        new_id
    }

    #[inline]
    pub fn and_id(&mut self, set1: NullsId, set2: NullsId) -> NullsId {
        if set1 > set2 {
            return self.and_id(set2, set1);
        }
        if NullsId::EMPTY == set1 {
            return NullsId::EMPTY;
        }
        let key = Key {
            op: Operation::Inter,
            left: set1,
            right: set2,
        };
        if let Some(v) = self.is_created(&key) {
            return *v;
        }
        if set1 == set2 {
            return set1;
        }
        let s1 = self.get_set_ref(set1).clone();
        let s2 = self.get_set_ref(set2).clone();
        let mut result: BTreeSet<NullState> = BTreeSet::new();
        for ns1 in &s1 {
            for ns2 in &s2 {
                if ns1.rel == ns2.rel {
                    let mask = ns1.mask.and(ns2.mask);
                    if mask != Nullability::NEVER {
                        result.insert(NullState::new(mask, ns1.rel));
                    }
                }
            }
        }
        let result = self.get_id(result);
        self.created.insert(key, result);
        result
    }

    #[inline]
    pub fn and_mask(&mut self, set1: NullsId, mask: Nullability) -> NullsId {
        if NullsId::EMPTY == set1 || mask == Nullability::NEVER {
            return NullsId::EMPTY;
        }
        if mask == Nullability::ALWAYS {
            return set1;
        }
        let remaining = self
            .get_set_ref(set1)
            .iter()
            .filter_map(|v| {
                let newmask = v.mask.and(mask);
                if newmask == Nullability::NEVER {
                    None
                } else {
                    Some(NullState::new(newmask, v.rel))
                }
            })
            .collect::<BTreeSet<_>>();

        self.get_id(remaining)
    }

    #[inline]
    pub fn not_id(&mut self, set_id: NullsId) -> NullsId {
        if set_id == NullsId::EMPTY {
            return NullsId::ALWAYS0;
        }
        if set_id == NullsId::ALWAYS0 {
            return NullsId::EMPTY;
        }
        if set_id == NullsId::BEGIN0 {
            return self.or_id(NullsId::CENTER0, NullsId::END0);
        }
        if set_id == NullsId::END0 {
            return self.or_id(NullsId::CENTER0, NullsId::BEGIN0);
        }
        NullsId::EMPTY
    }

    pub fn max_rel(&self, set_id: NullsId) -> u32 {
        self.get_set_ref(set_id).iter().next().map_or(0, |ns| ns.rel)
    }

    pub fn min_rel(&self, set_id: NullsId) -> u32 {
        self.get_set_ref(set_id).iter().next_back().map_or(0, |ns| ns.rel)
    }

    #[inline]
    pub fn add_rel(&mut self, set_id: NullsId, rel: u32) -> NullsId {
        if rel == 0 || rel == u32::MAX {
            return set_id;
        }
        if let Some(&cached) = self.add_rel_cache.get(&(set_id, rel)) {
            return cached;
        }
        let res = self.get_set_ref(set_id).clone();
        let with_rel = res
            .iter()
            .map(|v| NullState::new(v.mask, v.rel + rel))
            .collect();
        let result = self.get_id(with_rel);
        self.add_rel_cache.insert((set_id, rel), result);
        result
    }
}
