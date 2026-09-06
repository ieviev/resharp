use resharp_algebra::{Kind, NodeId, RegexBuilder, ResharpError};
use std::collections::HashMap;
use std::collections::HashSet;

fn tag_bits(b: &RegexBuilder, node: NodeId, memo: &mut HashMap<NodeId, u128>) -> Result<u128, ResharpError> {
    if let Some(&v) = memo.get(&node) {
        return Ok(v);
    }
    let v = match b.get_kind(node) {
        Kind::Tag => {
            let t = b.get_extra(node);
            if t >= 128 {
                return Err(ResharpError::UnsupportedPattern);
            }
            1u128 << t
        }
        Kind::Union | Kind::Concat | Kind::Inter | Kind::Lookahead | Kind::Lookbehind | Kind::Ordered => {
            let r = node.right(b);
            let rv = if r == NodeId::MISSING { 0 } else { tag_bits(b, r, memo)? };
            tag_bits(b, node.left(b), memo)? | rv
        }
        Kind::Star | Kind::Compl => tag_bits(b, node.left(b), memo)?,
        _ => 0,
    };
    memo.insert(node, v);
    Ok(v)
}

pub(crate) fn max_capture_tag(b: &RegexBuilder, root: NodeId) -> Result<u32, ResharpError> {
    let mut memo = HashMap::new();
    let bits = tag_bits(b, root, &mut memo)?;
    if bits == 0 {
        return Ok(0);
    }
    Ok(127 - bits.leading_zeros())
}

fn split_pair_union(b: &RegexBuilder, root: NodeId) -> Result<Option<NodeId>, ResharpError> {
    let mut memo: HashMap<NodeId, u128> = HashMap::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            continue;
        }
        let kind = b.get_kind(node);
        let r = node.right(b);
        if kind == Kind::Union && r != NodeId::MISSING {
            let l = node.left(b);
            let lv = tag_bits(b, l, &mut memo)?;
            let rv = tag_bits(b, r, &mut memo)?;
            for (own, other) in [(lv, rv), (rv, lv)] {
                let mut only = own & !other;
                while only != 0 {
                    let t = only.trailing_zeros();
                    only &= only - 1;
                    if own & (1u128 << (t ^ 1)) == 0 {
                        return Ok(Some(node));
                    }
                }
            }
        }
        match kind {
            Kind::Union | Kind::Concat | Kind::Inter | Kind::Lookahead | Kind::Lookbehind | Kind::Ordered => {
                stack.push(node.left(b));
                if r != NodeId::MISSING {
                    stack.push(r);
                }
            }
            Kind::Star | Kind::Compl => stack.push(node.left(b)),
            _ => {}
        }
    }
    Ok(None)
}

fn star_wraps_capture(b: &RegexBuilder, root: NodeId) -> Result<Option<NodeId>, ResharpError> {
    let mut tag_memo: HashMap<NodeId, u128> = HashMap::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            continue;
        }
        match b.get_kind(node) {
            Kind::Star => {
                let body = node.left(b);
                if tag_bits(b, body, &mut tag_memo)? != 0 {
                    return Ok(Some(node));
                }
                stack.push(body);
            }
            Kind::Ordered => {
                let body = node.left(b);
                if tag_bits(b, body, &mut tag_memo)? != 0 {
                    return Ok(Some(node));
                }
                stack.push(body);
                let chain = node.right(b);
                if chain != NodeId::MISSING {
                    stack.push(chain);
                }
            }
            Kind::Union | Kind::Concat | Kind::Inter | Kind::Lookahead | Kind::Lookbehind => {
                stack.push(node.left(b));
                let r = node.right(b);
                if r != NodeId::MISSING {
                    stack.push(r);
                }
            }
            Kind::Compl => stack.push(node.left(b)),
            _ => {}
        }
    }
    Ok(None)
}

fn tag_under_complement(b: &RegexBuilder, root: NodeId) -> Result<bool, ResharpError> {
    let mut tag_memo: HashMap<NodeId, u128> = HashMap::new();
    let mut visited: HashSet<NodeId> = HashSet::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if !visited.insert(node) {
            continue;
        }
        match b.get_kind(node) {
            Kind::Compl => {
                if tag_bits(b, node.left(b), &mut tag_memo)? != 0 {
                    return Ok(true);
                }
                stack.push(node.left(b));
            }
            Kind::Union | Kind::Concat | Kind::Inter | Kind::Lookahead | Kind::Lookbehind | Kind::Ordered => {
                stack.push(node.left(b));
                let r = node.right(b);
                if r != NodeId::MISSING {
                    stack.push(r);
                }
            }
            Kind::Star => stack.push(node.left(b)),
            _ => {}
        }
    }
    Ok(false)
}

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug)]
pub enum GroupOffset {
    FromBegin { open: u32, close: u32 },
    FromEnd { open: u32, close: u32 },
}

#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub enum CaptureDispatch {
    Empty,
    FixedOffsets(Vec<GroupOffset>),
    Dfa,
}

fn flatten_concat_spine(b: &RegexBuilder, node: NodeId, out: &mut Vec<NodeId>) {
    if b.get_kind(node) == Kind::Concat {
        flatten_concat_spine(b, node.left(b), out);
        flatten_concat_spine(b, node.right(b), out);
    } else {
        out.push(node);
    }
}

fn accumulate_tag_offsets(
    b: &RegexBuilder,
    terms: impl Iterator<Item = NodeId>,
) -> Result<HashMap<u32, u32>, ResharpError> {
    let mut offsets: HashMap<u32, Option<u32>> = HashMap::new();
    let mut running: Option<u32> = Some(0);
    let mut tag_bits_memo = HashMap::new();
    for t in terms {
        if b.get_kind(t) == Kind::Tag {
            let tag = b.get_extra(t);
            match (offsets.get(&tag), running) {
                (Some(_), _) => {
                    offsets.insert(tag, None);
                }
                (None, Some(off)) => {
                    offsets.insert(tag, Some(off));
                }
                (None, None) => {
                    offsets.insert(tag, None);
                }
            }
            continue;
        }
        if t.contains_tags(b) {
            let bits = tag_bits(b, t, &mut tag_bits_memo)?;
            let mut rest = bits;
            while rest != 0 {
                let bit = rest.trailing_zeros();
                rest &= rest - 1;
                offsets.insert(bit, None);
            }
        }
        running = match running {
            Some(o) if !t.contains_tags(b) => b.get_fixed_length(t).map(|w| o + w),
            _ => None,
        };
    }
    Ok(offsets.into_iter().filter_map(|(tag, off)| off.map(|o| (tag, o))).collect())
}

pub(crate) fn compute_capture_dispatch(
    b: &RegexBuilder,
    root: NodeId,
    num_groups: usize,
) -> Result<CaptureDispatch, ResharpError> {
    if num_groups == 0 {
        return Ok(CaptureDispatch::Empty);
    }
    let mut terms = Vec::new();
    flatten_concat_spine(b, root, &mut terms);
    let fwd = accumulate_tag_offsets(b, terms.iter().copied())?;
    let bwd = accumulate_tag_offsets(b, terms.iter().rev().copied())?;

    let mut out = Vec::with_capacity(num_groups);
    for idx in 1..=num_groups as u32 {
        let open_tag = 2 * idx;
        let close_tag = 2 * idx + 1;
        let resolved = match (fwd.get(&open_tag), fwd.get(&close_tag)) {
            (Some(&open), Some(&close)) if close >= open => Some(GroupOffset::FromBegin { open, close }),
            _ => match (bwd.get(&open_tag), bwd.get(&close_tag)) {
                (Some(&open), Some(&close)) if open >= close => Some(GroupOffset::FromEnd { open, close }),
                _ => None,
            },
        };
        match resolved {
            Some(g) => out.push(g),
            None => return Ok(CaptureDispatch::Dfa),
        }
    }
    Ok(CaptureDispatch::FixedOffsets(out))
}

pub(crate) fn ensure_captures_supported(
    b: &mut RegexBuilder,
    root: NodeId,
    group_names: &[Option<String>],
) -> Result<(), ResharpError> {
    if !root.contains_tags(b) {
        if root != NodeId::BOT && !group_names.is_empty() {
            return Err(ResharpError::UnsupportedPattern);
        }
        return Ok(());
    }
    let mut memo: HashMap<NodeId, u128> = HashMap::new();
    let bits = tag_bits(b, root, &mut memo)?;
    for (i, _name) in group_names.iter().enumerate() {
        let idx = (i + 1) as u32;
        let open = 2 * idx;
        let close = 2 * idx + 1;
        if open >= 128 || close >= 128 {
            return Err(ResharpError::UnsupportedPattern);
        }
        if bits & (1u128 << open) == 0 || bits & (1u128 << close) == 0 {
            return Err(ResharpError::UnsupportedPattern);
        }
    }
    if star_wraps_capture(b, root)?.is_some() {
        return Err(ResharpError::UnsupportedPattern);
    }
    if split_pair_union(b, root)?.is_some() {
        return Err(ResharpError::UnsupportedPattern);
    }
    if tag_under_complement(b, root)? {
        return Err(ResharpError::UnsupportedPattern);
    }
    let fwd_start = b.strip_lb(root)?;
    if fwd_start.contains_lookbehind(b) {
        return Err(ResharpError::UnsupportedPattern);
    }
    Ok(())
}

