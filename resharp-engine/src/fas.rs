use resharp_algebra::nulls::{
    EID_ALWAYS0, EID_BEGIN0, EID_CENTER0, EID_END0, EID_NONE, Nullability, StartPositions,
};
use resharp_algebra::RegexBuilder;
use rustc_hash::FxHashMap;

use crate::ldfa::{DFA_DEAD, DFA_INITIAL, LDFA};
use crate::{Error, Match};

pub const FAS_ACTION_MISSING: u32 = u32::MAX;
pub const FAS_DIED: u16 = 0xFFFF;
pub const FAS_LOW_BIT: u16 = 0x8000;
pub const FAS_SPAWN_NONE: u16 = 0xFFFF;
pub const FAS_SPAWN_DEAD: u16 = 0xFFFE;
pub const FAS_STATE_CAP: usize = 1024;

enum SpawnKind {
    Dead,
    LowPriority(usize),
    NewSlot(usize),
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct FwdAction {
    pub next_asid: u32,
    pub old_acts: Vec<u16>,
    pub new_end_rel: Vec<u32>,
    pub spawn: u16,
}

const FAS_NOT_NULLABLE: u32 = u32::MAX;
pub const NO_END: usize = usize::MAX;

// max where NO_END is -inf: an unset end never wins, a real end always does.
#[inline]
fn max_end(a: usize, b: usize) -> usize {
    if a == NO_END {
        b
    } else if b == NO_END || a >= b {
        a
    } else {
        b
    }
}

#[derive(Clone, Copy)]
pub struct SlotEntries {
    head: u32,
    tail: u32,
    max_e: usize,
}

pub const SLOT_NIL: u32 = u32::MAX;

impl Default for SlotEntries {
    fn default() -> Self {
        Self {
            head: SLOT_NIL,
            tail: SLOT_NIL,
            max_e: 0,
        }
    }
}

impl SlotEntries {
    #[inline]
    pub fn clear(&mut self) {
        self.head = SLOT_NIL;
        self.tail = SLOT_NIL;
        self.max_e = 0;
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head == SLOT_NIL
    }
    #[inline]
    fn push_spawn(&mut self, linker: &mut [u32], idx: u32, e: usize) {
        linker[idx as usize] = SLOT_NIL;
        if self.is_empty() {
            self.head = idx;
            self.tail = idx;
            self.max_e = e;
        } else {
            linker[self.tail as usize] = idx;
            self.tail = idx;
            self.max_e = max_end(self.max_e, e);
        }
    }
    #[inline]
    pub fn extend_e(&mut self, ce: usize) {
        if !self.is_empty() {
            self.max_e = max_end(self.max_e, ce);
        }
    }
    fn merge_from(&mut self, other: &mut Self, ce: Option<usize>, linker: &mut [u32]) {
        if let Some(ce) = ce {
            other.extend_e(ce);
        }
        if self.is_empty() {
            std::mem::swap(self, other);
            return;
        }
        if other.is_empty() {
            return;
        }
        linker[self.tail as usize] = other.head;
        self.tail = other.tail;
        self.max_e = max_end(self.max_e, other.max_e);
        other.clear();
    }
    pub fn drain_to_max(&mut self, linker: &[u32], max: &mut [usize]) {
        let e = self.max_e;
        let mut cur = self.head;
        while cur != SLOT_NIL {
            let i = cur as usize;
            if e != NO_END && e >= i {
                max[i] = max_end(max[i], e);
            }
            cur = linker[i];
        }
        self.clear();
    }
}
pub struct FwdDFA {
    pub states: Vec<Vec<u16>>,
    pub state_map: FxHashMap<Vec<u16>, u32>,
    pub trans: Vec<u32>,
    pub actions: Vec<FwdAction>,
    pub action_map: FxHashMap<FwdAction, u32>,
    pub stride: usize,
    pub initial_asid: u32,
    pub keep_spawn_on_merge: bool,
    max: Vec<usize>,
    linker: Vec<u32>,
    regs: Vec<SlotEntries>,
    new_regs: Vec<SlotEntries>,
}

impl FwdDFA {
    pub fn new(ldfa: &LDFA, keep_spawn_on_merge: bool) -> Self {
        let stride = 1usize << ldfa.mt_log;
        let mut fas = FwdDFA {
            states: Vec::new(),
            state_map: FxHashMap::default(),
            trans: Vec::new(),
            actions: Vec::new(),
            action_map: FxHashMap::default(),
            stride,
            initial_asid: 0,
            keep_spawn_on_merge,
            max: Vec::new(),
            linker: Vec::new(),
            regs: Vec::new(),
            new_regs: Vec::new(),
        };
        // state 0 = empty set
        let _ = fas.register(Vec::new());
        fas.initial_asid = fas.register(vec![DFA_INITIAL]);
        fas
    }

    fn register(&mut self, set: Vec<u16>) -> u32 {
        if let Some(&id) = self.state_map.get(&set) {
            return id;
        }
        let id = self.states.len() as u32;
        self.states.push(set.clone());
        self.state_map.insert(set, id);
        let new_len = (id as usize + 1) * self.stride;
        if self.trans.len() < new_len {
            self.trans.resize(new_len, FAS_ACTION_MISSING);
        }
        id
    }

    fn intern_action(&mut self, action: FwdAction) -> u32 {
        if let Some(&id) = self.action_map.get(&action) {
            return id;
        }
        let id = self.actions.len() as u32;
        self.actions.push(action.clone());
        self.action_map.insert(action, id);
        id
    }

    fn compute_action(
        &mut self,
        b: &mut RegexBuilder,
        ldfa: &mut LDFA,
        asid: u32,
        mt: u32,
    ) -> Result<u32, Error> {
        let source: Vec<u16> = self.states[asid as usize].clone();
        let mut next_targets: Vec<u16> = Vec::with_capacity(source.len() + 1);
        let mut old_acts: Vec<u16> = Vec::with_capacity(source.len());

        // For each source slot, transition on `mt` and record:
        //   - FAS_DIED if the target is dead;
        //   - bucket index (winner: first slot to map to this target) without LOW_BIT;
        //   - bucket index | LOW_BIT (loser: a later slot merging into an existing bucket).
        for &s in source.iter() {
            let target = ldfa.lazy_transition(b, s, mt)?;
            if target <= DFA_DEAD {
                old_acts.push(FAS_DIED);
            } else if let Some(pos) = next_targets.iter().position(|&t| t == target) {
                old_acts.push((pos as u16) | FAS_LOW_BIT);
            } else {
                old_acts.push(next_targets.len() as u16);
                next_targets.push(target);
            }
        }

        let spawn_target = ldfa.lazy_transition(b, DFA_INITIAL, mt)?;
        let spawn_kind = if spawn_target <= DFA_DEAD {
            SpawnKind::Dead
        } else if let Some(pos) = next_targets.iter().position(|&t| t == spawn_target) {
            SpawnKind::LowPriority(pos)
        } else {
            let p = next_targets.len();
            next_targets.push(spawn_target);
            SpawnKind::NewSlot(p) // appended at end = youngest age
        };

        let canonical = next_targets;

        if self.states.len() > FAS_STATE_CAP && !self.state_map.contains_key(&canonical) {
            return Err(Error::CapacityExceeded);
        }

        let mut new_end_rel: Vec<u32> = Vec::with_capacity(canonical.len());
        for &s in &canonical {
            let eid = ldfa.effects_id[s as usize] as u32;
            let rel = match eid {
                EID_NONE | EID_BEGIN0 | EID_END0 => FAS_NOT_NULLABLE,
                EID_CENTER0 | EID_ALWAYS0 => 0,
                _ => ldfa.effects[eid as usize]
                    .iter()
                    .rev()
                    .find(|n| n.mask.has(Nullability::CENTER))
                    .map(|n| n.rel)
                    .unwrap_or(FAS_NOT_NULLABLE),
            };
            new_end_rel.push(rel);
        }

        let spawn = match spawn_kind {
            SpawnKind::Dead => FAS_SPAWN_DEAD,
            SpawnKind::LowPriority(idx) => (idx as u16) | FAS_LOW_BIT,
            SpawnKind::NewSlot(idx) => idx as u16,
        };

        #[cfg(feature = "debug")]
        {
            let pp_set = |set: &[u16]| -> String {
                set.iter()
                    .map(|&s| format!("  s{} = {:.80}", s, b.pp(ldfa.state_nodes[s as usize])))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            eprintln!(
                "[fas-action] mt={} asid={} -> next_asid_pending\n source:\n{}\n target:\n{}",
                mt,
                asid,
                pp_set(&source),
                pp_set(&canonical)
            );
        }

        let next_asid = self.register(canonical);
        let action = FwdAction {
            next_asid,
            old_acts,
            new_end_rel,
            spawn,
        };
        Ok(self.intern_action(action))
    }
}

#[inline(always)]
fn fas_apply(
    act: &FwdAction,
    regs: &mut [SlotEntries],
    new_regs: &mut Vec<SlotEntries>,
    linker: &mut [u32],
    max: &mut [usize],
    pos: usize,
    data_end: usize,
    keep_spawn_on_merge: bool,
    spawn_allowed: bool,
) {
    for r in new_regs.iter_mut() {
        r.clear();
    }
    new_regs.resize_with(act.new_end_rel.len(), SlotEntries::default);
    let next_pos = pos + 1;
    for (slot, &code) in act.old_acts.iter().enumerate() {
        if code == FAS_DIED {
            regs[slot].drain_to_max(linker, max);
            continue;
        }
        let idx = (code & 0x7FFF) as usize;
        let rel = act.new_end_rel[idx];
        let candidate_end = if rel != FAS_NOT_NULLABLE {
            next_pos.checked_sub(rel as usize)
        } else {
            None
        };
        let candidate_end = match candidate_end {
            Some(e) if e == data_end => None,
            other => other,
        };
        if (code & FAS_LOW_BIT) != 0 {
            if let Some(ce) = candidate_end {
                regs[slot].extend_e(ce);
            }
            let loser_head = regs[slot].head;
            let loser_tail = regs[slot].tail;
            regs[slot].drain_to_max(linker, max);
            if loser_head != SLOT_NIL {
                if new_regs[idx].is_empty() {
                    new_regs[idx].head = loser_head;
                    new_regs[idx].tail = loser_tail;
                } else {
                    linker[new_regs[idx].tail as usize] = loser_head;
                    new_regs[idx].tail = loser_tail;
                }
            }
        } else {
            new_regs[idx].merge_from(&mut regs[slot], candidate_end, linker);
        }
    }
    if spawn_allowed {
        match act.spawn {
            FAS_SPAWN_NONE => {}
            FAS_SPAWN_DEAD => {}
            s_code if (s_code & FAS_LOW_BIT) != 0 => {
                if keep_spawn_on_merge {
                    let idx = (s_code & 0x7FFF) as usize;
                    let rel = act.new_end_rel[idx];
                    let candidate_end = if rel != FAS_NOT_NULLABLE {
                        next_pos.checked_sub(rel as usize).unwrap_or(0)
                    } else {
                        0
                    };
                    let candidate_end = if candidate_end == data_end { pos } else { candidate_end };
                    new_regs[idx].push_spawn(linker, pos as u32, candidate_end);
                }
            }
            idx_u16 => {
                let idx = idx_u16 as usize;
                let rel = act.new_end_rel[idx];
                let candidate_end = if rel != FAS_NOT_NULLABLE {
                    next_pos.checked_sub(rel as usize).unwrap_or(0)
                } else {
                    0
                };
                let candidate_end = if candidate_end == data_end { pos } else { candidate_end };
                new_regs[idx].push_spawn(linker, pos as u32, candidate_end);
            }
        }
    }
}

#[inline]
fn fas_step(
    ldfa: &mut LDFA,
    b: &mut RegexBuilder,
    fas: &mut FwdDFA,
    data: &[u8],
    pos: usize,
    asid: &mut u32,
    regs: &mut Vec<SlotEntries>,
    new_regs: &mut Vec<SlotEntries>,
    linker: &mut [u32],
    max: &mut [usize],
    spawn_allowed: bool,
) -> Result<(), Error> {
    let mt = ldfa.mt_lookup[data[pos] as usize] as u32;
    let trans_idx = ((*asid as usize) * fas.stride) | mt as usize;
    let cached = unsafe { *fas.trans.get_unchecked(trans_idx) };
    let action_id = if cached != FAS_ACTION_MISSING {
        cached
    } else {
        let id = fas.compute_action(b, ldfa, *asid, mt)?;
        fas.trans[trans_idx] = id;
        id
    };
    let act = &fas.actions[action_id as usize];
    fas_apply(
        act,
        regs,
        new_regs,
        linker,
        max,
        pos,
        data.len(),
        fas.keep_spawn_on_merge,
        spawn_allowed,
    );
    std::mem::swap(regs, new_regs);
    *asid = act.next_asid;
    Ok(())
}

#[cfg_attr(feature = "serialize", allow(missing_docs))]
impl LDFA {
    pub fn scan_fwd_active_set<const ALWAYS_NULLABLE: bool>(
        &mut self,
        b: &mut RegexBuilder,
        fas: &mut FwdDFA,
        data: &[u8],
        nulls: &StartPositions,
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let data_end = data.len();
        if data_end == 0 {
            return Ok(());
        }
        let nulls: Vec<usize> = nulls.positions_desc().collect();
        let nulls = nulls.as_slice();
        let mut ni: usize = nulls.len();
        let mut max = std::mem::take(&mut fas.max);
        let mut linker = std::mem::take(&mut fas.linker);
        let mut regs = std::mem::take(&mut fas.regs);
        let mut new_regs = std::mem::take(&mut fas.new_regs);
        // max[i] = best end position seen for any spawn at pos i.
        max.clear();
        if ALWAYS_NULLABLE {
            // every position is nullable in every context: seed the zero-width floor everywhere.
            max.extend(0..=data_end);
        } else {
            max.resize(data_end + 1, NO_END);
            let null_begin = self.initial_nullability.has(Nullability::BEGIN);
            let null_center = self.initial_nullability.has(Nullability::CENTER);
            let null_end = self.initial_nullability.has(Nullability::END);
            for &i in nulls.iter() {
                let nullable_here = if i == 0 {
                    null_begin
                } else if i == data_end {
                    null_end
                } else {
                    null_center
                };
                if nullable_here {
                    max[i] = i;
                }
            }
        }
        // linker[i] = next spawn-pos in the same slot's list, or SLOT_NIL.
        linker.clear();
        linker.resize(data_end + 1, SLOT_NIL);
        regs.clear();
        new_regs.clear();
        let mut asid: u32 = 0;
        let mut pos: usize;
        // one-off begin step
        {
            let spawn_allowed_0 = ALWAYS_NULLABLE || (ni > 0 && nulls[ni - 1] == 0);
            if spawn_allowed_0 {
                let mt0 = self.mt_lookup[data[0] as usize] as u32;
                let bs = self.begin_table[mt0 as usize];
                if bs > DFA_DEAD {
                    let bs = bs as u32;
                    let eid = self.effects_id[bs as usize] as u32;
                    let rel = match eid {
                        EID_NONE => FAS_NOT_NULLABLE,
                        EID_CENTER0 | EID_ALWAYS0 => 0,
                        _ => self.effects[eid as usize]
                            .iter()
                            .rev()
                            .find(|n| n.mask.has(Nullability::CENTER))
                            .map(|n| n.rel)
                            .unwrap_or(FAS_NOT_NULLABLE),
                    };
                    let begin_floor = if self.initial_nullability.has(Nullability::BEGIN) {
                        0
                    } else {
                        NO_END
                    };
                    let me = if rel != FAS_NOT_NULLABLE {
                        1usize.saturating_sub(rel as usize)
                    } else {
                        begin_floor
                    };
                    let me = if me == data_end { begin_floor } else { me };
                    let mut s0 = SlotEntries::default();
                    s0.push_spawn(&mut linker, 0u32, me);
                    regs.push(s0);
                    asid = fas.register(vec![bs as u16]);
                }
            }
            pos = 1;
        }

        let nulls_ptr = nulls.as_ptr();
        // there's still matches that haven't spawned
        'phase1: while pos < data_end {
            let spawn_allowed = if ALWAYS_NULLABLE {
                true
            } else {
                // SAFETY: ni starts at nulls.len() and only decrements; the
                // `ni > 0` guard inside ensures `ni - 1 < nulls.len()`.
                while ni > 0 && unsafe { *nulls_ptr.add(ni - 1) } < pos {
                    ni -= 1;
                }
                if ni == 0 {
                    break 'phase1; // fall through to phase 2
                }
                (unsafe { *nulls_ptr.add(ni - 1) }) == pos
            };
            if !ALWAYS_NULLABLE && !spawn_allowed && regs.iter().all(|r| r.is_empty()) {
                // SAFETY: ni > 0 (would have broken out above otherwise).
                pos = unsafe { *nulls_ptr.add(ni - 1) };
                asid = 0;
                for r in regs.iter_mut() {
                    r.clear();
                }
                continue;
            }
            fas_step(self, b, fas, data, pos, &mut asid, &mut regs, &mut new_regs, &mut linker, &mut max, spawn_allowed)?;
            pos += 1;
        }
        // process remaining bytes only as long as some slot is still alive
        if !ALWAYS_NULLABLE {
            while pos < data_end {
                if regs.iter().all(|r| r.is_empty()) {
                    break;
                }
                fas_step(self, b, fas, data, pos, &mut asid, &mut regs, &mut new_regs, &mut linker, &mut max, false)?;
                pos += 1;
            }
        }
        // end
        {
            let states = &fas.states[asid as usize];
            for (slot, &sid) in states.iter().enumerate() {
                if regs[slot].is_empty() {
                    continue;
                }
                let eid = self.effects_id[sid as usize] as u32;
                let cand_end = match eid {
                    EID_NONE | EID_CENTER0 | EID_BEGIN0 => continue,
                    EID_ALWAYS0 | EID_END0 => Some(data_end),
                    _ => self.effects[eid as usize]
                        .iter()
                        .rev()
                        .find(|n| n.mask.has(Nullability::END))
                        .map(|n| data_end.saturating_sub(n.rel as usize)),
                };
                if let Some(ce) = cand_end {
                    regs[slot].extend_e(ce);
                }
            }
        }

        for entries in regs.iter_mut() {
            entries.drain_to_max(&linker, &mut max);
        }
        let mut skip_until = 0usize;
        let mut emit = |i: usize, e: usize, skip_until: &mut usize| {
            matches.push(Match { start: i, end: e });
            *skip_until = if e > i { e } else { i + 1 };
        };
        if ALWAYS_NULLABLE {
            for i in 0..=data_end {
                if i < skip_until {
                    continue;
                }
                emit(i, max[i], &mut skip_until);
            }
        } else {
            for &i in nulls.iter().rev() {
                if i < skip_until {
                    continue;
                }
                if max[i] == NO_END {
                    continue;
                }
                emit(i, max[i], &mut skip_until);
            }
        }
        fas.max = max;
        fas.linker = linker;
        fas.regs = regs;
        fas.new_regs = new_regs;
        Ok(())
    }
}
