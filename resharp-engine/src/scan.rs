use std::collections::HashMap;

use resharp_algebra::nulls::{
    push_null_desc, StartPositions, EID_BEGIN0, EID_CENTER0, EID_END0, EID_NONE, NullState, Nullability,
};
use resharp_algebra::{NodeId, RegexBuilder};

use crate::accel::{MintermSearchValue, Skipper};
use crate::ldfa::{dfa_delta, DFA_DEAD, DFA_MISSING, NO_MATCH};

pub(crate) struct ScanTables {
    pub(crate) center_table: *const u16,
    pub(crate) center_effect_id: *const u16,
    pub(crate) effects: *const Vec<NullState>,
    pub(crate) data: *const u8,
    pub(crate) minterms_lookup: *const u8,
    pub(crate) mt_log: u32,
}

#[cold]
#[inline(never)]
fn collect_rev_center_simple(
    effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    nulls: &mut StartPositions,
) {
    unsafe {
        let v = &*effects.add(eid as usize); // bounds: see `register_state`
        for n in v {
            push_null_desc(nulls, pos + n.rel as usize);
        }
    }
}

#[cold]
#[inline(never)]
pub(crate) fn collect_rev_complex(
    effects: *const Vec<NullState>,
    eid: u32,
    pos: usize,
    mask: Nullability,
    nulls: &mut StartPositions,
) {
    unsafe {
        let effects_vec = &*effects.add(eid as usize); // bounds: see `register_state`
        for n in effects_vec {
            if n.mask.has(mask) {
                push_null_desc(nulls, pos + n.rel as usize);
            }
        }
    }
}

#[inline(always)]
fn collect_max<const REV: bool>(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    let eid = effects_id[state as usize] as u32;
    if eid == EID_NONE as u32 {
        return;
    }
    if eid == EID_CENTER0 as u32 {
        if mask.has(Nullability::ALWAYS) {
            if REV {
                *best = (*best).min(pos);
            } else {
                *best = if *best == NO_MATCH {
                    pos
                } else {
                    (*best).max(pos)
                };
            }
        }
        return;
    }
    let v = &effects[eid as usize];
    if let Some(n) = v.iter().rev().find(|n| n.mask.has(mask)) {
        if REV {
            *best = (*best).min(pos + n.rel as usize);
        } else {
            let cand = pos - n.rel as usize;
            *best = if *best == NO_MATCH {
                cand
            } else {
                (*best).max(cand)
            };
        }
    }
}

#[inline(always)]
pub(crate) fn collect_max_fwd(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    collect_max::<false>(effects_id, effects, state, pos, mask, best);
}

#[inline(always)]
pub(crate) fn collect_max_rev(
    effects_id: &[u16],
    effects: &[Vec<NullState>],
    state: u32,
    pos: usize,
    mask: Nullability,
    best: &mut usize,
) {
    collect_max::<true>(effects_id, effects, state, pos, mask, best);
}

enum SkipStep {
    Continue,
    Fall,
    Done,
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn skip_rev(
    t: &ScanTables,
    skipper: &Skipper,
    data: &[u8],
    curr: &mut u32,
    pos: &mut usize,
    prev_entry: &mut usize,
    nulls: &mut StartPositions,
    b: &mut RegexBuilder,
    conv_b: &mut Option<&mut crate::ldfa::LDFA>,
) -> Result<SkipStep, crate::Error> {
    match skipper {
        Skipper::Inner {
            search,
            resume,
            pruned,
            window,
        } => {
            let resume = *resume;
            let pruned = *pruned;
            let window = *window as usize;
            let orig_pos = *pos;
            let mut search_from = *pos;
            loop {
                match search.find_rev(data, search_from) {
                    Some(skip_pos) if skip_pos >= *prev_entry => {
                        if *pos > skip_pos {
                            return Ok(SkipStep::Fall);
                        }
                        *prev_entry = usize::MAX;
                        if skip_pos == 0 {
                            return Ok(SkipStep::Done);
                        }
                        *pos = skip_pos - 1;
                        return Ok(SkipStep::Continue);
                    }
                    Some(skip_pos) => {
                        let cb = conv_b.as_deref_mut().unwrap();
                        if cb.conv_start_matches(b, skip_pos + 1, data)? {
                            *prev_entry = skip_pos;
                            let entry = (skip_pos + 1 + window).min(orig_pos).max(skip_pos + 1);
                            *pos = entry;
                            if entry > skip_pos + 1 {
                                *curr = pruned;
                                return Ok(SkipStep::Fall);
                            }
                            *curr = resume;
                            return Ok(if skip_pos == 0 {
                                SkipStep::Continue
                            } else {
                                SkipStep::Fall
                            });
                        } else if skip_pos == 0 {
                            return Ok(SkipStep::Done);
                        } else {
                            search_from = skip_pos - 1;
                        }
                    }
                    None => {
                        *pos = 0;
                        return Ok(SkipStep::Continue);
                    }
                }
            }
        }
        Skipper::Prefix(search) => match search.find_rev(data, *pos) {
            Some(skip_pos) => {
                if skip_pos != *pos {
                    *pos = skip_pos + 1;
                    let eid = unsafe { *t.center_effect_id.add(*curr as usize) };
                    if eid != EID_NONE as _ {
                        if eid == EID_CENTER0 as _ {
                            nulls.add(*pos + 1);
                        } else {
                            collect_rev_center_simple(t.effects, eid as u32, *pos + 1, nulls);
                        }
                    }
                    if skip_pos == 0 {
                        return Ok(SkipStep::Continue);
                    }
                }
                Ok(SkipStep::Fall)
            }
            None => {
                *pos = 0;
                Ok(SkipStep::Continue)
            }
        },
        Skipper::State(searcher) => {
            let lo = searcher.find_rev(&data[..*pos]).unwrap_or(0);
            let eid = unsafe { *t.center_effect_id.add(*curr as usize) };
            if eid == EID_NONE as _ {
            } else if eid == EID_CENTER0 as _ {
                nulls.add_range(lo + 1, *pos);
            } else {
                let v = unsafe { &*t.effects.add(eid as usize) };
                if v.len() == 1 {
                    let rel = v[0].rel as usize;
                    nulls.add_range(lo + 1 + rel, *pos + rel);
                } else {
                    for p in (lo + 1..*pos).rev() {
                        collect_rev_center_simple(t.effects, eid as u32, p, nulls);
                    }
                }
            }
            *pos = lo + 1;
            if lo == 0 {
                Ok(SkipStep::Continue)
            } else {
                Ok(SkipStep::Fall)
            }
        }
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_rev<const EARLY_EXIT: bool, const SKIP: bool>(
    t: &ScanTables,
    skip_ids: &[u8],
    skip_searchers: &[Skipper],
    mut curr: u32,
    mut pos: usize,
    data: &[u8],
    nulls: &mut StartPositions,
    b: &mut RegexBuilder,
    mut conv_b: Option<&mut crate::ldfa::LDFA>,
) -> Result<(u32, usize, bool), crate::Error> {
    let center_table = t.center_table;
    let center_effect_id = t.center_effect_id;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
    let mut prev_entry = usize::MAX;
    while pos > 1 {
        if SKIP {
            let sid = skip_ids[curr as usize];
            if sid != 0 {
                match skip_rev(
                    t,
                    skipper(skip_searchers, sid),
                    data,
                    &mut curr,
                    &mut pos,
                    &mut prev_entry,
                    nulls,
                    b,
                    &mut conv_b,
                )? {
                    SkipStep::Continue => continue,
                    SkipStep::Fall => {}
                    SkipStep::Done => return Ok((curr, 1, false)),
                }
            }
        }
        pos -= 1;
        unsafe {
            let mt = *minterms_lookup.add(*data.as_ptr().add(pos) as usize) as u32;
            let next = *center_table.add(dfa_delta(curr, mt, mt_log));
            if next == DFA_MISSING {
                return Ok((curr, pos, true));
            }
            curr = next as u32;
            let eid = *center_effect_id.add(curr as usize); // bounds: see `register_state`
            if eid != EID_NONE as _ {
                if eid == EID_CENTER0 as _ {
                    nulls.add(pos);
                    if EARLY_EXIT {
                        return Ok((curr, pos, false));
                    }
                } else {
                    collect_rev_center_simple(t.effects, eid as u32, pos, nulls);
                    if EARLY_EXIT && !nulls.is_empty() {
                        return Ok((curr, pos, false));
                    }
                }
            }
        }
    }

    Ok((curr, 1, false))
}

#[inline(always)]
pub(crate) unsafe fn fwd_update<const IS_END: bool>(
    effect_id: *const u16,
    effects: *const Vec<NullState>,
    state: u32,
    pos: usize,
    max_end: usize,
) -> usize {
    let eid = unsafe { *effect_id.add(state as usize) }; // bounds: see `register_state`
    if eid == EID_NONE as u16 {
        return max_end;
    }
    if eid == EID_CENTER0 as u16 {
        return if max_end == NO_MATCH {
            pos
        } else {
            max_end.max(pos)
        };
    }
    let v = unsafe { &*effects.add(eid as usize) }; // bounds: see `register_state`
    debug_assert!(v.windows(2).all(|w| w[0].rel >= w[1].rel));
    let pick = if IS_END {
        v.iter().rev().find(|n| n.mask.has(Nullability::END))
    } else {
        v.last()
    };
    match pick {
        Some(n) => {
            let cand = pos - n.rel as usize;
            if max_end == NO_MATCH {
                cand
            } else {
                max_end.max(cand)
            }
        }
        None => max_end,
    }
}

#[inline(always)]
unsafe fn skip_find_fwd(
    searcher: &MintermSearchValue,
    data: *const u8,
    pos: usize,
    end: usize,
) -> Option<usize> {
    searcher.find_fwd(std::slice::from_raw_parts(data.add(pos), end - pos))
}

#[inline(always)]
pub(crate) fn skipper(skips: &[Skipper], sid: u8) -> &Skipper {
    &skips[sid as usize - 1]
}

#[inline(always)]
pub(crate) fn state_searcher(skips: &[Skipper], sid: u8) -> &MintermSearchValue {
    match skipper(skips, sid) {
        Skipper::State(s) => s,
        _ => unreachable!(),
    }
}

#[inline(never)]
pub(crate) fn scan_fwd_verify<const SKIP: bool>(
    t: &ScanTables,
    effects_id: *const u16,
    skip_ids: &[u8],
    skip_searchers: &[Skipper],
    mut curr: u32,
    mut pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects = t.effects;
    let center_effect_id = t.center_effect_id;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;

    'outer: while pos < end {
        if SKIP {
            {
                let sid = skip_ids[curr as usize];
                if sid != 0 {
                    let searcher = state_searcher(skip_searchers, sid);
                    match unsafe { skip_find_fwd(searcher, data, pos, end) } {
                        Some(offset) => {
                            if offset > 0 {
                                unsafe {
                                    max_end = fwd_update::<false>(
                                        center_effect_id,
                                        effects,
                                        curr,
                                        pos + offset,
                                        max_end,
                                    );
                                }
                            }
                            pos += offset;
                        }
                        None => {
                            unsafe {
                                max_end =
                                    fwd_update::<true>(effects_id, effects, curr, end, max_end);
                            }
                            return (curr, end, max_end, false);
                        }
                    }
                }
            }
        }

        let mut prev_state: u32 = curr;
        let mut has_prev = false;
        while pos < end {
            unsafe {
                let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
                if has_prev {
                    max_end = fwd_update::<false>(
                        center_effect_id,
                        effects,
                        prev_state,
                        pos,
                        max_end,
                    );
                }
                let delta = dfa_delta(curr, mt, mt_log);
                let next = *center_table.add(delta);
                if next == DFA_MISSING {
                    return (curr, pos, max_end, true);
                }
                if next == DFA_DEAD {
                    return (DFA_DEAD as u32, pos, max_end, false);
                }
                curr = next as u32;
                prev_state = curr;
                has_prev = true;
            }
            pos += 1;
            if SKIP && skip_ids[curr as usize] != 0 {
                if has_prev {
                    if pos >= end {
                        unsafe {
                            max_end =
                                fwd_update::<true>(effects_id, effects, prev_state, pos, max_end);
                        }
                    } else {
                        unsafe {
                            max_end = fwd_update::<false>(
                                center_effect_id,
                                effects,
                                prev_state,
                                pos,
                                max_end,
                            );
                        }
                    }
                }
                continue 'outer;
            }
        }
        if has_prev {
            unsafe {
                max_end = fwd_update::<true>(effects_id, effects, prev_state, pos, max_end);
            }
        }
        if !SKIP {
            break 'outer;
        }
    }
    (curr, pos, max_end, false)
}

/// Like `scan_fwd_verify` but stops at the first potentially CENTER-nullable state.
/// Class skip is safe: self-looping non-nullable states produce identical transitions.
#[inline(never)]
#[cfg_attr(not(feature = "stream"), allow(dead_code))]
pub(crate) fn scan_fwd_first_null<const SKIP: bool>(
    t: &ScanTables,
    effects_id: *const u16,
    skip_ids: &[u8],
    skip_searchers: &[Skipper],
    mut curr: u32,
    mut pos: usize,
    end: usize,
) -> (u32, usize, bool, bool) {
    let center_table = t.center_table;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;

    'outer: while pos < end {
        if SKIP {
            let sid = skip_ids[curr as usize];
            if sid != 0 {
                let searcher = state_searcher(skip_searchers, sid);
                match unsafe { skip_find_fwd(searcher, data, pos, end) } {
                    Some(offset) => {
                        pos += offset;
                    }
                    None => {
                        return (curr, end, false, false);
                    }
                }
            }
        }
        while pos < end {
            unsafe {
                let mt = *minterms_lookup.add(*data.add(pos) as usize) as u32;
                let delta = dfa_delta(curr, mt, mt_log);
                let next = *center_table.add(delta);
                if next == DFA_MISSING {
                    return (curr, pos, false, true);
                }
                if next == DFA_DEAD {
                    return (DFA_DEAD as u32, pos, false, false);
                }
                curr = next as u32;
            }
            pos += 1;
            let eid = unsafe { *effects_id.add(curr as usize) as u32 }; // bounds: see `register_state`
            if eid != 0 && eid != EID_BEGIN0 && eid != EID_END0 {
                return (curr, pos, true, false);
            }
            if SKIP && skip_ids[curr as usize] != 0 {
                continue 'outer;
            }
        }
        if !SKIP {
            break;
        }
    }
    (curr, pos, false, false)
}

#[inline(never)]
pub(crate) fn scan_fwd<const SKIP: bool>(
    t: &ScanTables,
    effects_id: *const u16,
    skip_ids: &[u8],
    skip_searchers: &[Skipper],
    mut l_state: u32,
    mut l_pos: usize,
    end: usize,
    mut max_end: usize,
) -> (u32, usize, usize, bool) {
    let center_table = t.center_table;
    let effects = t.effects;
    let center_effect_id = t.center_effect_id;
    let data = t.data;
    let minterms_lookup = t.minterms_lookup;
    let mt_log = t.mt_log;
    unsafe {
        if l_pos >= end && l_state != DFA_DEAD as u32 {
            max_end = fwd_update::<true>(effects_id, effects, l_state, end, max_end);
            return (l_state, end, max_end, false);
        }
        while l_state != DFA_DEAD as u32 {
            if SKIP {
                {
                    let sid = skip_ids[l_state as usize];
                    if sid != 0 {
                        let searcher = state_searcher(skip_searchers, sid);
                        match skip_find_fwd(searcher, data, l_pos, end) {
                            Some(offset) => {
                                if offset > 0 {
                                    max_end = fwd_update::<false>(
                                        center_effect_id,
                                        effects,
                                        l_state,
                                        l_pos + offset,
                                        max_end,
                                    );
                                }
                                l_pos += offset;
                            }
                            None => {
                                // no non-self-loop byte: entire rest is self-loop
                                max_end = fwd_update::<false>(
                                    center_effect_id,
                                    effects,
                                    l_state,
                                    end - 1,
                                    max_end,
                                );
                                max_end =
                                    fwd_update::<true>(effects_id, effects, l_state, end, max_end);
                                return (l_state, end, max_end, false);
                            }
                        }
                    }
                }
            }
            max_end =
                fwd_update::<false>(center_effect_id, effects, l_state, l_pos, max_end);
            let mt = *minterms_lookup.add(*data.add(l_pos) as usize) as u32;
            let delta = dfa_delta(l_state, mt, mt_log);
            let next = *center_table.add(delta) as u32;
            if next == DFA_MISSING as u32 {
                return (l_state, l_pos, max_end, true);
            }
            if next == DFA_DEAD as u32 {
                return (DFA_DEAD as u32, l_pos, max_end, false);
            }
            // eprintln!("[pos] {:?}; {}->{}", l_pos, l_state, next);
            l_state = next;
            l_pos += 1;
            if l_pos == end {
                max_end = fwd_update::<true>(effects_id, effects, l_state, l_pos, max_end);
                l_state = DFA_DEAD as _;
            }
        }
    }
    (l_state, l_pos, max_end, false)
}

pub(crate) fn register_state(
    state_nodes: &mut Vec<NodeId>,
    node_to_state: &mut HashMap<NodeId, u16>,
    effects_id: &mut Vec<u16>,
    center_effect_id: &mut Vec<u16>,
    effects: &mut Vec<Vec<NullState>>,
    b: &mut RegexBuilder,
    node: NodeId,
    force: bool,
) -> u16 {
    if !force {
        if let Some(&sid) = node_to_state.get(&node) {
            return sid;
        }
    }
    let sid = state_nodes.len() as u16;
    state_nodes.push(node);
    if !node_to_state.contains_key(&node) {
        node_to_state.insert(node, sid);
    }
    let eff_id = b.get_nulls_id(node);
    let eid = b.center_nulls_id(eff_id);
    if sid as usize >= effects_id.len() {
        effects_id.resize(sid as usize + 1, 0u16);
    }
    if sid as usize >= center_effect_id.len() {
        center_effect_id.resize(sid as usize + 1, EID_NONE as u16);
    }
    effects_id[sid as usize] = eff_id.0 as u16;
    center_effect_id[sid as usize] = eid.0 as u16;
    while effects.len() <= eff_id.0 as usize || effects.len() <= eid.0 as usize {
        effects.push(b.nulls_entry_vec(effects.len() as u32));
    }
    // all of these must hold while matching
    debug_assert!((sid as usize) < effects_id.len());
    debug_assert!((sid as usize) < center_effect_id.len());
    debug_assert!((effects_id[sid as usize] as usize) < effects.len());
    debug_assert!((center_effect_id[sid as usize] as usize) < effects.len());
    sid
}
