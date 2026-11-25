//! AST-to-HIR translation, with crate-local case and perl tables.

use regex_syntax::{
    ast::{self, Ast, ClassAsciiKind, ClassPerlKind, ClassSet, ClassSetItem},
    hir::{Class, ClassBytes, ClassBytesRange, ClassUnicode, ClassUnicodeRange, Hir},
};

#[cfg(not(feature = "unicode-case"))]
use crate::unicode_tables::CASE_EXTRA;
use crate::unicode_tables::{PERL_DIGIT, PERL_SPACE, PERL_WORD};

/// Unsupported AST form.
#[derive(Debug)]
pub struct TranslateError;

#[derive(Clone, Copy, Debug)]
pub struct TranslatorBuilder {
    unicode: bool,
    case_insensitive: bool,
}

impl Default for TranslatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslatorBuilder {
    pub fn new() -> Self {
        Self { unicode: true, case_insensitive: false }
    }

    pub fn unicode(&mut self, yes: bool) -> &mut Self {
        self.unicode = yes;
        self
    }

    pub fn case_insensitive(&mut self, yes: bool) -> &mut Self {
        self.case_insensitive = yes;
        self
    }

    pub fn build(&self) -> Translator {
        Translator { unicode: self.unicode, case_insensitive: self.case_insensitive }
    }
}

#[derive(Clone, Debug)]
pub struct Translator {
    unicode: bool,
    case_insensitive: bool,
}

enum Scalar {
    Char(char),
    Byte(u8),
}

impl Translator {
    pub fn translate(&self, ast: &Ast) -> Result<Hir, TranslateError> {
        match ast {
            Ast::Literal(l) => Ok(self.literal(l)),
            Ast::ClassBracketed(c) => self.bracketed(c),
            Ast::ClassUnicode(c) => Ok(Hir::class(Class::Unicode(self.unicode_prop(c)?))),
            _ => Err(TranslateError),
        }
    }

    fn scalar(&self, l: &ast::Literal) -> Scalar {
        if self.unicode {
            return Scalar::Char(l.c);
        }
        match l.byte() {
            None => Scalar::Char(l.c),
            Some(b) if b <= 0x7F => Scalar::Char(char::from(b)),
            Some(b) => Scalar::Byte(b),
        }
    }

    fn literal(&self, l: &ast::Literal) -> Hir {
        match self.scalar(l) {
            Scalar::Byte(b) => Hir::literal(vec![b]),
            Scalar::Char(c) => self.char_hir(c),
        }
    }

    fn char_hir(&self, c: char) -> Hir {
        if self.case_insensitive {
            if self.unicode {
                let mut cls = ClassUnicode::new([ClassUnicodeRange::new(c, c)]);
                fold_unicode(&mut cls);
                if cls.ranges().len() > 1 {
                    return Hir::class(Class::Unicode(cls));
                }
            } else if c.is_ascii_alphabetic() {
                let b = u8::try_from(c).expect("ascii checked");
                let mut cls = ClassBytes::new([ClassBytesRange::new(b, b)]);
                cls.case_fold_simple();
                return Hir::class(Class::Bytes(cls));
            }
        }
        Hir::literal(c.encode_utf8(&mut [0; 4]).as_bytes().to_vec())
    }

    fn bracketed(&self, c: &ast::ClassBracketed) -> Result<Hir, TranslateError> {
        if self.unicode {
            let cls = self.unicode_set(set_item(&c.kind)?)?;
            Ok(Hir::class(Class::Unicode(self.fold_negate_unicode(cls, c.negated))))
        } else {
            let cls = self.bytes_set(set_item(&c.kind)?)?;
            Ok(Hir::class(Class::Bytes(self.fold_negate_bytes(cls, c.negated))))
        }
    }

    fn unicode_set(&self, item: &ClassSetItem) -> Result<ClassUnicode, TranslateError> {
        Ok(match item {
            ClassSetItem::Empty(_) => ClassUnicode::empty(),
            ClassSetItem::Literal(x) => ClassUnicode::new([ClassUnicodeRange::new(x.c, x.c)]),
            ClassSetItem::Range(x) => {
                ClassUnicode::new([ClassUnicodeRange::new(x.start.c, x.end.c)])
            }
            ClassSetItem::Ascii(x) => {
                self.fold_negate_unicode(ascii_unicode(&x.kind), x.negated)
            }
            ClassSetItem::Perl(x) => {
                let mut cls = perl_unicode(&x.kind);
                if x.negated {
                    cls.negate();
                }
                cls
            }
            ClassSetItem::Unicode(x) => self.unicode_prop(x)?,
            ClassSetItem::Bracketed(b) => {
                let inner = self.unicode_set(set_item(&b.kind)?)?;
                self.fold_negate_unicode(inner, b.negated)
            }
            ClassSetItem::Union(u) => {
                let mut cls = ClassUnicode::empty();
                for i in &u.items {
                    cls.union(&self.unicode_set(i)?);
                }
                cls
            }
        })
    }

    fn bytes_set(&self, item: &ClassSetItem) -> Result<ClassBytes, TranslateError> {
        Ok(match item {
            ClassSetItem::Empty(_) => ClassBytes::empty(),
            ClassSetItem::Literal(x) => {
                let b = self.class_byte(x)?;
                ClassBytes::new([ClassBytesRange::new(b, b)])
            }
            ClassSetItem::Range(x) => {
                let (s, e) = (self.class_byte(&x.start)?, self.class_byte(&x.end)?);
                ClassBytes::new([ClassBytesRange::new(s, e)])
            }
            ClassSetItem::Ascii(x) => self.fold_negate_bytes(ascii_bytes(&x.kind), x.negated),
            ClassSetItem::Perl(x) => {
                let mut cls = perl_bytes(&x.kind);
                if x.negated {
                    cls.negate();
                }
                cls
            }
            ClassSetItem::Unicode(_) => return Err(TranslateError),
            ClassSetItem::Bracketed(b) => {
                let inner = self.bytes_set(set_item(&b.kind)?)?;
                self.fold_negate_bytes(inner, b.negated)
            }
            ClassSetItem::Union(u) => {
                let mut cls = ClassBytes::empty();
                for i in &u.items {
                    cls.union(&self.bytes_set(i)?);
                }
                cls
            }
        })
    }

    fn class_byte(&self, l: &ast::Literal) -> Result<u8, TranslateError> {
        match self.scalar(l) {
            Scalar::Byte(b) => Ok(b),
            Scalar::Char(c) if c.is_ascii() => Ok(c as u8),
            Scalar::Char(_) => Err(TranslateError),
        }
    }

    fn fold_negate_unicode(&self, mut cls: ClassUnicode, negated: bool) -> ClassUnicode {
        if self.case_insensitive {
            fold_unicode(&mut cls);
        }
        if negated {
            cls.negate();
        }
        cls
    }

    fn fold_negate_bytes(&self, mut cls: ClassBytes, negated: bool) -> ClassBytes {
        if self.case_insensitive {
            cls.case_fold_simple();
        }
        if negated {
            cls.negate();
        }
        cls
    }

    fn unicode_prop(&self, c: &ast::ClassUnicode) -> Result<ClassUnicode, TranslateError> {
        use regex_syntax::hir::{translate::TranslatorBuilder as RegexBuilder, HirKind};
        let mut builder = RegexBuilder::new();
        builder.unicode(self.unicode).case_insensitive(self.case_insensitive);
        let hir = builder
            .build()
            .translate("", &Ast::class_unicode(c.clone()))
            .map_err(|_| TranslateError)?;
        match hir.kind() {
            HirKind::Class(Class::Unicode(cls)) => Ok(cls.clone()),
            _ => Err(TranslateError),
        }
    }
}

fn set_item(set: &ClassSet) -> Result<&ClassSetItem, TranslateError> {
    match set {
        ClassSet::Item(item) => Ok(item),
        ClassSet::BinaryOp(_) => Err(TranslateError),
    }
}

#[cfg(feature = "unicode-case")]
fn fold_unicode(cls: &mut ClassUnicode) {
    cls.case_fold_simple();
}

#[cfg(not(feature = "unicode-case"))]
fn fold_unicode(cls: &mut ClassUnicode) {
    let extra: Vec<_> = cls
        .iter()
        .flat_map(|r| {
            let (lo, hi) = (r.start(), r.end());
            let first = CASE_EXTRA.partition_point(|&(from, _)| from < lo);
            CASE_EXTRA[first..]
                .iter()
                .take_while(move |&&(from, _)| from <= hi)
                .map(|&(_, to)| ClassUnicodeRange::new(to, to))
        })
        .collect();
    cls.union(&ClassUnicode::new(extra));
}

fn perl_unicode(kind: &ClassPerlKind) -> ClassUnicode {
    let table = match kind {
        ClassPerlKind::Digit => PERL_DIGIT,
        ClassPerlKind::Space => PERL_SPACE,
        ClassPerlKind::Word => PERL_WORD,
    };
    ClassUnicode::new(table.iter().map(|&(s, e)| ClassUnicodeRange::new(s, e)))
}

fn perl_bytes(kind: &ClassPerlKind) -> ClassBytes {
    ascii_bytes(match kind {
        ClassPerlKind::Digit => &ClassAsciiKind::Digit,
        ClassPerlKind::Space => &ClassAsciiKind::Space,
        ClassPerlKind::Word => &ClassAsciiKind::Word,
    })
}

fn ascii_unicode(kind: &ClassAsciiKind) -> ClassUnicode {
    ClassUnicode::new(
        ascii_ranges(kind)
            .iter()
            .map(|&(s, e)| ClassUnicodeRange::new(char::from(s), char::from(e))),
    )
}

fn ascii_bytes(kind: &ClassAsciiKind) -> ClassBytes {
    ClassBytes::new(ascii_ranges(kind).iter().map(|&(s, e)| ClassBytesRange::new(s, e)))
}

fn ascii_ranges(kind: &ClassAsciiKind) -> &'static [(u8, u8)] {
    match kind {
        ClassAsciiKind::Alnum => &[(b'0', b'9'), (b'A', b'Z'), (b'a', b'z')],
        ClassAsciiKind::Alpha => &[(b'A', b'Z'), (b'a', b'z')],
        ClassAsciiKind::Ascii => &[(0x00, 0x7F)],
        ClassAsciiKind::Blank => &[(b'\t', b'\t'), (b' ', b' ')],
        ClassAsciiKind::Cntrl => &[(0x00, 0x1F), (0x7F, 0x7F)],
        ClassAsciiKind::Digit => &[(b'0', b'9')],
        ClassAsciiKind::Graph => &[(b'!', b'~')],
        ClassAsciiKind::Lower => &[(b'a', b'z')],
        ClassAsciiKind::Print => &[(b' ', b'~')],
        ClassAsciiKind::Punct => &[(b'!', b'/'), (b':', b'@'), (b'[', b'`'), (b'{', b'~')],
        ClassAsciiKind::Space => &[
            (b'\t', b'\t'),
            (b'\n', b'\n'),
            (0x0B, 0x0B),
            (0x0C, 0x0C),
            (b'\r', b'\r'),
            (b' ', b' '),
        ],
        ClassAsciiKind::Upper => &[(b'A', b'Z')],
        ClassAsciiKind::Word => &[(b'0', b'9'), (b'A', b'Z'), (b'_', b'_'), (b'a', b'z')],
        ClassAsciiKind::Xdigit => &[(b'0', b'9'), (b'A', b'F'), (b'a', b'f')],
    }
}
