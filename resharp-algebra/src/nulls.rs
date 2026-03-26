use rustc_hash::FxHashMap;
use std::fmt::Debug;

#[derive(Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord)]
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
    pub const ALWAYS0: NullsId = NullsId(1);
    pub const CENTER0: NullsId = NullsId(2);
    pub const BEGIN0: NullsId = NullsId(3);
    pub const END0: NullsId = NullsId(4);
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
        };
        let _ = inst.init(BTreeSet::new());
        let _ = inst.init1(NullState::new0(Nullability::ALWAYS));
        let _ = inst.init1(NullState::new0(Nullability::CENTER));
        let _ = inst.init1(NullState::new0(Nullability::BEGIN));
        let _ = inst.init1(NullState::new0(Nullability::END));
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
        if set1 == NullsId::END0 && set2 == NullsId::ALWAYS0 {
            return NullsId::ALWAYS0;
        }

        let all = self.get_set_ref(set1) | self.get_set_ref(set2);
        let mut result: BTreeSet<&NullState> = BTreeSet::new();
        for m in all.iter().rev() {
            let found = result.iter().find(|v| v.mask == m.mask && v.rel == m.rel);
            if found.is_none() {
                result.insert(m);
            }
        }

        let result = result
            .into_iter().cloned()
            .collect::<BTreeSet<_>>();

        let new_id = self.get_id(result);
        self.created.insert(key, new_id);
        new_id
    }

    #[inline]
    pub fn and_id(&mut self, set1: NullsId, set2: NullsId) -> NullsId {
        if NullsId::EMPTY == set1 {
            return NullsId::EMPTY;
        }
        if NullsId::EMPTY == set2 {
            return NullsId::EMPTY;
        }
        if set1 > set2 {
            return self.and_id(set2, set1);
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
        if set1 == NullsId::ALWAYS0 && set2 == NullsId::END0 {
            return NullsId::END0;
        }
        if set1 == NullsId::END0 && set2 == NullsId::ALWAYS0 {
            return NullsId::END0;
        }

        let result = self.get_id(self.get_set_ref(set1) | self.get_set_ref(set2));
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

    #[inline]
    pub fn add_rel(&mut self, set_id: NullsId, rel: u32) -> NullsId {
        if rel == 0 || rel == u32::MAX {
            return set_id;
        }
        let res = self.get_set_ref(set_id).clone();
        let with_rel = res
            .iter()
            .map(|v| NullState::new(v.mask, v.rel + rel))
            .collect();

        self.get_id(with_rel)
    }
}
