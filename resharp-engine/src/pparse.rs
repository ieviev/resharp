use resharp_algebra::nulls::Nullability;
use resharp_algebra::{Kind, NodeId, RegexBuilder};
use resharp_parser::{SkNode, Skeleton};
use rustc_hash::FxHashMap;
use std::sync::Arc;

use crate::ldfa::LDFA;
use crate::Error;

const UNSET: usize = usize::MAX;

type TagVec = Arc<Vec<usize>>;
type Sol = Option<(usize, TagVec)>;

pub(crate) struct PosixParser {
    max_cap: usize,
    setup: Option<(usize, u32)>,
    ldfas: FxHashMap<NodeId, LDFA>,
    rev_ldfas: FxHashMap<NodeId, LDFA>,
    ends: FxHashMap<(NodeId, usize, usize), Arc<Vec<usize>>>,
    rev_starts: FxHashMap<(NodeId, usize), Arc<Vec<usize>>>,
    memo: FxHashMap<(u32, usize, NodeId, usize, bool), Sol>,

    scratch_nulls: Vec<usize>,
    span_hi: usize,
    input_scope: Option<(usize, usize)>,
    parent_group: Option<Vec<Option<usize>>>,
}

impl PosixParser {
    pub(crate) fn new(max_cap: usize) -> Self {
        PosixParser {
            max_cap,
            setup: None,
            ldfas: FxHashMap::default(),
            rev_ldfas: FxHashMap::default(),
            ends: FxHashMap::default(),
            rev_starts: FxHashMap::default(),
            memo: FxHashMap::default(),

            scratch_nulls: Vec::new(),
            span_hi: 0,
            input_scope: None,
            parent_group: None,
        }
    }

    fn parent_group(&mut self, sk: &Skeleton, num_tags: u32) -> &[Option<usize>] {
        if self.parent_group.is_none() {
            self.parent_group = Some(build_parent_group(sk, num_tags));
        }
        self.parent_group.as_ref().unwrap()
    }

    pub(crate) fn begin_input(&mut self, input: &[u8]) {
        self.ends.clear();
        self.rev_starts.clear();
        self.memo.clear();
        self.input_scope = Some((input.as_ptr() as usize, input.len()));
    }

    fn ensure_setup(&mut self, b: &mut RegexBuilder, root: NodeId) -> Result<(usize, u32), Error> {
        if let Some(s) = self.setup {
            return Ok(s);
        }
        let max_real_tag = crate::captures::max_capture_tag(b, root)?;
        let num_groups = (max_real_tag / 2) as usize;
        let num_tags = if max_real_tag == 0 { 2 } else { max_real_tag + 1 };
        let s = (num_groups, num_tags);
        self.setup = Some(s);
        Ok(s)
    }

    fn ends(&mut self, b: &mut RegexBuilder, node: NodeId, i: usize, input: &[u8]) -> Result<Arc<Vec<usize>>, Error> {
        self.ends_upto(b, node, i, input, input.len())
    }

    fn ends_upto(
        &mut self,
        b: &mut RegexBuilder,
        node: NodeId,
        i: usize,
        input: &[u8],
        hi: usize,
    ) -> Result<Arc<Vec<usize>>, Error> {
        let key = (node, i, hi);
        if let Some(v) = self.ends.get(&key) {
            return Ok(v.clone());
        }
        let mut peeled = node;
        while b.get_kind(peeled) == Kind::Concat && b.get_kind(peeled.left(b)) == Kind::Tag {
            peeled = peeled.right(b);
        }
        if peeled != node {
            let v = self.ends_upto(b, peeled, i, input, hi)?;
            self.ends.insert(key, v.clone());
            return Ok(v);
        }
        if b.get_kind(node) == Kind::Lookahead {
            let body = node.left(b);
            let tail = b.get_lookahead_tail(node);
            let v = if !self.matches_somewhere_from(b, body, i, input)? {
                Arc::new(Vec::new())
            } else if tail == NodeId::MISSING {
                Arc::new(vec![i])
            } else {
                self.ends_upto(b, tail, i, input, input.len())?
            };
            self.ends.insert(key, v.clone());
            return Ok(v);
        }
        if b.contains_lookbehind(node) {
            let stripped = b.strip_lb(node)?;
            let cand = self.ends(b, stripped, i, input)?;
            let rev_node = drop_trailing_zero_width_lookahead(b, node);
            let mut kept = Vec::with_capacity(cand.len());
            for &k in cand.iter() {
                if self.rev_starts(b, rev_node, k, input)?.binary_search(&i).is_ok() {
                    kept.push(k);
                }
            }
            let v = Arc::new(kept);
            self.ends.insert(key, v.clone());
            return Ok(v);
        }
        if i >= input.len() {
            let ctx = if input.is_empty() { Nullability::EMPTYSTRING } else { Nullability::END };
            let v = Arc::new(if null_at_eof(b, node, ctx) { vec![i] } else { Vec::new() });
            self.ends.insert(key, v.clone());
            return Ok(v);
        }
        let mut ldfa = match self.ldfas.remove(&node) {
            Some(l) => l,
            None => LDFA::new_fwd(b, node, self.max_cap)?,
        };
        self.scratch_nulls.clear();
        let r = ldfa.scan_fwd_all_nulls_to(b, i, hi.max(i), input, &mut self.scratch_nulls);
        self.ldfas.insert(node, ldfa);
        r?;
        self.scratch_nulls.sort_unstable();
        self.scratch_nulls.dedup();
        let maxlen = b.get_max_length_only(node);
        if let Some(cap) = i.checked_add(maxlen as usize) {
            self.scratch_nulls.retain(|&k| k <= cap);
        }
        let v = Arc::new(self.scratch_nulls.clone());
        self.ends.insert(key, v.clone());
        Ok(v)
    }

    fn matches_somewhere_from(
        &mut self,
        b: &mut RegexBuilder,
        node: NodeId,
        i: usize,
        input: &[u8],
    ) -> Result<bool, Error> {
        let near = self.span_hi.max(i).min(input.len());
        if !self.ends_upto(b, node, i, input, near)?.is_empty() {
            return Ok(true);
        }
        let any_suffix = b.mk_concat(node, NodeId::TS);
        match self.rev_starts(b, any_suffix, input.len(), input) {
            Ok(v) => Ok(v.binary_search(&i).is_ok()),
            Err(Error::Algebra(_)) => {
                Ok(!self.ends_upto(b, node, i, input, input.len())?.is_empty())
            }
            Err(e) => Err(e),
        }
    }

    fn rev_starts(&mut self, b: &mut RegexBuilder, node: NodeId, k: usize, input: &[u8]) -> Result<Arc<Vec<usize>>, Error> {
        let key = (node, k);
        if let Some(v) = self.rev_starts.get(&key) {
            return Ok(v.clone());
        }
        let mut ldfa = match self.rev_ldfas.remove(&node) {
            Some(l) => l,
            None => {
                let rev = b.reverse(node)?;
                LDFA::new_rev(b, rev, self.max_cap)?
            }
        };
        self.scratch_nulls.clear();
        let r = ldfa.scan_rev_all_nulls_from(b, k, input, &mut self.scratch_nulls);
        self.rev_ldfas.insert(node, ldfa);
        r?;
        self.scratch_nulls.sort_unstable();
        self.scratch_nulls.dedup();
        let v = Arc::new(self.scratch_nulls.clone());
        self.rev_starts.insert(key, v.clone());
        Ok(v)
    }

    fn feasible_cont(
        &mut self,
        b: &mut RegexBuilder,
        cont: NodeId,
        j: usize,
        j_final: usize,
        input: &[u8],
    ) -> Result<bool, Error> {
        if j > j_final {
            return Ok(false);
        }
        if cont == NodeId::EPS {
            return Ok(j == j_final);
        }
        Ok(self.ends(b, cont, j, input)?.binary_search(&j_final).is_ok())
    }

    fn rep_node(&mut self, b: &mut RegexBuilder, body: NodeId, lo: u32, hi: u32) -> NodeId {
        if hi == u32::MAX {
            let mut node = b.mk_star(body);
            for _ in 0..lo {
                node = b.mk_concat(body, node);
            }
            node
        } else {
            b.mk_repeat(body, lo, hi)
        }
    }

    fn solve_arm_at_shorter_end(
        &mut self,
        b: &mut RegexBuilder,
        sk: &Skeleton,
        id: u32,
        siblings: &[u32],
        self_idx: usize,
        i: usize,
        cont: NodeId,
        j_final: usize,
        input: &[u8],
        num_tags: u32,
    ) -> Result<Sol, Error> {
        let node = sk.nodes[id as usize].1;
        let cand = self.ends(b, node, i, input)?;
        let Some(&k) = cand.last() else {
            return Ok(None);
        };
        if k > j_final {
            return Ok(None);
        }
        for (sidx, &sib) in siblings.iter().enumerate() {
            if sidx == self_idx {
                continue;
            }
            let sib_node = sk.nodes[sib as usize].1;
            let sib_ends = self.ends(b, sib_node, i, input)?;
            let dominates = sib_ends.len() > cand.len()
                && cand.iter().all(|e| sib_ends.binary_search(e).is_ok());
            if dominates {
                return Ok(None);
            }
        }
        if !self.matches_somewhere_from(b, cont, k, input)? {
            return Ok(None);
        }
        self.solve(b, sk, id, i, NodeId::EPS, k, true, input, num_tags)
    }

    #[allow(clippy::too_many_arguments)]
    fn solve(
        &mut self,
        b: &mut RegexBuilder,
        sk: &Skeleton,
        id: u32,
        i: usize,
        cont: NodeId,
        j_final: usize,
        tail: bool,
        input: &[u8],
        num_tags: u32,
    ) -> Result<Sol, Error> {
        let key = (id, i, cont, j_final, tail);
        if let Some(v) = self.memo.get(&key) {
            return Ok(v.clone());
        }
        let node = sk.nodes[id as usize].1;
        let unset = || Arc::new(vec![UNSET; num_tags as usize]);
        let result = match &sk.nodes[id as usize].0 {
            SkNode::Leaf => {
                let cand = self.ends(b, node, i, input)?;
                let mut best: Sol = None;
                for &k in cand.iter().rev() {
                    if self.feasible_cont(b, cont, k, j_final, input)? {
                        best = Some((k, unset()));
                        break;
                    }
                }
                best
            }
            SkNode::Group(index, child) => {
                let (index, child) = (*index, *child);
                match self.solve(b, sk, child, i, cont, j_final, tail, input, num_tags)? {
                    Some((j, v)) => {
                        let mut v = v.as_ref().clone();
                        let (open, close) = (2 * index as usize, 2 * index as usize + 1);
                        if close >= v.len() {
                            return Err(Error::InternalError("tag out of range"));
                        }
                        v[open] = i;
                        v[close] = j;
                        Some((j, Arc::new(v)))
                    }
                    None => None,
                }
            }
            SkNode::Optional(child) => {
                let child = *child;
                let enter = self.solve(b, sk, child, i, cont, j_final, tail, input, num_tags)?;
                let decline = if self.feasible_cont(b, cont, i, j_final, input)? {
                    Some((i, unset()))
                } else {
                    None
                };
                pick_arm(enter, decline)
            }
            SkNode::Union(children) => {
                let children = children.clone();
                let mut acc: Sol = None;
                for (idx, c) in children.iter().enumerate() {
                    let mut cv = self.solve(b, sk, *c, i, cont, j_final, tail, input, num_tags)?;
                    if cv.is_none() && tail {
                        cv = self.solve_arm_at_shorter_end(
                            b, sk, *c, &children, idx, i, cont, j_final, input, num_tags,
                        )?;
                    }
                    acc = merge_union_arms(acc, cv);
                }
                acc
            }
            SkNode::Inter(children) => {
                let children = children.clone();
                let cand = self.ends(b, node, i, input)?;
                let mut chosen: Option<usize> = None;
                for &k in cand.iter().rev() {
                    if self.feasible_cont(b, cont, k, j_final, input)? {
                        chosen = Some(k);
                        break;
                    }
                }
                match chosen {
                    None => None,
                    Some(k) => {
                        let mut acc = unset();
                        let mut ok = true;
                        for c in children {
                            match self.solve(b, sk, c, i, NodeId::EPS, k, false, input, num_tags)? {
                                Some((_, v)) => acc = merge(&acc, &v),
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        ok.then_some((k, acc))
                    }
                }
            }
            SkNode::Repeat(child, lo, hi) => {
                let (child, lo, hi) = (*child, *lo, *hi);
                let body = sk.nodes[child as usize].1;
                let cand = self.ends(b, node, i, input)?;
                let mut end = None;
                for &k in cand.iter().rev() {
                    if self.feasible_cont(b, cont, k, j_final, input)? {
                        end = Some(k);
                        break;
                    }
                }
                match end {
                    None => None,
                    Some(end) if !body.contains_tags(b) => Some((end, unset())),
                    Some(end) => {
                        let mut pos = i;
                        let mut acc = unset();
                        let mut copies = 0u32;
                        while copies < hi && (pos < end || copies < lo) {
                            let lo_rem = lo.saturating_sub(copies + 1);
                            let hi_rem =
                                if hi == u32::MAX { u32::MAX } else { hi - copies - 1 };
                            let rest = self.rep_node(b, body, lo_rem, hi_rem);
                            match self.solve(b, sk, child, pos, rest, end, false, input, num_tags)? {
                                Some((k, v)) => {
                                    let parent_group = self.parent_group(sk, num_tags);
                                    acc = merge_repeat_iter(&acc, &v, parent_group);
                                    copies += 1;
                                    if k == pos {
                                        break;
                                    }
                                    pos = k;
                                }
                                None => break,
                            }
                        }
                        Some((end, acc))
                    }
                }
            }
            SkNode::Concat(children) => {
                let children = children.clone();
                let mut conts = vec![cont; children.len()];
                for d in (0..children.len().saturating_sub(1)).rev() {
                    let next = sk.nodes[children[d + 1] as usize].1;
                    conts[d] = b.mk_concat(next, conts[d + 1]);
                }
                let mut pos = i;
                let mut acc = unset();
                let mut ok = true;
                for (d, c) in children.iter().enumerate() {
                    match self.solve(b, sk, *c, pos, conts[d], j_final, tail, input, num_tags)? {
                        Some((k, v)) => {
                            acc = merge(&acc, &v);
                            pos = k;
                        }
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if children.is_empty() {
                    ok = self.feasible_cont(b, cont, i, j_final, input)?;
                }
                ok.then_some((pos, acc))
            }
        };
        self.memo.insert(key, result.clone());
        Ok(result)
    }
}

// reversing a trailing zero-width lookahead would turn it into a leading lookbehind peeking past the walk's start, which a backward scan can't do; drop it instead
fn drop_trailing_zero_width_lookahead(b: &mut RegexBuilder, node: NodeId) -> NodeId {
    if node.is_lookahead(b) {
        return if b.get_lookahead_tail(node) == NodeId::MISSING {
            NodeId::EPS
        } else {
            node
        };
    }
    if !node.is_concat(b) {
        return node;
    }
    let right = node.right(b);
    let new_right = drop_trailing_zero_width_lookahead(b, right);
    if new_right == right {
        return node;
    }
    b.mk_concat(node.left(b), new_right)
}

fn null_at_eof(b: &RegexBuilder, node: NodeId, ctx: Nullability) -> bool {
    if node == NodeId::EPS {
        return true;
    }
    match b.get_kind(node) {
        Kind::Begin => ctx.has(Nullability::BEGIN),
        Kind::End => ctx.has(Nullability::END),
        Kind::Tag | Kind::Star => true,
        Kind::Pred => false,
        Kind::Concat | Kind::Inter => {
            null_at_eof(b, node.left(b), ctx) && null_at_eof(b, node.right(b), ctx)
        }
        Kind::Union => {
            null_at_eof(b, node.left(b), ctx) || null_at_eof(b, node.right(b), ctx)
        }
        Kind::Compl => !null_at_eof(b, node.left(b), ctx),
        Kind::Lookbehind => {
            let body = null_at_eof(b, node.left(b), ctx);
            let prev = node.right(b);
            if prev == NodeId::MISSING {
                body
            } else {
                body && null_at_eof(b, prev, ctx)
            }
        }
        Kind::Lookahead => {
            let body = null_at_eof(b, node.left(b), ctx);
            let tail = node.lookahead_tail(b);
            if tail == NodeId::MISSING {
                body
            } else {
                body && null_at_eof(b, tail, ctx)
            }
        }
        Kind::Ordered => null_at_eof(b, node.left(b), ctx),
    }
}

fn build_parent_group(sk: &Skeleton, num_tags: u32) -> Vec<Option<usize>> {
    let mut parent = vec![None; num_tags as usize / 2 + 1];
    fn walk(sk: &Skeleton, id: u32, current: Option<usize>, parent: &mut Vec<Option<usize>>) {
        match &sk.nodes[id as usize].0 {
            SkNode::Leaf => {}
            SkNode::Concat(cs) | SkNode::Inter(cs) => {
                for &c in cs {
                    walk(sk, c, current, parent);
                }
            }
            SkNode::Union(cs) => {
                for &c in cs {
                    walk(sk, c, current, parent);
                }
            }
            SkNode::Group(index, child) => {
                let index = *index as usize;
                parent[index] = current;
                walk(sk, *child, Some(index), parent);
            }
            SkNode::Optional(child) => walk(sk, *child, current, parent),
            SkNode::Repeat(child, _, _) => walk(sk, *child, current, parent),
        }
    }
    walk(sk, sk.root, None, &mut parent);
    parent
}

fn merge_repeat_iter(acc: &[usize], v: &[usize], parent_group: &[Option<usize>]) -> TagVec {
    let mut out = acc.to_vec();
    for g in 1..parent_group.len() {
        let (open, close) = (2 * g, 2 * g + 1);
        if close >= v.len() {
            continue;
        }
        let forced = match parent_group[g] {
            Some(p) => v[2 * p] != UNSET,
            None => false,
        };
        if v[open] != UNSET || forced {
            out[open] = v[open];
        }
        if v[close] != UNSET || forced {
            out[close] = v[close];
        }
    }
    Arc::new(out)
}

fn merge(l: &[usize], r: &[usize]) -> TagVec {
    let mut v = l.to_vec();
    for (dst, &src) in v.iter_mut().zip(r.iter()) {
        if src != UNSET {
            *dst = src;
        }
    }
    Arc::new(v)
}

fn group_span(v: &[usize], g: usize) -> Option<(usize, usize)> {
    let open = v[2 * g];
    let close = v[2 * g + 1];
    if open != UNSET && close != UNSET {
        Some((open, close))
    } else {
        None
    }
}

fn participation_dominates(a: &[usize], bv: &[usize]) -> bool {
    let num_groups = (a.len() - 2) / 2;
    let mut strict = false;
    for g in 1..=num_groups {
        let pa = a[2 * g] != UNSET || a[2 * g + 1] != UNSET;
        let pb = bv[2 * g] != UNSET || bv[2 * g + 1] != UNSET;
        if pb && !pa {
            return false;
        }
        if pa && !pb {
            strict = true;
        }
    }
    strict
}


fn cmp_tags(a: &[usize], bv: &[usize]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    for t in 2..a.len() {
        let (va, vb) = (a[t], bv[t]);
        match (va == UNSET, vb == UNSET) {
            (true, true) => continue,
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            (false, false) => {
                let ord = if t % 2 == 0 { vb.cmp(&va) } else { va.cmp(&vb) };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
    Ordering::Equal
}

fn span_list(v: &[usize]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut g = 1usize;
    while 2 * g + 1 < v.len() {
        let (o, c) = (v[2 * g], v[2 * g + 1]);
        if o != UNSET && c != UNSET {
            out.push((o, c));
        }
        g += 1;
    }
    out.sort_by(|p, q| p.0.cmp(&q.0).then(q.1.cmp(&p.1)));
    out.dedup();
    out
}

fn cmp_spans(a: &[usize], bv: &[usize]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (sa, sb) = (span_list(a), span_list(bv));
    for (p, q) in sa.iter().zip(sb.iter()) {
        match q.0.cmp(&p.0).then_with(|| p.1.cmp(&q.1)) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }
    sa.len().cmp(&sb.len())
}

fn pick_arm(a: Sol, b: Sol) -> Sol {
    use std::cmp::Ordering;
    match (a, b) {
        (Some(a), Some(b)) => {
            if participation_dominates(&b.1, &a.1) {
                return Some(b);
            }
            if participation_dominates(&a.1, &b.1) {
                return Some(a);
            }
            match b
                .0
                .cmp(&a.0)
                .then_with(|| cmp_spans(&b.1, &a.1))
                .then_with(|| cmp_tags(&b.1, &a.1))
            {
                Ordering::Greater => Some(b),
                _ => Some(a),
            }
        }
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}

pub(crate) fn extract_captures(
    b: &mut RegexBuilder,
    pp: &mut PosixParser,
    root: NodeId,
    sk: Option<&Skeleton>,
    input: &[u8],
    begin: usize,
    end: usize,
) -> Result<Vec<Option<(usize, usize)>>, Error> {
    let (num_groups, num_tags) = pp.ensure_setup(b, root)?;
    let mut result = vec![None; num_groups];
    if num_groups == 0 {
        return Ok(result);
    }
    let Some(sk) = sk else {
        return Err(Error::InternalError("missing capture skeleton"));
    };
    if pp.input_scope != Some((input.as_ptr() as usize, input.len())) {
        return Err(Error::InternalError("capture extraction outside a declared input scope"));
    }
    pp.ends.clear();
    pp.memo.clear();
    pp.span_hi = end;
    let root_node = sk.nodes[sk.root as usize].1;
    if pp.ends(b, root_node, begin, input)?.binary_search(&end).is_err() {
        return Err(Error::InternalError("match end is not an accepting end"));
    }
    let Some((_, v)) = pp.solve(b, sk, sk.root, begin, NodeId::EPS, end, true, input, num_tags)? else {
        return Err(Error::InternalError("match span does not parse"));
    };
    for (g, slot) in result.iter_mut().enumerate() {
        *slot = group_span(&v, g + 1);
    }
    Ok(result)
}

fn merge_tags_into(base: &mut [usize], other: &[usize]) {
    for (t, slot) in base.iter_mut().enumerate() {
        if *slot == UNSET {
            *slot = other[t];
        }
    }
}

fn merge_union_arms(a: Sol, b: Sol) -> Sol {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (mut lead, follow) = if b.0 > a.0 { (b, a) } else { (a, b) };
            let mut tags = lead.1.as_ref().clone();
            merge_tags_into(&mut tags, &follow.1);
            lead.1 = Arc::new(tags);
            Some(lead)
        }
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
}
