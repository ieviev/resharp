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

/// A `NullsId`'s value: a sorted list of maximal, non-overlapping, same-mask `[lo, hi]` runs.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct NullRun {
    mask: Nullability,
    lo: u32,
    hi: u32,
}
impl NullRun {
    fn new(mask: Nullability, lo: u32, hi: u32) -> NullRun {
        debug_assert!(lo <= hi);
        debug_assert!(mask != Nullability::NEVER);
        NullRun { mask, lo, hi }
    }
}

type Nulls = Vec<NullRun>;

/// Append-only arena of `RunList` nodes, addressed by index instead of `Rc`.
#[derive(Default, Clone)]
struct RunArena {
    nodes: Vec<(NullRun, Option<u32>)>,
}

impl RunArena {
    fn push(&mut self, run: NullRun, tail: Option<u32>) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push((run, tail));
        idx
    }

    fn run(&self, idx: u32) -> NullRun {
        self.nodes[idx as usize].0
    }

    fn tail(&self, idx: u32) -> Option<u32> {
        self.nodes[idx as usize].1
    }
}

/// Persistent list of `NullRun`s, O(1)-extendable only at the current extreme (new min or new
/// max) -- everything else falls back to plain `Vec<NullRun>`.
#[derive(Clone, Copy)]
struct RunList {
    head: Option<u32>,
    ascending: bool,
    min: u32,
    max: u32,
}

impl RunList {
    fn empty(ascending: bool) -> RunList {
        RunList { head: None, ascending, min: 0, max: 0 }
    }

    fn head(&self, arena: &RunArena) -> Option<NullRun> {
        self.head.map(|idx| arena.run(idx))
    }

    /// `run` must be strictly beyond the current extreme, else `None`.
    fn try_insert_extreme(&self, arena: &mut RunArena, run: NullRun) -> Option<RunList> {
        match self.head(arena) {
            None => {
                let idx = arena.push(run, self.head);
                Some(RunList { head: Some(idx), ascending: self.ascending, min: run.lo, max: run.hi })
            }
            Some(h) => {
                if self.ascending {
                    if run.mask == h.mask && run.hi + 1 == h.lo {
                        Some(self.replace_head(arena, NullRun::new(run.mask, run.lo, h.hi), run.lo, self.max))
                    } else if run.hi < h.lo {
                        let idx = arena.push(run, self.head);
                        Some(RunList { head: Some(idx), ascending: true, min: run.lo, max: self.max })
                    } else {
                        None
                    }
                } else if run.mask == h.mask && run.lo == h.hi + 1 {
                    Some(self.replace_head(arena, NullRun::new(run.mask, h.lo, run.hi), self.min, run.hi))
                } else if run.lo > h.hi {
                    let idx = arena.push(run, self.head);
                    Some(RunList { head: Some(idx), ascending: false, min: self.min, max: run.hi })
                } else {
                    None
                }
            }
        }
    }

    fn replace_head(&self, arena: &mut RunArena, merged: NullRun, min: u32, max: u32) -> RunList {
        let tail = self.head.and_then(|idx| arena.tail(idx));
        let idx = arena.push(merged, tail);
        RunList { head: Some(idx), ascending: self.ascending, min, max }
    }

    fn to_vec(&self, arena: &RunArena) -> Nulls {
        let mut out: Nulls = Vec::new();
        let mut node = self.head;
        while let Some(idx) = node {
            out.push(arena.run(idx));
            node = arena.tail(idx);
        }
        if !self.ascending {
            out.reverse();
        }
        out
    }

    fn single_run(&self, arena: &RunArena) -> Option<NullRun> {
        let idx = self.head?;
        if arena.tail(idx).is_some() {
            return None;
        }
        Some(arena.run(idx))
    }
}

#[cfg(test)]
mod runlist_tests {
    use super::*;

    fn r(mask: Nullability, lo: u32, hi: u32) -> NullRun {
        NullRun::new(mask, lo, hi)
    }

    #[test]
    fn empty_list_flattens_to_empty() {
        let a = RunArena::default();
        assert_eq!(RunList::empty(true).to_vec(&a), Vec::<NullRun>::new());
        assert_eq!(RunList::empty(false).to_vec(&a), Vec::<NullRun>::new());
    }

    #[test]
    fn ascending_prepend_builds_correct_order() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 5)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 3, 3)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 0, 1)).unwrap();
        assert_eq!(
            l.to_vec(&a),
            vec![r(Nullability::ALWAYS, 0, 1), r(Nullability::ALWAYS, 3, 3), r(Nullability::ALWAYS, 5, 5)]
        );
    }

    #[test]
    fn descending_prepend_builds_correct_order() {
        let mut a = RunArena::default();
        let l = RunList::empty(false);
        let l = l.try_insert_extreme(&mut a, r(Nullability::END, 0, 0)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::END, 2, 2)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::END, 10, 12)).unwrap();
        assert_eq!(
            l.to_vec(&a),
            vec![r(Nullability::END, 0, 0), r(Nullability::END, 2, 2), r(Nullability::END, 10, 12)]
        );
    }

    #[test]
    fn touching_same_mask_coalesces_at_head_only() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 5)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 4, 4)).unwrap();
        assert_eq!(l.to_vec(&a), vec![r(Nullability::ALWAYS, 4, 5)]);
    }

    #[test]
    fn touching_different_mask_does_not_coalesce() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 5)).unwrap();
        let l = l.try_insert_extreme(&mut a, r(Nullability::END, 4, 4)).unwrap();
        assert_eq!(l.to_vec(&a), vec![r(Nullability::END, 4, 4), r(Nullability::ALWAYS, 5, 5)]);
    }

    #[test]
    fn overlapping_extreme_is_rejected() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 7)).unwrap();
        assert!(l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 6, 6)).is_none());
        assert!(l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 3, 4)).is_some());
    }

    #[test]
    fn min_max_track_correctly_in_both_directions() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 10, 10)).unwrap();
        assert_eq!((l.min, l.max), (10, 10));
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 5)).unwrap();
        assert_eq!((l.min, l.max), (5, 10));
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 4, 4)).unwrap();
        assert_eq!((l.min, l.max), (4, 10));

        let d = RunList::empty(false);
        let d = d.try_insert_extreme(&mut a, r(Nullability::END, 0, 0)).unwrap();
        assert_eq!((d.min, d.max), (0, 0));
        let d = d.try_insert_extreme(&mut a, r(Nullability::END, 5, 5)).unwrap();
        assert_eq!((d.min, d.max), (0, 5));
        let d = d.try_insert_extreme(&mut a, r(Nullability::END, 6, 6)).unwrap();
        assert_eq!((d.min, d.max), (0, 6));
    }

    #[test]
    fn wrong_direction_is_rejected() {
        let mut a = RunArena::default();
        let l = RunList::empty(true);
        let l = l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 5, 5)).unwrap();
        assert!(l.try_insert_extreme(&mut a, r(Nullability::ALWAYS, 10, 10)).is_none());
    }

    #[test]
    fn matches_or_runs_for_random_extreme_sequences() {
        let mut state: u64 = 0x9E3779B97F4A7C15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let masks = [Nullability::ALWAYS, Nullability::END, Nullability::BEGIN, Nullability::CENTER];
        for _ in 0..200 {
            let mut arena = RunArena::default();
            let ascending = next() % 2 == 0;
            let mut list = RunList::empty(ascending);
            let mut eager: Nulls = Vec::new();
            let mut boundary: i64 = if ascending { 1_000_000 } else { -1 };
            for _ in 0..30 {
                let mask = masks[(next() % masks.len() as u64) as usize];
                let width = (next() % 4) as i64;
                let gap = 1 + (next() % 5) as i64;
                let (lo, hi) = if ascending {
                    let hi = boundary - gap;
                    let lo = hi - width;
                    boundary = lo;
                    (lo, hi)
                } else {
                    let lo = boundary + gap;
                    let hi = lo + width;
                    boundary = hi;
                    (lo, hi)
                };
                if lo < 0 {
                    break;
                }
                let run = r(mask, lo as u32, hi as u32);
                list = list.try_insert_extreme(&mut arena, run).expect("strictly-extreme by construction");
                eager = or_runs(&eager, &vec![run]);
            }
            assert_eq!(list.to_vec(&arena), eager, "ascending={ascending}");
        }
    }
}

#[inline]
fn push_coalesced(runs: &mut Nulls, mask: Nullability, lo: u32, hi: u32) {
    if let Some(last) = runs.last_mut() {
        debug_assert!(last.hi < lo);
        if last.mask == mask && last.hi + 1 == lo {
            last.hi = hi;
            return;
        }
    }
    runs.push(NullRun::new(mask, lo, hi));
}

/// Merge arbitrary (possibly duplicate-`rel`, unsorted) `NullState`s into the canonical run
/// form, OR-ing masks at the same `rel` (safe: every reader only tests mask bits present).
fn normalize_from_states(raw: &std::collections::BTreeSet<NullState>) -> Nulls {
    if raw.is_empty() {
        return Vec::new();
    }
    let mut merged: Vec<(u32, Nullability)> = raw.iter().map(|ns| (ns.rel, ns.mask)).collect();
    merged.sort_unstable_by_key(|&(rel, _)| rel);
    let mut by_rel: Vec<(u32, Nullability)> = Vec::with_capacity(merged.len());
    for (rel, mask) in merged {
        if let Some(last) = by_rel.last_mut() {
            if last.0 == rel {
                last.1 = last.1.or(mask);
                continue;
            }
        }
        by_rel.push((rel, mask));
    }
    let mut runs: Nulls = Vec::new();
    for (rel, mask) in by_rel {
        push_coalesced(&mut runs, mask, rel, rel);
    }
    runs
}

/// Flatten to `Vec<NullState>` in descending `rel` order (matches the old `BTreeSet` order).
fn expand_to_states(runs: &Nulls) -> Vec<NullState> {
    let mut out = Vec::new();
    for r in runs.iter().rev() {
        for rel in (r.lo..=r.hi).rev() {
            out.push(NullState::new(r.mask, rel));
        }
    }
    out
}

fn or_runs(a: &Nulls, b: &Nulls) -> Nulls {
    if a.is_empty() {
        return b.clone();
    }
    if b.is_empty() {
        return a.clone();
    }
    let mut bounds: Vec<u32> = Vec::with_capacity((a.len() + b.len()) * 2);
    for r in a.iter().chain(b.iter()) {
        bounds.push(r.lo);
        bounds.push(r.hi + 1);
    }
    bounds.sort_unstable();
    bounds.dedup();
    let mut runs: Nulls = Vec::new();
    for w in bounds.windows(2) {
        let (lo, hi_excl) = (w[0], w[1]);
        if lo >= hi_excl {
            continue;
        }
        let hi = hi_excl - 1;
        let mut mask = Nullability::NEVER;
        for r in a.iter().chain(b.iter()) {
            if r.lo <= lo && hi <= r.hi {
                mask = mask.or(r.mask);
            }
        }
        if mask != Nullability::NEVER {
            push_coalesced(&mut runs, mask, lo, hi);
        }
    }
    runs
}

fn and_runs(a: &Nulls, b: &Nulls) -> Nulls {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut bounds: Vec<u32> = Vec::with_capacity((a.len() + b.len()) * 2);
    for r in a.iter().chain(b.iter()) {
        bounds.push(r.lo);
        bounds.push(r.hi + 1);
    }
    bounds.sort_unstable();
    bounds.dedup();
    let mut runs: Nulls = Vec::new();
    for w in bounds.windows(2) {
        let (lo, hi_excl) = (w[0], w[1]);
        if lo >= hi_excl {
            continue;
        }
        let hi = hi_excl - 1;
        let ma = a.iter().find(|r| r.lo <= lo && hi <= r.hi).map(|r| r.mask);
        let mb = b.iter().find(|r| r.lo <= lo && hi <= r.hi).map(|r| r.mask);
        if let (Some(ma), Some(mb)) = (ma, mb) {
            let mask = ma.and(mb);
            if mask != Nullability::NEVER {
                push_coalesced(&mut runs, mask, lo, hi);
            }
        }
    }
    runs
}

fn and_mask_runs(a: &Nulls, mask: Nullability) -> Nulls {
    let mut runs: Nulls = Vec::new();
    for r in a {
        let m = r.mask.and(mask);
        if m != Nullability::NEVER {
            push_coalesced(&mut runs, m, r.lo, r.hi);
        }
    }
    runs
}

fn add_rel_runs(a: &Nulls, rel: u32) -> Nulls {
    a.iter().map(|r| NullRun::new(r.mask, r.lo + rel, r.hi + rel)).collect()
}

/// Merges sorted, same-mask, possibly-overlapping runs into non-overlapping ones, one linear pass.
fn merge_overlapping_same_mask(sorted: Vec<NullRun>) -> Nulls {
    let mut out: Nulls = Vec::with_capacity(sorted.len());
    for r in sorted {
        if let Some(last) = out.last_mut() {
            if r.lo <= last.hi.saturating_add(1) {
                if r.hi > last.hi {
                    last.hi = r.hi;
                }
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// `union over s in shifts of add_rel(body, s)`, O(runs(body) * runs(shifts)).
fn union_shifted_runs(body: &Nulls, shifts: &Nulls) -> Nulls {
    let mut result: Nulls = Vec::new();
    for br in body {
        let shifted: Vec<NullRun> = shifts
            .iter()
            .map(|sr| NullRun::new(br.mask, br.lo + sr.lo, br.hi + sr.hi))
            .collect();
        let merged = merge_overlapping_same_mask(shifted);
        result = or_runs(&result, &merged);
    }
    result
}

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

#[repr(u8)]
#[derive(Hash, PartialEq, Eq, Clone)]
enum Operation {
    Or,
    Inter,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key {
    op: Operation,
    left: NullsId,
    right: NullsId,
}

#[derive(Clone)]
pub struct NullsBuilder {
    cache: FxHashMap<Nulls, NullsId>,
    created: FxHashMap<Key, NullsId>,
    add_rel_cache: FxHashMap<(NullsId, u32), NullsId>,
    // lazily-shifted id -> (root id, cumulative offset); root's own entry is always None.
    shift_of: Vec<Option<(NullsId, u32)>>,
    shift_cache: FxHashMap<(NullsId, u32), NullsId>,
    // lazily-list-backed id -> its `RunList`, not yet flattened into `array`.
    run_list_of: Vec<Option<RunList>>,
    run_arena: RunArena,
    array: Vec<Nulls>,
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
            shift_of: Vec::new(),
            shift_cache: FxHashMap::default(),
            run_list_of: Vec::new(),
            run_arena: RunArena::default(),
        };
        let _empty = inst.init(Vec::new());
        let _center = inst.init1(Nullability::CENTER);
        let _always = inst.init1(Nullability::ALWAYS);
        let _begin = inst.init1(Nullability::BEGIN);
        let _end = inst.init1(Nullability::END);
        debug_assert!(_empty == NullsId::EMPTY);
        debug_assert!(_center == NullsId::CENTER0);
        debug_assert!(_always == NullsId::ALWAYS0);
        debug_assert!(_begin == NullsId::BEGIN0);
        debug_assert!(_end == NullsId::END0);
        inst
    }

    fn init(&mut self, inst: Nulls) -> NullsId {
        let new_id = NullsId(self.array.len() as u32);
        self.cache.insert(inst.clone(), new_id);
        self.array.push(inst);
        self.shift_of.push(None);
        self.run_list_of.push(None);
        new_id
    }

    fn alloc_run_list_id(&mut self, list: RunList) -> NullsId {
        let new_id = NullsId(self.array.len() as u32);
        self.array.push(Nulls::new());
        self.shift_of.push(None);
        self.run_list_of.push(Some(list));
        new_id
    }

    fn init1(&mut self, mask: Nullability) -> NullsId {
        self.init(vec![NullRun::new(mask, 0, 0)])
    }

    fn materialized_pair(&mut self, a: NullsId, b: NullsId) -> (Nulls, Nulls) {
        self.get_set_ref(a);
        self.get_set_ref(b);
        (self.array[a.0 as usize].clone(), self.array[b.0 as usize].clone())
    }

    fn get_set_ref(&mut self, set_id: NullsId) -> &Nulls {
        if let Some(list) = self.run_list_of[set_id.0 as usize] {
            self.array[set_id.0 as usize] = list.to_vec(&self.run_arena);
            self.run_list_of[set_id.0 as usize] = None;
        } else if let Some((root, offset)) = self.shift_of[set_id.0 as usize] {
            debug_assert!(self.shift_of[root.0 as usize].is_none());
            self.get_set_ref(root);
            let materialized = add_rel_runs(&self.array[root.0 as usize], offset);
            self.array[set_id.0 as usize] = materialized;
            self.shift_of[set_id.0 as usize] = None;
        }
        &self.array[set_id.0 as usize]
    }

    fn single_run_view(&self, id: NullsId) -> Option<NullRun> {
        if let Some(list) = &self.run_list_of[id.0 as usize] {
            return list.single_run(&self.run_arena);
        }
        let (root, offset) = match self.shift_of[id.0 as usize] {
            Some((r, o)) => (r, o),
            None => (id, 0u32),
        };
        if let Some(list) = &self.run_list_of[root.0 as usize] {
            return list
                .single_run(&self.run_arena)
                .map(|r| NullRun::new(r.mask, r.lo + offset, r.hi + offset));
        }
        match self.array[root.0 as usize].as_slice() {
            [only] => Some(NullRun::new(only.mask, only.lo + offset, only.hi + offset)),
            _ => None,
        }
    }

    fn is_always0(&self, id: NullsId) -> bool {
        self.single_run_view(id) == Some(NullRun::new(Nullability::ALWAYS, 0, 0))
    }

    pub fn nulls_entry_states(&mut self, id: u32) -> Vec<NullState> {
        expand_to_states(self.get_set_ref(NullsId(id)))
    }

    pub fn nulls_count(&self) -> usize {
        self.array.len()
    }

    pub fn nulls_as_vecs(&mut self) -> Vec<Vec<NullState>> {
        for id in 0..self.array.len() as u32 {
            self.get_set_ref(NullsId(id));
        }
        self.array.iter().map(expand_to_states).collect()
    }

    pub fn contains_rel_with_mask(&mut self, set_id: NullsId, rel: u32, mask: Nullability) -> bool {
        self.get_set_ref(set_id)
            .iter()
            .any(|r| r.lo <= rel && rel <= r.hi && r.mask.has(mask))
    }

    pub fn contains_rel_unconditionally(&mut self, set_id: NullsId, rel: u32) -> bool {
        self.get_set_ref(set_id)
            .iter()
            .any(|r| r.lo <= rel && rel <= r.hi && r.mask == Nullability::ALWAYS)
    }

    pub fn max_rel(&self, set_id: NullsId) -> u32 {
        if let Some(list) = &self.run_list_of[set_id.0 as usize] {
            return list.max;
        }
        match self.shift_of[set_id.0 as usize] {
            Some((root, offset)) => match &self.run_list_of[root.0 as usize] {
                Some(list) => list.max + offset,
                None => self.array[root.0 as usize].last().map_or(0, |r| r.hi) + offset,
            },
            None => self.array[set_id.0 as usize].last().map_or(0, |r| r.hi),
        }
    }

    pub fn min_rel(&self, set_id: NullsId) -> u32 {
        if let Some(list) = &self.run_list_of[set_id.0 as usize] {
            return list.min;
        }
        match self.shift_of[set_id.0 as usize] {
            Some((root, offset)) => match &self.run_list_of[root.0 as usize] {
                Some(list) => list.min + offset,
                None => self.array[root.0 as usize].first().map_or(0, |r| r.lo) + offset,
            },
            None => self.array[set_id.0 as usize].first().map_or(0, |r| r.lo),
        }
    }

    pub fn get_id(&mut self, inst: std::collections::BTreeSet<NullState>) -> NullsId {
        let runs = normalize_from_states(&inst);
        match self.cache.get(&runs) {
            Some(&id) => id,
            None => self.init(runs),
        }
    }

    fn get_runs_id(&mut self, runs: Nulls) -> NullsId {
        match self.cache.get(&runs) {
            Some(&id) => id,
            None => self.init(runs),
        }
    }

    /// Intern a single contiguous `[lo, hi]` run directly, O(1).
    pub fn single_run(&mut self, mask: Nullability, lo: u32, hi: u32) -> NullsId {
        self.get_runs_id(vec![NullRun::new(mask, lo, hi)])
    }

    #[inline]
    fn is_created(&self, inst: &Key) -> Option<&NullsId> {
        self.created.get(inst)
    }

    // Builds (once) a `RunList` view of `big` extendable in the given direction; `None` if `big`
    // is already list-backed the other way.
    fn run_list_for_extend(&mut self, big: NullsId, ascending: bool) -> Option<RunList> {
        if let Some(list) = self.run_list_of[big.0 as usize] {
            return if list.ascending == ascending { Some(list) } else { None };
        }
        let runs = self.get_set_ref(big).clone();
        let mut list = RunList::empty(ascending);
        if ascending {
            for r in runs.iter().rev() {
                list = list.try_insert_extreme(&mut self.run_arena, *r)?;
            }
        } else {
            for r in runs.iter() {
                list = list.try_insert_extreme(&mut self.run_arena, *r)?;
            }
        }
        Some(list)
    }

    // `None` unless `small` is a single run that is a clean new extreme of `big`.
    fn try_or_id_extreme(&mut self, small: NullsId, big: NullsId) -> Option<NullsId> {
        let run = self.single_run_view(small)?;
        let ascending = if run.hi < self.min_rel(big) {
            true
        } else if run.lo > self.max_rel(big) {
            false
        } else {
            return None;
        };
        let list = self.run_list_for_extend(big, ascending)?;
        let new_list = list.try_insert_extreme(&mut self.run_arena, run)?;
        Some(self.alloc_run_list_id(new_list))
    }

    #[inline]
    pub fn or_id(&mut self, set1: NullsId, set2: NullsId) -> NullsId {
        if set1 == NullsId::EMPTY {
            return set2;
        }
        if set2 == NullsId::EMPTY {
            return set1;
        }
        if set1 > set2 {
            return self.or_id(set2, set1);
        }
        if set1 == set2 {
            return set1;
        }
        let key = Key { op: Operation::Or, left: set1, right: set2 };
        if let Some(v) = self.is_created(&key) {
            return *v;
        }
        if set1 == NullsId::ALWAYS0 && set2 == NullsId::END0 {
            return NullsId::ALWAYS0;
        }
        if let Some(new_id) = self
            .try_or_id_extreme(set1, set2)
            .or_else(|| self.try_or_id_extreme(set2, set1))
        {
            self.created.insert(key, new_id);
            return new_id;
        }
        let (a, b) = self.materialized_pair(set1, set2);
        let result = or_runs(&a, &b);
        let new_id = self.get_runs_id(result);
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
        if set1 == set2 {
            return set1;
        }
        let key = Key { op: Operation::Inter, left: set1, right: set2 };
        if let Some(v) = self.is_created(&key) {
            return *v;
        }
        let (a, b) = self.materialized_pair(set1, set2);
        let result = and_runs(&a, &b);
        let new_id = self.get_runs_id(result);
        self.created.insert(key, new_id);
        new_id
    }

    #[inline]
    pub fn and_mask(&mut self, set1: NullsId, mask: Nullability) -> NullsId {
        if NullsId::EMPTY == set1 || mask == Nullability::NEVER {
            return NullsId::EMPTY;
        }
        if mask == Nullability::ALWAYS {
            return set1;
        }
        let result = and_mask_runs(self.get_set_ref(set1), mask);
        self.get_runs_id(result)
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

    // Represents the shift lazily as (root, cumulative offset) instead of rebuilding the run list.
    #[inline]
    pub fn add_rel(&mut self, set_id: NullsId, rel: u32) -> NullsId {
        if rel == 0 || rel == u32::MAX {
            return set_id;
        }
        if let Some(&cached) = self.add_rel_cache.get(&(set_id, rel)) {
            return cached;
        }
        let (root, base_offset) = match self.shift_of[set_id.0 as usize] {
            Some((r, o)) => (r, o),
            None => (set_id, 0u32),
        };
        let new_offset = base_offset + rel;
        let result = if let Some(&cached) = self.shift_cache.get(&(root, new_offset)) {
            cached
        } else {
            let new_id = NullsId(self.array.len() as u32);
            self.array.push(Nulls::new());
            self.shift_of.push(Some((root, new_offset)));
            self.run_list_of.push(None);
            self.shift_cache.insert((root, new_offset), new_id);
            new_id
        };
        self.add_rel_cache.insert((set_id, rel), result);
        result
    }

    /// `union over rel in shifts of add_rel(body, rel)`.
    pub fn union_shifted(&mut self, body: NullsId, shifts: NullsId) -> NullsId {
        if body == NullsId::EMPTY || shifts == NullsId::EMPTY {
            return NullsId::EMPTY;
        }
        // body == ALWAYS0 (a degenerate point): shifting it by every `shifts` offset reproduces
        // `shifts` byte-for-byte.
        if self.is_always0(body) {
            return shifts;
        }
        let (b, s) = self.materialized_pair(body, shifts);
        let result = union_shifted_runs(&b, &s);
        self.get_runs_id(result)
    }
}
