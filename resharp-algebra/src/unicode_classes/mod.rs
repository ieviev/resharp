mod classes;

use crate::{NodeId, RegexBuilder};

pub use classes::{
    build_digit_class, build_digit_class_full, build_space_class, build_word_class,
    build_word_class_full,
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
}
