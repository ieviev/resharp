//! Boolean algebra and symbolic rewriting engine for resharp regex.
//!
//! Provides regex node construction, symbolic derivatives, nullability analysis,
//! and algebraic simplification via rewrite rules.

#![warn(dead_code)]

pub mod unicode_classes;
use rustc_hash::FxHashMap;
use solver::{Solver, TSetId};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Debug;
use std::fmt::Write;
use std::hash::Hash;
pub use unicode_classes::UnicodeClassCache;

use crate::nulls::{NullState, Nullability, NullsBuilder, NullsId};
pub mod nulls;
pub mod solver;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgebraError {
    AnchorLimit,
    StateSpaceExplosion,
    UnsupportedPattern,
}

impl std::fmt::Display for AlgebraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlgebraError::AnchorLimit => write!(f, "anchor limit exceeded"),
            AlgebraError::StateSpaceExplosion => {
                write!(f, "too many states, likely infinite state space")
            }
            AlgebraError::UnsupportedPattern => write!(f, "unsupported lookaround pattern"),
        }
    }
}

impl std::error::Error for AlgebraError {}

mod id {
    pub const MISSING: u32 = 0;
    pub const BOT: u32 = 1;
    pub const EPS: u32 = 2;
    pub const TOP: u32 = 3;
    pub const TOPSTAR: u32 = 4;
    pub const TOPPLUS: u32 = 5;
    pub const BEGIN: u32 = 6;
    pub const END: u32 = 7;
}

#[derive(Clone, Copy, PartialEq, Hash, Eq, Debug)]
pub(crate) struct MetadataId(u32);
impl MetadataId {
    pub(crate) const MISSING: MetadataId = MetadataId(id::MISSING);
}

#[derive(Clone, Copy, PartialEq, Hash, Eq, PartialOrd, Ord, Default)]
pub struct NodeId(pub u32);
impl NodeId {
    pub const MISSING: NodeId = NodeId(id::MISSING);
    pub const BOT: NodeId = NodeId(id::BOT);
    pub const EPS: NodeId = NodeId(id::EPS);
    pub const TOP: NodeId = NodeId(id::TOP);
    pub const TS: NodeId = NodeId(id::TOPSTAR);
    pub const TOPPLUS: NodeId = NodeId(id::TOPPLUS);
    pub const BEGIN: NodeId = NodeId(id::BEGIN);
    pub const END: NodeId = NodeId(id::END);

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub fn from_u32(v: u32) -> NodeId {
        NodeId(v)
    }
}
impl Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let num = &self.0;
        f.write_str(format!("{num}").as_str())
    }
}

#[derive(Clone, Copy, PartialEq, Hash, Eq, Debug, PartialOrd, Ord)]
pub struct TRegexId(u32);
impl TRegexId {
    pub const MISSING: TRegexId = TRegexId(id::MISSING);
    pub const EPS: TRegexId = TRegexId(id::EPS);
    pub const BOT: TRegexId = TRegexId(id::BOT);
    pub const TOP: TRegexId = TRegexId(id::TOP);
    pub const TOPSTAR: TRegexId = TRegexId(id::TOPSTAR);
}

#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub(crate) struct MetaFlags(u8);
impl MetaFlags {
    const NULL_MASK: u8 = 0b111; // first 3 bits for nullability

    pub(crate) const ZERO: MetaFlags = MetaFlags(0);
    pub(crate) const CONTAINS_LOOKAROUND: MetaFlags = MetaFlags(1 << 3);
    pub(crate) const INFINITE_LENGTH: MetaFlags = MetaFlags(1 << 4);
    pub(crate) const CONTAINS_COMPL: MetaFlags = MetaFlags(1 << 5);
    pub(crate) const CONTAINS_INTER: MetaFlags = MetaFlags(1 << 6);
    pub(crate) const CONTAINS_ANCHORS: MetaFlags = MetaFlags(1 << 7);

    #[inline]
    pub(crate) fn nullability(self) -> Nullability {
        Nullability(self.0 & Self::NULL_MASK)
    }

    #[inline]
    pub(crate) const fn with_nullability(n: Nullability, flags: MetaFlags) -> MetaFlags {
        MetaFlags((flags.0 & !Self::NULL_MASK) | n.0)
    }

    #[inline]
    pub(crate) fn has(self, flag: MetaFlags) -> bool {
        self.0 & flag.0 != 0
    }
    #[inline]
    const fn and(self, other: MetaFlags) -> MetaFlags {
        MetaFlags(self.0 & other.0)
    }
    #[inline]
    const fn or(self, other: MetaFlags) -> MetaFlags {
        MetaFlags(self.0 | other.0)
    }

    pub(crate) fn contains_lookaround(self) -> bool {
        self.has(MetaFlags::CONTAINS_LOOKAROUND)
    }
    pub(crate) fn contains_inter(self) -> bool {
        self.has(MetaFlags::CONTAINS_INTER)
    }

    pub(crate) fn all_contains_flags(self) -> MetaFlags {
        self.and(
            MetaFlags::CONTAINS_LOOKAROUND
                .or(MetaFlags::CONTAINS_ANCHORS)
                .or(MetaFlags::CONTAINS_INTER),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Kind {
    #[default]
    Pred,
    Star,
    Begin,
    End,
    Concat,
    Union,
    Compl,
    Lookbehind,
    Lookahead,
    Inter,
    Counted,
}

#[derive(Eq, Hash, PartialEq, Clone)]
struct Metadata {
    flags: MetaFlags,
    nulls: NullsId,
}

struct MetadataBuilder {
    num_created: u32,
    solver: Solver,
    nb: NullsBuilder,
    index: FxHashMap<Metadata, MetadataId>,
    pub array: Vec<Metadata>,
}

mod helpers {
    pub(crate) fn incr_rel(n1: u32) -> u32 {
        match n1.overflowing_add(1) {
            (_, true) => u32::MAX,
            (res, false) => res,
        }
    }
}

impl MetadataBuilder {
    fn new() -> MetadataBuilder {
        Self {
            index: FxHashMap::default(),
            array: vec![Metadata {
                flags: MetaFlags::ZERO,
                nulls: NullsId::EMPTY,
            }],
            solver: Solver::new(),
            num_created: 0,
            nb: NullsBuilder::new(),
        }
    }

    fn init(&mut self, inst: Metadata) -> MetadataId {
        self.num_created += 1;
        let new_id = MetadataId(self.num_created);
        self.index.insert(inst.clone(), new_id);
        self.array.push(inst);
        new_id
    }

    fn get_meta_id(&mut self, inst: Metadata) -> MetadataId {
        match self.index.get(&inst) {
            Some(&id) => id,
            None => self.init(inst),
        }
    }

    fn get_meta_ref(&mut self, inst: MetadataId) -> &Metadata {
        &self.array[inst.0 as usize]
    }

    fn get_contains(&self, setflags: MetaFlags) -> MetaFlags {
        setflags.all_contains_flags()
    }

    fn flags_star(&self, body: MetadataId, body_id: NodeId) -> MetaFlags {
        let left = &self.array[body.0 as usize].flags;
        let contains = left.and(MetaFlags::CONTAINS_LOOKAROUND.or(MetaFlags::CONTAINS_INTER));
        // BOT* = EPS (empty string only), not infinite
        let inf = if body_id == NodeId::BOT {
            MetaFlags::ZERO
        } else {
            MetaFlags::INFINITE_LENGTH
        };
        MetaFlags::with_nullability(Nullability::ALWAYS, contains.or(inf))
    }

    fn flags_compl(&self, left_id: MetadataId) -> MetaFlags {
        let left = &self.array[left_id.0 as usize].flags;
        let null = left.nullability().not().and(Nullability::ALWAYS);
        let contains = self.get_contains(*left);
        MetaFlags::with_nullability(
            null,
            contains
                .or(MetaFlags::INFINITE_LENGTH)
                .or(MetaFlags::CONTAINS_COMPL),
        )
    }

    fn flags_concat(&self, left_id: MetadataId, right_id: MetadataId) -> MetaFlags {
        let left = &self.array[left_id.0 as usize].flags;
        let right = &self.array[right_id.0 as usize].flags;
        let null = left.nullability().and(right.nullability());
        let contains = self.get_contains(left.or(*right));
        let len = (left.or(*right)).and(MetaFlags::INFINITE_LENGTH);
        MetaFlags::with_nullability(null, contains.or(len))
    }

    fn flags_inter(&self, left_id: MetadataId, right_id: MetadataId) -> MetaFlags {
        let left = &self.array[left_id.0 as usize].flags;
        let right = &self.array[right_id.0 as usize].flags;
        let null = left.nullability().and(right.nullability());
        let contains = self
            .get_contains(left.or(*right))
            .or(MetaFlags::CONTAINS_INTER);
        let len = (left.and(*right)).and(MetaFlags::INFINITE_LENGTH);
        MetaFlags::with_nullability(null, contains.or(len))
    }

    fn flags_union(&self, left_id: MetadataId, right_id: MetadataId) -> MetaFlags {
        let left = &self.array[left_id.0 as usize].flags;
        let right = &self.array[right_id.0 as usize].flags;
        let null = left.nullability().or(right.nullability());
        let contains = self.get_contains(left.or(*right));
        let len = (left.or(*right)).and(MetaFlags::INFINITE_LENGTH);
        MetaFlags::with_nullability(null, contains.or(len))
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Debug, Default)]
pub struct NodeKey {
    pub(crate) kind: Kind,
    pub(crate) left: NodeId,
    pub(crate) right: NodeId,
    pub(crate) extra: u32,
}

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub enum TRegex<TSet> {
    Leaf(NodeId),
    ITE(TSet, TRegexId, TRegexId),
}

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub(crate) struct NodeFlags(u8);
impl NodeFlags {
    pub(crate) const ZERO: NodeFlags = NodeFlags(0);
    pub(crate) const IS_CHECKED: NodeFlags = NodeFlags(1);
    pub(crate) const IS_EMPTY: NodeFlags = NodeFlags(1 << 1);

    fn is_checked(self) -> bool {
        self.0 >= NodeFlags::IS_CHECKED.0
    }
    fn is_empty(self) -> bool {
        self.0 & NodeFlags::IS_EMPTY.0 == NodeFlags::IS_EMPTY.0
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Copy)]
pub(crate) struct BuilderFlags(u8);
impl BuilderFlags {
    pub(crate) const ZERO: BuilderFlags = BuilderFlags(0);
    pub(crate) const SUBSUME: BuilderFlags = BuilderFlags(1);
}

pub struct RegexBuilder {
    mb: MetadataBuilder,
    temp_vec: Vec<NodeId>,
    num_created: u32,
    index: FxHashMap<NodeKey, NodeId>,
    array: Vec<NodeKey>,
    metadata: Vec<MetadataId>,
    reversed: Vec<NodeId>,
    cache_empty: FxHashMap<NodeId, NodeFlags>,
    tr_cache: FxHashMap<TRegex<TSetId>, TRegexId>,
    tr_array: Vec<TRegex<TSetId>>,
    tr_der_center: Vec<TRegexId>,
    tr_der_begin: Vec<TRegexId>,
    flags: BuilderFlags,
    /// maximum lookahead context distance before returning `AnchorLimit`.
    pub lookahead_context_max: u32,
    mk_binary_memo: FxHashMap<(TRegexId, TRegexId), TRegexId>,
    clean_cache: FxHashMap<(TSetId, TRegexId), TRegexId>,
}

macro_rules! iter_inter {
    ($compiler:ident, $start:ident, $expression:expr) => {
        let mut curr = $start;
        while $compiler.get_kind(curr) == Kind::Inter {
            let left = $compiler.get_left(curr);
            $expression(left);
            curr = $compiler.get_right(curr);
        }
        $expression(curr);
    };
}

impl NodeId {
    fn is_missing(&self) -> bool {
        *self == NodeId::MISSING
    }
    #[inline]
    fn flags_contains(self, b: &RegexBuilder) -> MetaFlags {
        b.get_flags_contains(self)
    }
    pub(crate) fn has_concat_tail(self, b: &RegexBuilder, tail: NodeId) -> bool {
        if self == tail {
            true
        } else if self.is_kind(b, Kind::Concat) {
            self.right(b).has_concat_tail(b, tail)
        } else {
            false
        }
    }
    fn missing_to_eps(&self) -> NodeId {
        if *self == NodeId::MISSING {
            NodeId::EPS
        } else {
            *self
        }
    }

    #[inline]
    fn kind(self, b: &RegexBuilder) -> Kind {
        b.get_kind(self)
    }
    #[inline]
    fn is_kind(self, b: &RegexBuilder, k: Kind) -> bool {
        b.get_kind(self) == k
    }
    #[inline]
    fn is_never_nullable(self, b: &RegexBuilder) -> bool {
        b.nullability(self) == Nullability::NEVER
    }
    #[inline]
    fn nullability(self, b: &RegexBuilder) -> Nullability {
        b.nullability(self)
    }

    #[inline]
    fn is_center_nullable(self, b: &RegexBuilder) -> bool {
        b.nullability(self).and(Nullability::CENTER) != Nullability::NEVER
    }
    #[inline]
    pub fn left(self, b: &RegexBuilder) -> NodeId {
        b.get_left(self)
    }

    #[inline]
    pub fn right(self, b: &RegexBuilder) -> NodeId {
        b.get_right(self)
    }

    #[inline]
    fn der(self, b: &mut RegexBuilder, mask: Nullability) -> Result<TRegexId, AlgebraError> {
        b.der(self, mask)
    }

    #[inline]
    fn extra(self, b: &RegexBuilder) -> u32 {
        b.get_extra(self)
    }

    #[inline]
    fn is_pred(self, b: &RegexBuilder) -> bool {
        b.get_kind(self) == Kind::Pred
    }
    #[inline]
    fn is_lookahead(self, b: &RegexBuilder) -> bool {
        b.get_kind(self) == Kind::Lookahead
    }
    #[inline]
    pub fn pred_tset(self, b: &RegexBuilder) -> TSetId {
        debug_assert!(self.is_pred(b));
        TSetId(b.get_extra(self))
    }
    #[inline]
    fn is_star(self, b: &RegexBuilder) -> bool {
        if NodeId::EPS == self {
            return false;
        }
        b.get_kind(self) == Kind::Star
    }

    #[inline]
    pub(crate) fn is_inter(self, b: &RegexBuilder) -> bool {
        b.get_kind(self) == Kind::Inter
    }

    #[inline]
    pub(crate) fn is_plus(self, b: &RegexBuilder) -> bool {
        if self.is_concat(b) {
            let r = self.right(b);
            return r.is_star(b) && r.left(b) == self.left(b);
        }
        false
    }

    #[inline]
    fn is_concat(self, b: &RegexBuilder) -> bool {
        b.get_kind(self) == Kind::Concat
    }

    #[inline]
    fn is_opt_v(self, b: &RegexBuilder) -> Option<NodeId> {
        if b.get_kind(self) == Kind::Union && b.get_left(self) == NodeId::EPS {
            Some(b.get_right(self))
        } else {
            None
        }
    }

    #[inline]
    fn is_compl_plus_end(self, b: &RegexBuilder) -> bool {
        if b.get_kind(self) == Kind::Concat {
            let left = self.left(b);
            let right = self.right(b);
            if left.is_kind(b, Kind::Compl) && left.left(b) == NodeId::TOPPLUS {
                return right == NodeId::END;
            }
        }
        false
    }

    #[inline]
    fn is_pred_star(self, b: &RegexBuilder) -> Option<NodeId> {
        if NodeId::EPS == self {
            return None;
        }
        if self.is_star(b) && self.left(b).is_pred(b) {
            Some(self.left(b))
        } else {
            None
        }
    }

    #[inline]
    fn is_contains(self, b: &RegexBuilder) -> Option<NodeId> {
        if b.get_kind(self) == Kind::Concat && self.left(b) == NodeId::TS {
            let middle = self.right(b);
            if middle.kind(b) == Kind::Concat && middle.right(b) == NodeId::TS {
                return Some(middle.left(b));
            }
        }
        None
    }

    #[inline]
    pub(crate) fn iter_union(
        self,
        b: &mut RegexBuilder,
        f: &mut impl FnMut(&mut RegexBuilder, NodeId),
    ) {
        let mut curr: NodeId = self;
        while curr.kind(b) == Kind::Union {
            f(b, curr.left(b));
            curr = curr.right(b);
        }
        f(b, curr);
    }

    #[inline]
    pub(crate) fn iter_union_while(
        self,
        b: &mut RegexBuilder,
        f: &mut impl FnMut(&mut RegexBuilder, NodeId) -> bool,
    ) {
        let mut curr: NodeId = self;
        let mut continue_loop = true;
        while continue_loop && curr.kind(b) == Kind::Union {
            continue_loop = f(b, curr.left(b));
            curr = curr.right(b);
        }
        if continue_loop {
            f(b, curr);
        }
    }
}

impl Default for RegexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl RegexBuilder {
    pub fn new() -> RegexBuilder {
        let mut inst = Self {
            mb: MetadataBuilder::new(),
            array: Vec::new(),
            index: FxHashMap::default(),
            cache_empty: FxHashMap::default(),
            tr_array: Vec::new(),
            tr_cache: FxHashMap::default(),
            flags: BuilderFlags::ZERO,
            lookahead_context_max: 800,
            num_created: 0,
            metadata: Vec::new(),
            reversed: Vec::new(),
            tr_der_center: Vec::new(),
            tr_der_begin: Vec::new(),
            temp_vec: Vec::new(),
            mk_binary_memo: FxHashMap::default(),
            clean_cache: FxHashMap::default(),
        };
        inst.array.push(NodeKey::default());
        inst.mk_pred(TSetId::EMPTY);
        inst.mk_star(NodeId::BOT);
        inst.mk_pred(TSetId::FULL);
        inst.mk_star(NodeId::TOP);
        let top_plus_id = inst.mk_plus(NodeId::TOP);
        inst.mk_unset(Kind::Begin);
        inst.mk_unset(Kind::End);
        debug_assert!(top_plus_id == NodeId::TOPPLUS);

        inst.tr_array.push(TRegex::Leaf(NodeId::MISSING));
        inst.mk_leaf(NodeId::BOT);
        inst.mk_leaf(NodeId::EPS);
        inst.mk_leaf(NodeId::TOP);
        inst.mk_leaf(NodeId::TS);

        inst.flags = BuilderFlags::SUBSUME;
        inst
    }

    #[inline]
    pub fn solver_ref(&self) -> &Solver {
        &self.mb.solver
    }

    #[inline]
    pub fn solver(&mut self) -> &mut Solver {
        &mut self.mb.solver
    }

    fn tr_init(&mut self, inst: TRegex<TSetId>) -> TRegexId {
        let new_id = TRegexId(self.tr_cache.len() as u32 + 1);
        self.tr_cache.insert(inst.clone(), new_id);
        self.tr_array.push(inst);
        new_id
    }

    fn get_tregex_id(&mut self, inst: TRegex<TSetId>) -> TRegexId {
        match self.tr_cache.get(&inst) {
            Some(&id) => id,
            None => self.tr_init(inst),
        }
    }

    pub fn get_tregex(&self, inst: TRegexId) -> &TRegex<TSetId> {
        &self.tr_array[inst.0 as usize]
    }

    fn unsat(&mut self, cond1: TSetId, cond2: TSetId) -> bool {
        let solv = self.solver();
        !solv.is_sat_id(cond1, cond2)
    }

    pub(crate) fn mk_leaf(&mut self, node_id: NodeId) -> TRegexId {
        let term = TRegex::<TSetId>::Leaf(node_id);
        self.get_tregex_id(term)
    }

    fn mk_ite(&mut self, cond: TSetId, _then: TRegexId, _else: TRegexId) -> TRegexId {
        let tmp_inst = TRegex::ITE(cond, _then, _else);
        if let Some(cached) = self.tr_cache.get(&tmp_inst) {
            return *cached;
        }
        if _then == _else {
            return _then;
        }
        if self.solver().is_full_id(cond) {
            return _then;
        }
        if self.solver().is_empty_id(cond) {
            return _else;
        }
        let _clean_then = match self.tr_array[_then.0 as usize] {
            TRegex::Leaf(_) => _then,
            _ => self.clean(cond, _then),
        };
        let notcond = self.solver().not_id(cond);
        let _clean_else = match self.tr_array[_else.0 as usize] {
            TRegex::Leaf(_) => _else,
            _ => self.clean(notcond, _else),
        };

        if _clean_then == _clean_else {
            return _clean_then;
        }
        // attempt left side flattening: ITE(.,ITE(a,ε,⊥),⊥) -> ITE(a,ε,⊥)
        match self.get_tregex(_clean_then) {
            TRegex::ITE(leftcond, _inner_then, leftg) if *leftg == _clean_else => {
                let _cond_then = *leftcond;
                let new_then = *_inner_then;
                let sand = self.solver().and_id(cond, _cond_then);
                return self.mk_ite(sand, new_then, _clean_else);
            }
            _ => {}
        }
        // attempt right side flattening:
        match self.get_tregex(_clean_else) {
            // ITE(a,ε,ITE(b,ε,⊥)) -> ITE([ab],ε,⊥)
            TRegex::ITE(_c2, _t2, _e2) if *_t2 == _clean_then => {
                let e2clone = *_e2;
                let new_cond = self.mb.solver.or_id(cond, *_c2);
                return self.mk_ite(new_cond, _clean_then, e2clone);
            }
            _ => {}
        }

        if _clean_then == TRegexId::BOT {
            let flip_cond = self.solver().not_id(cond);
            let cleaned_id = self.get_tregex_id(TRegex::ITE(flip_cond, _clean_else, _clean_then));
            return cleaned_id;
        }

        self.get_tregex_id(TRegex::ITE(cond, _clean_then, _clean_else))
    }

    fn clean(&mut self, beta: TSetId, tterm: TRegexId) -> TRegexId {
        if let Some(&cached) = self.clean_cache.get(&(beta, tterm)) {
            return cached;
        }
        let result = match *self.get_tregex(tterm) {
            TRegex::Leaf(_) => tterm,
            TRegex::ITE(alpha, _then_id, _else_id) => {
                let notalpha = self.mb.solver.not_id(alpha);
                if self.mb.solver.unsat_id(beta, alpha) {
                    // beta ⊆ ¬alpha, so beta ∧ ¬alpha = beta
                    self.clean(beta, _else_id)
                } else if self.unsat(beta, notalpha) {
                    // beta ⊆ alpha, so beta ∧ alpha = beta
                    self.clean(beta, _then_id)
                } else {
                    let tc = self.mb.solver.and_id(beta, alpha);
                    let ec = self.mb.solver.and_id(beta, notalpha);
                    let new_then = self.clean(tc, _then_id);
                    let new_else = self.clean(ec, _else_id);
                    self.mk_ite(alpha, new_then, new_else)
                }
            }
        };
        self.clean_cache.insert((beta, tterm), result);
        result
    }

    fn mk_unary(
        &mut self,
        term: TRegexId,
        apply: &mut impl FnMut(&mut RegexBuilder, NodeId) -> NodeId,
    ) -> TRegexId {
        match self.tr_array[term.0 as usize] {
            TRegex::Leaf(node_id) => {
                let applied = apply(self, node_id);
                self.mk_leaf(applied)
            }
            TRegex::ITE(c1, _then, _else) => {
                let _then1 = self.mk_unary(_then, apply);
                let _else1 = self.mk_unary(_else, apply);
                self.mk_ite(c1, _then1, _else1)
            }
        }
    }

    fn mk_binary_result(
        &mut self,
        left: TRegexId,
        right: TRegexId,
        apply: &mut impl FnMut(&mut RegexBuilder, NodeId, NodeId) -> Result<NodeId, AlgebraError>,
    ) -> Result<TRegexId, AlgebraError> {
        match self.tr_array[left.0 as usize] {
            TRegex::Leaf(left_node_id) => match self.tr_array[right.0 as usize] {
                TRegex::Leaf(right_node_id) => {
                    let applied = apply(self, left_node_id, right_node_id)?;
                    Ok(self.mk_leaf(applied))
                }
                TRegex::ITE(c2, _then, _else) => {
                    let then2 = self.mk_binary_result(left, _then, apply)?;
                    let else2 = self.mk_binary_result(left, _else, apply)?;
                    Ok(self.mk_ite(c2, then2, else2))
                }
            },
            TRegex::ITE(c1, _then1, _else1) => match self.tr_array[right.0 as usize] {
                TRegex::Leaf(_) => {
                    let then2 = self.mk_binary_result(_then1, right, apply)?;
                    let else2 = self.mk_binary_result(_else1, right, apply)?;
                    Ok(self.mk_ite(c1, then2, else2))
                }
                TRegex::ITE(c2, _then2, _else2) => {
                    if c1 == c2 {
                        let _then = self.mk_binary_result(_then1, _then2, apply)?;
                        let _else = self.mk_binary_result(_else1, _else2, apply)?;
                        return Ok(self.mk_ite(c1, _then, _else));
                    }
                    if c1.0 > c2.0 {
                        let _then = self.mk_binary_result(_then1, right, apply)?;
                        let _else = self.mk_binary_result(_else1, right, apply)?;
                        Ok(self.mk_ite(c1, _then, _else))
                    } else {
                        let _then = self.mk_binary_result(left, _then2, apply)?;
                        let _else = self.mk_binary_result(left, _else2, apply)?;
                        Ok(self.mk_ite(c2, _then, _else))
                    }
                }
            },
        }
    }

    fn mk_binary(
        &mut self,
        left: TRegexId,
        right: TRegexId,
        apply: &mut impl FnMut(&mut RegexBuilder, NodeId, NodeId) -> NodeId,
    ) -> TRegexId {
        self.mk_binary_memo.clear();
        self.mk_binary_inner(left, right, apply)
    }

    fn mk_binary_inner(
        &mut self,
        left: TRegexId,
        right: TRegexId,
        apply: &mut impl FnMut(&mut RegexBuilder, NodeId, NodeId) -> NodeId,
    ) -> TRegexId {
        if left == right {
            return self.mk_unary(left, &mut |b, n| apply(b, n, n));
        }
        if let Some(&cached) = self.mk_binary_memo.get(&(left, right)) {
            return cached;
        }
        let result = match self.tr_array[left.0 as usize] {
            TRegex::Leaf(left_node_id) => match self.tr_array[right.0 as usize] {
                TRegex::Leaf(right_node_id) => {
                    let applied = apply(self, left_node_id, right_node_id);
                    self.mk_leaf(applied)
                }
                TRegex::ITE(c2, _then, _else) => {
                    let then2 = self.mk_binary_inner(left, _then, apply);
                    let else2 = self.mk_binary_inner(left, _else, apply);
                    self.mk_ite(c2, then2, else2)
                }
            },
            TRegex::ITE(c1, _then1, _else1) => match self.tr_array[right.0 as usize] {
                TRegex::Leaf(_) => {
                    let then2 = self.mk_binary_inner(_then1, right, apply);
                    let else2 = self.mk_binary_inner(_else1, right, apply);
                    self.mk_ite(c1, then2, else2)
                }
                TRegex::ITE(c2, _then2, _else2) => {
                    if c1 == c2 {
                        let _then = self.mk_binary_inner(_then1, _then2, apply);
                        let _else = self.mk_binary_inner(_else1, _else2, apply);
                        self.mk_ite(c1, _then, _else)
                    } else if c1.0 > c2.0 {
                        let _then = self.mk_binary_inner(_then1, right, apply);
                        let _else = self.mk_binary_inner(_else1, right, apply);
                        self.mk_ite(c1, _then, _else)
                    } else {
                        let _then = self.mk_binary_inner(left, _then2, apply);
                        let _else = self.mk_binary_inner(left, _else2, apply);
                        self.mk_ite(c2, _then, _else)
                    }
                }
            },
        };
        self.mk_binary_memo.insert((left, right), result);
        result
    }

    pub fn get_nulls(
        &mut self,
        pending_rel: u32,
        mask: Nullability,
        acc: &mut BTreeSet<NullState>,
        node_id: NodeId,
    ) {
        debug_assert!(node_id != NodeId::MISSING);
        if !self.is_nullable(node_id, mask) {
            return;
        }
        match self.get_kind(node_id) {
            Kind::Pred => {}
            Kind::End => {
                if mask.has(Nullability::END) {
                    acc.insert(NullState::new(mask.and(Nullability::END), pending_rel));
                }
            }
            Kind::Begin => {
                if mask.has(Nullability::BEGIN) {
                    acc.insert(NullState::new(mask.and(Nullability::BEGIN), pending_rel));
                }
            }
            Kind::Concat => {
                let new_mask = self.nullability(node_id).and(mask);
                self.get_nulls(pending_rel, new_mask, acc, node_id.left(self));
                if self.is_nullable(node_id.left(self), mask) {
                    self.get_nulls(pending_rel, new_mask, acc, node_id.right(self));
                }
            }
            Kind::Union => {
                self.get_nulls(pending_rel, mask, acc, node_id.left(self));
                self.get_nulls(pending_rel, mask, acc, node_id.right(self));
            }
            Kind::Inter => {
                let new_mask = self.nullability(node_id).and(mask);
                self.get_nulls(pending_rel, new_mask, acc, node_id.left(self));
                self.get_nulls(pending_rel, new_mask, acc, node_id.right(self));
            }
            Kind::Star => {
                acc.insert(NullState::new(mask, pending_rel));
                self.get_nulls(pending_rel, mask, acc, node_id.left(self));
            }
            Kind::Compl => {
                if !self.is_nullable(node_id.left(self), mask) {
                    acc.insert(NullState::new(mask, 0));
                }
            }
            Kind::Lookbehind => {
                let new_mask = self.nullability(node_id).and(mask);
                self.get_nulls(pending_rel, new_mask, acc, node_id.left(self));
                if node_id.right(self) != NodeId::MISSING {
                    self.get_nulls(pending_rel, new_mask, acc, node_id.right(self));
                }
            }
            Kind::Lookahead => {
                let la_inner = self.get_lookahead_inner(node_id);
                if self.is_nullable(la_inner, mask) {
                    let rel = self.get_lookahead_rel(node_id);
                    if rel != u32::MAX {
                        self.get_nulls(pending_rel + rel, mask, acc, la_inner);
                    }
                    // tail only contributes when body is satisfied
                    let la_tail = self.get_lookahead_tail(node_id);
                    if la_tail != NodeId::MISSING {
                        self.get_nulls(pending_rel, mask, acc, la_tail);
                    }
                }
            }
            Kind::Counted => {
                let packed = self.get_extra(node_id);
                let best = packed >> 16;
                if best > 0 {
                    acc.insert(NullState::new(mask, pending_rel + best));
                }
            }
        }
    }

    pub fn contains_look(&mut self, node_id: NodeId) -> bool {
        self.get_meta_flags(node_id)
            .has(MetaFlags::CONTAINS_LOOKAROUND)
    }

    /// whether node contains `^`, `$`, `\A`, `\z` anchors.
    pub fn contains_anchors(&self, node_id: NodeId) -> bool {
        self.get_meta_flags(node_id)
            .has(MetaFlags::CONTAINS_ANCHORS)
    }

    pub fn is_infinite(&self, node_id: NodeId) -> bool {
        self.get_meta_flags(node_id).has(MetaFlags::INFINITE_LENGTH)
    }

    /// returns (min_length, max_length). max = u32::MAX means unbounded.
    pub fn get_min_max_length(&self, node_id: NodeId) -> (u32, u32) {
        if self.is_infinite(node_id) {
            if self.get_kind(node_id) == Kind::Inter {
                self.get_bounded_length(node_id)
            } else {
                (self.get_min_length_only(node_id), u32::MAX)
            }
        } else {
            self.get_bounded_length(node_id)
        }
    }

    fn get_bounded_length(&self, node_id: NodeId) -> (u32, u32) {
        if node_id == NodeId::EPS {
            return (0, 0);
        }
        match self.get_kind(node_id) {
            Kind::End | Kind::Begin => (0, 0),
            Kind::Pred => (1, 1),
            Kind::Concat => {
                let (lmin, lmax) = self.get_bounded_length(node_id.left(self));
                let (rmin, rmax) = self.get_bounded_length(node_id.right(self));
                (lmin + rmin, lmax.saturating_add(rmax))
            }
            Kind::Union => {
                let (lmin, lmax) = self.get_bounded_length(node_id.left(self));
                let (rmin, rmax) = self.get_bounded_length(node_id.right(self));
                (lmin.min(rmin), lmax.max(rmax))
            }
            Kind::Inter => {
                let (lmin, lmax) = self.get_min_max_length(node_id.left(self));
                let (rmin, rmax) = self.get_min_max_length(node_id.right(self));
                (lmin.max(rmin), lmax.min(rmax))
            }
            Kind::Lookahead => {
                let body = node_id.left(self);
                if self.is_infinite(body) {
                    return (0, u32::MAX);
                }
                let right = node_id.right(self);
                if right.is_missing() {
                    (0, 0)
                } else {
                    self.get_min_max_length(right)
                }
            }
            Kind::Counted => (0, 0),
            Kind::Star | Kind::Lookbehind | Kind::Compl => (0, u32::MAX),
        }
    }

    pub fn get_fixed_length(&self, node_id: NodeId) -> Option<u32> {
        match self.get_kind(node_id) {
            Kind::End | Kind::Begin => Some(0),
            Kind::Pred => Some(1),
            Kind::Concat => {
                let l = self.get_fixed_length(node_id.left(self))?;
                let r = self.get_fixed_length(node_id.right(self))?;
                Some(l + r)
            }
            Kind::Union => {
                let l = self.get_fixed_length(node_id.left(self))?;
                let r = self.get_fixed_length(node_id.right(self))?;
                if l == r {
                    Some(l)
                } else {
                    None
                }
            }
            Kind::Inter => {
                let l = self.get_fixed_length(node_id.left(self))?;
                let r = self.get_fixed_length(node_id.right(self))?;
                if l == r {
                    Some(l)
                } else {
                    None
                }
            }
            Kind::Lookahead => {
                let right = node_id.right(self);
                if right.is_missing() {
                    Some(0)
                } else {
                    self.get_fixed_length(right)
                }
            }
            Kind::Counted => Some(0),
            Kind::Star | Kind::Lookbehind | Kind::Compl => None,
        }
    }

    fn get_min_length_only(&self, node_id: NodeId) -> u32 {
        match self.get_kind(node_id) {
            Kind::End | Kind::Begin => 0,
            Kind::Pred => 1,
            Kind::Concat => {
                self.get_min_length_only(node_id.left(self))
                    + self.get_min_length_only(node_id.right(self))
            }
            Kind::Union => self
                .get_min_length_only(node_id.left(self))
                .min(self.get_min_length_only(node_id.right(self))),
            Kind::Inter => self
                .get_min_length_only(node_id.left(self))
                .max(self.get_min_length_only(node_id.right(self))),
            Kind::Star | Kind::Lookbehind | Kind::Lookahead | Kind::Counted => 0,
            Kind::Compl => {
                if self.nullability(node_id.left(self)) == Nullability::NEVER {
                    0
                } else {
                    1
                }
            }
        }
    }

    fn starts_with_ts(&self, node_id: NodeId) -> bool {
        if node_id == NodeId::TS {
            return true;
        }
        match self.get_kind(node_id) {
            Kind::Inter => {
                self.starts_with_ts(node_id.left(self)) && self.starts_with_ts(node_id.right(self))
            }
            Kind::Union => {
                self.starts_with_ts(node_id.left(self)) && self.starts_with_ts(node_id.right(self))
            }
            Kind::Concat => self.starts_with_ts(node_id.left(self)),
            _ => false,
        }
    }

    #[inline]
    pub(crate) fn ends_with_ts(&self, node_id: NodeId) -> bool {
        if self.get_kind(node_id) == Kind::Concat {
            self.ends_with_ts(node_id.right(self))
        } else {
            node_id == NodeId::TS
        }
    }

    pub(crate) fn is_nullable(&mut self, node_id: NodeId, mask: Nullability) -> bool {
        debug_assert!(node_id != NodeId::MISSING);
        self.nullability(node_id).0 & mask.0 != Nullability::NEVER.0
    }

    pub(crate) fn cache_der(
        &mut self,
        node_id: NodeId,
        result: TRegexId,
        mask: Nullability,
    ) -> TRegexId {
        if mask == Nullability::CENTER {
            self.tr_der_center[node_id.0 as usize] = result
        } else {
            self.tr_der_begin[node_id.0 as usize] = result
        };
        result
    }

    pub(crate) fn try_cached_der(
        &mut self,
        node_id: NodeId,
        mask: Nullability,
    ) -> Option<TRegexId> {
        let cache = if mask == Nullability::CENTER {
            &mut self.tr_der_center
        } else {
            &mut self.tr_der_begin
        };
        match cache.get(node_id.0 as usize) {
            Some(&TRegexId::MISSING) => {}
            Some(&result) => {
                return Some(result);
            }
            None => {
                cache.resize(node_id.0 as usize + 1, TRegexId::MISSING);
            }
        }
        None
    }

    pub fn transition_term(&mut self, der: TRegexId, set: TSetId) -> NodeId {
        let mut term = self.get_tregex(der);
        loop {
            match *term {
                TRegex::Leaf(node_id) => return node_id,
                TRegex::ITE(cond, _then, _else) => {
                    if self.solver().is_sat_id(set, cond) {
                        term = self.get_tregex(_then);
                    } else {
                        term = self.get_tregex(_else);
                    }
                }
            }
        }
    }

    pub fn der(&mut self, node_id: NodeId, mask: Nullability) -> Result<TRegexId, AlgebraError> {
        debug_assert!(mask != Nullability::ALWAYS, "attempting to derive w always");
        debug_assert!(
            node_id != NodeId::MISSING,
            "attempting to derive missing node"
        );
        if let Some(result) = self.try_cached_der(node_id, mask) {
            return Ok(result);
        }

        let result = match node_id.kind(self) {
            Kind::Compl => {
                let leftd = node_id.left(self).der(self, mask)?;
                self.mk_unary(leftd, &mut (|b, v| b.mk_compl(v)))
            }
            Kind::Inter => {
                let leftd = node_id.left(self).der(self, mask)?;
                let rightd = node_id.right(self).der(self, mask)?;
                {
                    let this = &mut *self;
                    this.mk_binary(
                        leftd,
                        rightd,
                        &mut (|b, left, right| b.mk_inter(left, right)),
                    )
                }
            }
            Kind::Union => {
                let leftd = self.der(node_id.left(self), mask)?;
                let rightd = self.der(node_id.right(self), mask)?;
                {
                    let this = &mut *self;
                    this.mk_binary(
                        leftd,
                        rightd,
                        &mut (|b, left, right| b.mk_union(left, right)),
                    )
                }
            }
            Kind::Concat => {
                let head = node_id.left(self);
                let tail = node_id.right(self);
                let tail_leaf = self.mk_leaf(tail);
                let head_der = self.der(head, mask)?;

                if self.is_nullable(head, mask) {
                    let rightd = self.der(tail, mask)?;
                    let ldr = {
                        let this = &mut *self;
                        this.mk_binary(
                            head_der,
                            tail_leaf,
                            &mut (|b, left, right| b.mk_concat(left, right)),
                        )
                    };
                    {
                        let this = &mut *self;
                        this.mk_binary(ldr, rightd, &mut (|b, left, right| b.mk_union(left, right)))
                    }
                } else {
                    let this = &mut *self;
                    this.mk_binary(
                        head_der,
                        tail_leaf,
                        &mut (|b, left, right| b.mk_concat(left, right)),
                    )
                }
            }
            Kind::Star => {
                if node_id == NodeId::EPS {
                    TRegexId::BOT
                } else {
                    let left = node_id.left(self);
                    let r_decr_leaf = self.mk_leaf(node_id);
                    let r_der = self.der(left, mask)?;
                    let this = &mut *self;
                    this.mk_binary(
                        r_der,
                        r_decr_leaf,
                        &mut (|b, left, right| b.mk_concat(left, right)),
                    )
                }
            }
            Kind::Lookbehind => {
                let lb_prev_der = {
                    let lb_prev = self.get_lookbehind_prev(node_id);
                    if lb_prev == NodeId::MISSING {
                        TRegexId::MISSING
                    } else {
                        self.der(lb_prev, mask)?
                    }
                };
                let lb_inner = self.get_lookbehind_inner(node_id);
                let lb_inner_der = self.der(lb_inner, mask)?;
                {
                    let this = &mut *self;
                    this.mk_binary_result(
                        lb_inner_der,
                        lb_prev_der,
                        &mut (|b, left, right| b.mk_lookbehind_internal(left, right)),
                    )?
                }
            }
            Kind::Lookahead => {
                let la_tail = self.get_lookahead_tail(node_id);
                let la_body = node_id.left(self);
                let rel = self.get_lookahead_rel(node_id);

                if self.is_nullable(la_body, mask) {
                    // nullabilty is taken once, just keep the body
                    let right = node_id.right(self).missing_to_eps();
                    let rder = self.der(right, mask).clone();
                    return rder;
                }

                if rel == u32::MAX {
                    let la_body_der = self.der(la_body, mask)?;
                    if la_tail.is_kind(self, Kind::Pred) {
                        let transitioned =
                            self.transition_term(la_body_der, la_tail.pred_tset(self));
                        let new_la = self.mk_lookahead_internal(transitioned, NodeId::MISSING, 0);
                        let concated = self.mk_concat(la_tail, new_la);
                        return self.der(concated, mask);
                    }
                    if la_tail.is_kind(self, Kind::Concat) && la_tail.left(self).is_pred(self) {
                        let left = la_tail.left(self);
                        let tset = left.pred_tset(self);
                        let transitioned = self.transition_term(la_body_der, tset);
                        let new_la = self.mk_lookahead_internal(transitioned, NodeId::MISSING, 0);
                        let tail_right = la_tail.right(self);
                        let concated = self.mk_concat(new_la, tail_right);
                        let concated = self.mk_concat(left, concated);
                        return self.der(concated, mask);
                    }
                }

                if la_tail != NodeId::MISSING && self.is_nullable(la_tail, mask) {
                    let nulls_mask = self.extract_nulls_mask(la_tail, mask);
                    let concated = self.mk_concat(la_body, nulls_mask);
                    let concated_look = self.mk_lookahead_internal(concated, NodeId::MISSING, 0);
                    let non_nulled = self.mk_non_nullable_safe(la_tail);
                    let new_look = self.mk_lookahead_internal(la_body, non_nulled, rel);
                    let new_union = self.mk_union(concated_look, new_look);
                    return self.der(new_union, mask);
                }

                let la_tail_der = if la_tail == NodeId::MISSING {
                    TRegexId::MISSING
                } else {
                    if self.is_nullable(la_tail, mask) {
                        let nulls_mask = self.extract_nulls_mask(la_tail, mask);
                        let nulls_la = self.mk_lookahead_internal(nulls_mask, NodeId::MISSING, 0);
                        let la_union = self.mk_union(la_tail, nulls_la);
                        self.der(la_union, mask)?
                    } else {
                        self.der(la_tail, mask)?
                    }
                };

                let la_body_der = self.der(la_body, mask)?;

                if rel != u32::MAX && rel > self.lookahead_context_max {
                    return Err(AlgebraError::AnchorLimit);
                }

                let la = {
                    let this = &mut *self;
                    let rel = helpers::incr_rel(rel);
                    this.mk_binary(
                        la_body_der,
                        la_tail_der,
                        &mut (|b, left, right| b.mk_lookahead_internal(left, right, rel)),
                    )
                };

                if rel != u32::MAX
                    && la_tail_der != TRegexId::MISSING
                    && self.is_nullable(la_tail, mask)
                {
                    let look_only = {
                        let this = &mut *self;
                        let right = TRegexId::MISSING;
                        let rel = helpers::incr_rel(rel);
                        this.mk_binary(
                            la_body_der,
                            right,
                            &mut (|b, left, right| b.mk_lookahead_internal(left, right, rel)),
                        )
                    };
                    {
                        let this = &mut *self;
                        this.mk_binary(
                            look_only,
                            la,
                            &mut (|b, left, right| b.mk_union(left, right)),
                        )
                    }
                } else {
                    la
                }
            }
            Kind::Counted => {
                let body = node_id.left(self);
                let chain = node_id.right(self);
                let packed = self.get_extra(node_id);
                let step = (packed & 0xFFFF) as u16;
                let best = (packed >> 16) as u16;

                let mid_best = if self.is_nullable(body, mask) && step >= best {
                    step
                } else {
                    best
                };

                let body_der = self.der(body, mask)?;
                let new_step = step.saturating_add(1);
                self.mk_unary(body_der, &mut |b, new_body| {
                    let final_best = if b.is_nullable(new_body, mask) && new_step >= mid_best {
                        new_step
                    } else {
                        mid_best
                    };
                    let packed = (final_best as u32) << 16 | new_step as u32;
                    b.mk_counted(new_body, chain, packed)
                })
            }
            Kind::Begin | Kind::End => TRegexId::BOT,
            Kind::Pred => {
                let psi = node_id.pred_tset(self);
                if psi == TSetId::EMPTY {
                    TRegexId::BOT
                } else {
                    self.mk_ite(psi, TRegexId::EPS, TRegexId::BOT)
                }
            }
        };

        // println!("{} {}", node_id.0, self.pp(node_id));
        // println!("node: {} (total: {})", node_id.0, self.num_created);

        self.cache_der(node_id, result, mask);
        Ok(result)
    }

    fn init_metadata(&mut self, node_id: NodeId, meta_id: MetadataId) {
        debug_assert!(meta_id != MetadataId::MISSING);
        match self.metadata.get_mut(node_id.0 as usize) {
            Some(v) => *v = meta_id,
            None => {
                self.metadata
                    .resize(node_id.0 as usize + 1, MetadataId::MISSING);
                self.metadata[node_id.0 as usize] = meta_id;
            }
        }
    }

    fn init_reversed(&mut self, node_id: NodeId, reversed_id: NodeId) {
        debug_assert!(reversed_id != NodeId::MISSING);
        match self.reversed.get_mut(node_id.0 as usize) {
            Some(v) => *v = reversed_id,
            None => {
                self.reversed
                    .resize(node_id.0 as usize + 1, NodeId::MISSING);
                self.reversed[node_id.0 as usize] = reversed_id;
            }
        }
    }

    fn init(&mut self, inst: NodeKey) -> NodeId {
        self.num_created += 1;
        let node_id = NodeId(self.num_created);
        self.index.insert(inst.clone(), node_id);
        match inst.kind {
            Kind::Pred => {
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::ZERO,
                    nulls: NullsId::EMPTY,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Begin => {
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::with_nullability(
                        Nullability::BEGIN,
                        MetaFlags::CONTAINS_ANCHORS,
                    ),
                    nulls: NullsId::BEGIN0,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::End => {
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::with_nullability(
                        Nullability::END,
                        MetaFlags::CONTAINS_ANCHORS,
                    ),
                    nulls: NullsId::END0,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Inter => {
                let m1 = self.get_node_meta_id(inst.left);
                let m2 = self.get_node_meta_id(inst.right);
                let meta_id = {
                    let left_nulls = self.mb.get_meta_ref(m1).nulls;
                    let mask_l = inst.left.nullability(self);
                    let mask_r = inst.right.nullability(self);
                    let right_nulls = self.mb.get_meta_ref(m2).nulls;
                    let mut nulls = self.mb.nb.and_id(left_nulls, right_nulls);
                    nulls = self.mb.nb.and_mask(nulls, mask_l);
                    nulls = self.mb.nb.and_mask(nulls, mask_r);
                    let new_meta = Metadata {
                        flags: self.mb.flags_inter(m1, m2),
                        nulls,
                    };
                    self.mb.get_meta_id(new_meta)
                };
                self.init_metadata(node_id, meta_id);
            }
            Kind::Union => {
                let m1 = self.get_node_meta_id(inst.left);
                let m2 = self.get_node_meta_id(inst.right);
                let meta_id = {
                    let left_nulls = self.mb.get_meta_ref(m1).nulls;
                    let right_nulls = self.mb.get_meta_ref(m2).nulls;
                    let nulls = self.mb.nb.or_id(left_nulls, right_nulls);
                    let new_meta = Metadata {
                        flags: self.mb.flags_union(m1, m2),
                        nulls,
                    };
                    self.mb.get_meta_id(new_meta)
                };
                self.init_metadata(node_id, meta_id);
            }
            Kind::Concat => {
                let flags = self.mb.flags_concat(
                    self.get_node_meta_id(inst.left),
                    self.get_node_meta_id(inst.right),
                );

                let right_nullability = inst.right.nullability(self);
                let left_nullability = inst.left.nullability(self);
                let nulls_left = self.get_nulls_id(inst.left);
                let nulls_right = self.get_nulls_id(inst.right);
                let mut nulls = self.mb.nb.or_id(nulls_left, nulls_right);
                let mask = right_nullability.and(left_nullability);
                nulls = self.mb.nb.and_mask(nulls, mask);

                let new_id = self.mb.get_meta_id(Metadata { flags, nulls });
                self.init_metadata(node_id, new_id);
            }
            Kind::Star => {
                let left_nulls = self.get_nulls_id(inst.left);
                let nulls = self.mb.nb.or_id(left_nulls, NullsId::ALWAYS0);
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: self
                        .mb
                        .flags_star(self.get_node_meta_id(inst.left), inst.left),
                    nulls,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Compl => {
                let nulls = self.mb.nb.not_id(self.get_nulls_id(inst.left));
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: self.mb.flags_compl(self.get_node_meta_id(inst.left)),
                    nulls,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Lookbehind => {
                let mut null = self.get_meta_flags(inst.left).nullability();
                let mut contains_flags = self.get_flags_contains(inst.left);
                if !inst.right.is_missing() {
                    null = null.and(self.get_meta_flags(inst.right).nullability());
                    contains_flags = contains_flags.or(self.get_flags_contains(inst.right));
                }

                let left_nulls = self.get_nulls_id(inst.left);
                let right_nulls = self.get_nulls_id(inst.right);
                let nulls = self.mb.nb.or_id(left_nulls, right_nulls);
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::with_nullability(
                        null,
                        contains_flags.or(MetaFlags::CONTAINS_LOOKAROUND),
                    ),
                    nulls,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Lookahead => {
                let mut nulls = self.get_nulls_id(inst.left);
                let left_nullability = inst.left.nullability(self);
                let nulls_right = self.get_nulls_id_w_mask(inst.right, left_nullability);
                nulls = self.mb.nb.or_id(nulls, nulls_right);
                nulls = self.mb.nb.add_rel(nulls, inst.extra);

                let la_inner = inst.left;
                let la_tail = inst.right;
                let null = self
                    .get_meta_flags(la_inner)
                    .nullability()
                    .and(self.get_meta_flags(la_tail.missing_to_eps()).nullability());
                let contains_flags = self
                    .get_flags_contains(la_inner)
                    .or(self.get_flags_contains(la_tail));

                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::with_nullability(
                        null,
                        contains_flags.or(MetaFlags::CONTAINS_LOOKAROUND),
                    ),
                    nulls,
                });
                self.init_metadata(node_id, meta_id);
            }
            Kind::Counted => {
                let best = inst.extra >> 16;
                let (null, nulls) = if best > 0 {
                    let mut ns = BTreeSet::new();
                    ns.insert(NullState::new(Nullability::CENTER, best));
                    (Nullability::CENTER, self.mb.nb.get_id(ns))
                } else {
                    (Nullability::NEVER, NullsId::EMPTY)
                };
                let meta_id = self.mb.get_meta_id(Metadata {
                    flags: MetaFlags::with_nullability(null, MetaFlags::ZERO),
                    nulls,
                });
                self.init_metadata(node_id, meta_id);
            }
        }

        self.array.push(inst);

        if let Some(rw) = self.post_init_simplify(node_id) {
            self.override_as(node_id, rw)
        } else {
            node_id
        }
    }

    fn post_init_simplify(&mut self, node_id: NodeId) -> Option<NodeId> {
        match self.get_kind(node_id) {
            Kind::Concat => {
                let lhs = node_id.left(self);
                let rhs = node_id.right(self);
                if lhs.is_pred_star(self).is_some() {
                    if let Some(opttail) = rhs.is_opt_v(self) {
                        if let Some(true) = self.subsumes(lhs, opttail) {
                            // return self.override_as(node_id, lhs);
                            return Some(lhs);
                        }
                    }
                }
            }
            Kind::Union => {
                let lhs = node_id.left(self);
                let rhs = node_id.right(self);
                let mut subsumed = false;
                rhs.iter_union_while(self, &mut |b, branch| {
                    if b.nullable_subsumes(branch, lhs) {
                        subsumed = true;
                    }
                    !subsumed
                });
                if subsumed {
                    return Some(rhs);
                }
                if lhs != rhs && self.union_branches_subset(lhs, rhs) {
                    return Some(rhs);
                }
            }
            _ => {}
        }

        None
    }

    /// checks if every branch of `lhs` (as a union tree) appears in `rhs` (as a union tree).
    fn union_branches_subset(&self, lhs: NodeId, rhs: NodeId) -> bool {
        if self.get_kind(lhs) != Kind::Union {
            return false; // single branch already checked by nullable_subsumes
        }
        let mut rhs_branches = Vec::new();
        let mut curr = rhs;
        while self.get_kind(curr) == Kind::Union {
            rhs_branches.push(self.get_left(curr));
            curr = self.get_right(curr);
        }
        rhs_branches.push(curr);

        curr = lhs;
        while self.get_kind(curr) == Kind::Union {
            if !rhs_branches.contains(&self.get_left(curr)) {
                return false;
            }
            curr = self.get_right(curr);
        }
        rhs_branches.contains(&curr)
    }

    /// checks if `node` structurally subsumes `target` via nullable concat chains and union branches
    fn nullable_subsumes(&self, node: NodeId, target: NodeId) -> bool {
        if node == target {
            return true;
        }
        match self.get_kind(node) {
            Kind::Union => {
                self.nullable_subsumes(self.get_left(node), target)
                    || self.nullable_subsumes(self.get_right(node), target)
            }
            Kind::Concat if self.is_always_nullable(self.get_left(node)) => {
                self.nullable_subsumes(self.get_right(node), target)
            }
            _ => false,
        }
    }

    pub fn num_nodes(&self) -> u32 {
        self.num_created
    }
    fn get_node_id(&mut self, inst: NodeKey) -> NodeId {
        match self.index.get(&inst) {
            Some(&id) => id,
            None => self.init(inst),
        }
    }
    #[inline]
    fn key_is_created(&self, inst: &NodeKey) -> Option<&NodeId> {
        self.index.get(inst)
    }

    fn init_as(&mut self, key: NodeKey, subsumed: NodeId) -> NodeId {
        self.index.insert(key, subsumed);
        subsumed
    }

    pub(crate) fn override_as(&mut self, key: NodeId, subsumed: NodeId) -> NodeId {
        let key = &self.array[key.0 as usize];
        let entry = self.index.get_mut(key).unwrap();
        *entry = subsumed;
        subsumed
    }

    #[inline]
    pub(crate) fn get_left(&self, node_id: NodeId) -> NodeId {
        self.array[node_id.0 as usize].left
    }

    #[inline]
    pub(crate) fn get_right(&self, node_id: NodeId) -> NodeId {
        self.array[node_id.0 as usize].right
    }

    #[inline]
    pub fn get_extra(&self, node_id: NodeId) -> u32 {
        self.array[node_id.0 as usize].extra
    }

    #[inline]
    pub(crate) fn get_concat_end(&self, node_id: NodeId) -> NodeId {
        debug_assert!(self.get_kind(node_id) == Kind::Concat);
        let mut curr = node_id;
        while self.get_kind(curr) == Kind::Concat {
            curr = curr.right(self);
        }
        curr
    }

    fn has_trailing_la(&self, node: NodeId) -> bool {
        let end = match self.get_kind(node) {
            Kind::Concat => self.get_concat_end(node),
            Kind::Lookahead => node,
            _ => return false,
        };
        self.get_kind(end) == Kind::Lookahead && end.right(self).is_missing()
    }

    fn strip_trailing_la(&mut self, node: NodeId) -> (NodeId, NodeId) {
        if self.get_kind(node) == Kind::Lookahead {
            return (NodeId::EPS, node);
        }
        debug_assert!(self.get_kind(node) == Kind::Concat);
        let right = node.right(self);
        if self.get_kind(right) != Kind::Concat {
            return (node.left(self), right);
        }
        let (stripped, la) = self.strip_trailing_la(right);
        (self.mk_concat(node.left(self), stripped), la)
    }
    #[inline]
    pub(crate) fn get_lookahead_inner(&self, lookahead_node_id: NodeId) -> NodeId {
        debug_assert!(matches!(
            self.get_kind(lookahead_node_id),
            Kind::Lookahead | Kind::Counted
        ));
        lookahead_node_id.left(self)
    }
    #[inline]
    pub(crate) fn get_lookahead_tail(&self, lookahead_node_id: NodeId) -> NodeId {
        debug_assert!(self.get_kind(lookahead_node_id) == Kind::Lookahead);
        lookahead_node_id.right(self)
    }
    #[inline]
    pub(crate) fn get_lookahead_rel(&self, lookahead_node_id: NodeId) -> u32 {
        debug_assert!(
            matches!(
                self.get_kind(lookahead_node_id),
                Kind::Lookahead | Kind::Counted
            ),
            "not lookahead/counted: {:?}",
            self.pp(lookahead_node_id)
        );
        self.get_extra(lookahead_node_id)
    }
    #[inline]
    pub fn get_lookbehind_inner(&self, lookbehind_node_id: NodeId) -> NodeId {
        debug_assert!(self.get_kind(lookbehind_node_id) == Kind::Lookbehind);
        lookbehind_node_id.left(self)
    }
    #[inline]
    pub(crate) fn get_lookbehind_prev(&self, lookbehind_node_id: NodeId) -> NodeId {
        debug_assert!(self.get_kind(lookbehind_node_id) == Kind::Lookbehind);
        lookbehind_node_id.right(self)
    }

    #[inline]
    pub fn get_kind(&self, node_id: NodeId) -> Kind {
        debug_assert!(
            self.array.len() > node_id.0 as usize,
            "array len: {:?}",
            node_id
        );
        self.array[node_id.0 as usize].kind
    }

    #[inline]
    pub fn get_node(&self, node_id: NodeId) -> &NodeKey {
        &self.array[node_id.0 as usize]
    }

    #[inline]
    fn get_node_meta_id(&self, node_id: NodeId) -> MetadataId {
        self.metadata[node_id.0 as usize]
    }

    #[inline]
    fn get_meta(&self, node_id: NodeId) -> &Metadata {
        debug_assert!(node_id.0 != u32::MAX);
        let meta_id = self.get_node_meta_id(node_id);
        debug_assert!(meta_id != MetadataId::MISSING);
        &self.mb.array[meta_id.0 as usize]
    }

    #[inline]
    pub fn get_nulls_id(&self, node_id: NodeId) -> NullsId {
        if node_id == NodeId::MISSING {
            NullsId::EMPTY
        } else {
            self.get_meta(node_id).nulls
        }
    }

    pub fn nulls_as_vecs(&self) -> Vec<Vec<NullState>> {
        self.mb
            .nb
            .array
            .iter()
            .map(|set| set.iter().cloned().collect())
            .collect()
    }

    pub fn nulls_count(&self) -> usize {
        self.mb.nb.array.len()
    }

    pub fn nulls_entry_vec(&self, id: u32) -> Vec<NullState> {
        self.mb.nb.array[id as usize].iter().cloned().collect()
    }

    #[inline]
    pub(crate) fn get_nulls_id_w_mask(&mut self, node_id: NodeId, mask: Nullability) -> NullsId {
        if node_id == NodeId::MISSING {
            NullsId::EMPTY
        } else {
            let nulls = self.get_meta(node_id).nulls;
            self.mb.nb.and_mask(nulls, mask)
        }
    }

    #[inline]
    pub(crate) fn get_meta_flags(&self, node_id: NodeId) -> MetaFlags {
        let meta_id = self.get_node_meta_id(node_id);
        let meta = &self.mb.array[meta_id.0 as usize];
        meta.flags
    }

    #[inline]
    pub(crate) fn get_only_nullability(&self, node_id: NodeId) -> Nullability {
        self.get_meta(node_id).flags.nullability()
    }

    #[inline]
    pub(crate) fn get_flags_contains(&self, node_id: NodeId) -> MetaFlags {
        let meta_id = self.get_node_meta_id(node_id);
        let meta = &self.mb.array[meta_id.0 as usize];
        meta.flags.all_contains_flags()
    }

    pub fn strip_lb(&mut self, node_id: NodeId) -> Result<NodeId, AlgebraError> {
        if self.get_kind(node_id) == Kind::Concat && node_id.left(self) == NodeId::BEGIN {
            return self.strip_lb(node_id.right(self));
        }
        self.strip_lb_inner(node_id)
    }

    fn strip_lb_inner(&mut self, node_id: NodeId) -> Result<NodeId, AlgebraError> {
        if !self.contains_look(node_id) {
            return Ok(node_id);
        }
        if self.get_kind(node_id) == Kind::Concat
            && self.get_kind(node_id.left(self)) == Kind::Lookbehind
        {
            let lb = node_id.left(self);
            let prev = self.get_lookbehind_prev(lb);
            let tail = self.strip_lb_inner(node_id.right(self))?;
            if prev != NodeId::MISSING {
                let stripped_prev = self.strip_lb_inner(prev)?;
                return Ok(self.mk_concat(stripped_prev, tail));
            }
            return Ok(tail);
        }
        if self.get_kind(node_id) == Kind::Inter {
            let left = self.strip_lb_inner(node_id.left(self))?;
            let right = self.strip_lb_inner(node_id.right(self))?;
            return Ok(self.mk_inter(left, right));
        }
        if self.get_kind(node_id) == Kind::Union {
            let left = self.strip_lb_inner(node_id.left(self))?;
            let right = self.strip_lb_inner(node_id.right(self))?;
            return Ok(self.mk_union(left, right));
        }
        match self.get_kind(node_id) {
            Kind::Lookbehind => {
                let prev = self.get_lookbehind_prev(node_id);
                if prev != NodeId::MISSING {
                    self.strip_lb_inner(prev)
                } else {
                    Ok(NodeId::EPS)
                }
            }
            Kind::Lookahead if self.get_lookahead_tail(node_id).is_missing() => {
                Err(AlgebraError::UnsupportedPattern)
            }
            _ => Ok(node_id),
        }
    }

    // for prefix purposes we prune any \A leading paths
    pub fn nonbegins(&mut self, node_id: NodeId) -> NodeId {
        if !self.contains_anchors(node_id) {
            return node_id;
        }
        match self.get_kind(node_id) {
            Kind::Begin => NodeId::BOT,
            Kind::Concat => {
                let left = self.nonbegins(node_id.left(self));
                if left == NodeId::BOT {
                    return NodeId::BOT;
                }
                self.mk_concat(left, node_id.right(self))
            }
            Kind::Union => {
                let left = self.nonbegins(node_id.left(self));
                let right = self.nonbegins(node_id.right(self));
                self.mk_union(left, right)
            }
            _ => node_id,
        }
    }

    pub fn strip_prefix_safe(&mut self, node_id: NodeId) -> NodeId {
        match self.get_kind(node_id) {
            Kind::Concat => {
                let head = node_id.left(self);
                match self.get_kind(head) {
                    _ if self.any_nonbegin_nullable(head) => {
                        self.strip_prefix_safe(node_id.right(self))
                    }
                    _ => node_id,
                }
            }
            _ => node_id,
        }
    }
    pub fn prune_begin(&mut self, node_id: NodeId) -> NodeId {
        match self.get_kind(node_id) {
            Kind::Begin => NodeId::BOT,
            Kind::Concat => {
                let head = self.prune_begin(node_id.left(self));
                let tail = self.prune_begin(node_id.right(self));
                self.mk_concat(head, tail)
            }
            Kind::Lookbehind => {
                if !node_id.right(self).is_missing() {
                    return node_id;
                }
                let head = self.prune_begin(node_id.left(self));
                head
            }
            Kind::Union => {
                let left = self.prune_begin(node_id.left(self));
                let right = self.prune_begin(node_id.right(self));
                self.mk_union(left, right)
            }
            _ => node_id,
        }
    }

    pub fn normalize_rev(&mut self, node_id: NodeId) -> Result<NodeId, AlgebraError> {
        if !self.contains_look(node_id) {
            return Ok(node_id);
        }
        if self.get_kind(node_id) == Kind::Concat
            && self.get_kind(node_id.left(self)) == Kind::Lookbehind
        {
            let left = node_id.left(self);
            let ll = left.left(self).missing_to_eps();
            let lr = left.right(self).missing_to_eps();
            let new_l = self.mk_concat(ll, lr);
            let new = self.mk_concat(new_l, node_id.right(self));
            return Ok(new);
        }
        if self.get_kind(node_id) == Kind::Inter {
            let left = self.normalize_rev(node_id.left(self))?;
            let right = self.normalize_rev(node_id.right(self))?;
            return Ok(self.mk_inter(left, right));
        }
        if self.get_kind(node_id) == Kind::Union {
            let left = self.normalize_rev(node_id.left(self))?;
            let right = self.normalize_rev(node_id.right(self))?;
            return Ok(self.mk_union(left, right));
        }
        match self.get_kind(node_id) {
            Kind::Lookbehind => Err(AlgebraError::UnsupportedPattern),
            Kind::Lookahead if self.get_lookahead_tail(node_id).is_missing() => {
                Err(AlgebraError::UnsupportedPattern)
            }
            _ => Ok(node_id),
        }
    }

    pub fn reverse(&mut self, node_id: NodeId) -> Result<NodeId, AlgebraError> {
        debug_assert!(node_id != NodeId::MISSING);
        if let Some(rev) = self.reversed.get(node_id.0 as usize) {
            if *rev != NodeId::MISSING {
                return Ok(*rev);
            }
        }
        let rw = match self.get_kind(node_id) {
            Kind::End => NodeId::BEGIN,
            Kind::Begin => NodeId::END,
            Kind::Pred => node_id,
            Kind::Concat => {
                let left = self.reverse(node_id.left(self))?;
                let right = self.reverse(node_id.right(self))?;
                self.mk_concat(right, left)
            }
            Kind::Union => {
                let left = self.reverse(node_id.left(self))?;
                let right = self.reverse(node_id.right(self))?;
                self.mk_union(left, right)
            }
            Kind::Inter => {
                let left = self.reverse(node_id.left(self))?;
                let right = self.reverse(node_id.right(self))?;
                self.mk_inter(left, right)
            }
            Kind::Star => {
                let body = self.reverse(node_id.left(self))?;
                self.mk_star(body)
            }
            Kind::Compl => {
                if self.contains_look(node_id.left(self)) {
                    return Err(AlgebraError::UnsupportedPattern);
                }
                let body = self.reverse(node_id.left(self))?;
                self.mk_compl(body)
            }
            Kind::Lookbehind => {
                let prev = self.get_lookbehind_prev(node_id);
                let inner_id = self.get_lookbehind_inner(node_id);
                let rev_inner = self.reverse(inner_id)?;
                let rev_prev = if prev != NodeId::MISSING {
                    self.reverse(prev)?
                } else {
                    NodeId::MISSING
                };
                self.mk_lookahead(rev_inner, rev_prev, 0)
            }
            Kind::Lookahead => {
                let rel = self.get_lookahead_rel(node_id);
                if rel == u32::MAX {
                    // rel MAX holds no nullability - rewrite to intersection
                    let lbody = self.get_lookahead_inner(node_id);
                    let ltail = self.get_lookahead_tail(node_id).missing_to_eps();
                    let lbody_ts = self.mk_concat(lbody, NodeId::TS);
                    let ltail_ts = self.mk_concat(ltail, NodeId::TS);
                    let as_inter = self.mk_inter(lbody_ts, ltail_ts);
                    let rev = self.reverse(as_inter)?;
                    self.init_reversed(node_id, rev);
                    return Ok(rev);
                }
                if rel != 0 {
                    return Err(AlgebraError::UnsupportedPattern);
                }
                let tail_node = self.get_lookahead_tail(node_id);
                let rev_tail = if tail_node != NodeId::MISSING {
                    self.reverse(tail_node)?
                } else {
                    NodeId::MISSING
                };
                let inner_id = self.get_lookahead_inner(node_id);
                let rev_inner = self.reverse(inner_id)?;
                self.mk_lookbehind(rev_inner, rev_tail)
            }
            Kind::Counted => {
                return Err(AlgebraError::UnsupportedPattern);
            }
        };
        self.init_reversed(node_id, rw);
        Ok(rw)
    }

    pub(crate) fn mk_pred(&mut self, pred: TSetId) -> NodeId {
        let node = NodeKey {
            kind: Kind::Pred,
            left: NodeId::MISSING,
            right: NodeId::MISSING,
            extra: pred.0,
        };
        self.get_node_id(node)
    }

    pub fn mk_compl(&mut self, body: NodeId) -> NodeId {
        let key = NodeKey {
            kind: Kind::Compl,
            left: body,
            right: NodeId::MISSING,
            extra: u32::MAX,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }
        if body == NodeId::BOT {
            return NodeId::TS;
        }
        if body == NodeId::TS {
            return NodeId::BOT;
        }

        if let Some(contains_body) = body.is_contains(self) {
            if contains_body.is_pred(self) {
                let pred = contains_body.pred_tset(self);
                let notpred = self.mk_pred_not(pred);
                let node = self.mk_star(notpred);
                return self.init_as(key, node);
            } else if contains_body == NodeId::END {
                return self.init_as(key, NodeId::BOT);
            }
        };

        if self.get_kind(body) == Kind::Compl {
            return self.get_node(body).left;
        }

        self.get_node_id(key)
    }

    pub(crate) fn extract_nulls_mask(&mut self, body: NodeId, mask: Nullability) -> NodeId {
        let nid = self.get_nulls_id(body);
        let nref = self.mb.nb.get_set_ref(nid).clone();
        let mut futures = NodeId::BOT;
        for n in nref.iter() {
            if !n.is_mask_nullable(mask) {
                continue;
            }

            let eff = if n.rel == 0 {
                NodeId::EPS
            } else {
                self.mk_lookahead_internal(NodeId::EPS, NodeId::MISSING, n.rel)
            };
            futures = self.mk_union(futures, eff)
        }
        futures
    }

    fn attempt_rw_concat_2(&mut self, head: NodeId, tail: NodeId) -> Option<NodeId> {
        if cfg!(feature = "norewrite") {
            return None;
        }

        if self.get_kind(tail) == Kind::Lookbehind {
            let lbleft = self.mk_concat(head, self.get_lookbehind_prev(tail).missing_to_eps());
            return self
                .mk_lookbehind_internal(self.get_lookbehind_inner(tail).missing_to_eps(), lbleft)
                .ok();
        }
        if self.get_kind(head) == Kind::Lookahead {
            let la_tail = self.get_lookahead_tail(head);
            let new_la_tail = self.mk_concat(la_tail.missing_to_eps(), tail);
            if new_la_tail.is_center_nullable(self) {
                let non_null_tail = self.mk_non_nullable_safe(new_la_tail);
                if non_null_tail == NodeId::BOT {
                    return None;
                }
                let la_body = self.get_lookahead_inner(head);
                return Some(self.mk_lookahead_internal(la_body, non_null_tail, u32::MAX));
            }
            let la_body = self.get_lookahead_inner(head);
            let la_rel = self.get_lookahead_rel(head);
            let la_rel = if new_la_tail.is_kind(self, Kind::Lookahead) {
                let tail_rel = self.get_lookahead_rel(new_la_tail);
                tail_rel + la_rel
            } else {
                u32::MAX
            };

            return Some(self.mk_lookahead_internal(la_body, new_la_tail, la_rel));
        }

        if head.is_kind(self, Kind::End) && tail == NodeId::TS {
            return Some(head);
        }
        if head == NodeId::TS && tail == NodeId::END {
            return Some(head);
        }

        if head == NodeId::TS && self.nullability(tail) == Nullability::ALWAYS {
            return Some(NodeId::TS);
        }

        if tail == NodeId::TS && self.nullability(head) == Nullability::ALWAYS {
            return Some(NodeId::TS);
        }

        if self.get_kind(tail) == Kind::Union && head == NodeId::TS {
            let mut should_distribute_top = false;
            self.iter_unions(tail, |v| {
                if v == NodeId::BEGIN || self.starts_with_ts(v) {
                    should_distribute_top = true;
                }
            });
            if should_distribute_top {
                let mut new_union = NodeId::BOT;
                let mut curr = tail;
                while self.get_kind(curr) == Kind::Union {
                    let new_node = self.mk_concat(NodeId::TS, curr.left(self));
                    new_union = self.mk_union(new_node, new_union);
                    curr = curr.right(self);
                }
                let new_node = self.mk_concat(NodeId::TS, curr);
                new_union = self.mk_union(new_union, new_node);
                return Some(new_union);
            }
        }

        if self.get_kind(head) == Kind::Union && head == NodeId::TS {
            let mut should_distribute_top = false;
            self.iter_unions(head, |v| {
                if v == NodeId::END || self.starts_with_ts(v) {
                    should_distribute_top = true;
                }
            });
            if should_distribute_top {
                let mut new_union = NodeId::BOT;
                let mut curr = head;
                while self.get_kind(curr) == Kind::Union {
                    let new_node = self.mk_concat(curr.left(self), NodeId::TS);
                    new_union = self.mk_union(new_node, new_union);
                    curr = curr.right(self);
                }
                let new_node = self.mk_concat(curr, NodeId::TS);
                new_union = self.mk_union(new_union, new_node);
                return Some(new_union);
            }
        }

        if self.get_kind(head) == Kind::Inter && tail == NodeId::TS {
            let mut alltopstar = true;
            iter_inter!(self, head, |v| {
                alltopstar = self.ends_with_ts(v);
            });
            if alltopstar {
                return Some(head);
            }
        }

        if head.is_star(self) && head == tail {
            return Some(head);
        }

        None
    }

    fn attempt_rw_union_2(&mut self, left: NodeId, right: NodeId) -> Option<NodeId> {
        use Kind::*;

        if cfg!(feature = "norewrite") {
            return None;
        }
        if left == right {
            return Some(left);
        }

        if right.is_kind(self, Kind::Union) && left == right.left(self) {
            return Some(right);
        }

        if self.get_kind(left) == Kind::Pred && self.get_kind(right) == Kind::Pred {
            let l = left.pred_tset(self);
            let r = right.pred_tset(self);
            let solver = self.solver();
            let psi = solver.or_id(l, r);
            let rewrite = self.mk_pred(psi);
            return Some(rewrite);
        }

        if left == NodeId::EPS
            && self.get_extra(left) == 0
            && self.nullability(right) == Nullability::ALWAYS
        {
            return Some(right);
        }

        if self.get_kind(left) == Kind::Lookahead && self.get_kind(right) == Kind::Lookahead {
            let lb = left.left(self);
            let lt = left.right(self);
            let lrel = left.extra(self);

            let rb = right.left(self);
            let rt = right.right(self);
            let rrel = right.extra(self);

            if lrel == rrel && lt.is_missing() && rt.is_missing() {
                let unioned = self.mk_union(lb, rb);
                let node = self.mk_lookahead_internal(unioned, NodeId::MISSING, lrel);
                return Some(node);
            }
        }

        if right.is_kind(self, Concat) {
            if left == NodeId::END
                && right.left(self) == NodeId::END
                && self.nullability(right).has(Nullability::END)
            {
                return Some(right);
            }
            // .*|.*a.* => .*(a.*|)
            if left == right.left(self) {
                let rhs = self.mk_union(NodeId::EPS, right.right(self));
                let rw = self.mk_concat(left, rhs);
                return Some(rw);
            }
            if left.is_kind(self, Concat) {
                let head1 = left.left(self);
                let head2 = right.left(self);

                if head1 == head2 {
                    let t1 = left.right(self);
                    let t2 = right.right(self);
                    // opportunistic rewrites
                    if head1 == NodeId::TS {
                        if t2.has_concat_tail(self, t1) {
                            return Some(left);
                        }
                        if t1.has_concat_tail(self, t2) {
                            return Some(right);
                        }
                    }
                    let un = self.mk_union(t1, t2);
                    return Some(self.mk_concat(left.left(self), un));
                }

                // xa|ya => (x|y)a - suffix factoring via reverse
                // TODO: valid and looks prettier but .reverse is not good for builder perf,
                // leaving out unless i find a case where it helps significantly
                if false {
                    let end1 = self.get_concat_end(left);
                    let end2 = self.get_concat_end(right);
                    if end1 == end2 {
                        let flags = left.flags_contains(self).or(right.flags_contains(self));
                        if !flags.contains_lookaround() && !flags.has(MetaFlags::CONTAINS_ANCHORS) {
                            let rev1 = self.reverse(left).unwrap();
                            let rev2 = self.reverse(right).unwrap();

                            let union_rev = self.mk_union(rev1, rev2);
                            return Some(self.reverse(union_rev).unwrap());
                        }
                    }
                }
            }
            if left.is_pred(self) && left == right.left(self) {
                let un = self.mk_opt(right.right(self));
                let conc = self.mk_concat(left, un);
                return Some(conc);
            }
        }

        if left == NodeId::EPS && right.is_plus(self) {
            let result = self.mk_star(right.left(self));
            return Some(result);
        }

        // (.*&X{19}_*&C) | (.*&X{20}_*&C) => (.*&X{19}_*&C)
        if left.is_inter(self) && right.is_inter(self) {
            if let Some(rw) = self.try_subsume_inter_union(left, right) {
                return Some(rw);
            }
        }

        None
    }

    fn collect_inter_components(&self, node: NodeId, out: &mut Vec<NodeId>) {
        let mut curr = node;
        while self.get_kind(curr) == Kind::Inter {
            out.push(self.get_left(curr));
            curr = self.get_right(curr);
        }
        out.push(curr);
    }

    fn as_pred_chain_star(&self, node: NodeId) -> Option<(bool, TSetId, NodeId, u32)> {
        let mut curr = node;
        let has_prefix = self.get_kind(curr) == Kind::Concat && self.get_left(curr) == NodeId::TS;
        if has_prefix {
            curr = self.get_right(curr);
        }
        let mut count = 0u32;
        let mut pred_set = None;
        while self.get_kind(curr) == Kind::Concat {
            let head = self.get_left(curr);
            if self.get_kind(head) != Kind::Pred {
                return None;
            }
            let ps = head.pred_tset(self);
            match pred_set {
                None => pred_set = Some(ps),
                Some(existing) => {
                    if existing != ps {
                        return None;
                    }
                }
            }
            curr = self.get_right(curr);
            count += 1;
        }
        if count == 0 || self.get_kind(curr) != Kind::Star {
            return None;
        }
        Some((has_prefix, pred_set.unwrap(), curr, count))
    }

    fn is_sorted_subset(sub: &[NodeId], sup: &[NodeId]) -> bool {
        let mut j = 0;
        for &s in sub {
            while j < sup.len() && sup[j] < s {
                j += 1;
            }
            if j >= sup.len() || sup[j] != s {
                return false;
            }
            j += 1;
        }
        true
    }

    fn try_subsume_inter_union(&mut self, left: NodeId, right: NodeId) -> Option<NodeId> {
        if self.get_kind(left) != Kind::Inter || self.get_kind(right) != Kind::Inter {
            return None;
        }

        let mut lc: Vec<NodeId> = Vec::new();
        let mut rc: Vec<NodeId> = Vec::new();
        self.collect_inter_components(left, &mut lc);
        self.collect_inter_components(right, &mut rc);

        // component subset check: fewer constraints = larger language
        if lc.len() <= rc.len() && Self::is_sorted_subset(&lc, &rc) {
            return Some(left);
        }
        // if rc ⊆ lc then L(right) ⊇ L(left), keep right
        if rc.len() <= lc.len() && Self::is_sorted_subset(&rc, &lc) {
            return Some(right);
        }

        if lc.len() == rc.len() {
            let mut diff_idx = None;
            for i in 0..lc.len() {
                if lc[i] != rc[i] {
                    if diff_idx.is_some() {
                        return None;
                    }
                    diff_idx = Some(i);
                }
            }
            if let Some(idx) = diff_idx {
                let a = lc[idx];
                let b = rc[idx];
                if let (Some((pfa, pa, sa, ca)), Some((pfb, pb, sb, cb))) =
                    (self.as_pred_chain_star(a), self.as_pred_chain_star(b))
                {
                    if pfa == pfb && pa == pb && sa == sb && ca != cb {
                        return if ca < cb { Some(left) } else { Some(right) };
                    }
                }
            }
        }

        None
    }

    fn attempt_rw_inter_2(&mut self, left: NodeId, right: NodeId) -> Option<NodeId> {
        if cfg!(feature = "norewrite") {
            return None;
        }
        if left == right {
            return Some(left);
        }

        if self.get_kind(right) == Kind::Union {
            let mut result = NodeId::BOT;
            self.iter_unions_b(
                right,
                &mut (|b, v| {
                    let new_inter = b.mk_inter(v, left);
                    result = b.mk_union(result, new_inter);
                }),
            );
            return Some(result);
        }
        if self.get_kind(left) == Kind::Union {
            let mut result = NodeId::BOT;
            self.iter_unions_b(
                left,
                &mut (|b, v| {
                    let new_inter = b.mk_inter(v, right);
                    result = b.mk_union(result, new_inter);
                }),
            );
            return Some(result);
        }

        if self.get_kind(right) == Kind::Compl && right.left(self) == left {
            return Some(NodeId::BOT);
        }

        if left.kind(self) == Kind::Compl && right.kind(self) == Kind::Compl {
            let bodies = self.mk_union(left.left(self), right.left(self));
            return Some(self.mk_compl(bodies));
        }

        if left == NodeId::TOPPLUS {
            if right.is_pred_star(self).is_some() {
                let newloop = self.mk_plus(right.left(self));
                return Some(newloop);
            }
            if right.is_never_nullable(self) {
                return Some(right);
            }
            if right.is_kind(self, Kind::Lookahead) && self.get_lookahead_tail(right).is_missing() {
                return Some(NodeId::BOT);
            }
            if right.is_kind(self, Kind::Concat) {}
        }

        {
            let l_is_la = left.is_lookahead(self);
            let r_is_la = right.is_lookahead(self);
            let l_is_cla = !l_is_la
                && self.get_kind(left) == Kind::Concat
                && self.get_kind(left.left(self)) == Kind::Lookahead;
            let r_is_cla = !r_is_la
                && self.get_kind(right) == Kind::Concat
                && self.get_kind(right.left(self)) == Kind::Lookahead;
            if l_is_la || r_is_la || l_is_cla || r_is_cla {
                let (la_node, other, concat_body) = if r_is_la {
                    (right, left, NodeId::MISSING)
                } else if l_is_la {
                    (left, right, NodeId::MISSING)
                } else if r_is_cla {
                    (right.left(self), left, right.right(self))
                } else {
                    (left.left(self), right, left.right(self))
                };
                let la_body = la_node.left(self);
                let la_tail = self.get_lookahead_tail(la_node).missing_to_eps();
                let inter_right = if concat_body.is_missing() {
                    la_tail
                } else {
                    self.mk_concat(la_tail, concat_body)
                };
                let new_body = self.mk_inter(other, inter_right);
                let la = self.mk_lookahead_internal(la_body, NodeId::MISSING, 0);
                return Some(self.mk_concat(la, new_body));
            }
        }

        if self.get_kind(right) == Kind::Compl {
            let compl_body = right.left(self);
            if left == compl_body {
                return Some(NodeId::BOT);
            }
            if self.get_kind(compl_body) == Kind::Concat {
                let compl_head = compl_body.left(self);
                if compl_body.right(self) == NodeId::TS && compl_head == left {
                    return Some(NodeId::BOT);
                }
            }
        }

        if let Some(pleft) = left.is_pred_star(self) {
            if let Some(pright) = right.is_pred_star(self) {
                let merged = self.mk_inter(pleft, pright);
                return Some(self.mk_star(merged));
            }
        }

        {
            let l_is_clb = self.get_kind(left) == Kind::Concat
                && self.get_kind(left.left(self)) == Kind::Lookbehind;
            let r_is_clb = self.get_kind(right) == Kind::Concat
                && self.get_kind(right.left(self)) == Kind::Lookbehind;
            if l_is_clb || r_is_clb {
                let (lb, body) = if l_is_clb && r_is_clb {
                    let lb1 = left.left(self);
                    let lb2 = right.left(self);
                    let inner = self.mk_inter(
                        self.get_lookbehind_inner(lb1),
                        self.get_lookbehind_inner(lb2),
                    );
                    let lb = self.mk_lookbehind_internal(inner, NodeId::MISSING).unwrap();
                    let body = self.mk_inter(left.right(self), right.right(self));
                    (lb, body)
                } else if l_is_clb {
                    let lb = left.left(self);
                    let body = self.mk_inter(left.right(self), right);
                    (lb, body)
                } else {
                    let lb = right.left(self);
                    let body = self.mk_inter(left, right.right(self));
                    (lb, body)
                };
                return Some(self.mk_concat(lb, body));
            }
        }

        {
            let l_has_la = self.has_trailing_la(left);
            let r_has_la = self.has_trailing_la(right);
            if l_has_la || r_has_la {
                let (body, la) = if l_has_la && r_has_la {
                    let (lbody, l_la) = self.strip_trailing_la(left);
                    let (rbody, r_la) = self.strip_trailing_la(right);
                    let inner = self.mk_inter(
                        self.get_lookahead_inner(l_la),
                        self.get_lookahead_inner(r_la),
                    );
                    let la = self.mk_lookahead_internal(inner, NodeId::MISSING, 0);
                    let body = self.mk_inter(lbody, rbody);
                    (body, la)
                } else if l_has_la {
                    let (lbody, la) = self.strip_trailing_la(left);
                    let body = self.mk_inter(lbody, right);
                    (body, la)
                } else {
                    let (rbody, la) = self.strip_trailing_la(right);
                    let body = self.mk_inter(left, rbody);
                    (body, la)
                };
                return Some(self.mk_concat(body, la));
            }
        }

        None
    }

    fn attempt_rw_unions(&mut self, left: NodeId, right_union: NodeId) -> Option<NodeId> {
        if cfg!(feature = "norewrite") {
            return None;
        }
        debug_assert!(self.get_kind(right_union) == Kind::Union);

        let mut rewritten = None;
        right_union.iter_union_while(
            self,
            &mut (|b, v| {
                if let Some(rw) = b.attempt_rw_union_2(left, v) {
                    rewritten = Some((v, rw));
                    false
                } else {
                    true
                }
            }),
        );

        if let Some(rw) = rewritten {
            let mut new_union = NodeId::BOT;
            right_union.iter_union(
                self,
                &mut (|b, v| {
                    if v == rw.0 {
                        new_union = b.mk_union(rw.1, new_union)
                    } else {
                        new_union = b.mk_union(v, new_union)
                    }
                }),
            );
            return Some(new_union);
        };

        None
    }

    pub fn mk_concat(&mut self, head: NodeId, tail: NodeId) -> NodeId {
        debug_assert!(head != NodeId::MISSING, "missing to {}", self.pp(tail));
        debug_assert!(tail != NodeId::MISSING);
        let key = NodeKey {
            kind: Kind::Concat,
            left: head,
            right: tail,
            extra: u32::MAX,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }

        if head == NodeId::BOT || tail == NodeId::BOT {
            return NodeId::BOT;
        }
        if head == NodeId::EPS {
            return tail;
        }
        if tail == NodeId::EPS {
            return head;
        }

        match tail {
            // this is only valid if direction known;
            NodeId::BEGIN => {
                if !self.is_nullable(head, Nullability::BEGIN) {
                    return NodeId::BOT;
                } else {
                    return NodeId::BEGIN;
                }
            }
            _ => {}
        }

        // normalize concats to right
        if head.is_kind(self, Kind::Concat) {
            let left = head.left(self);
            let newright = self.mk_concat(head.right(self), tail);
            let updated = self.mk_concat(left, newright);
            return self.init_as(key, updated);
        }

        if head == NodeId::TS && tail == NodeId::END {
            return NodeId::TS;
        }

        if self.get_kind(head) == Kind::End && !self.is_nullable(tail, Nullability::END) {
            return NodeId::BOT;
        }

        if self.get_kind(tail) == Kind::Concat {
            if let Some(rw) = self.attempt_rw_concat_2(head, tail.left(self)) {
                let upd = self.mk_concat(rw, tail.right(self));
                return self.init_as(key, upd);
            }
        }

        if let Some(new) = self.attempt_rw_concat_2(head, tail) {
            return self.init_as(key, new);
        }

        match (self.get_kind(head), self.get_kind(tail)) {
            // merge stars
            (Kind::Star, Kind::Concat) if head.is_star(self) => {
                let rl = tail.left(self);
                if head == rl {
                    return self.init_as(key, tail);
                }
            }
            // attempt longer concat rw
            (_, Kind::Concat) => {
                let curr = head;
                let h2 = self.mk_concat(curr, tail.left(self));
                let tr = tail.right(self);
                if let Some(new) = self.attempt_rw_concat_2(h2, tr) {
                    return self.init_as(key, new);
                }
            }
            _ if head == NodeId::TS && self.starts_with_ts(tail) => {
                return self.init_as(key, tail);
            }
            _ => {}
        }

        self.init(key)
    }

    pub fn mk_lookbehind(&mut self, lb_body: NodeId, lb_prev: NodeId) -> NodeId {
        // LNF: lookbehind must start with ts
        let lb_body = {
            match self.starts_with_ts(lb_body) {
                true => lb_body,
                false => self.mk_concat(NodeId::TS, lb_body),
            }
        };
        // lb_body starts with TS after normalization above, so EPS case cannot trigger
        self.mk_lookbehind_internal(lb_body, lb_prev).unwrap()
    }

    fn mk_lookbehind_internal(
        &mut self,
        lb_body: NodeId,
        lb_prev: NodeId,
    ) -> Result<NodeId, AlgebraError> {
        debug_assert!(lb_body != NodeId::MISSING);
        debug_assert!(lb_prev.0 != u32::MAX, "pattern_left missing");
        if lb_body == NodeId::BOT || lb_prev == NodeId::BOT {
            return Ok(NodeId::BOT);
        }
        if lb_body == NodeId::TS {
            return Ok(lb_prev);
        }
        if lb_body == NodeId::EPS {
            match lb_prev {
                NodeId::MISSING => return Ok(NodeId::EPS),
                NodeId::EPS => return Ok(NodeId::EPS),
                _ => return Ok(lb_prev),
            }
        }

        let key = NodeKey {
            kind: Kind::Lookbehind,
            left: lb_body,
            right: lb_prev,
            extra: u32::MAX,
        };
        match self.key_is_created(&key) {
            Some(id) => Ok(*id),
            None => {
                if lb_prev == NodeId::TS {
                    return Ok(self.mk_concat(lb_prev, lb_body));
                }

                Ok(self.init(key))
            }
        }
    }

    pub fn mk_lookahead(&mut self, la_body: NodeId, la_tail: NodeId, rel: u32) -> NodeId {
        // LNF: lookahead must end with ts
        let la_body = {
            match self.ends_with_ts(la_body) {
                true => la_body,
                false => self.mk_concat(la_body, NodeId::TS),
            }
        };
        let rel = if NodeId::MISSING == la_tail {
            rel
        } else {
            match la_tail.is_center_nullable(self) {
                false => u32::MAX,
                true => rel,
            }
        };

        self.mk_lookahead_internal(la_body, la_tail, rel)
    }

    // rel max = carries no nullability, can potentially rw to intersection
    pub fn mk_lookahead_internal(&mut self, la_body: NodeId, la_tail: NodeId, rel: u32) -> NodeId {
        let key = NodeKey {
            kind: Kind::Lookahead,
            left: la_body,
            right: la_tail,
            extra: rel,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }
        if la_body == NodeId::TS {
            if rel == 0 {
                return la_tail.missing_to_eps();
            } else {
                return self.mk_lookahead_internal(NodeId::EPS, la_tail, rel);
            }
        }
        if la_body == NodeId::BOT || la_tail == NodeId::BOT {
            return NodeId::BOT;
        }
        if la_tail.is_missing() && rel == u32::MAX {
            return NodeId::BOT;
        }

        if la_body == NodeId::EPS && la_tail.is_missing() && rel == 0 {
            return la_body;
        }

        if la_tail == NodeId::TS {
            if rel == 0 || rel == u32::MAX {
                return self.mk_concat(la_body, NodeId::TS);
            } else if rel == u32::MAX {
                return self.mk_begins_with(la_body);
            }
        }

        if rel == u32::MAX {
            if la_tail.is_missing() {
                return NodeId::BOT;
            }

            if self.is_always_nullable(la_body) {
                return la_tail.missing_to_eps();
            }

            if la_tail != NodeId::MISSING {
                match self.get_kind(la_tail) {
                    _ => {
                        if la_body.is_compl_plus_end(self) {
                            let minlen = self.get_min_length_only(la_tail);
                            if minlen >= 1 {
                                return NodeId::BOT;
                            }
                        }
                    }
                }
            }
        }

        if la_tail != NodeId::MISSING && self.get_kind(la_tail) == Kind::Lookahead {
            let la_body2 = self.get_lookahead_inner(la_tail);
            let body1_ts = self.mk_concat(la_body, NodeId::TS);
            let body2_ts = self.mk_concat(la_body2, NodeId::TS);
            let new_la_body = self.mk_inter(body1_ts, body2_ts);
            let new_la_rel = self.get_lookahead_rel(la_tail);
            let new_la_tail = self.get_lookahead_tail(la_tail);
            return self.mk_lookahead_internal(new_la_body, new_la_tail, new_la_rel);
        }

        if self.get_kind(la_body) == Kind::Concat && la_body.left(self) == NodeId::TS {
            let la_body_right = la_body.right(self);
            if self.is_always_nullable(la_body_right) {
                return self.mk_lookahead_internal(la_body_right, la_tail, rel);
            }
            if la_body.right(self) == NodeId::END {
                return self.mk_lookahead_internal(NodeId::EPS, la_tail, rel);
            }
            let bodyright = la_body.right(self);
            if self.get_kind(bodyright) == Kind::Concat && bodyright.left(self) == NodeId::END {
                let strippedanchor = self.mk_concat(NodeId::TS, bodyright.right(self));
                return self.mk_lookahead_internal(strippedanchor, la_tail, rel);
            }
        }

        if la_tail != NodeId::MISSING {
            if let (Kind::Concat, Kind::Pred) = (self.get_kind(la_body), self.get_kind(la_tail)) {
                let lpred = la_body.left(self);
                if self.get_kind(lpred) == Kind::Pred {
                    let l = lpred.pred_tset(self);
                    let r = la_tail.pred_tset(self);
                    let psi_and = self.solver().and_id(l, r);
                    let rewrite = self.mk_pred(psi_and);
                    let new_rel = if rel == usize::MAX as u32 { 0 } else { rel + 1 };
                    let new_right =
                        self.mk_lookahead_internal(la_body.right(self), NodeId::MISSING, new_rel);
                    return self.mk_concat(rewrite, new_right);
                }
            }
        }

        self.get_node_id(key)
    }

    pub fn mk_counted(&mut self, body: NodeId, chain: NodeId, packed: u32) -> NodeId {
        let has_match = (packed >> 16) > 0;
        if body == NodeId::BOT && chain == NodeId::MISSING && !has_match {
            return NodeId::BOT;
        }
        debug_assert!(
            body == NodeId::BOT || !self.is_infinite(body),
            "Counted body must have finite max length"
        );
        let chain = self.prune_counted_chain(body, chain);
        let key = NodeKey {
            kind: Kind::Counted,
            left: body,
            right: chain,
            extra: packed,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }
        self.get_node_id(key)
    }

    fn prune_counted_chain(&mut self, body: NodeId, chain: NodeId) -> NodeId {
        if chain == NodeId::MISSING || body == NodeId::BOT {
            return chain;
        }
        if self.nullability(body) != Nullability::NEVER {
            return NodeId::MISSING;
        }
        let chain_body = chain.left(self);
        if chain_body == NodeId::BOT {
            return chain;
        }
        let not_begins = self.mk_not_begins_with(body);
        let inter = self.mk_inter(chain_body, not_begins);
        let is_empty = inter == NodeId::BOT;
        if is_empty {
            self.prune_counted_chain(body, chain.right(self))
        } else {
            chain
        }
    }

    pub fn mk_neg_lookahead(&mut self, body: NodeId, rel: u32) -> NodeId {
        let neg_inner = self.mk_concat(body, NodeId::TS);
        let neg_part = self.mk_compl(neg_inner);
        let conc = self.mk_concat(neg_part, NodeId::END);
        self.mk_lookahead(conc, NodeId::MISSING, rel)
    }

    pub fn mk_neg_lookbehind(&mut self, body: NodeId) -> NodeId {
        match self.get_node(body).kind {
            Kind::Pred => {
                let psi = body.pred_tset(self);
                let negated = self.mk_pred_not(psi);
                let union = self.mk_union(NodeId::BEGIN, negated);
                // lb_prev is MISSING - cannot trigger UnsupportedPattern
                self.mk_lookbehind_internal(union, NodeId::MISSING).unwrap()
            }
            _ => {
                // ~body ∩ utf8_char: non-nullable (utf8_char requires ≥1 byte),
                // so \A | negated won't be stripped by strip_prefix_safe
                let uc = crate::unicode_classes::utf8_char(self);
                let neg = self.mk_compl(body);
                let negated = self.mk_inter(neg, uc);
                let union = self.mk_union(NodeId::BEGIN, negated);
                // lb_prev is MISSING - cannot trigger UnsupportedPattern
                self.mk_lookbehind_internal(union, NodeId::MISSING).unwrap()
            }
        }
    }

    pub fn mk_union(&mut self, left: NodeId, right: NodeId) -> NodeId {
        debug_assert!(left != NodeId::MISSING);
        debug_assert!(right != NodeId::MISSING);
        if left > right {
            return self.mk_union(right, left);
        }
        let key = NodeKey {
            kind: Kind::Union,
            left,
            right,
            extra: u32::MAX,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }

        if left == right {
            return left;
        }
        if left == NodeId::BOT {
            return right;
        }
        if right == NodeId::BOT {
            return left;
        }
        if right == NodeId::TS {
            return right;
        }
        if left == NodeId::TS {
            return left;
        }

        match (self.get_kind(left), self.get_kind(right)) {
            (Kind::Union, _) => {
                self.iter_unions_b(left, &mut |b, v| {
                    b.temp_vec.push(v);
                });
                self.iter_unions_b(right, &mut |b, v| {
                    b.temp_vec.push(v);
                });
                self.temp_vec.sort();
                let tree = self.temp_vec.clone();
                self.temp_vec.clear();
                let newnode = tree
                    .iter()
                    .rev()
                    .fold(NodeId::BOT, |acc, x| self.mk_union(*x, acc));
                return self.init_as(key, newnode);
            }
            (_, Kind::Union) => {
                let rleft = right.left(self);
                // if left_node id is smaller than rleft, just create a new union
                if left > rleft {
                    self.iter_unions_b(left, &mut |b, v| {
                        b.temp_vec.push(v);
                    });
                    self.iter_unions_b(right, &mut |b, v| {
                        b.temp_vec.push(v);
                    });
                    self.temp_vec.sort();
                    let tree = self.temp_vec.clone();
                    self.temp_vec.clear();
                    let newnode = tree
                        .iter()
                        .rev()
                        .fold(NodeId::BOT, |acc, x| self.mk_union(*x, acc));
                    return self.init_as(key, newnode);
                } else {
                    if let Some(rw) = self.attempt_rw_unions(left, right) {
                        return self.init_as(key, rw);
                    }
                }
            }
            _ => {}
        }

        if let Some(rw) = self.attempt_rw_union_2(left, right) {
            return self.init_as(key, rw);
        }
        self.init(key)
    }

    pub fn mk_inter(&mut self, left_id: NodeId, right_id: NodeId) -> NodeId {
        debug_assert!(left_id != NodeId::MISSING);
        debug_assert!(right_id != NodeId::MISSING);
        if left_id == right_id {
            return left_id;
        }
        if left_id == NodeId::BOT || right_id == NodeId::BOT {
            return NodeId::BOT;
        }
        if left_id == NodeId::TS {
            return right_id;
        }
        if right_id == NodeId::TS {
            return left_id;
        }
        if left_id > right_id {
            return self.mk_inter(right_id, left_id);
        }
        let key = NodeKey {
            kind: Kind::Inter,
            left: left_id,
            right: right_id,
            extra: u32::MAX,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }

        if let Some(rw) = self.attempt_rw_inter_2(left_id, right_id) {
            return self.init_as(key, rw);
        }

        self.init(key)
    }

    fn mk_unset(&mut self, kind: Kind) -> NodeId {
        let node = NodeKey {
            kind,
            left: NodeId::MISSING,
            right: NodeId::MISSING,
            extra: u32::MAX,
        };
        self.init(node)
    }

    pub fn mk_plus(&mut self, body_id: NodeId) -> NodeId {
        let star = self.mk_star(body_id);
        self.mk_concat(body_id, star)
    }
    pub fn mk_repeat(&mut self, body_id: NodeId, lower: u32, upper: u32) -> NodeId {
        let opt = self.mk_opt(body_id);
        let mut nodes1 = vec![];
        for _ in lower..upper {
            nodes1.push(opt);
        }
        for _ in 0..lower {
            nodes1.push(body_id);
        }
        self.mk_concats(nodes1.into_iter())
    }
    pub fn mk_opt(&mut self, body_id: NodeId) -> NodeId {
        self.mk_union(NodeId::EPS, body_id)
    }

    pub fn mk_star(&mut self, body_id: NodeId) -> NodeId {
        let key = NodeKey {
            kind: Kind::Star,
            left: body_id,
            right: NodeId::MISSING,
            extra: 0,
        };
        if let Some(id) = self.key_is_created(&key) {
            return *id;
        }
        // _*{500} is still _*
        if body_id.is_kind(self, Kind::Star) {
            return body_id;
        }
        self.get_node_id(key)
    }

    /// it's cheaper to check this once as an edge-case
    /// than to compute a 4th nullability bit for every node
    pub fn nullability_emptystring(&self, node_id: NodeId) -> Nullability {
        match self.get_kind(node_id) {
            Kind::End => Nullability::EMPTYSTRING,
            Kind::Begin => Nullability::EMPTYSTRING,
            Kind::Pred => Nullability::NEVER,
            Kind::Star => Nullability::ALWAYS,
            Kind::Inter | Kind::Concat => {
                let lnull = self.nullability_emptystring(node_id.left(self));
                let rnull = self.nullability_emptystring(node_id.right(self));
                lnull.and(rnull) // left = 010, right = 001, left & right = 000
            }
            Kind::Union => {
                let lnull = self.nullability_emptystring(node_id.left(self));
                let rnull = self.nullability_emptystring(node_id.right(self));
                lnull.or(rnull)
            }
            Kind::Compl => self.nullability_emptystring(node_id.left(self)).not(),
            Kind::Lookbehind => self.nullability_emptystring(node_id.left(self)),
            Kind::Lookahead => self.nullability_emptystring(node_id.left(self)),
            Kind::Counted => self.nullability_emptystring(node_id.left(self)),
        }
    }

    #[inline(always)]
    pub fn any_nonbegin_nullable(&self, node_id: NodeId) -> bool {
        self.get_meta(node_id)
            .flags
            .nullability()
            .has(Nullability::CENTER.or(Nullability::END))
    }

    pub fn nullability(&self, node_id: NodeId) -> Nullability {
        self.get_only_nullability(node_id)
    }

    pub(crate) fn is_always_nullable(&self, node_id: NodeId) -> bool {
        self.get_only_nullability(node_id).and(Nullability::ALWAYS) == Nullability::ALWAYS
    }

    pub fn pp(&self, node_id: NodeId) -> String {
        let mut s = String::new();
        self.ppw(&mut s, node_id).unwrap();
        s
    }

    #[allow(dead_code)]
    pub fn pp_nulls(&self, node_id: NodeId) -> String {
        let nu = self.get_nulls_id(node_id);
        let nr = self.mb.nb.get_set_ref(nu);
        let s1 = format!("{:?}", nr);
        s1
    }

    #[allow(dead_code)]
    pub(crate) fn ppt(&self, term_id: TRegexId) -> String {
        match self.get_tregex(term_id) {
            TRegex::Leaf(node_id) => {
                let mut s = String::new();
                self.ppw(&mut s, *node_id).unwrap();
                s
            }
            TRegex::ITE(cond, then_id, else_id) => {
                format!(
                    "ITE({},{},{})",
                    self.solver_ref().pp(*cond),
                    self.ppt(*then_id),
                    self.ppt(*else_id)
                )
            }
        }
    }

    fn ppw(&self, s: &mut String, node_id: NodeId) -> Result<(), std::fmt::Error> {
        if cfg!(feature = "graphviz") {
            match node_id {
                NodeId::MISSING => return write!(s, "MISSING"),
                NodeId::BOT => return write!(s, "⊥"),
                NodeId::TS => return write!(s, "_*"),
                NodeId::TOP => return write!(s, "_"),
                NodeId::EPS => return write!(s, "ε"),
                _ => {}
            }
        }

        match node_id {
            NodeId::MISSING => return write!(s, "MISSING"),
            NodeId::BOT => return write!(s, "⊥"),
            NodeId::TS => return write!(s, "_*"),
            NodeId::TOP => return write!(s, "_"),
            NodeId::EPS => return write!(s, ""),
            _ => {}
        }

        match self.get_kind(node_id) {
            Kind::End => write!(s, r"\z"),
            Kind::Begin => write!(s, r"\A"),
            Kind::Pred => {
                let psi = node_id.pred_tset(self);
                if psi == TSetId::EMPTY {
                    write!(s, r"⊥")
                } else if psi == TSetId::FULL {
                    write!(s, r"_")
                } else {
                    write!(s, "{}", self.solver_ref().pp(psi))
                }
            }
            Kind::Inter => {
                write!(s, "(")?;
                self.ppw(s, node_id.left(self))?;
                write!(s, "&")?;
                let mut curr = node_id.right(self);
                while self.get_kind(curr) == Kind::Inter {
                    let n = curr.left(self);
                    self.ppw(s, n)?;
                    write!(s, "&")?;
                    curr = curr.right(self);
                }
                self.ppw(s, curr)?;
                write!(s, ")")
            }
            Kind::Union => {
                let left = node_id.left(self);
                let right = node_id.right(self);
                write!(s, "(")?;
                self.ppw(s, left)?;
                write!(s, "|")?;
                let mut curr = right;
                while self.get_kind(curr) == Kind::Union {
                    let n = curr.left(self);
                    self.ppw(s, n)?;
                    write!(s, "|")?;
                    curr = curr.right(self);
                }
                self.ppw(s, curr)?;
                write!(s, ")")
            }
            Kind::Concat => {
                let left = node_id.left(self);
                let right = node_id.right(self);
                if right.is_star(self) && right.left(self) == left {
                    self.ppw(s, left)?;
                    write!(s, "+")?;
                    return Ok(());
                }
                if right.is_concat(self) {
                    let rl = right.left(self);
                    if rl.is_star(self) && rl.left(self) == left {
                        self.ppw(s, left)?;
                        write!(s, "+")?;
                        return self.ppw(s, right.right(self));
                    }
                }
                if right.is_concat(self) && right.left(self) == left {
                    let mut num = 1;
                    let mut right = right;
                    while right.is_concat(self) && right.left(self) == left {
                        num += 1;
                        right = right.right(self);
                    }
                    // (|X){n} followed by X{m} -> X{m,m+n}
                    if let Some(inner) = left.is_opt_v(self) {
                        let mut inner_count = 0;
                        let mut right2 = right;
                        while right2.is_concat(self) && right2.left(self) == inner {
                            inner_count += 1;
                            right2 = right2.right(self);
                        }
                        if right2 == inner {
                            inner_count += 1;
                            self.ppw(s, inner)?;
                            return write!(s, "{{{},{}}}", inner_count, inner_count + num);
                        }
                        if inner_count > 0 {
                            self.ppw(s, inner)?;
                            write!(s, "{{{},{}}}", inner_count, inner_count + num)?;
                            return self.ppw(s, right2);
                        }
                    }
                    self.ppw(s, left)?;
                    if right == left {
                        num += 1;
                        return write!(s, "{{{}}}", num);
                    }
                    if num <= 3 && left.is_pred(self) {
                        for _ in 1..num {
                            self.ppw(s, left)?;
                        }
                        return self.ppw(s, right);
                    } else {
                        write!(s, "{{{}}}", num)?;
                        return self.ppw(s, right);
                    }
                }
                self.ppw(s, left)?;
                self.ppw(s, right)
            }
            Kind::Star => {
                let left = node_id.left(self);
                let leftkind = self.get_kind(left);
                match leftkind {
                    Kind::Concat | Kind::Star | Kind::Compl => {
                        write!(s, "(")?;
                        self.ppw(s, left)?;
                        write!(s, ")")?;
                    }
                    _ => {
                        self.ppw(s, left)?;
                    }
                };
                write!(s, "*")
            }
            Kind::Compl => {
                write!(s, "~(")?;
                self.ppw(s, node_id.left(self))?;
                write!(s, ")")
            }
            Kind::Lookbehind => {
                let lbleft = self.get_lookbehind_prev(node_id);
                let lbinner = self.get_lookbehind_inner(node_id);
                debug_assert!(lbleft.0 != u32::MAX, "lookbehind right is not u32::MAX");
                if lbleft != NodeId::MISSING {
                    write!(s, "「")?;
                    self.ppw(s, lbleft)?;
                    write!(s, "」")?;
                }

                write!(s, "(?<=")?;
                self.ppw(s, lbinner)?;
                write!(s, ")")
            }
            Kind::Lookahead => {
                let inner = self.get_lookahead_inner(node_id);
                write!(s, "(?=")?;
                self.ppw(s, inner)?;
                write!(s, ")")?;
                if self.get_lookahead_rel(node_id) != 0 {
                    write!(s, "{{")?;
                    let rel = self.get_lookahead_rel(node_id);
                    if rel == u32::MAX {
                        write!(s, "∅")?;
                    } else {
                        write!(s, "{}", rel)?;
                    }
                    write!(s, "}}")?;
                }
                if node_id.right(self) == NodeId::MISSING {
                    Ok(())
                } else {
                    write!(s, "『")?;
                    self.ppw(s, node_id.right(self))?;
                    write!(s, "』")
                }
            }
            Kind::Counted => {
                let body = node_id.left(self);
                let packed = self.get_extra(node_id);
                let step = packed & 0xFFFF;
                let best = packed >> 16;
                write!(s, "#(")?;
                self.ppw(s, body)?;
                write!(s, ")s{}b{}", step, best)
            }
        }
    }

    pub(crate) fn mk_begins_with(&mut self, node: NodeId) -> NodeId {
        self.mk_concat(node, NodeId::TS)
    }

    pub fn mk_not_begins_with(&mut self, node: NodeId) -> NodeId {
        let node_ts = self.mk_concat(node, NodeId::TS);
        self.mk_compl(node_ts)
    }

    pub fn mk_pred_not(&mut self, set: TSetId) -> NodeId {
        let notset = self.solver().not_id(set);
        self.mk_pred(notset)
    }

    pub fn mk_u8(&mut self, char: u8) -> NodeId {
        let set_id = self.solver().u8_to_set_id(char);
        self.mk_pred(set_id)
    }

    pub fn mk_range_u8(&mut self, start: u8, end_inclusive: u8) -> NodeId {
        let rangeset = self.solver().range_to_set_id(start, end_inclusive);
        self.mk_pred(rangeset)
    }

    pub fn mk_ranges_u8(&mut self, ranges: &[(u8, u8)]) -> NodeId {
        let mut node = self.mk_range_u8(ranges[0].0, ranges[0].1);
        for &(lo, hi) in &ranges[1..] {
            let r = self.mk_range_u8(lo, hi);
            node = self.mk_union(node, r);
        }
        node
    }

    pub fn extract_literal_prefix(&self, node: NodeId) -> (Vec<u8>, bool) {
        let mut prefix = Vec::new();
        let mut curr = node;
        loop {
            if curr == NodeId::EPS {
                let full = !prefix.is_empty();
                return (prefix, full);
            }
            if curr == NodeId::BOT {
                break;
            }
            if self.get_kind(curr) == Kind::Pred {
                match self.solver_ref().single_byte(TSetId(self.get_extra(curr))) {
                    Some(byte) => {
                        prefix.push(byte);
                        return (prefix, true);
                    }
                    None => break, // multi-byte pred: pattern not fully consumed
                }
            }
            if self.get_kind(curr) != Kind::Concat {
                break;
            }
            let left = curr.left(self);
            if self.get_kind(left) != Kind::Pred {
                break;
            }
            match self.solver_ref().single_byte(TSetId(self.get_extra(left))) {
                Some(byte) => prefix.push(byte),
                None => break,
            }
            curr = curr.right(self);
        }
        (prefix, false)
    }

    #[allow(dead_code)]
    pub(crate) fn mk_bytestring(&mut self, raw_str: &[u8]) -> NodeId {
        let mut result = NodeId::EPS;
        for byte in raw_str.iter().rev() {
            let node = self.mk_u8(*byte);
            result = self.mk_concat(node, result);
        }
        result
    }

    pub fn mk_string(&mut self, raw_str: &str) -> NodeId {
        let mut result = NodeId::EPS;
        for byte in raw_str.bytes().rev() {
            let node = self.mk_u8(byte);
            result = self.mk_concat(node, result);
        }
        result
    }

    pub fn mk_unions(&mut self, nodes: impl DoubleEndedIterator<Item = NodeId>) -> NodeId {
        let mut sorted: Vec<NodeId> = nodes.collect();
        if sorted.len() <= 1 {
            return sorted.pop().unwrap_or(NodeId::BOT);
        }
        sorted.sort();
        sorted.dedup();
        sorted.retain(|&x| x != NodeId::BOT);
        if sorted.is_empty() {
            return NodeId::BOT;
        }
        if sorted.len() > 16 {
            let mut by_head: FxHashMap<NodeId, Vec<NodeId>> = FxHashMap::default();
            let mut non_concat: Vec<NodeId> = Vec::new();
            for &n in &sorted {
                if self.get_kind(n) == Kind::Concat {
                    by_head.entry(self.get_left(n)).or_default().push(n);
                } else {
                    non_concat.push(n);
                }
            }
            let mut absorbed: Vec<NodeId> = Vec::new();
            for &n in &non_concat {
                if by_head.contains_key(&n) {
                    absorbed.push(n);
                }
            }
            if !absorbed.is_empty() {
                non_concat.retain(|n| !absorbed.contains(n));
            }
            if by_head.len() < sorted.len() {
                let mut groups: Vec<NodeId> = non_concat;
                for (head, tails) in by_head {
                    let mut tail_nodes: Vec<NodeId> =
                        tails.iter().map(|&n| self.get_right(n)).collect();
                    if absorbed.contains(&head) {
                        tail_nodes.push(NodeId::EPS);
                    }
                    let tail_union = self.mk_unions(tail_nodes.into_iter());
                    let factored = self.mk_concat(head, tail_union);
                    groups.push(factored);
                }
                groups.sort();
                groups.dedup();
                return self.mk_unions_balanced(&groups);
            }
        }
        self.mk_unions_balanced(&sorted)
    }

    fn mk_unions_balanced(&mut self, nodes: &[NodeId]) -> NodeId {
        match nodes.len() {
            0 => NodeId::BOT,
            1 => nodes[0],
            n => {
                let mid = n / 2;
                let left = self.mk_unions_balanced(&nodes[..mid]);
                let right = self.mk_unions_balanced(&nodes[mid..]);
                self.mk_union(left, right)
            }
        }
    }

    pub fn mk_inters(&mut self, nodes: impl DoubleEndedIterator<Item = NodeId>) -> NodeId {
        nodes.rev().fold(NodeId::TS, |acc, v| self.mk_inter(acc, v))
    }

    pub fn mk_concats(&mut self, nodes: impl DoubleEndedIterator<Item = NodeId>) -> NodeId {
        nodes
            .rev()
            .fold(NodeId::EPS, |acc, x| self.mk_concat(x, acc))
    }
}

impl RegexBuilder {
    #[allow(dead_code)]
    pub(crate) fn extract_sat(&self, term_id: TRegexId) -> Vec<NodeId> {
        match self.get_tregex(term_id).clone() {
            TRegex::Leaf(node_id) => {
                if NodeId::BOT == node_id {
                    vec![]
                } else {
                    vec![node_id]
                }
            }
            TRegex::ITE(_, then_id, else_id) => {
                let mut then_nodes = self.extract_sat(then_id);
                let mut else_nodes = self.extract_sat(else_id);
                then_nodes.append(&mut else_nodes);
                then_nodes
            }
        }
    }

    pub(crate) fn iter_unions(&self, start: NodeId, mut f: impl FnMut(NodeId)) {
        debug_assert!(self.get_kind(start) == Kind::Union);
        let mut curr = start;
        while self.get_kind(curr) == Kind::Union {
            f(curr.left(self));
            curr = curr.right(self);
        }
        f(curr);
    }

    pub(crate) fn iter_unions_b(
        &mut self,
        curr: NodeId,
        f: &mut impl FnMut(&mut RegexBuilder, NodeId),
    ) {
        let mut curr = curr;
        while self.get_kind(curr) == Kind::Union {
            f(self, curr.left(self));
            curr = curr.right(self);
        }
        f(self, curr);
    }

    pub fn try_elim_lookarounds(&mut self, node_id: NodeId) -> Option<NodeId> {
        if !self.contains_look(node_id) {
            return Some(node_id);
        }
        match self.get_kind(node_id) {
            Kind::Pred | Kind::Begin | Kind::End => Some(node_id),
            Kind::Concat => {
                let left = node_id.left(self);
                let right = node_id.right(self);
                let elim_l = self.try_elim_lookarounds(left)?;
                let elim_r = self.try_elim_lookarounds(right)?;
                let rw = self.mk_concat(elim_l, elim_r);
                Some(rw)
            }
            Kind::Union => {
                let left = node_id.left(self);
                let right = node_id.right(self);
                let elim_l = self.try_elim_lookarounds(left)?;
                let elim_r = self.try_elim_lookarounds(right)?;
                let rw = self.mk_union(elim_l, elim_r);
                Some(rw)
            }

            Kind::Star => {
                let body = node_id.left(self);
                let elim_l = self.try_elim_lookarounds(body)?;
                Some(self.mk_star(elim_l))
            }
            Kind::Compl => {
                let left = node_id.left(self);
                let elim_l = self.try_elim_lookarounds(left)?;
                Some(self.mk_compl(elim_l))
            }
            Kind::Lookahead => {
                let rel = self.get_lookahead_rel(node_id);
                if rel != 0 {
                    return None;
                }
                let lbody = self.get_lookahead_inner(node_id);
                let ltail = self.get_lookahead_tail(node_id).missing_to_eps();
                let elim_l = self.try_elim_lookarounds(lbody)?;
                let elim_r = self.try_elim_lookarounds(ltail)?;
                let lbody_ts = self.mk_concat(elim_l, NodeId::TS);
                let ltail_ts = self.mk_concat(elim_r, NodeId::TS);
                let rw = self.mk_inter(lbody_ts, ltail_ts);
                Some(rw)
            }
            Kind::Lookbehind => {
                let linner = self.get_lookbehind_inner(node_id);
                let lprev = self.get_lookbehind_prev(node_id).missing_to_eps();
                let elim_l = self.try_elim_lookarounds(linner)?;
                let elim_r = self.try_elim_lookarounds(lprev)?;
                let lbody_ts = self.mk_concat(NodeId::TS, elim_l);
                let ltail_ts = self.mk_concat(NodeId::TS, elim_r);
                let rw = self.mk_inter(lbody_ts, ltail_ts);
                Some(rw)
            }
            Kind::Inter => {
                let left = node_id.left(self);
                let right = node_id.right(self);
                let elim_l = self.try_elim_lookarounds(left)?;
                let elim_r = self.try_elim_lookarounds(right)?;
                let rw = self.mk_inter(elim_l, elim_r);
                Some(rw)
            }
            Kind::Counted => None,
        }
    }

    /// R & _+ is a safe overapproximation of R that's nonempty
    pub(crate) fn mk_non_nullable_safe(&mut self, node: NodeId) -> NodeId {
        if self.nullability(node) == Nullability::NEVER {
            node
        } else {
            self.mk_inter(NodeId::TOPPLUS, node)
        }
    }

    fn iter_find_stack(
        &self,
        stack: &mut Vec<TRegexId>,
        mut f: impl FnMut(NodeId) -> bool,
    ) -> bool {
        loop {
            match stack.pop() {
                None => return false,
                Some(curr) => match self.get_tregex(curr) {
                    TRegex::Leaf(n) => {
                        let mut curr = *n;
                        while curr != NodeId::BOT {
                            match self.get_kind(curr) {
                                Kind::Union => {
                                    if f(curr.left(self)) {
                                        return true;
                                    }
                                    curr = curr.right(self);
                                }
                                _ => {
                                    if f(*n) {
                                        return true;
                                    }
                                    curr = NodeId::BOT;
                                }
                            }
                        }
                    }
                    TRegex::ITE(_, then_id, else_id) => {
                        if *else_id != TRegexId::BOT {
                            stack.push(*else_id);
                        }
                        stack.push(*then_id);
                    }
                },
            }
        }
    }

    pub(crate) fn is_empty_lang(&mut self, node: NodeId) -> Option<bool> {
        if node == NodeId::BOT {
            return Some(true);
        }
        if self.nullability(node) != Nullability::NEVER {
            return Some(false);
        }
        if let Some(cached) = self.cache_empty.get(&node) {
            if cached.is_checked() {
                return Some(cached.is_empty());
            }
        }
        let node = if !self.contains_look(node) {
            node
        } else {
            self.try_elim_lookarounds(node)?
        };
        let isempty_flag = self.is_empty_lang_internal(node);

        Some(isempty_flag == Ok(NodeFlags::IS_EMPTY))
    }

    fn is_empty_lang_internal(&mut self, initial_node: NodeId) -> Result<NodeFlags, AlgebraError> {
        // without inter, no need to check
        if !self.get_meta_flags(initial_node).contains_inter() {
            return Ok(NodeFlags::ZERO);
        }

        let mut visited: FxHashMap<NodeId, NodeId> = FxHashMap::default();
        let mut worklist: VecDeque<NodeId> = VecDeque::new();
        let begin_der = self.der(initial_node, Nullability::BEGIN)?;
        let mut stack = Vec::new();
        stack.push(begin_der);
        let found_nullable_right_away = self.iter_find_stack(&mut stack, |node| {
            visited.insert(node, initial_node);
            let nullability = self.nullability(node);
            if nullability != Nullability::NEVER {
                true
            } else {
                worklist.push_back(node);
                false
            }
        });
        if found_nullable_right_away {
            return Ok(NodeFlags::ZERO);
        }

        worklist.push_back(initial_node);
        let isempty_flag: NodeFlags;
        let mut found_node = NodeId::BOT;

        loop {
            match worklist.pop_front() {
                None => {
                    isempty_flag = NodeFlags::IS_EMPTY;
                    break;
                }
                Some(outer) => {
                    if let Some(cached) = self.cache_empty.get(&outer) {
                        if cached.is_checked() {
                            if cached.is_empty() {
                                continue;
                            } else {
                                return Ok(NodeFlags::ZERO);
                            }
                        }
                    }

                    let derivative = self.der(outer, Nullability::CENTER)?;

                    stack.push(derivative);

                    let found_null = self.iter_find_stack(&mut stack, |node| {
                        if let std::collections::hash_map::Entry::Vacant(e) = visited.entry(node) {
                            found_node = node;
                            if !self.get_meta_flags(node).contains_inter() {
                                true
                            } else {
                                e.insert(outer);
                                worklist.push_front(node);
                                self.any_nonbegin_nullable(node)
                            }
                        } else {
                            false
                        }
                    });
                    if found_null {
                        self.cache_empty.insert(outer, NodeFlags::IS_CHECKED);
                        isempty_flag = NodeFlags::ZERO;
                        break;
                    }
                }
            }
        }

        self.cache_empty.insert(
            initial_node,
            NodeFlags(isempty_flag.0 | NodeFlags::IS_CHECKED.0),
        );
        Ok(isempty_flag)
    }

    /// check if `larger_lang` subsumes `smaller_lang` (i.e. L(smaller) ⊆ L(larger)).
    pub fn subsumes(&mut self, larger_lang: NodeId, smaller_lang: NodeId) -> Option<bool> {
        if larger_lang == smaller_lang {
            return Some(true);
        }

        // assess initial nullability
        if self
            .nullability(larger_lang)
            .not()
            .and(self.nullability(smaller_lang))
            != Nullability::NEVER
        {
            return Some(false);
        }

        // check language nullability
        // if (B &~ A) ≡ ⊥ then B ⊆ A
        // this means  L(B) \ L(A) = {}
        // eg. (a &~ .*) = ⊥ means .* subsumes a
        let (smaller_lang, larger_lang) =
            if self.contains_look(smaller_lang) || self.contains_look(larger_lang) {
                let wrap = |b: &mut Self, n: NodeId| {
                    let tmp = b.mk_concat(n, NodeId::TS);
                    b.mk_concat(NodeId::TS, tmp)
                };
                (wrap(self, smaller_lang), wrap(self, larger_lang))
            } else {
                (smaller_lang, larger_lang)
            };

        let nota = self.mk_compl(larger_lang);
        let diff = self.mk_inter(smaller_lang, nota);
        self.is_empty_lang(diff)
    }
}
