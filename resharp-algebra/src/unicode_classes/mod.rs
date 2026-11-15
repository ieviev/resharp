#[allow(clippy::let_and_return)]
mod classes;

use crate::{NodeId, RegexBuilder};

pub use classes::{
    build_digit_class, build_digit_class_full, build_space_class, build_space_class_full,
    build_word_class, build_word_class_full,
};

/// Node matching any single UTF-8 codepoint.
pub fn utf8_char(b: &mut RegexBuilder) -> NodeId {
    let ascii = b.mk_range_u8(0, 127);
    let cont = b.mk_range_u8(0x80, 0xBF);
    let c2 = b.mk_range_u8(0xC0, 0xDF);
    let c2s = b.mk_concat(c2, cont);
    let e0 = b.mk_range_u8(0xE0, 0xEF);
    let e0s = b.mk_concats([e0, cont, cont].into_iter());
    let f0 = b.mk_range_u8(0xF0, 0xF7);
    let f0s = b.mk_concats([f0, cont, cont, cont].into_iter());
    b.mk_unions([ascii, c2s, e0s, f0s].into_iter())
}

/// Complement of `positive` restricted to the UTF-8 codepoint universe.
pub fn neg_class(b: &mut RegexBuilder, positive: NodeId) -> NodeId {
    let neg = b.mk_compl(positive);
    let uc = utf8_char(b);
    b.mk_inters([neg, uc].into_iter())
}

#[derive(Clone, Debug)]
pub struct UnicodeClassCache {
    pub word: NodeId,
    pub non_word: NodeId,
    pub digit: NodeId,
    pub non_digit: NodeId,
    pub space: NodeId,
    pub non_space: NodeId,
    pub wb: NodeId,
    pub non_wb: NodeId,
}

impl Default for UnicodeClassCache {
    fn default() -> Self {
        UnicodeClassCache {
            word: NodeId::MISSING,
            non_word: NodeId::MISSING,
            digit: NodeId::MISSING,
            non_digit: NodeId::MISSING,
            space: NodeId::MISSING,
            non_space: NodeId::MISSING,
            wb: NodeId::MISSING,
            non_wb: NodeId::MISSING,
        }
    }
}

impl UnicodeClassCache {
    pub fn ensure_word(&mut self, b: &mut RegexBuilder) {
        if self.word == NodeId::MISSING {
            self.word = build_word_class(b);
            self.non_word = neg_class(b, self.word);
        }
    }

    pub fn ensure_word_ascii(&mut self, b: &mut RegexBuilder) {
        if self.word != NodeId::MISSING {
            return;
        }
        let az = b.mk_range_u8(b'a', b'z');
        let big = b.mk_range_u8(b'A', b'Z');
        let dig = b.mk_range_u8(b'0', b'9');
        let us = b.mk_u8(b'_');
        self.word = b.mk_unions([az, big, dig, us].into_iter());
        self.non_word = neg_class(b, self.word);
    }

    pub fn ensure_word_full(&mut self, b: &mut RegexBuilder) {
        if self.word == NodeId::MISSING {
            self.word = build_word_class_full(b);
            self.non_word = neg_class(b, self.word);
        }
    }

    pub fn ensure_digit(&mut self, b: &mut RegexBuilder) {
        if self.digit == NodeId::MISSING {
            self.digit = build_digit_class(b);
            self.non_digit = neg_class(b, self.digit);
        }
    }

    pub fn ensure_digit_full(&mut self, b: &mut RegexBuilder) {
        if self.digit == NodeId::MISSING {
            self.digit = build_digit_class_full(b);
            self.non_digit = neg_class(b, self.digit);
        }
    }

    pub fn ensure_space(&mut self, b: &mut RegexBuilder) {
        if self.space == NodeId::MISSING {
            self.space = build_space_class(b);
            self.non_space = neg_class(b, self.space);
        }
    }

    pub fn ensure_space_full(&mut self, b: &mut RegexBuilder) {
        if self.space == NodeId::MISSING {
            self.space = build_space_class_full(b);
            self.non_space = neg_class(b, self.space);
        }
    }

    // \b  = (?<=\w)(?!\w) | (?<!\w)(?=\w)
    // \B  = (?<=\w)(?=\w)  | (?<!\w)(?!\w)
    pub fn ensure_wb(&mut self, b: &mut RegexBuilder) {
        if self.wb != NodeId::MISSING {
            return;
        }
        debug_assert!(
            self.word != NodeId::MISSING,
            "call ensure_word(_full|_ascii) first"
        );
        let w = self.word;
        let lb_w = b.mk_lookbehind(w, NodeId::MISSING);
        let lb_nw = b.mk_neg_lookbehind(w);
        let la_w = {
            let tail = b.mk_concat(w, NodeId::TS);
            b.mk_lookahead(tail, NodeId::MISSING, 0)
        };
        let la_nw = b.mk_neg_lookahead(w, 0);
        let wb_a = b.mk_concat(lb_w, la_nw);
        let wb_b = b.mk_concat(lb_nw, la_w);
        self.wb = b.mk_union(wb_a, wb_b);
        let nwb_a = b.mk_concat(lb_w, la_w);
        let nwb_b = b.mk_concat(lb_nw, la_nw);
        self.non_wb = b.mk_union(nwb_a, nwb_b);
    }
}
