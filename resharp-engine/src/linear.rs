use resharp_algebra::nulls::{
    EID_ALWAYS0, EID_BEGIN0, EID_CENTER0, EID_END0, EID_NONE, Nullability, StartPositions,
};
use resharp_algebra::RegexBuilder;
use rustc_hash::FxHashMap;

use crate::ldfa::{context_at, DFA_DEAD, DFA_INITIAL, LDFA};
use crate::{Error, Match};

const NOT_NULLABLE: u32 = u32::MAX;
const REL_UNKNOWN: u32 = u32::MAX - 1;
const MISSING: u32 = u32::MAX;
const DIED: u16 = u16::MAX;
const NO_END: usize = usize::MAX;
const SET_CAP: usize = 1024;
const LIVE_CAP: usize = 1 << 16;

struct Action {
    from: u32,
    next: u32,
    map: Box<[u16]>,
    spawn: u16,
}

pub struct Linear {
    stride: usize,
    sets: Vec<Box<[u16]>>,
    set_map: FxHashMap<Box<[u16]>, u32>,
    actions: Vec<Action>,
    act_tab: Vec<u32>,
    begin_acts: FxHashMap<u32, u32>,
    center_rel: Vec<u32>,
    end_rel: Vec<u32>,
    live_bits: Vec<u64>,
    live_off: Vec<u32>,
    live_map: FxHashMap<Box<[u64]>, u32>,
    back: FxHashMap<(u32, u32), u32>,
    entries: Vec<(u32, u32)>,
    memo: Vec<(u32, u32)>,
    buf: Vec<u32>,
}

fn best_rel(ldfa: &LDFA, sid: u16, ctx: Nullability) -> u32 {
    let eid = ldfa.effects_id[sid as usize] as u32;
    let hit = match eid {
        EID_NONE => false,
        EID_ALWAYS0 => true,
        EID_CENTER0 => ctx.has(Nullability::CENTER),
        EID_BEGIN0 => ctx.has(Nullability::BEGIN),
        EID_END0 => ctx.has(Nullability::END),
        _ => {
            return ldfa.effects[eid as usize]
                .iter()
                .rev()
                .find(|n| n.mask.has(ctx))
                .map_or(NOT_NULLABLE, |n| n.rel)
        }
    };
    if hit { 0 } else { NOT_NULLABLE }
}

impl Linear {
    pub fn new(ldfa: &LDFA) -> Self {
        let mut lin = Linear {
            stride: 1usize << ldfa.mt_log,
            sets: Vec::new(),
            set_map: FxHashMap::default(),
            actions: Vec::new(),
            act_tab: Vec::new(),
            begin_acts: FxHashMap::default(),
            center_rel: Vec::new(),
            end_rel: Vec::new(),
            live_bits: Vec::new(),
            live_off: vec![0],
            live_map: FxHashMap::default(),
            back: FxHashMap::default(),
            entries: Vec::new(),
            memo: Vec::new(),
            buf: Vec::new(),
        };
        lin.live_map.insert(Box::new([]), 0);
        lin.live_off.push(0);
        lin.sets.push(Box::new([]));
        lin.set_map.insert(Box::new([]), 0);
        lin.act_tab.resize(2 * lin.stride, MISSING);
        lin
    }

    fn register(&mut self, ldfa: &LDFA, set: Box<[u16]>) -> Result<u32, Error> {
        if let Some(&id) = self.set_map.get(&set) {
            return Ok(id);
        }
        if self.sets.len() >= SET_CAP {
            return Err(Error::CapacityExceeded);
        }
        for &sid in set.iter() {
            let s = sid as usize;
            if self.center_rel.len() <= s {
                self.center_rel.resize(s + 1, REL_UNKNOWN);
                self.end_rel.resize(s + 1, REL_UNKNOWN);
            }
            if self.center_rel[s] == REL_UNKNOWN {
                self.center_rel[s] = best_rel(ldfa, sid, Nullability::CENTER);
                self.end_rel[s] = best_rel(ldfa, sid, Nullability::END);
            }
        }
        let id = self.sets.len() as u32;
        self.sets.push(set.clone());
        self.set_map.insert(set, id);
        self.act_tab.resize((id as usize + 1) * 2 * self.stride, MISSING);
        Ok(id)
    }

    #[cold]
    #[inline(never)]
    fn compute_action(
        &mut self,
        b: &mut RegexBuilder,
        ldfa: &mut LDFA,
        from: u32,
        mt: u32,
        spawn: Option<u16>,
    ) -> Result<u32, Error> {
        let src = self.sets[from as usize].clone();
        let mut targets: Vec<u16> = Vec::with_capacity(src.len() + 1);
        for &s in src.iter() {
            targets.push(ldfa.lazy_transition(b, s, mt)?);
        }
        let mut next: Vec<u16> = targets.iter().copied().filter(|&t| t > DFA_DEAD).collect();
        if let Some(s) = spawn.filter(|&s| s > DFA_DEAD) {
            next.push(s);
        }
        next.sort_unstable();
        next.dedup();
        let slot = |t: u16| -> u16 {
            if t > DFA_DEAD { next.binary_search(&t).unwrap() as u16 } else { DIED }
        };
        let map: Box<[u16]> = targets.iter().map(|&t| slot(t)).collect();
        let spawn = spawn.map_or(DIED, slot);
        let next = self.register(ldfa, next.into_boxed_slice())?;
        let id = self.actions.len() as u32;
        self.actions.push(Action { from, next, map, spawn });
        self.memo.push((MISSING, MISSING));
        Ok(id)
    }

    #[inline(always)]
    fn action(
        &mut self,
        b: &mut RegexBuilder,
        ldfa: &mut LDFA,
        from: u32,
        mt: u32,
        spawn: bool,
    ) -> Result<u32, Error> {
        let idx = (from as usize * self.stride + mt as usize) * 2 + spawn as usize;
        let cached = self.act_tab[idx];
        if cached != MISSING {
            return Ok(cached);
        }
        let s = if spawn { Some(ldfa.lazy_transition(b, DFA_INITIAL, mt)?) } else { None };
        let id = self.compute_action(b, ldfa, from, mt, s)?;
        self.act_tab[idx] = id;
        Ok(id)
    }

    fn begin_action(
        &mut self,
        b: &mut RegexBuilder,
        ldfa: &mut LDFA,
        mt: u32,
        spawn: bool,
    ) -> Result<u32, Error> {
        if !spawn {
            return self.action(b, ldfa, 0, mt, false);
        }
        if let Some(&id) = self.begin_acts.get(&mt) {
            return Ok(id);
        }
        let s = ldfa.begin_table[mt as usize];
        let id = self.compute_action(b, ldfa, 0, mt, Some(s))?;
        self.begin_acts.insert(mt, id);
        Ok(id)
    }

    fn intern_live(&mut self, mut bits: Vec<u64>) -> Result<u32, Error> {
        while bits.last() == Some(&0) {
            bits.pop();
        }
        let bits = bits.into_boxed_slice();
        if let Some(&id) = self.live_map.get(&bits) {
            return Ok(id);
        }
        let id = self.live_map.len();
        if id >= LIVE_CAP {
            return Err(Error::CapacityExceeded);
        }
        self.live_bits.extend_from_slice(&bits);
        self.live_off.push(self.live_bits.len() as u32);
        self.live_map.insert(bits, id as u32);
        Ok(id as u32)
    }

    #[inline(always)]
    fn has(&self, live: u32, slot: u16) -> bool {
        let lo = self.live_off[live as usize] as usize;
        let hi = self.live_off[live as usize + 1] as usize;
        let w = lo + (slot >> 6) as usize;
        w < hi && self.live_bits[w] & (1u64 << (slot & 63)) != 0
    }

    fn end_live(&mut self, asid: u32) -> Result<u32, Error> {
        let set = &self.sets[asid as usize];
        let mut bits = vec![0u64; (set.len() + 63) / 64];
        for (i, &sid) in set.iter().enumerate() {
            if self.end_rel[sid as usize] != NOT_NULLABLE {
                bits[i >> 6] |= 1u64 << (i & 63);
            }
        }
        self.intern_live(bits)
    }

    #[inline(always)]
    fn step_back(&mut self, act: u32, live_next: u32) -> Result<u32, Error> {
        let (k, v) = self.memo[act as usize];
        if k == live_next {
            return Ok(v);
        }
        let id = match self.back.get(&(act, live_next)) {
            Some(&id) => id,
            None => self.step_back_slow(act, live_next)?,
        };
        self.memo[act as usize] = (live_next, id);
        Ok(id)
    }

    #[cold]
    #[inline(never)]
    fn step_back_slow(&mut self, act: u32, live_next: u32) -> Result<u32, Error> {
        let a = &self.actions[act as usize];
        let set = &self.sets[a.from as usize];
        let mut bits = vec![0u64; (set.len() + 63) / 64];
        for (i, &sid) in set.iter().enumerate() {
            let j = a.map[i];
            if self.center_rel[sid as usize] != NOT_NULLABLE
                || (j != DIED && self.has(live_next, j))
            {
                bits[i >> 6] |= 1u64 << (i & 63);
            }
        }
        let live = self.intern_live(bits)?;
        let id = self.entries.len() as u32;
        self.entries.push((act, live));
        self.back.insert((act, live_next), id);
        Ok(id)
    }
}

struct Starts<'a> {
    runs: &'a [(usize, usize)],
    ri: usize,
    all: Option<usize>,
}

impl Starts<'_> {
    #[inline(always)]
    fn first_ge(&mut self, p: usize) -> Option<usize> {
        if let Some(len) = self.all {
            return (p <= len).then_some(p);
        }
        while self.ri > 0 {
            let (lo, hi) = self.runs[self.ri - 1];
            if hi <= p {
                self.ri -= 1;
                continue;
            }
            return Some(lo.max(p));
        }
        None
    }
}

#[cfg_attr(feature = "serialize", allow(missing_docs))]
impl LDFA {
    pub fn scan_fwd_linear<const ALWAYS_NULLABLE: bool>(
        &mut self,
        b: &mut RegexBuilder,
        lin: &mut Linear,
        data: &[u8],
        nulls: &StartPositions,
        matches: &mut Vec<Match>,
    ) -> Result<(), Error> {
        let len = data.len();
        if len == 0 || (!ALWAYS_NULLABLE && nulls.is_empty()) {
            return Ok(());
        }
        let all = ALWAYS_NULLABLE.then_some(len);
        let runs = nulls.runs();
        let mut fwd = Starts { runs, ri: runs.len(), all };
        let mut starts = Starts { runs, ri: runs.len(), all };
        let mut buf = std::mem::take(&mut lin.buf);
        let mut next = 0usize;
        let mut cur = fwd.first_ge(0);
        while let Some(lo) = cur {
            buf.clear();
            let mut asid = 0u32;
            let mut p = lo;
            while p < len {
                let spawn = fwd.first_ge(p) == Some(p);
                if asid == 0 && !spawn {
                    break;
                }
                let mt = self.mt_lookup[data[p] as usize] as u32;
                let a = if p == 0 {
                    lin.begin_action(b, self, mt, spawn)?
                } else {
                    lin.action(b, self, asid, mt, spawn)?
                };
                buf.push(a);
                asid = lin.actions[a as usize].next;
                p += 1;
            }
            let hi = p;
            let end_live = if hi == len { lin.end_live(asid)? } else { 0 };
            let mut l = end_live;
            for slot in buf.iter_mut().rev() {
                let e = lin.step_back(*slot, l)?;
                *slot = e;
                l = lin.entries[e as usize].1;
            }

            let mut x = lo.max(next);
            while let Some(start) = starts.first_ge(x) {
                if start > hi {
                    break;
                }
                let mut best = if self.initial_nullability.has(context_at(start, len)) {
                    start
                } else {
                    NO_END
                };
                if start < len {
                    let mut a = &lin.actions[lin.entries[buf[start - lo] as usize].0 as usize];
                    let mut j = a.spawn;
                    let mut p = start + 1;
                    while j != DIED {
                        let (e_act, e_live) =
                            if p == hi { (MISSING, end_live) } else { lin.entries[buf[p - lo] as usize] };
                        if !lin.has(e_live, j) {
                            break;
                        }
                        let sid = lin.sets[a.next as usize][j as usize] as usize;
                        let rel = if p == len { lin.end_rel[sid] } else { lin.center_rel[sid] };
                        if rel != NOT_NULLABLE {
                            let e = p - rel as usize;
                            if best == NO_END || e > best {
                                best = e;
                            }
                        }
                        if p == hi {
                            break;
                        }
                        a = &lin.actions[e_act as usize];
                        j = a.map[j as usize];
                        p += 1;
                    }
                }
                debug_assert!(!ALWAYS_NULLABLE || best != NO_END, "always-nullable pattern has no end at {start}");
                if best != NO_END {
                    matches.push(Match { start, end: best });
                    next = if best > start { best } else { start + 1 };
                }
                x = (start + 1).max(next);
            }
            cur = if hi == len { None } else { fwd.first_ge(hi) };
        }
        lin.buf = buf;
        Ok(())
    }
}
