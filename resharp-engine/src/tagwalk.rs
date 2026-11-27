use resharp_algebra::{NodeId, RegexBuilder};
use resharp_parser::{SkNode, Skeleton};
use rustc_hash::FxHashMap;

use resharp_algebra::nulls::Nullability;

use crate::ldfa::{center_or_end, context_at, Sid, DFA_DEAD, DFA_INITIAL, LDFA};
use crate::pparse::UNSET;
use crate::Error;

#[derive(Clone, Copy)]
enum Op {
    Leaf(usize),
    Mark(usize),
    Split(usize, usize),
    Match,
}

struct Compiler {
    ops: Vec<Op>,
    leaf_of: FxHashMap<NodeId, usize>,
    nodes: Vec<NodeId>,
}

impl Compiler {
    fn leaf(&mut self, node: NodeId) {
        let nodes = &mut self.nodes;
        let l = *self.leaf_of.entry(node).or_insert_with(|| {
            nodes.push(node);
            nodes.len() - 1
        });
        self.ops.push(Op::Leaf(l));
    }

    fn emit(&mut self, b: &mut RegexBuilder, sk: &Skeleton, id: u32) -> bool {
        let (kind, node) = &sk.nodes[id as usize];
        let node = *node;
        if !node.contains_tags(b) {
            if b.contains_look(node) || b.contains_anchors(node) {
                return false;
            }
            self.leaf(node);
            return true;
        }
        match kind {
            SkNode::Group(index, child) => {
                self.ops.push(Op::Mark(2 * *index as usize));
                if !self.emit(b, sk, *child) {
                    return false;
                }
                self.ops.push(Op::Mark(2 * *index as usize + 1));
                true
            }
            SkNode::Concat(children) => children.iter().all(|&c| self.emit(b, sk, c)),
            SkNode::Optional(child) => {
                let split = self.ops.len();
                self.ops.push(Op::Split(split + 1, 0));
                if !self.emit(b, sk, *child) {
                    return false;
                }
                self.ops[split] = Op::Split(split + 1, self.ops.len());
                true
            }
            _ => false,
        }
    }
}

fn is_edge_look(b: &RegexBuilder, node: NodeId, first: bool) -> bool {
    if node.contains_tags(b) {
        return false;
    }
    if first {
        node.is_pure_lookbehind(b)
    } else {
        node.is_pure_lookahead(b)
    }
}

const DEAD: u32 = u32::MAX;
const ENTER: Sid = Sid::MAX;
const MISSING: u32 = u32::MAX;
const SLOW: u32 = u32::MAX - 1;

struct Src {
    from: u32,
    marks: Box<[u16]>,
}

struct Trans {
    next: u32,
    ident: bool,
    solo: bool,
    simple: u8,
    simple_marks: [u16; 4],
    conflict: bool,
    dups: Box<[(u32, u32)]>,
    regs: Box<[Src]>,
    acc: Box<[Src]>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Seed {
    pc: u32,
    st: Sid,
    reg: u32,
    nullable: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct State {
    ctx: u8,
    seeds: Vec<Seed>,
}

#[derive(Default)]
struct Build {
    thr: Vec<(u32, Sid, u32, Vec<u16>)>,
    regs: Vec<(u32, Vec<u16>)>,
    seeds: Vec<Seed>,
    acc: Vec<Src>,
    dups: Vec<(u32, u32)>,
    conflict: bool,
}

impl Build {
    fn fresh(&mut self, pc: u32, st: Sid, from: u32, marks: &[u16]) -> bool {
        for e in self.thr.iter() {
            if e.0 == pc && e.1 == st {
                if e.2 == from && e.3 == marks {
                    return false;
                }
                if e.3.is_empty() && marks.is_empty() {
                    self.dups.push((e.2, from));
                } else {
                    self.conflict = true;
                }
                return false;
            }
        }
        self.thr.push((pc, st, from, marks.to_vec()));
        true
    }

    fn reg(&mut self, from: u32, marks: &[u16]) -> u32 {
        let found = self.regs.iter().position(|r| r.0 == from && r.1 == marks);
        match found {
            Some(r) => r as u32,
            None => {
                self.regs.push((from, marks.to_vec()));
                (self.regs.len() - 1) as u32
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Step {
    rep: usize,
    ctx_next: Nullability,
}

pub(crate) struct TagWalk {
    ops: Vec<Op>,
    leaves: Vec<LDFA>,
    class_of: [u16; 256],
    reps: Vec<u8>,
    states: Vec<State>,
    state_ids: FxHashMap<State, u32>,
    inits: [u32; 8],
    trans: Vec<Option<Trans>>,
    fast: Vec<u32>,
    fin: Vec<Option<Trans>>,
    cur: Vec<usize>,
    nxt: Vec<usize>,
    tmp: Vec<usize>,
    pub(crate) out: Vec<usize>,
}

impl TagWalk {
    pub(crate) fn compile(
        b: &mut RegexBuilder,
        sk: &Skeleton,
        max_cap: usize,
    ) -> Result<Option<TagWalk>, Error> {
        let mut items: &[u32] = &[sk.root];
        if let SkNode::Concat(children) = &sk.nodes[sk.root as usize].0 {
            items = children;
            if let Some(&f) = items.first() {
                if is_edge_look(b, sk.nodes[f as usize].1, true) {
                    items = &items[1..];
                }
            }
            if let Some(&l) = items.last() {
                if is_edge_look(b, sk.nodes[l as usize].1, false) {
                    items = &items[..items.len() - 1];
                }
            }
        }
        let mut c = Compiler { ops: Vec::new(), leaf_of: FxHashMap::default(), nodes: Vec::new() };
        if !items.iter().all(|&id| c.emit(b, sk, id)) {
            return Ok(None);
        }
        c.ops.push(Op::Match);
        let leaves = c
            .nodes
            .iter()
            .map(|&n| LDFA::new_fwd(b, n, max_cap))
            .collect::<Result<Vec<_>, _>>()?;
        let mut class_ids: FxHashMap<Vec<u8>, u8> = FxHashMap::default();
        let mut class_of = [0u16; 256];
        let mut reps = Vec::new();
        for byte in 0..256usize {
            let sig: Vec<u8> = leaves.iter().map(|l| l.mt_lookup[byte]).collect();
            let next = reps.len();
            let id = *class_ids.entry(sig).or_insert_with(|| {
                reps.push(byte as u8);
                next as u8
            });
            class_of[byte] = id as u16 * 2;
        }
        Ok(Some(TagWalk {
            ops: c.ops,
            leaves,
            class_of,
            reps,
            states: Vec::new(),
            state_ids: FxHashMap::default(),
            inits: [DEAD; 8],
            trans: Vec::new(),
            fast: Vec::new(),
            fin: Vec::new(),
            cur: Vec::new(),
            nxt: Vec::new(),
            tmp: Vec::new(),
            out: Vec::new(),
        }))
    }

    fn intern(&mut self, state: State) -> u32 {
        if state.seeds.is_empty() {
            return DEAD;
        }
        if let Some(&id) = self.state_ids.get(&state) {
            return id;
        }
        let id = self.states.len() as u32;
        self.states.push(state.clone());
        self.state_ids.insert(state, id);
        let grown = self.trans.len() + self.reps.len() * 2;
        self.trans.resize_with(grown, || None);
        self.fast.resize(grown, MISSING);
        self.fin.push(None);
        id
    }

    fn init_state(&mut self, ctx: Nullability) -> u32 {
        let slot = ctx.0 as usize;
        if self.inits[slot] == DEAD {
            let seed = Seed { pc: 0, st: ENTER, reg: 0, nullable: false };
            self.inits[slot] = self.intern(State { ctx: ctx.0, seeds: vec![seed] });
        }
        self.inits[slot]
    }

    fn visit(
        &mut self,
        b: &mut RegexBuilder,
        pc: usize,
        st: Sid,
        from: u32,
        marks: &[u16],
        step: Option<Step>,
        bd: &mut Build,
    ) -> Result<bool, Error> {
        if !bd.fresh(pc as u32, st, from, marks) {
            return Ok(false);
        }
        if let Some(sp) = step {
            let Op::Leaf(l) = self.ops[pc] else { return Ok(true) };
            let ldfa = &mut self.leaves[l];
            let ns = ldfa.lazy_transition(b, st, ldfa.mt_lookup[sp.rep] as u32)?;
            if ns > DFA_DEAD {
                let nullable = ldfa.nullable_now(ns, sp.ctx_next);
                let reg = bd.reg(from, marks);
                bd.seeds.push(Seed { pc: pc as u32, st: ns, reg, nullable });
            }
        }
        Ok(true)
    }

    fn follow(
        &mut self,
        b: &mut RegexBuilder,
        pc: usize,
        from: u32,
        marks: &mut Vec<u16>,
        ctx: Nullability,
        step: Option<Step>,
        bd: &mut Build,
    ) -> Result<(), Error> {
        match self.ops[pc] {
            Op::Mark(slot) => {
                marks.push(slot as u16);
                self.follow(b, pc + 1, from, marks, ctx, step, bd)?;
                marks.pop();
            }
            Op::Split(a, c) => {
                self.follow(b, a, from, marks, ctx, step, bd)?;
                self.follow(b, c, from, marks, ctx, step, bd)?;
            }
            Op::Match => bd.acc.push(Src { from, marks: marks.as_slice().into() }),
            Op::Leaf(l) => {
                let st = self.leaves[l].pruned;
                if self.visit(b, pc, st, from, marks, step, bd)?
                    && self.leaves[l].nullable_now(DFA_INITIAL, ctx)
                {
                    self.follow(b, pc + 1, from, marks, ctx, step, bd)?;
                }
            }
        }
        Ok(())
    }

    fn close(
        &mut self,
        b: &mut RegexBuilder,
        state: u32,
        step: Option<Step>,
    ) -> Result<Trans, Error> {
        let st = self.states[state as usize].clone();
        let ctx = Nullability(st.ctx);
        let mut bd = Build::default();
        for seed in st.seeds.iter() {
            let pc = seed.pc as usize;
            if seed.st == ENTER {
                self.follow(b, pc, seed.reg, &mut Vec::new(), ctx, step, &mut bd)?;
            } else if self.visit(b, pc, seed.st, seed.reg, &[], step, &mut bd)? && seed.nullable {
                self.follow(b, pc + 1, seed.reg, &mut Vec::new(), ctx, step, &mut bd)?;
            }
        }
        let src_regs = st.seeds.iter().map(|s| s.reg as usize + 1).max().unwrap_or(0);
        let ident = !bd.conflict
            && bd.dups.is_empty()
            && bd.regs.len() == src_regs
            && bd.regs.iter().enumerate().all(|(i, r)| r.0 == i as u32 && r.1.is_empty());
        let next = match step {
            Some(sp) => {
                let ctx_next = sp.ctx_next.0;
                self.intern(State { ctx: ctx_next, seeds: bd.seeds })
            }
            None => DEAD,
        };
        let solo = bd.regs.len() == 1 && bd.regs[0].0 == 0;
        let mut simple = u8::MAX;
        let mut simple_marks = [0u16; 4];
        if solo && !bd.conflict && bd.dups.is_empty() && next != DEAD && bd.regs[0].1.len() <= 4 {
            simple = bd.regs[0].1.len() as u8;
            simple_marks[..simple as usize].copy_from_slice(&bd.regs[0].1);
        }
        Ok(Trans {
            next,
            ident,
            solo,
            simple,
            simple_marks,
            conflict: bd.conflict,
            dups: bd.dups.into(),
            regs: bd.regs.into_iter().map(|r| Src { from: r.0, marks: r.1.into() }).collect(),
            acc: bd.acc.into(),
        })
    }

    pub(crate) fn run(
        &mut self,
        b: &mut RegexBuilder,
        input: &[u8],
        begin: usize,
        end: usize,
        num_tags: u32,
    ) -> Result<bool, Error> {
        let nt = num_tags as usize;
        self.cur.clear();
        self.cur.resize(nt, UNSET);
        let len = input.len();
        let stride = self.reps.len() * 2;
        let mut at = self.init_state(context_at(begin, len)) as usize * stride;
        let mut pos = begin;
        while pos < end {
            let lim = if end == len { len - 1 } else { end };
            (pos, at) = scan_fast(&self.fast, &self.class_of, input, pos, lim, at);
            if pos == end {
                break;
            }
            let c = self.class_of[input[pos] as usize] as usize;
            let idx = at + c + (pos + 1 == len) as usize;
            let mut f = self.fast[idx];
            if f == MISSING {
                let ctx_next = center_or_end(pos + 1 == len);
                let step = Step { rep: self.reps[c / 2] as usize, ctx_next };
                let t = self.close(b, (at / stride) as u32, Some(step))?;
                f = if t.ident && t.next != DEAD { t.next * stride as u32 } else { SLOW };
                self.fast[idx] = f;
                self.trans[idx] = Some(t);
            }
            pos += 1;
            if f < SLOW {
                at = f as usize;
                continue;
            }
            let t = self.trans[idx].as_ref().unwrap();
            if t.simple != u8::MAX {
                self.cur.truncate(nt);
                for &m in &t.simple_marks[..t.simple as usize] {
                    self.cur[m as usize] = pos - 1;
                }
                at = t.next as usize * stride;
                continue;
            }
            if t.next == DEAD || !valid(t, &self.cur, nt) {
                return Ok(false);
            }
            if t.solo {
                self.cur.truncate(nt);
                for &m in t.regs[0].marks.iter() {
                    self.cur[m as usize] = pos - 1;
                }
            } else if !t.ident {
                apply(t, &self.cur, &mut self.nxt, nt, pos - 1);
                std::mem::swap(&mut self.cur, &mut self.nxt);
            }
            at = t.next as usize * stride;
        }
        let state = (at / stride) as u32;
        if self.fin[state as usize].is_none() {
            self.fin[state as usize] = Some(self.close(b, state, None)?);
        }
        let t = self.fin[state as usize].as_ref().unwrap();
        if !valid(t, &self.cur, nt) {
            return Ok(false);
        }
        Ok(resolve(t, &self.cur, &mut self.out, &mut self.tmp, nt, end))
    }
}

fn scan_fast(
    fast: &[u32],
    class_of: &[u16; 256],
    input: &[u8],
    mut pos: usize,
    lim: usize,
    mut at: usize,
) -> (usize, usize) {
    while pos < lim {
        let f = fast[at + class_of[input[pos] as usize] as usize];
        if f >= SLOW {
            break;
        }
        at = f as usize;
        pos += 1;
    }
    (pos, at)
}

fn reg(cur: &[usize], nt: usize, i: u32) -> &[usize] {
    &cur[i as usize * nt..(i as usize + 1) * nt]
}

fn valid(t: &Trans, cur: &[usize], nt: usize) -> bool {
    !t.conflict && t.dups.iter().all(|&(a, c)| reg(cur, nt, a) == reg(cur, nt, c))
}

fn fill(dst: &mut Vec<usize>, cur: &[usize], nt: usize, s: &Src, at: usize) {
    dst.clear();
    dst.extend_from_slice(reg(cur, nt, s.from));
    for &m in s.marks.iter() {
        dst[m as usize] = at;
    }
}

fn resolve(
    t: &Trans,
    cur: &[usize],
    out: &mut Vec<usize>,
    tmp: &mut Vec<usize>,
    nt: usize,
    end: usize,
) -> bool {
    let Some((first, rest)) = t.acc.split_first() else { return false };
    fill(out, cur, nt, first, end);
    rest.iter().all(|s| {
        fill(tmp, cur, nt, s, end);
        tmp == out
    })
}

fn apply(t: &Trans, cur: &[usize], nxt: &mut Vec<usize>, nt: usize, pos: usize) {
    nxt.clear();
    for s in t.regs.iter() {
        let start = nxt.len();
        nxt.extend_from_slice(reg(cur, nt, s.from));
        for &m in s.marks.iter() {
            nxt[start + m as usize] = pos;
        }
    }
}
