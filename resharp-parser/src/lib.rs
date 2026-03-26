//! Parser for resharp regex patterns.
//!
//! Converts regex pattern strings into the node representation used by resharp-algebra.

#![warn(dead_code)]
mod ast;
use std::cell::{Cell, RefCell};

use ast::{Ast, Concat, ErrorKind, GroupKind, LookaroundKind};
use regex_syntax::{
    ast::{
        ClassAscii, ClassBracketed, ClassPerl, ClassSet, ClassSetBinaryOpKind, ClassSetItem,
        ClassSetRange, ClassSetUnion, ClassUnicode, ClassUnicodeKind, ClassUnicodeOpKind,
        HexLiteralKind, Literal, LiteralKind, Position, Span, SpecialLiteralKind,
    },
    hir::{
        self,
        translate::{Translator, TranslatorBuilder},
    },
    utf8::Utf8Sequences,
};
use resharp_algebra::NodeId;

type TB<'s> = resharp_algebra::RegexBuilder;

/// global pattern-level flags, set from `EngineOptions`.
pub struct PatternFlags {
    /// `\w`/`\d`/`\s` match full Unicode (true) or ASCII only (false).
    pub unicode: bool,
    /// global case-insensitive matching.
    pub case_insensitive: bool,
    /// `.` matches `\n` (behaves like `_`).
    pub dot_matches_new_line: bool,
    /// allow whitespace and `#` comments in the pattern.
    pub ignore_whitespace: bool,
}

impl Default for PatternFlags {
    fn default() -> Self {
        Self {
            unicode: true,
            case_insensitive: false,
            dot_matches_new_line: false,
            ignore_whitespace: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum WordCharKind {
    Word,
    NonWord,
    MaybeWord,
    MaybeNonWord,
    Unknown,
    Edge,
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}


#[derive(Clone, Debug, Eq, PartialEq)]
enum Primitive {
    Literal(Literal),
    Assertion(ast::Assertion),
    Dot(Span),
    Top(Span),
    Perl(ClassPerl),
    Unicode(ClassUnicode),
}

impl Primitive {
    fn span(&self) -> &Span {
        match *self {
            Primitive::Literal(ref x) => &x.span,
            Primitive::Assertion(ref x) => &x.span,
            Primitive::Dot(ref span) => span,
            Primitive::Top(ref span) => span,
            Primitive::Perl(ref x) => &x.span,
            Primitive::Unicode(ref x) => &x.span,
        }
    }

    fn into_ast(self) -> Ast {
        match self {
            Primitive::Literal(lit) => Ast::literal(lit),
            Primitive::Assertion(assert) => Ast::assertion(assert),
            Primitive::Dot(span) => Ast::dot(span),
            Primitive::Top(span) => Ast::top(span),
            Primitive::Perl(cls) => Ast::class_perl(cls),
            Primitive::Unicode(cls) => Ast::class_unicode(cls),
        }
    }

    fn into_class_set_item(self, p: &ResharpParser) -> Result<regex_syntax::ast::ClassSetItem> {
        use self::Primitive::*;
        use regex_syntax::ast::ClassSetItem;

        match self {
            Literal(lit) => Ok(ClassSetItem::Literal(lit)),
            Perl(cls) => Ok(ClassSetItem::Perl(cls)),
            Unicode(cls) => Ok(ClassSetItem::Unicode(cls)),
            x => Err(p.error(*x.span(), ast::ErrorKind::ClassEscapeInvalid)),
        }
    }

    fn into_class_literal(self, p: &ResharpParser) -> Result<Literal> {
        use self::Primitive::*;

        match self {
            Literal(lit) => Ok(lit),
            x => Err(p.error(*x.span(), ast::ErrorKind::ClassRangeLiteral)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Either<Left, Right> {
    Left(Left),
    Right(Right),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResharpError {
    /// The kind of error.
    pub kind: ErrorKind,
    /// The original pattern that the parser generated the error from. Every
    /// span in an error is a valid range into this string.
    pattern: String,
    /// The span of this error.
    pub span: Span,
}

impl std::fmt::Display for ResharpError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}: {:?}", self.kind, self.span)
    }
}
impl std::error::Error for ResharpError {}

type Result<T> = core::result::Result<T, ResharpError>;

#[derive(Clone, Debug)]
enum GroupState {
    /// This state is pushed whenever an opening group is found.
    Group {
        /// The concatenation immediately preceding the opening group.
        concat: Concat,
        /// The group that has been opened. Its sub-AST is always empty.
        group: ast::Group,
        /// Whether this group has the `x` flag enabled or not.
        ignore_whitespace: bool,
    },
    /// This state is pushed whenever a new alternation branch is found. If
    /// an alternation branch is found and this state is at the top of the
    /// stack, then this state should be modified to include the new
    /// alternation.
    Alternation(ast::Alternation),
    Intersection(ast::Intersection),
}

#[derive(Clone, Debug)]
enum ClassState {
    /// This state is pushed whenever an opening bracket is found.
    Open {
        /// The union of class items immediately preceding this class.
        union: regex_syntax::ast::ClassSetUnion,
        /// The class that has been opened. Typically this just corresponds
        /// to the `[`, but it can also include `[^` since `^` indicates
        /// negation of the class.
        set: regex_syntax::ast::ClassBracketed,
    },
    /// This state is pushed when a operator is seen. When popped, the stored
    /// set becomes the left hand side of the operator.
    Op {
        /// The type of the operation, i.e., &&, -- or ~~.
        kind: regex_syntax::ast::ClassSetBinaryOpKind,
        /// The left-hand side of the operator.
        lhs: regex_syntax::ast::ClassSet,
    },
}

/// RE# syntax parser based on the regex-syntax crate.
pub struct ResharpParser<'s> {
    perl_classes: Vec<(bool, regex_syntax::ast::ClassPerlKind, NodeId)>,
    unicode_classes: resharp_algebra::UnicodeClassCache,
    pub translator: regex_syntax::hir::translate::Translator,
    pub pattern: &'s str,
    pos: Cell<Position>,
    capture_index: Cell<u32>,
    octal: bool,
    empty_min_range: bool,
    ignore_whitespace: Cell<bool>,
    dot_all: Cell<bool>,
    global_unicode: bool,
    global_case_insensitive: bool,
    comments: RefCell<Vec<ast::Comment>>,
    stack_group: RefCell<Vec<GroupState>>,
    stack_class: RefCell<Vec<ClassState>>,
    capture_names: RefCell<Vec<ast::CaptureName>>,
    scratch: RefCell<String>,
}

fn specialize_err<T>(result: Result<T>, from: ast::ErrorKind, to: ast::ErrorKind) -> Result<T> {
    result.map_err(|e| {
        if e.kind == from {
            ResharpError { kind: to, pattern: e.pattern, span: e.span }
        } else {
            e
        }
    })
}

fn is_capture_char(c: char, first: bool) -> bool {
    if first {
        c == '_' || c.is_alphabetic()
    } else {
        c == '_' || c == '.' || c == '[' || c == ']' || c.is_alphanumeric()
    }
}

pub fn is_meta_character(c: char) -> bool {
    matches!(c, '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        | '#' | '&' | '-' | '~' | '_')
}

/// escapes all resharp meta characters in `text`.
pub fn escape(text: &str) -> String {
    let mut buf = String::new();
    escape_into(text, &mut buf);
    buf
}

/// escapes all resharp meta characters in `text` and appends to `buf`.
pub fn escape_into(text: &str, buf: &mut String) {
    buf.reserve(text.len());
    for c in text.chars() {
        if is_meta_character(c) {
            buf.push('\\');
        }
        buf.push(c);
    }
}

pub fn is_escapeable_character(c: char) -> bool {
    // Certainly escapeable if it's a meta character.
    if is_meta_character(c) {
        return true;
    }
    // Any character that isn't ASCII is definitely not escapeable. There's
    // no real need to allow things like \☃ right?
    if !c.is_ascii() {
        return false;
    }
    // Otherwise, we basically say that everything is escapeable unless it's a
    // letter or digit. Things like \3 are either octal (when enabled) or an
    // error, and we should keep it that way. Otherwise, letters are reserved
    // for adding new syntax in a backwards compatible way.
    match c {
        '0'..='9' | 'A'..='Z' | 'a'..='z' => false,
        // While not currently supported, we keep these as not escapeable to
        // give us some flexibility with respect to supporting the \< and
        // \> word boundary assertions in the future. By rejecting them as
        // escapeable, \< and \> will result in a parse error. Thus, we can
        // turn them into something else in the future without it being a
        // backwards incompatible change.
        //
        // OK, now we support \< and \>, and we need to retain them as *not*
        // escapeable here since the escape sequence is significant.
        '<' | '>' => false,
        _ => true,
    }
}

fn is_hex(c: char) -> bool {
    c.is_ascii_digit() || ('a'..='f').contains(&c) || ('A'..='F').contains(&c)
}

impl<'s> ResharpParser<'s> {
    fn default_translator_builder(&self) -> TranslatorBuilder {
        let mut trb = TranslatorBuilder::new();
        trb.unicode(self.global_unicode);
        trb.utf8(false);
        trb.case_insensitive(self.global_case_insensitive);
        trb
    }

    pub fn new(pattern: &'s str) -> Self {
        Self::with_flags(pattern, &PatternFlags::default())
    }

    pub fn with_flags(pattern: &'s str, flags: &PatternFlags) -> Self {
        let mut trb = TranslatorBuilder::new();
        trb.unicode(flags.unicode);
        trb.utf8(false);
        trb.case_insensitive(flags.case_insensitive);
        Self {
            translator: trb.build(),
            pattern,
            perl_classes: vec![],
            unicode_classes: resharp_algebra::UnicodeClassCache::default(),
            pos: Cell::new(Position::new(0, 0, 0)),
            capture_index: Cell::new(0),
            octal: false,
            empty_min_range: false,
            ignore_whitespace: Cell::new(flags.ignore_whitespace),
            dot_all: Cell::new(flags.dot_matches_new_line),
            global_unicode: flags.unicode,
            global_case_insensitive: flags.case_insensitive,
            comments: RefCell::new(vec![]),
            stack_group: RefCell::new(vec![]),
            stack_class: RefCell::new(vec![]),
            capture_names: RefCell::new(vec![]),
            scratch: RefCell::new(String::new()),
        }
    }

    /// Return a reference to the parser state.
    fn parser(&'_ self) -> &'_ ResharpParser<'_> {
        self
    }

    /// Return a reference to the pattern being parsed.
    fn pattern(&self) -> &str {
        self.pattern
    }

    /// Create a new error with the given span and error type.
    fn error(&self, span: Span, kind: ast::ErrorKind) -> ResharpError {
        ResharpError {
            kind,
            pattern: self.pattern().to_string(),
            span,
        }
    }

    fn unsupported_error(&self, _: regex_syntax::hir::Error) -> ResharpError {
        self.error(Span::splat(self.pos()), ast::ErrorKind::UnsupportedResharpRegex)
    }

    /// Return the current offset of the parser.
    ///
    /// The offset starts at `0` from the beginning of the regular expression
    /// pattern string.
    fn offset(&self) -> usize {
        self.parser().pos.get().offset
    }

    /// Return the current line number of the parser.
    ///
    /// The line number starts at `1`.
    fn line(&self) -> usize {
        self.parser().pos.get().line
    }

    /// Return the current column of the parser.
    ///
    /// The column number starts at `1` and is reset whenever a `\n` is seen.
    fn column(&self) -> usize {
        self.parser().pos.get().column
    }

    /// Return the next capturing index. Each subsequent call increments the
    /// internal index.
    ///
    /// The span given should correspond to the location of the opening
    /// parenthesis.
    ///
    /// If the capture limit is exceeded, then an error is returned.
    fn next_capture_index(&self, span: Span) -> Result<u32> {
        let current = self.parser().capture_index.get();
        let i = current
            .checked_add(1)
            .ok_or_else(|| self.error(span, ast::ErrorKind::CaptureLimitExceeded))?;
        self.parser().capture_index.set(i);
        Ok(i)
    }

    fn add_capture_name(&self, cap: &ast::CaptureName) -> Result<()> {
        let mut names = self.parser().capture_names.borrow_mut();
        match names.binary_search_by_key(&cap.name.as_str(), |c| c.name.as_str()) {
            Err(i) => {
                names.insert(i, cap.clone());
                Ok(())
            }
            Ok(i) => Err(self.error(
                cap.span,
                ast::ErrorKind::GroupNameDuplicate {
                    original: names[i].span,
                },
            )),
        }
    }

    fn ignore_whitespace(&self) -> bool {
        self.parser().ignore_whitespace.get()
    }

    fn char(&self) -> char {
        self.char_at(self.offset())
    }

    fn char_at(&self, i: usize) -> char {
        self.pattern()[i..]
            .chars()
            .next()
            .unwrap_or_else(|| panic!("expected char at offset {}", i))
    }

    fn bump(&self) -> bool {
        if self.is_eof() {
            return false;
        }
        let Position {
            mut offset,
            mut line,
            mut column,
        } = self.pos();
        if self.char() == '\n' {
            line = line.checked_add(1).unwrap();
            column = 1;
        } else {
            column = column.checked_add(1).unwrap();
        }
        offset += self.char().len_utf8();
        self.parser().pos.set(Position {
            offset,
            line,
            column,
        });
        self.pattern()[self.offset()..].chars().next().is_some()
    }

    fn bump_if(&self, prefix: &str) -> bool {
        if self.pattern()[self.offset()..].starts_with(prefix) {
            for _ in 0..prefix.chars().count() {
                self.bump();
            }
            true
        } else {
            false
        }
    }

    fn is_lookaround_prefix(&self) -> Option<(bool, bool)> {
        if self.bump_if("?=") {
            return Some((true, true));
        }
        if self.bump_if("?!") {
            return Some((true, false));
        }
        if self.bump_if("?<=") {
            return Some((false, true));
        }
        if self.bump_if("?<!") {
            return Some((false, false));
        }
        None
    }

    fn bump_and_bump_space(&self) -> bool {
        if !self.bump() {
            return false;
        }
        self.bump_space();
        !self.is_eof()
    }

    fn bump_space(&self) {
        if !self.ignore_whitespace() {
            return;
        }
        while !self.is_eof() {
            if self.char().is_whitespace() {
                self.bump();
            } else if self.char() == '#' {
                let start = self.pos();
                let mut comment_text = String::new();
                self.bump();
                while !self.is_eof() {
                    let c = self.char();
                    self.bump();
                    if c == '\n' {
                        break;
                    }
                    comment_text.push(c);
                }
                let comment = ast::Comment {
                    span: Span::new(start, self.pos()),
                    comment: comment_text,
                };
                self.parser().comments.borrow_mut().push(comment);
            } else {
                break;
            }
        }
    }

    /// Peek at the next character in the input without advancing the parser.
    ///
    /// If the input has been exhausted, then this returns `None`.
    fn peek(&self) -> Option<char> {
        if self.is_eof() {
            return None;
        }
        self.pattern()[self.offset() + self.char().len_utf8()..]
            .chars()
            .next()
    }

    /// Like peek, but will ignore spaces when the parser is in whitespace
    /// insensitive mode.
    fn peek_space(&self) -> Option<char> {
        if !self.ignore_whitespace() {
            return self.peek();
        }
        if self.is_eof() {
            return None;
        }
        let mut start = self.offset() + self.char().len_utf8();
        let mut in_comment = false;
        for (i, c) in self.pattern()[start..].char_indices() {
            if c.is_whitespace() {
                continue;
            } else if !in_comment && c == '#' {
                in_comment = true;
            } else if in_comment && c == '\n' {
                in_comment = false;
            } else {
                start += i;
                break;
            }
        }
        self.pattern()[start..].chars().next()
    }

    /// Returns true if the next call to `bump` would return false.
    fn is_eof(&self) -> bool {
        self.offset() == self.pattern().len()
    }

    /// Return the current position of the parser, which includes the offset,
    /// line and column.
    fn pos(&self) -> Position {
        self.parser().pos.get()
    }

    /// Create a span at the current position of the parser. Both the start
    /// and end of the span are set.
    fn span(&self) -> Span {
        Span::splat(self.pos())
    }

    /// Create a span that covers the current character.
    fn span_char(&self) -> Span {
        let mut next = Position {
            offset: self.offset().checked_add(self.char().len_utf8()).unwrap(),
            line: self.line(),
            column: self.column().checked_add(1).unwrap(),
        };
        if self.char() == '\n' {
            next.line += 1;
            next.column = 1;
        }
        Span::new(self.pos(), next)
    }

    /// Parse and push a single alternation on to the parser's internal stack.
    /// If the top of the stack already has an alternation, then add to that
    /// instead of pushing a new one.
    ///
    /// The concatenation given corresponds to a single alternation branch.
    /// The concatenation returned starts the next branch and is empty.
    ///
    /// This assumes the parser is currently positioned at `|` and will advance
    /// the parser to the character following `|`.
    #[inline(never)]
    fn push_alternate(&self, mut concat: ast::Concat) -> Result<ast::Concat> {
        assert_eq!(self.char(), '|');
        concat.span.end = self.pos();
        self.push_or_add_alternation(concat);
        self.bump();
        Ok(ast::Concat {
            span: self.span(),
            asts: vec![],
        })
    }

    /// Pushes or adds the given branch of an alternation to the parser's
    /// internal stack of state.
    fn push_or_add_alternation(&self, concat: Concat) {
        use self::GroupState::*;

        let mut stack = self.parser().stack_group.borrow_mut();
        if let Some(&mut Alternation(ref mut alts)) = stack.last_mut() {
            alts.asts.push(concat.into_ast());
            return;
        }
        stack.push(Alternation(ast::Alternation {
            span: Span::new(concat.span.start, self.pos()),
            asts: vec![concat.into_ast()],
        }));
    }

    #[inline(never)]
    fn push_intersect(&self, mut concat: Concat) -> Result<Concat> {
        assert_eq!(self.char(), '&');
        concat.span.end = self.pos();
        self.push_or_add_intersect(concat);
        self.bump();
        Ok(Concat {
            span: self.span(),
            asts: vec![],
        })
    }

    /// Pushes or adds the given branch of an alternation to the parser's
    /// internal stack of state.
    fn push_or_add_intersect(&self, concat: Concat) {
        use self::GroupState::*;

        let mut stack = self.parser().stack_group.borrow_mut();
        if let Some(&mut Intersection(ref mut alts)) = stack.last_mut() {
            alts.asts.push(concat.into_ast());
            return;
        }
        stack.push(Intersection(ast::Intersection {
            span: Span::new(concat.span.start, self.pos()),
            asts: vec![concat.into_ast()],
        }));
    }

    /// Parse and push a group AST (and its parent concatenation) on to the
    /// parser's internal stack. Return a fresh concatenation corresponding
    /// to the group's sub-AST.
    ///
    /// If a set of flags was found (with no group), then the concatenation
    /// is returned with that set of flags added.
    ///
    /// This assumes that the parser is currently positioned on the opening
    /// parenthesis. It advances the parser to the character at the start
    /// of the sub-expression (or adjoining expression).
    ///
    /// If there was a problem parsing the start of the group, then an error
    /// is returned.
    #[inline(never)]
    fn push_group(&self, mut concat: Concat) -> Result<Concat> {
        assert_eq!(self.char(), '(');
        match self.parse_group()? {
            Either::Left(set) => {
                let ignore = set.flags.flag_state(ast::Flag::IgnoreWhitespace);
                if let Some(v) = ignore {
                    self.parser().ignore_whitespace.set(v);
                }

                concat.asts.push(Ast::flags(set));
                Ok(concat)
            }
            Either::Right(group) => {
                let old_ignore_whitespace = self.ignore_whitespace();
                let new_ignore_whitespace = group
                    .flags()
                    .and_then(|f| f.flag_state(ast::Flag::IgnoreWhitespace))
                    .unwrap_or(old_ignore_whitespace);
                self.parser()
                    .stack_group
                    .borrow_mut()
                    .push(GroupState::Group {
                        concat,
                        group,
                        ignore_whitespace: old_ignore_whitespace,
                    });
                self.parser().ignore_whitespace.set(new_ignore_whitespace);
                Ok(Concat {
                    span: self.span(),
                    asts: vec![],
                })
            }
        }
    }

    #[inline(never)]
    fn push_compl_group(&self, concat: Concat) -> Result<Concat> {
        assert_eq!(self.char(), '~');
        self.bump();
        if self.is_eof() || self.char() != '(' {
            return Err(self.error(self.span(), ast::ErrorKind::ComplementGroupExpected));
        }
        let open_span = self.span_char();
        self.bump();
        let group = ast::Group {
            span: open_span,
            kind: ast::GroupKind::Complement,
            ast: Box::new(Ast::empty(self.span())),
        };

        let old_ignore_whitespace = self.ignore_whitespace();
        let new_ignore_whitespace = group
            .flags()
            .and_then(|f| f.flag_state(ast::Flag::IgnoreWhitespace))
            .unwrap_or(old_ignore_whitespace);
        self.parser()
            .stack_group
            .borrow_mut()
            .push(GroupState::Group {
                concat,
                group,
                ignore_whitespace: old_ignore_whitespace,
            });
        self.parser().ignore_whitespace.set(new_ignore_whitespace);
        Ok(Concat {
            span: self.span(),
            asts: vec![],
        })
    }

    /// Pop a group AST from the parser's internal stack and set the group's
    /// AST to the given concatenation. Return the concatenation containing
    /// the group.
    ///
    /// This assumes that the parser is currently positioned on the closing
    /// parenthesis and advances the parser to the character following the `)`.
    ///
    /// If no such group could be popped, then an unopened group error is
    /// returned.
    #[inline(never)]
    fn pop_group(&self, mut group_concat: Concat) -> Result<Concat> {
        use self::GroupState::*;
        assert_eq!(self.char(), ')');
        let mut stack = self.parser().stack_group.borrow_mut();
        let topstack = stack.pop();

        let (mut prior_concat, mut group, ignore_whitespace, alt) = match topstack {
            Some(Group {
                concat,
                group,
                ignore_whitespace,
            }) => (concat, group, ignore_whitespace, None),
            Some(Alternation(alt)) => match stack.pop() {
                Some(Group {
                    concat,
                    group,
                    ignore_whitespace,
                }) => (
                    concat,
                    group,
                    ignore_whitespace,
                    Some(Either::Left::<ast::Alternation, ast::Intersection>(alt)),
                ),
                None | Some(Alternation(_)) | Some(Intersection(_)) => {
                    return Err(self.error(self.span_char(), ast::ErrorKind::GroupUnopened));
                }
            },
            Some(Intersection(int)) => match stack.pop() {
                Some(Group {
                    concat,
                    group,
                    ignore_whitespace,
                }) => (
                    concat,
                    group,
                    ignore_whitespace,
                    Some(Either::Right::<ast::Alternation, ast::Intersection>(int)),
                ),
                None | Some(Alternation(_)) | Some(Intersection(_)) => {
                    return Err(self.error(self.span_char(), ast::ErrorKind::GroupUnopened));
                }
            },

            None => {
                return Err(self.error(self.span_char(), ast::ErrorKind::GroupUnopened));
            }
        };
        self.parser().ignore_whitespace.set(ignore_whitespace);
        group_concat.span.end = self.pos();
        self.bump();
        group.span.end = self.pos();
        match alt {
            Some(Either::Left(mut alt)) => {
                alt.span.end = group_concat.span.end;
                alt.asts.push(group_concat.into_ast());
                group.ast = Box::new(alt.into_ast());
            }
            Some(Either::Right(mut int)) => {
                int.span.end = group_concat.span.end;
                int.asts.push(group_concat.into_ast());
                group.ast = Box::new(int.into_ast());
            }
            None => {
                group.ast = Box::new(group_concat.into_ast());
            }
        }

        if group.kind == GroupKind::Complement {
            let complement = ast::Complement {
                span: self.span(),
                ast: group.ast,
            };
            prior_concat.asts.push(Ast::complement(complement));
        }
        // ignore groups for now
        else {
            prior_concat.asts.push(Ast::group(group));
        }
        Ok(prior_concat)
    }

    /// Pop the last state from the parser's internal stack, if it exists, and
    /// add the given concatenation to it. There either must be no state or a
    /// single alternation item on the stack. Any other scenario produces an
    /// error.
    ///
    /// This assumes that the parser has advanced to the end.
    #[inline(never)]
    fn pop_group_end(&self, mut concat: ast::Concat) -> Result<Ast> {
        concat.span.end = self.pos();
        let mut stack = self.parser().stack_group.borrow_mut();
        let ast = match stack.pop() {
            None => Ok(concat.into_ast()),
            Some(GroupState::Alternation(mut alt)) => {
                alt.span.end = self.pos();
                alt.asts.push(concat.into_ast());
                Ok(Ast::alternation(alt))
            }
            Some(GroupState::Intersection(mut int)) => {
                int.span.end = self.pos();
                int.asts.push(concat.into_ast());

                Ok(Ast::intersection(int))
            }
            Some(GroupState::Group { group, .. }) => {
                return Err(self.error(group.span, ast::ErrorKind::GroupUnclosed));
            }
        };
        // If we try to pop again, there should be nothing.
        match stack.pop() {
            None => ast,
            Some(GroupState::Alternation(_)) => {
                // This unreachable is unfortunate. This case can't happen
                // because the only way we can be here is if there were two
                // `GroupState::Alternation`s adjacent in the parser's stack,
                // which we guarantee to never happen because we never push a
                // `GroupState::Alternation` if one is already at the top of
                // the stack.
                unreachable!()
            }
            Some(GroupState::Intersection(_)) => {
                unreachable!()
            }
            Some(GroupState::Group { group, .. }) => {
                Err(self.error(group.span, ast::ErrorKind::GroupUnclosed))
            }
        }
    }

    /// Parse the opening of a character class and push the current class
    /// parsing context onto the parser's stack. This assumes that the parser
    /// is positioned at an opening `[`. The given union should correspond to
    /// the union of set items built up before seeing the `[`.
    ///
    /// If there was a problem parsing the opening of the class, then an error
    /// is returned. Otherwise, a new union of set items for the class is
    /// returned (which may be populated with either a `]` or a `-`).
    #[inline(never)]
    fn push_class_open(
        &self,
        parent_union: regex_syntax::ast::ClassSetUnion,
    ) -> Result<regex_syntax::ast::ClassSetUnion> {
        assert_eq!(self.char(), '[');

        let (nested_set, nested_union) = self.parse_set_class_open()?;
        self.parser()
            .stack_class
            .borrow_mut()
            .push(ClassState::Open {
                union: parent_union,
                set: nested_set,
            });
        Ok(nested_union)
    }

    /// Parse the end of a character class set and pop the character class
    /// parser stack. The union given corresponds to the last union built
    /// before seeing the closing `]`. The union returned corresponds to the
    /// parent character class set with the nested class added to it.
    ///
    /// This assumes that the parser is positioned at a `]` and will advance
    /// the parser to the byte immediately following the `]`.
    ///
    /// If the stack is empty after popping, then this returns the final
    /// "top-level" character class AST (where a "top-level" character class
    /// is one that is not nested inside any other character class).
    ///
    /// If there is no corresponding opening bracket on the parser's stack,
    /// then an error is returned.
    #[inline(never)]
    fn pop_class(
        &self,
        nested_union: regex_syntax::ast::ClassSetUnion,
    ) -> Result<Either<regex_syntax::ast::ClassSetUnion, regex_syntax::ast::ClassBracketed>> {
        assert_eq!(self.char(), ']');

        let item = regex_syntax::ast::ClassSet::Item(nested_union.into_item());
        let prevset = self.pop_class_op(item);
        let mut stack = self.parser().stack_class.borrow_mut();
        match stack.pop() {
            None => {
                // We can never observe an empty stack:
                //
                // 1) We are guaranteed to start with a non-empty stack since
                //    the character class parser is only initiated when it sees
                //    a `[`.
                // 2) If we ever observe an empty stack while popping after
                //    seeing a `]`, then we signal the character class parser
                //    to terminate.
                panic!("unexpected empty character class stack")
            }
            Some(ClassState::Op { .. }) => {
                // This panic is unfortunate, but this case is impossible
                // since we already popped the Op state if one exists above.
                // Namely, every push to the class parser stack is guarded by
                // whether an existing Op is already on the top of the stack.
                // If it is, the existing Op is modified. That is, the stack
                // can never have consecutive Op states.
                panic!("unexpected ClassState::Op")
            }
            Some(ClassState::Open { mut union, mut set }) => {
                self.bump();
                set.span.end = self.pos();
                set.kind = prevset;
                if stack.is_empty() {
                    Ok(Either::Right(set))
                } else {
                    union.push(regex_syntax::ast::ClassSetItem::Bracketed(Box::new(set)));
                    Ok(Either::Left(union))
                }
            }
        }
    }

    /// Return an "unclosed class" error whose span points to the most
    /// recently opened class.
    ///
    /// This should only be called while parsing a character class.
    #[inline(never)]
    fn unclosed_class_error(&self) -> ResharpError {
        for state in self.parser().stack_class.borrow().iter().rev() {
            if let ClassState::Open { ref set, .. } = *state {
                return self.error(set.span, ast::ErrorKind::ClassUnclosed);
            }
        }
        // We are guaranteed to have a non-empty stack with at least
        // one open bracket, so we should never get here.
        panic!("no open character class found")
    }

    /// Push the current set of class items on to the class parser's stack as
    /// the left hand side of the given operator.
    ///
    /// A fresh set union is returned, which should be used to build the right
    /// hand side of this operator.
    #[inline(never)]
    fn push_class_op(
        &self,
        next_kind: regex_syntax::ast::ClassSetBinaryOpKind,
        next_union: regex_syntax::ast::ClassSetUnion,
    ) -> regex_syntax::ast::ClassSetUnion {
        let item = regex_syntax::ast::ClassSet::Item(next_union.into_item());
        let new_lhs = self.pop_class_op(item);
        self.parser().stack_class.borrow_mut().push(ClassState::Op {
            kind: next_kind,
            lhs: new_lhs,
        });
        regex_syntax::ast::ClassSetUnion {
            span: self.span(),
            items: vec![],
        }
    }

    /// Pop a character class set from the character class parser stack. If the
    /// top of the stack is just an item (not an operation), then return the
    /// given set unchanged. If the top of the stack is an operation, then the
    /// given set will be used as the rhs of the operation on the top of the
    /// stack. In that case, the binary operation is returned as a set.
    #[inline(never)]
    fn pop_class_op(&self, rhs: regex_syntax::ast::ClassSet) -> regex_syntax::ast::ClassSet {
        let mut stack = self.parser().stack_class.borrow_mut();
        let (kind, lhs) = match stack.pop() {
            Some(ClassState::Op { kind, lhs }) => (kind, lhs),
            Some(state @ ClassState::Open { .. }) => {
                stack.push(state);
                return rhs;
            }
            None => unreachable!(),
        };
        let span = Span::new(lhs.span().start, rhs.span().end);
        regex_syntax::ast::ClassSet::BinaryOp(regex_syntax::ast::ClassSetBinaryOp {
            span,
            kind,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }

    fn hir_to_node_id(&self, hir: &hir::Hir, tb: &mut TB<'s>) -> Result<NodeId> {
        match hir.kind() {
            hir::HirKind::Empty => Ok(NodeId::EPS),
            hir::HirKind::Literal(l) => {
                if l.0.len() == 1 {
                    let node = tb.mk_u8(l.0[0]);
                    Ok(node)
                } else {
                    let ws: Vec<_> = l.0.iter().map(|l| tb.mk_u8(*l)).collect();
                    let conc = tb.mk_concats(ws.iter().copied());
                    Ok(conc)
                }
            }
            hir::HirKind::Class(class) => {
                match class {
                    hir::Class::Unicode(class_unicode) => {
                        let ranges = class_unicode.ranges();
                        let mut nodes = Vec::new();
                        for range in ranges {
                            for seq in Utf8Sequences::new(range.start(), range.end()) {
                                let sl = seq.as_slice();
                                let bytes: Vec<_> = sl.iter().map(|s| (s.start, s.end)).collect();
                                let node = match bytes.len() {
                                    1 => tb.mk_range_u8(bytes[0].0, bytes[0].1),
                                    n => {
                                        let last = tb.mk_range_u8(bytes[n - 1].0, bytes[n - 1].1);
                                        let mut conc = last;
                                        for i in (0..n - 1).rev() {
                                            let b = tb.mk_range_u8(bytes[i].0, bytes[i].1);
                                            conc = tb.mk_concat(b, conc);
                                        }
                                        conc
                                    }
                                };
                                nodes.push(node);
                            }
                        }
                        let merged = tb.mk_unions(nodes.into_iter());
                        Ok(merged)
                    }
                    hir::Class::Bytes(class_bytes) => {
                        let ranges = class_bytes.ranges();
                        let mut result = NodeId::BOT;
                        for range in ranges {
                            let start = range.start();
                            let end = range.end();
                            let node = tb.mk_range_u8(start, end);
                            result = tb.mk_union(result, node);
                        }
                        Ok(result)
                    }
                }
            }
            hir::HirKind::Look(_) => todo!(),
            hir::HirKind::Repetition(_) => todo!(),
            hir::HirKind::Capture(_) => todo!(),
            hir::HirKind::Concat(body) => {
                let mut result = NodeId::EPS;
                for child in body {
                    let node = self.hir_to_node_id(child, tb)?;
                    result = tb.mk_concat(result, node);
                }
                Ok(result)
            }
            hir::HirKind::Alternation(_) => todo!(),
        }
    }

    fn translate_ast_to_hir(
        &mut self,
        orig_ast: &regex_syntax::ast::Ast,
        tb: &mut TB<'s>,
    ) -> Result<NodeId> {
        match self.translator.translate("", orig_ast) {
            Err(_) => Err(self.error(self.span(), ast::ErrorKind::UnicodeClassInvalid)),
            Ok(hir) => self.hir_to_node_id(&hir, tb),
        }
    }

    fn translator_to_node_id(
        &mut self,
        orig_ast: &regex_syntax::ast::Ast,
        translator: &mut Option<Translator>,
        tb: &mut TB<'s>,
    ) -> Result<NodeId> {
        match translator {
            Some(tr) => {
                let hir = tr
                    .translate("", orig_ast)
                    .map_err(|e| self.unsupported_error(e))?;
                self.hir_to_node_id(&hir, tb)
            }
            None => self.translate_ast_to_hir(orig_ast, tb),
        }
    }

    fn get_class(
        &mut self,
        negated: bool,
        kind: regex_syntax::ast::ClassPerlKind,
        tb: &mut TB<'s>,
    ) -> Result<NodeId> {
        let w = self
            .perl_classes
            .iter()
            .find(|(c_neg, c_kind, _)| *c_kind == kind && *c_neg == negated);
        match w {
            Some((_, _, value)) => Ok(*value),
            None => {
                let translated = if self.global_unicode {
                    match kind {
                        regex_syntax::ast::ClassPerlKind::Word => {
                            self.unicode_classes.ensure_word(tb);
                            if negated { self.unicode_classes.non_word } else { self.unicode_classes.word }
                        }
                        regex_syntax::ast::ClassPerlKind::Digit => {
                            self.unicode_classes.ensure_digit(tb);
                            if negated { self.unicode_classes.non_digit } else { self.unicode_classes.digit }
                        }
                        regex_syntax::ast::ClassPerlKind::Space => {
                            self.unicode_classes.ensure_space(tb);
                            if negated { self.unicode_classes.non_space } else { self.unicode_classes.space }
                        }
                    }
                } else {
                    let pos = match kind {
                        regex_syntax::ast::ClassPerlKind::Word => {
                            let az = tb.mk_range_u8(b'a', b'z');
                            let big = tb.mk_range_u8(b'A', b'Z');
                            let dig = tb.mk_range_u8(b'0', b'9');
                            let us = tb.mk_u8(b'_');
                            tb.mk_unions([az, big, dig, us].into_iter())
                        }
                        regex_syntax::ast::ClassPerlKind::Digit => {
                            tb.mk_range_u8(b'0', b'9')
                        }
                        regex_syntax::ast::ClassPerlKind::Space => {
                            let sp = tb.mk_u8(b' ');
                            let tab = tb.mk_u8(b'\t');
                            let nl = tb.mk_u8(b'\n');
                            let cr = tb.mk_u8(b'\r');
                            let ff = tb.mk_u8(0x0C);
                            let vt = tb.mk_u8(0x0B);
                            tb.mk_unions([sp, tab, nl, cr, ff, vt].into_iter())
                        }
                    };
                    if negated { tb.mk_compl(pos) } else { pos }
                };
                self.perl_classes.push((negated, kind, translated));
                Ok(translated)
            }
        }
    }

    fn word_char_kind(ast: &Ast, left: bool) -> WordCharKind {
        use WordCharKind::*;
        match ast {
            Ast::Literal(lit) => {
                if is_word_byte(lit.c as u8) {
                    Word
                } else {
                    NonWord
                }
            }
            Ast::ClassPerl(c) => match (&c.kind, c.negated) {
                (&regex_syntax::ast::ClassPerlKind::Word, false) => Word,
                (&regex_syntax::ast::ClassPerlKind::Word, true) => NonWord,
                (&regex_syntax::ast::ClassPerlKind::Space, false) => NonWord,
                (&regex_syntax::ast::ClassPerlKind::Space, true) => Unknown,
                (&regex_syntax::ast::ClassPerlKind::Digit, false) => Word,
                (&regex_syntax::ast::ClassPerlKind::Digit, true) => Unknown,
            },
            Ast::Dot(_) | Ast::Top(_) => Unknown,
            Ast::Group(g) => Self::word_char_kind(&g.ast, left),
            Ast::Concat(c) if !c.asts.is_empty() => {
                Self::word_char_kind(&c.asts[if left { c.asts.len() - 1 } else { 0 }], left)
            }
            Ast::Repetition(r) => {
                let inner = Self::word_char_kind(&r.ast, left);
                let nullable = matches!(
                    &r.op.kind,
                    ast::RepetitionKind::ZeroOrMore
                        | ast::RepetitionKind::ZeroOrOne
                        | ast::RepetitionKind::Range(ast::RepetitionRange::Bounded(0, _))
                );
                if nullable {
                    match inner {
                        Word => MaybeWord,
                        NonWord => MaybeNonWord,
                        _ => Unknown,
                    }
                } else {
                    inner
                }
            }
            Ast::Lookaround(la) => Self::word_char_kind(&la.ast, left),
            _ => Unknown,
        }
    }

    fn edge_class_ast(ast: &Ast, left: bool) -> Option<&Ast> {
        match ast {
            Ast::Literal(_) | Ast::ClassPerl(_) | Ast::ClassBracketed(_)
            | Ast::ClassUnicode(_) | Ast::Dot(_) | Ast::Top(_) => Some(ast),
            Ast::Group(g) => Self::edge_class_ast(&g.ast, left),
            Ast::Concat(c) if !c.asts.is_empty() => {
                Self::edge_class_ast(&c.asts[if left { c.asts.len() - 1 } else { 0 }], left)
            }
            Ast::Repetition(r) => Self::edge_class_ast(&r.ast, left),
            Ast::Lookaround(la) => Self::edge_class_ast(&la.ast, left),
            _ => None,
        }
    }

    fn resolve_word_kind(
        &mut self,
        asts: &[Ast],
        idx: usize,
        dir: isize,
        translator: &mut Option<Translator>,
        tb: &mut TB<'s>,
        word_id: NodeId,
        not_word_id: NodeId,
    ) -> Result<WordCharKind> {
        use WordCharKind::*;
        let fast = Self::concat_neighbor_kind(asts, idx, dir);
        if fast != Unknown {
            return Ok(fast);
        }
        let neighbor_idx = (idx as isize + dir) as usize;
        let edge = match Self::edge_class_ast(&asts[neighbor_idx], dir < 0) {
            Some(e) => e,
            None => return Ok(Unknown),
        };
        let node = self.ast_to_node_id(edge, translator, tb)?;
        if tb.subsumes(word_id, node) == Some(true) {
            Ok(Word)
        } else if tb.subsumes(not_word_id, node) == Some(true) {
            Ok(NonWord)
        } else {
            Ok(Unknown)
        }
    }

    fn concat_neighbor_kind(asts: &[Ast], idx: usize, dir: isize) -> WordCharKind {
        use WordCharKind::*;
        let next = idx as isize + dir;
        if next < 0 || next >= asts.len() as isize {
            return Edge;
        }
        let kind = Self::word_char_kind(&asts[next as usize], dir < 0);
        match kind {
            MaybeWord => match Self::concat_neighbor_kind(asts, next as usize, dir) {
                Word => Word,
                _ => Unknown,
            },
            MaybeNonWord => match Self::concat_neighbor_kind(asts, next as usize, dir) {
                NonWord => NonWord,
                _ => Unknown,
            },
            other => other,
        }
    }

    fn rewrite_word_boundary_in_concat(
        &mut self,
        asts: &[Ast],
        idx: usize,
        translator: &mut Option<Translator>,
        tb: &mut TB<'s>,
    ) -> Result<(NodeId, usize)> {
        use WordCharKind::*;
        let (word_id, not_word_id) = if self.global_unicode {
            self.unicode_classes.ensure_word(tb);
            (self.unicode_classes.word, self.unicode_classes.non_word)
        } else {
            let az = tb.mk_range_u8(b'a', b'z');
            let big = tb.mk_range_u8(b'A', b'Z');
            let dig = tb.mk_range_u8(b'0', b'9');
            let us = tb.mk_u8(b'_');
            let w = tb.mk_unions([az, big, dig, us].into_iter());
            (w, tb.mk_compl(w))
        };
        let left = self.resolve_word_kind(asts, idx, -1, translator, tb, word_id, not_word_id)?;
        let right =
            self.resolve_word_kind(asts, idx, 1, translator, tb, word_id, not_word_id)?;

        match (left, right) {
            (NonWord, Word) | (Word, NonWord) => Ok((NodeId::EPS, idx + 1)),
            (Word, _) => {
                let neg = tb.mk_neg_lookahead(word_id, 0);
                Ok((neg, idx + 1))
            }
            (NonWord, _) => {
                let set = tb.mk_union(NodeId::END, word_id);
                let tail = tb.mk_concat(set, NodeId::TS);
                self.merge_boundary_with_following_lookaheads(asts, idx, tail, translator, tb)
            }
            (_, Word) => {
                Ok((tb.mk_neg_lookbehind(word_id), idx + 1))
            }
            (_, NonWord) => {
                let body = tb.mk_union(NodeId::BEGIN, word_id);
                Ok((tb.mk_lookbehind(body, NodeId::MISSING), idx + 1))
            }
            // TODO: (Unknown, Unknown) is possible via make_full_word_boundary but
            // the full expansion (lb(\w)·la(\W) | lb(\W)·la(\w)) is too expensive
            _ => Err(self.error(self.span(), ast::ErrorKind::UnsupportedResharpRegex)),
        }
    }

    fn merge_boundary_with_following_lookaheads(
        &mut self,
        asts: &[Ast],
        wb_idx: usize,
        boundary_tail: NodeId,
        translator: &mut Option<Translator>,
        tb: &mut TB<'s>,
    ) -> Result<(NodeId, usize)> {
        let mut next = wb_idx + 1;
        let mut la_bodies = vec![boundary_tail];
        while next < asts.len() {
            match &asts[next] {
                Ast::Lookaround(la) if la.kind == ast::LookaroundKind::PositiveLookahead => {
                    let body = self.ast_to_node_id(&la.ast, translator, tb)?;
                    la_bodies.push(tb.mk_concat(body, NodeId::TS));
                    next += 1;
                }
                _ => break,
            }
        }
        let merged = tb.mk_inters(la_bodies.into_iter());
        Ok((tb.mk_lookahead(merged, NodeId::MISSING, 0), next))
    }

    fn ast_to_node_id(
        &mut self,
        ast: &Ast,
        translator: &mut Option<Translator>,
        tb: &mut TB<'s>,
    ) -> Result<NodeId> {
        match ast {
            Ast::Empty(_) => Ok(NodeId::EPS),
            Ast::Flags(f) => {
                let mut translator_builder = self.default_translator_builder();
                if let Some(state) = f.flags.flag_state(ast::Flag::CaseInsensitive) {
                    translator_builder.case_insensitive(state);
                }
                if let Some(state) = f.flags.flag_state(ast::Flag::Unicode) {
                    translator_builder.unicode(state);
                }
                if let Some(state) = f.flags.flag_state(ast::Flag::DotMatchesNewLine) {
                    self.dot_all.set(state);
                }
                let concat_translator = Some(translator_builder.build());
                *translator = concat_translator;
                Ok(NodeId::EPS)
            }
            Ast::Literal(l) => {
                let ast_lit = regex_syntax::ast::Ast::literal(*l.to_owned());
                self.translator_to_node_id(&ast_lit, translator, tb)
            }
            Ast::Top(_) => Ok(NodeId::TOP),
            Ast::Dot(_) => {
                if self.dot_all.get() {
                    Ok(NodeId::TOP)
                } else {
                    let hirv = hir::Hir::dot(hir::Dot::AnyByteExceptLF);
                    self.hir_to_node_id(&hirv, tb)
                }
            }
            Ast::Assertion(a) => match &a.kind {
                ast::AssertionKind::StartText => Ok(NodeId::BEGIN),
                ast::AssertionKind::EndText => Ok(NodeId::END),
                ast::AssertionKind::WordBoundary => {
                    Err(
                        self.error(self.span(), ast::ErrorKind::UnsupportedResharpRegex),
                    )
                }
                ast::AssertionKind::NotWordBoundary => {
                    Err(self.error(self.span(), ast::ErrorKind::UnsupportedResharpRegex))
                }
                ast::AssertionKind::StartLine => {
                    let left = NodeId::BEGIN;
                    let right = tb.mk_u8(b'\n');
                    let union = tb.mk_union(left, right);
                    Ok(tb.mk_lookbehind(union, NodeId::MISSING))
                }
                ast::AssertionKind::EndLine => {
                    let left = NodeId::END;
                    let right = tb.mk_u8(b'\n');
                    let union = tb.mk_union(left, right);
                    Ok(tb.mk_lookahead(union, NodeId::MISSING, 0))
                }
                ast::AssertionKind::WordBoundaryStart => todo!(),
                ast::AssertionKind::WordBoundaryEnd => todo!(),
                ast::AssertionKind::WordBoundaryStartAngle => todo!(),
                ast::AssertionKind::WordBoundaryEndAngle => Ok(tb.mk_string(">")),
                ast::AssertionKind::WordBoundaryStartHalf => todo!(),
                ast::AssertionKind::WordBoundaryEndHalf => todo!(),
            },
            Ast::ClassUnicode(c) => {
                let tmp = regex_syntax::ast::ClassUnicode {
                    span: c.span,
                    negated: c.negated,
                    kind: c.kind.clone(),
                };
                if !c.negated {
                    if let regex_syntax::ast::ClassUnicodeKind::Named(s) = &c.kind { match s.as_str() {
                        // \p{ascii} for ascii, \p{ascii}&\p{Letter} => [A-Za-z]
                        "ascii" => return Ok(tb.mk_range_u8(0, 127)),
                        // restricts matches to valid utf8, \p{utf8}*&~(a) => non a, but valid utf8
                        "utf8" => {
                            let ascii = tb.mk_range_u8(0, 127);
                            let beta = tb.mk_range_u8(128, 0xBF);
                            let c0 = tb.mk_range_u8(0xC0, 0xDF);
                            let c0s = tb.mk_concats([c0, beta].into_iter());
                            let e0 = tb.mk_range_u8(0xE0, 0xEF);
                            let e0s = tb.mk_concats([e0, beta, beta].into_iter());
                            let f0 = tb.mk_range_u8(0xF0, 0xF7);
                            let f0s = tb.mk_concats([f0, beta, beta, beta].into_iter());
                            let merged = tb.mk_unions([ascii, c0s, e0s, f0s].into_iter());
                            return Ok(tb.mk_star(merged));
                        }
                        "hex" => {
                            let nums = tb.mk_range_u8(b'0', b'9');
                            let lets = tb.mk_range_u8(b'a', b'f');
                            let lets2 = tb.mk_range_u8(b'A', b'F');
                            let merged = tb.mk_unions([nums, lets, lets2].into_iter());
                            return Ok(merged);
                        }
                        _ => {}
                    } };
                }

                let orig_ast = regex_syntax::ast::Ast::class_unicode(tmp);
                self.translator_to_node_id(&orig_ast, translator, tb)
            }
            Ast::ClassPerl(c) => {
                self.get_class(c.negated, c.kind.clone(), tb)
            }
            Ast::ClassBracketed(c) => match &c.kind {
                regex_syntax::ast::ClassSet::Item(_) => {
                    let tmp = regex_syntax::ast::ClassBracketed {
                        span: c.span,
                        negated: c.negated,
                        kind: c.kind.clone(),
                    };
                    let orig_ast = regex_syntax::ast::Ast::class_bracketed(tmp);
                    self.translator_to_node_id(&orig_ast, translator, tb)
                }
                regex_syntax::ast::ClassSet::BinaryOp(_) => todo!(),
            },
            Ast::Repetition(r) => {
                let body = self.ast_to_node_id(&r.ast, translator, tb);
                match body {
                    Ok(body) => match &r.op.kind {
                        ast::RepetitionKind::ZeroOrOne => Ok(tb.mk_opt(body)),
                        ast::RepetitionKind::ZeroOrMore => Ok(tb.mk_star(body)),
                        ast::RepetitionKind::OneOrMore => Ok(tb.mk_plus(body)),
                        ast::RepetitionKind::Range(r) => match r {
                            ast::RepetitionRange::Exactly(n) => Ok(tb.mk_repeat(body, *n, *n)),
                            ast::RepetitionRange::AtLeast(n) => {
                                let rep = tb.mk_repeat(body, *n, *n);
                                let st = tb.mk_star(body);
                                Ok(tb.mk_concat(rep, st))
                            }

                            ast::RepetitionRange::Bounded(n, m) => Ok(tb.mk_repeat(body, *n, *m)),
                        },
                    },
                    Err(_) => body,
                }
            }
            Ast::Lookaround(g) => {
                let body = self.ast_to_node_id(&g.ast, translator, tb)?;
                match g.kind {
                    ast::LookaroundKind::PositiveLookahead => {
                        Ok(tb.mk_lookahead(body, NodeId::MISSING, 0))
                    }
                    ast::LookaroundKind::PositiveLookbehind => {
                        Ok(tb.mk_lookbehind(body, NodeId::MISSING))
                    }
                    ast::LookaroundKind::NegativeLookahead => Ok(tb.mk_neg_lookahead(body, 0)),
                    ast::LookaroundKind::NegativeLookbehind => Ok(tb.mk_neg_lookbehind(body)),
                }
            }
            Ast::Group(g) => {
                if let ast::GroupKind::NonCapturing(ref flags) = g.kind {
                    if !flags.items.is_empty() {
                        let mut translator_builder = self.default_translator_builder();
                        if let Some(state) = flags.flag_state(ast::Flag::CaseInsensitive) {
                            translator_builder.case_insensitive(state);
                        }
                        if let Some(state) = flags.flag_state(ast::Flag::Unicode) {
                            translator_builder.unicode(state);
                        }
                        let saved_dot_all = self.dot_all.get();
                        if let Some(state) = flags.flag_state(ast::Flag::DotMatchesNewLine) {
                            self.dot_all.set(state);
                        }
                        let mut scoped = Some(translator_builder.build());
                        let result = self.ast_to_node_id(&g.ast, &mut scoped, tb);
                        self.dot_all.set(saved_dot_all);
                        return result;
                    }
                }
                self.ast_to_node_id(&g.ast, translator, tb)
            }
            Ast::Alternation(a) => {
                let mut children = vec![];
                for ast in &a.asts {
                    match self.ast_to_node_id(ast, translator, tb) {
                        Ok(node_id) => children.push(node_id),
                        Err(err) => return Err(err),
                    }
                }
                Ok(tb.mk_unions(children.iter().copied()))
            }
            Ast::Concat(c) => {
                let mut concat_translator: Option<Translator> = None;
                let mut children = vec![];
                let mut i = 0;
                while i < c.asts.len() {
                    let ast = &c.asts[i];
                    match ast {
                        Ast::Flags(f) => {
                            let mut translator_builder = self.default_translator_builder();
                            if let Some(state) = f.flags.flag_state(ast::Flag::CaseInsensitive) {
                                translator_builder.case_insensitive(state);
                            }
                            if let Some(state) = f.flags.flag_state(ast::Flag::Unicode) {
                                translator_builder.unicode(state);
                            }
                            if let Some(state) = f.flags.flag_state(ast::Flag::DotMatchesNewLine) {
                                self.dot_all.set(state);
                            }
                            concat_translator = Some(translator_builder.build());
                            i += 1;
                            continue;
                        }
                        Ast::Assertion(a) if a.kind == ast::AssertionKind::WordBoundary => {
                            let node =
                                self.rewrite_word_boundary_in_concat(&c.asts, i, translator, tb)?;
                            children.push(node.0);
                            i = node.1; // skip consumed lookaheads
                            continue;
                        }
                        _ => {}
                    }
                    match concat_translator {
                        Some(_) => match self.ast_to_node_id(ast, &mut concat_translator, tb) {
                            Ok(node_id) => children.push(node_id),
                            Err(err) => return Err(err),
                        },
                        None => match self.ast_to_node_id(ast, translator, tb) {
                            Ok(node_id) => children.push(node_id),
                            Err(err) => return Err(err),
                        },
                    }
                    i += 1;
                }
                Ok(tb.mk_concats(children.iter().cloned()))
            }
            Ast::Intersection(intersection) => {
                let mut children = vec![];
                for ast in &intersection.asts {
                    match self.ast_to_node_id(ast, translator, tb) {
                        Ok(node_id) => children.push(node_id),
                        Err(err) => return Err(err),
                    }
                }
                Ok(tb.mk_inters(children.into_iter()))
            }
            Ast::Complement(complement) => {
                let body = self.ast_to_node_id(&complement.ast, translator, tb);
                body.map(|x| tb.mk_compl(x))
            }
        }
    }

    /// Parse the regular expression and return an abstract syntax tree with
    /// all of the comments found in the pattern.
    fn parse(&mut self, tb: &mut TB<'s>) -> Result<NodeId> {
        let mut concat = Concat {
            span: self.span(),
            asts: vec![],
        };
        loop {
            self.bump_space();
            if self.is_eof() {
                break;
            }
            match self.char() {
                '(' => concat = self.push_group(concat)?,
                ')' => concat = self.pop_group(concat)?,
                '|' => concat = self.push_alternate(concat)?,
                '&' => concat = self.push_intersect(concat)?,
                '~' => concat = self.push_compl_group(concat)?,
                '[' => {
                    let class = self.parse_set_class()?;
                    concat.asts.push(Ast::class_bracketed(class));
                }
                '?' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::ZeroOrOne)?;
                }
                '*' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::ZeroOrMore)?;
                }
                '+' => {
                    concat =
                        self.parse_uncounted_repetition(concat, ast::RepetitionKind::OneOrMore)?;
                }
                '{' => {
                    concat = self.parse_counted_repetition(concat)?;
                }
                _ => concat.asts.push(self.parse_primitive()?.into_ast()),
            }
        }
        let ast = self.pop_group_end(concat)?;
        self.ast_to_node_id(&ast, &mut None, tb)
    }

    #[inline(never)]
    fn parse_uncounted_repetition(
        &self,
        mut concat: ast::Concat,
        kind: ast::RepetitionKind,
    ) -> Result<ast::Concat> {
        // assert!(self.char() == '?' || self.char() == '*' || self.char() == '+');
        let op_start = self.pos();
        let ast = match concat.asts.pop() {
            Some(ast) => ast,
            None => return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing)),
        };
        match ast {
            Ast::Empty(_) | Ast::Flags(_) => {
                return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing))
            }
            _ => {}
        }
        if self.bump() && self.char() == '?' {
            return Err(self.error(
                Span::new(op_start, self.pos()),
                ast::ErrorKind::UnsupportedLazyQuantifier,
            ));
        }
        concat.asts.push(Ast::repetition(ast::Repetition {
            span: ast.span().with_end(self.pos()),
            op: ast::RepetitionOp {
                span: Span::new(op_start, self.pos()),
                kind,
            },
            greedy: true,
            ast: Box::new(ast),
        }));
        Ok(concat)
    }

    #[inline(never)]
    fn parse_counted_repetition(&self, mut concat: ast::Concat) -> Result<ast::Concat> {
        assert!(self.char() == '{');
        let start = self.pos();
        let ast = match concat.asts.pop() {
            Some(ast) => ast,
            None => return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing)),
        };
        match ast {
            Ast::Empty(_) | Ast::Flags(_) => {
                return Err(self.error(self.span(), ast::ErrorKind::RepetitionMissing))
            }
            _ => {}
        }
        if !self.bump_and_bump_space() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }
        let count_start = specialize_err(
            self.parse_decimal(),
            ast::ErrorKind::DecimalEmpty,
            ast::ErrorKind::RepetitionCountDecimalEmpty,
        );
        if self.is_eof() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }
        let range = if self.char() == ',' {
            if !self.bump_and_bump_space() {
                return Err(self.error(
                    Span::new(start, self.pos()),
                    ast::ErrorKind::RepetitionCountUnclosed,
                ));
            }
            if self.char() != '}' {
                let count_start = match count_start {
                    Ok(c) => c,
                    Err(err) if err.kind == ast::ErrorKind::RepetitionCountDecimalEmpty => {
                        if self.parser().empty_min_range {
                            0
                        } else {
                            return Err(err);
                        }
                    }
                    err => err?,
                };
                let count_end = specialize_err(
                    self.parse_decimal(),
                    ast::ErrorKind::DecimalEmpty,
                    ast::ErrorKind::RepetitionCountDecimalEmpty,
                )?;
                ast::RepetitionRange::Bounded(count_start, count_end)
            } else {
                ast::RepetitionRange::AtLeast(count_start?)
            }
        } else {
            ast::RepetitionRange::Exactly(count_start?)
        };

        if self.is_eof() || self.char() != '}' {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::RepetitionCountUnclosed,
            ));
        }

        if self.bump_and_bump_space() && self.char() == '?' {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::UnsupportedLazyQuantifier,
            ));
        }

        let op_span = Span::new(start, self.pos());
        if !range.is_valid() {
            return Err(self.error(op_span, ast::ErrorKind::RepetitionCountInvalid));
        }
        concat.asts.push(Ast::repetition(ast::Repetition {
            span: ast.span().with_end(self.pos()),
            op: ast::RepetitionOp {
                span: op_span,
                kind: ast::RepetitionKind::Range(range),
            },
            greedy: true,
            ast: Box::new(ast),
        }));
        Ok(concat)
    }

    #[inline(never)]
    fn parse_group(&self) -> Result<Either<ast::SetFlags, ast::Group>> {
        assert_eq!(self.char(), '(');
        let open_span = self.span_char();
        self.bump();
        self.bump_space();
        if let Some((ahead, pos)) = self.is_lookaround_prefix() {
            let kind = match (pos, ahead) {
                (true, true) => LookaroundKind::PositiveLookahead,
                (true, false) => LookaroundKind::PositiveLookbehind,
                (false, true) => LookaroundKind::NegativeLookahead,
                (false, false) => LookaroundKind::NegativeLookbehind,
            };
            return Ok(Either::Right(ast::Group {
                span: open_span,
                kind: ast::GroupKind::Lookaround(kind),
                ast: Box::new(Ast::empty(self.span())),
            }));
        }
        let inner_span = self.span();
        let mut starts_with_p = true;
        if self.bump_if("?P<") || {
            starts_with_p = false;
            self.bump_if("?<")
        } {
            let capture_index = self.next_capture_index(open_span)?;
            let name = self.parse_capture_name(capture_index)?;
            Ok(Either::Right(ast::Group {
                span: open_span,
                kind: ast::GroupKind::CaptureName {
                    starts_with_p,
                    name,
                },
                ast: Box::new(Ast::empty(self.span())),
            }))
        } else if self.bump_if("?") {
            if self.is_eof() {
                return Err(self.error(open_span, ast::ErrorKind::GroupUnclosed));
            }
            let flags = self.parse_flags()?;
            let char_end = self.char();
            self.bump();
            if char_end == ')' {
                // We don't allow empty flags, e.g., `(?)`. We instead
                // interpret it as a repetition operator missing its argument.
                if flags.items.is_empty() {
                    return Err(self.error(inner_span, ast::ErrorKind::RepetitionMissing));
                }
                Ok(Either::Left(ast::SetFlags {
                    span: Span {
                        end: self.pos(),
                        ..open_span
                    },
                    flags,
                }))
            } else {
                assert_eq!(char_end, ':');
                Ok(Either::Right(ast::Group {
                    span: open_span,
                    kind: ast::GroupKind::NonCapturing(flags),
                    ast: Box::new(Ast::empty(self.span())),
                }))
            }
        } else {
            let capture_index = self.next_capture_index(open_span)?;
            Ok(Either::Right(ast::Group {
                span: open_span,
                kind: ast::GroupKind::CaptureIndex(capture_index),
                ast: Box::new(Ast::empty(self.span())),
            }))
        }
    }

    #[inline(never)]
    fn parse_capture_name(&self, capture_index: u32) -> Result<ast::CaptureName> {
        if self.is_eof() {
            return Err(self.error(self.span(), ast::ErrorKind::GroupNameUnexpectedEof));
        }
        let start = self.pos();
        loop {
            if self.char() == '>' {
                break;
            }
            if !is_capture_char(self.char(), self.pos() == start) {
                return Err(self.error(self.span_char(), ast::ErrorKind::GroupNameInvalid));
            }
            if !self.bump() {
                break;
            }
        }
        let end = self.pos();
        if self.is_eof() {
            return Err(self.error(self.span(), ast::ErrorKind::GroupNameUnexpectedEof));
        }
        assert_eq!(self.char(), '>');
        self.bump();
        let name = &self.pattern()[start.offset..end.offset];
        if name.is_empty() {
            return Err(self.error(Span::new(start, start), ast::ErrorKind::GroupNameEmpty));
        }
        let capname = ast::CaptureName {
            span: Span::new(start, end),
            name: name.to_string(),
            index: capture_index,
        };
        self.add_capture_name(&capname)?;
        Ok(capname)
    }

    #[inline(never)]
    fn parse_flags(&self) -> Result<ast::Flags> {
        let mut flags = ast::Flags {
            span: self.span(),
            items: vec![],
        };
        let mut last_was_negation = None;
        while self.char() != ':' && self.char() != ')' {
            if self.char() == '-' {
                last_was_negation = Some(self.span_char());
                let item = ast::FlagsItem {
                    span: self.span_char(),
                    kind: ast::FlagsItemKind::Negation,
                };
                if let Some(i) = flags.add_item(item) {
                    return Err(self.error(
                        self.span_char(),
                        ast::ErrorKind::FlagRepeatedNegation {
                            original: flags.items[i].span,
                        },
                    ));
                }
            } else {
                last_was_negation = None;
                let item = ast::FlagsItem {
                    span: self.span_char(),
                    kind: ast::FlagsItemKind::Flag(self.parse_flag()?),
                };
                if let Some(i) = flags.add_item(item) {
                    return Err(self.error(
                        self.span_char(),
                        ast::ErrorKind::FlagDuplicate {
                            original: flags.items[i].span,
                        },
                    ));
                }
            }
            if !self.bump() {
                return Err(self.error(self.span(), ast::ErrorKind::FlagUnexpectedEof));
            }
        }
        if let Some(span) = last_was_negation {
            return Err(self.error(span, ast::ErrorKind::FlagDanglingNegation));
        }
        flags.span.end = self.pos();
        Ok(flags)
    }

    #[inline(never)]
    fn parse_flag(&self) -> Result<ast::Flag> {
        match self.char() {
            'i' => Ok(ast::Flag::CaseInsensitive),
            'm' => Ok(ast::Flag::MultiLine),
            's' => Ok(ast::Flag::DotMatchesNewLine),
            'U' => Ok(ast::Flag::SwapGreed),
            'u' => Ok(ast::Flag::Unicode),
            'R' => Ok(ast::Flag::CRLF),
            'x' => Ok(ast::Flag::IgnoreWhitespace),
            _ => Err(self.error(self.span_char(), ast::ErrorKind::FlagUnrecognized)),
        }
    }

    fn parse_primitive(&self) -> Result<Primitive> {
        match self.char() {
            '\\' => self.parse_escape(),
            '_' => {
                let ast = Primitive::Top(self.span_char());
                self.bump();
                Ok(ast)
            }
            '.' => {
                let ast = Primitive::Dot(self.span_char());
                self.bump();
                Ok(ast)
            }
            '^' => {
                let ast = Primitive::Assertion(ast::Assertion {
                    span: self.span_char(),
                    kind: ast::AssertionKind::StartLine,
                });
                self.bump();
                Ok(ast)
            }
            '$' => {
                let ast = Primitive::Assertion(ast::Assertion {
                    span: self.span_char(),
                    kind: ast::AssertionKind::EndLine,
                });
                self.bump();
                Ok(ast)
            }
            c => {
                let ast = Primitive::Literal(Literal {
                    span: self.span_char(),
                    kind: LiteralKind::Verbatim,
                    c,
                });
                self.bump();
                Ok(ast)
            }
        }
    }

    #[inline(never)]
    fn parse_escape(&self) -> Result<Primitive> {
        assert_eq!(self.char(), '\\');
        let start = self.pos();
        if !self.bump() {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::EscapeUnexpectedEof,
            ));
        }
        let c = self.char();
        // Put some of the more complicated routines into helpers.
        match c {
            '0'..='9' => {
                if !self.parser().octal {
                    return Err(self.error(
                        Span::new(start, self.span_char().end),
                        ast::ErrorKind::UnsupportedBackreference,
                    ));
                }
                let mut lit = self.parse_octal();
                lit.span.start = start;
                return Ok(Primitive::Literal(lit));
            }
            // '8'..='9' if !self.parser().octal => {
            //     return Err(self.error(
            //         Span::new(start, self.span_char().end),
            //         ast::ErrorKind::UnsupportedBackreference,
            //     ));
            // }
            'x' | 'u' | 'U' => {
                let mut lit = self.parse_hex()?;
                lit.span.start = start;
                return Ok(Primitive::Literal(lit));
            }
            'p' | 'P' => {
                let mut cls = self.parse_unicode_class()?;
                cls.span.start = start;
                return Ok(Primitive::Unicode(cls));
            }
            'd' | 's' | 'w' | 'D' | 'S' | 'W' => {
                let mut cls = self.parse_perl_class();
                cls.span.start = start;
                return Ok(Primitive::Perl(cls));
            }
            _ => {}
        }

        // Handle all of the one letter sequences inline.
        self.bump();
        let span = Span::new(start, self.pos());
        if is_meta_character(c) {
            return Ok(Primitive::Literal(Literal {
                span,
                kind: LiteralKind::Meta,
                c,
            }));
        }
        if is_escapeable_character(c) {
            return Ok(Primitive::Literal(Literal {
                span,
                kind: LiteralKind::Superfluous,
                c,
            }));
        }
        let special = |kind, c| {
            Ok(Primitive::Literal(Literal {
                span,
                kind: LiteralKind::Special(kind),
                c,
            }))
        };
        match c {
            'a' => special(SpecialLiteralKind::Bell, '\x07'),
            'f' => special(SpecialLiteralKind::FormFeed, '\x0C'),
            't' => special(SpecialLiteralKind::Tab, '\t'),
            'n' => special(SpecialLiteralKind::LineFeed, '\n'),
            'r' => special(SpecialLiteralKind::CarriageReturn, '\r'),
            'v' => special(SpecialLiteralKind::VerticalTab, '\x0B'),
            'A' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::StartText,
            })),
            'z' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::EndText,
            })),
            'b' => {
                let mut wb = ast::Assertion {
                    span,
                    kind: ast::AssertionKind::WordBoundary,
                };
                // After a \b, we "try" to parse things like \b{start} for
                // special word boundary assertions.
                if !self.is_eof() && self.char() == '{' {
                    if let Some(kind) = self.maybe_parse_special_word_boundary(start)? {
                        wb.kind = kind;
                        wb.span.end = self.pos();
                    }
                }
                Ok(Primitive::Assertion(wb))
            }
            'B' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::NotWordBoundary,
            })),
            '<' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::WordBoundaryStartAngle,
            })),
            '>' => Ok(Primitive::Assertion(ast::Assertion {
                span,
                kind: ast::AssertionKind::WordBoundaryEndAngle,
            })),
            _ => Err(self.error(span, ast::ErrorKind::EscapeUnrecognized)),
        }
    }

    fn maybe_parse_special_word_boundary(
        &self,
        wb_start: Position,
    ) -> Result<Option<ast::AssertionKind>> {
        assert_eq!(self.char(), '{');

        let is_valid_char = |c| matches!(c, 'A'..='Z' | 'a'..='z' | '-');
        let start = self.pos();
        if !self.bump_and_bump_space() {
            return Err(self.error(
                Span::new(wb_start, self.pos()),
                ast::ErrorKind::SpecialWordOrRepetitionUnexpectedEof,
            ));
        }
        let start_contents = self.pos();
        // This is one of the critical bits: if the first non-whitespace
        // character isn't in [-A-Za-z] (i.e., this can't be a special word
        // boundary), then we bail and let the counted repetition parser deal
        // with this.
        if !is_valid_char(self.char()) {
            self.parser().pos.set(start);
            return Ok(None);
        }

        // Now collect up our chars until we see a '}'.
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();
        while !self.is_eof() && is_valid_char(self.char()) {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        if self.is_eof() || self.char() != '}' {
            return Err(self.error(
                Span::new(start, self.pos()),
                ast::ErrorKind::SpecialWordBoundaryUnclosed,
            ));
        }
        let end = self.pos();
        self.bump();
        let kind = match scratch.as_str() {
            "start" => ast::AssertionKind::WordBoundaryStart,
            "end" => ast::AssertionKind::WordBoundaryEnd,
            "start-half" => ast::AssertionKind::WordBoundaryStartHalf,
            "end-half" => ast::AssertionKind::WordBoundaryEndHalf,
            _ => {
                return Err(self.error(
                    Span::new(start_contents, end),
                    ast::ErrorKind::SpecialWordBoundaryUnrecognized,
                ))
            }
        };
        Ok(Some(kind))
    }

    #[inline(never)]
    fn parse_octal(&self) -> Literal {
        assert!(self.parser().octal);
        assert!('0' <= self.char() && self.char() <= '7');
        let start = self.pos();
        // Parse up to two more digits.
        while self.bump()
            && '0' <= self.char()
            && self.char() <= '7'
            && self.pos().offset - start.offset <= 2
        {}
        let end = self.pos();
        let octal = &self.pattern()[start.offset..end.offset];
        // Parsing the octal should never fail since the above guarantees a
        // valid number.
        let codepoint = u32::from_str_radix(octal, 8).expect("valid octal number");
        // The max value for 3 digit octal is 0777 = 511 and [0, 511] has no
        // invalid Unicode scalar values.
        let c = char::from_u32(codepoint).expect("Unicode scalar value");
        Literal {
            span: Span::new(start, end),
            kind: LiteralKind::Octal,
            c,
        }
    }

    #[inline(never)]
    fn parse_hex(&self) -> Result<Literal> {
        assert!(self.char() == 'x' || self.char() == 'u' || self.char() == 'U');

        let hex_kind = match self.char() {
            'x' => HexLiteralKind::X,
            'u' => HexLiteralKind::UnicodeShort,
            _ => HexLiteralKind::UnicodeLong,
        };
        if !self.bump_and_bump_space() {
            return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
        }
        if self.char() == '{' {
            self.parse_hex_brace(hex_kind)
        } else {
            self.parse_hex_digits(hex_kind)
        }
    }

    #[inline(never)]
    fn parse_hex_digits(&self, kind: HexLiteralKind) -> Result<Literal> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();

        let start = self.pos();
        for i in 0..kind.digits() {
            if i > 0 && !self.bump_and_bump_space() {
                return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
            }
            if !is_hex(self.char()) {
                return Err(self.error(self.span_char(), ast::ErrorKind::EscapeHexInvalidDigit));
            }
            scratch.push(self.char());
        }
        // The final bump just moves the parser past the literal, which may
        // be EOF.
        self.bump_and_bump_space();
        let end = self.pos();
        let hex = scratch.as_str();
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            None => Err(self.error(Span::new(start, end), ast::ErrorKind::EscapeHexInvalid)),
            Some(c) => Ok(Literal {
                span: Span::new(start, end),
                kind: LiteralKind::HexFixed(kind),
                c,
            }),
        }
    }

    #[inline(never)]
    fn parse_hex_brace(&self, kind: HexLiteralKind) -> Result<Literal> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();

        let brace_pos = self.pos();
        let start = self.span_char().end;
        while self.bump_and_bump_space() && self.char() != '}' {
            if !is_hex(self.char()) {
                return Err(self.error(self.span_char(), ast::ErrorKind::EscapeHexInvalidDigit));
            }
            scratch.push(self.char());
        }
        if self.is_eof() {
            return Err(self.error(
                Span::new(brace_pos, self.pos()),
                ast::ErrorKind::EscapeUnexpectedEof,
            ));
        }
        let end = self.pos();
        let hex = scratch.as_str();
        assert_eq!(self.char(), '}');
        self.bump_and_bump_space();

        if hex.is_empty() {
            return Err(self.error(
                Span::new(brace_pos, self.pos()),
                ast::ErrorKind::EscapeHexEmpty,
            ));
        }
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            None => Err(self.error(Span::new(start, end), ast::ErrorKind::EscapeHexInvalid)),
            Some(c) => Ok(Literal {
                span: Span::new(start, self.pos()),
                kind: LiteralKind::HexBrace(kind),
                c,
            }),
        }
    }

    fn parse_decimal(&self) -> Result<u32> {
        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();

        while !self.is_eof() && self.char().is_whitespace() {
            self.bump();
        }
        let start = self.pos();
        while !self.is_eof() && '0' <= self.char() && self.char() <= '9' {
            scratch.push(self.char());
            self.bump_and_bump_space();
        }
        let span = Span::new(start, self.pos());
        while !self.is_eof() && self.char().is_whitespace() {
            self.bump_and_bump_space();
        }
        let digits = scratch.as_str();
        if digits.is_empty() {
            return Err(self.error(span, ast::ErrorKind::DecimalEmpty));
        }
        match digits.parse::<u32>().ok() {
            Some(n) => Ok(n),
            None => Err(self.error(span, ast::ErrorKind::DecimalInvalid)),
        }
    }

    #[inline(never)]
    fn parse_set_class(&self) -> Result<ClassBracketed> {
        assert_eq!(self.char(), '[');

        let mut union = ClassSetUnion {
            span: self.span(),
            items: vec![],
        };
        loop {
            self.bump_space();
            if self.is_eof() {
                return Err(self.unclosed_class_error());
            }
            match self.char() {
                '[' => {
                    // If we've already parsed the opening bracket, then
                    // attempt to treat this as the beginning of an ASCII
                    // class. If ASCII class parsing fails, then the parser
                    // backs up to `[`.
                    if !self.parser().stack_class.borrow().is_empty() {
                        if let Some(cls) = self.maybe_parse_ascii_class() {
                            union.push(ClassSetItem::Ascii(cls));
                            continue;
                        }
                    }
                    union = self.push_class_open(union)?;
                }
                ']' => match self.pop_class(union)? {
                    Either::Left(nested_union) => {
                        union = nested_union;
                    }
                    Either::Right(class) => return Ok(class),
                },
                '&' if self.peek() == Some('&') => {
                    assert!(self.bump_if("&&"));
                    union = self.push_class_op(ClassSetBinaryOpKind::Intersection, union);
                }
                '-' if self.peek() == Some('-') => {
                    assert!(self.bump_if("--"));
                    union = self.push_class_op(ClassSetBinaryOpKind::Difference, union);
                }
                '~' if self.peek() == Some('~') => {
                    assert!(self.bump_if("~~"));
                    union = self.push_class_op(ClassSetBinaryOpKind::SymmetricDifference, union);
                }
                _ => {
                    union.push(self.parse_set_class_range()?);
                }
            }
        }
    }

    #[inline(never)]
    fn parse_set_class_range(&self) -> Result<ClassSetItem> {
        let prim1 = self.parse_set_class_item()?;
        self.bump_space();
        if self.is_eof() {
            return Err(self.unclosed_class_error());
        }
        if self.char() != '-' || self.peek_space() == Some(']') || self.peek_space() == Some('-') {
            return prim1.into_class_set_item(self);
        }
        if !self.bump_and_bump_space() {
            return Err(self.unclosed_class_error());
        }
        let prim2 = self.parse_set_class_item()?;
        let range = ClassSetRange {
            span: Span::new(prim1.span().start, prim2.span().end),
            start: prim1.into_class_literal(self)?,
            end: prim2.into_class_literal(self)?,
        };
        if !range.is_valid() {
            return Err(self.error(range.span, ast::ErrorKind::ClassRangeInvalid));
        }
        Ok(ClassSetItem::Range(range))
    }

    #[inline(never)]
    fn parse_set_class_item(&self) -> Result<Primitive> {
        if self.char() == '\\' {
            self.parse_escape()
        } else {
            let x = Primitive::Literal(Literal {
                span: self.span_char(),
                kind: LiteralKind::Verbatim,
                c: self.char(),
            });
            self.bump();
            Ok(x)
        }
    }

    #[inline(never)]
    fn parse_set_class_open(&self) -> Result<(ClassBracketed, ClassSetUnion)> {
        assert_eq!(self.char(), '[');
        let start = self.pos();
        if !self.bump_and_bump_space() {
            return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
        }

        let negated = if self.char() != '^' {
            false
        } else {
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
            }
            true
        };
        // Accept any number of `-` as literal `-`.
        let mut union = ClassSetUnion {
            span: self.span(),
            items: vec![],
        };
        while self.char() == '-' {
            union.push(ClassSetItem::Literal(Literal {
                span: self.span_char(),
                kind: LiteralKind::Verbatim,
                c: '-',
            }));
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, start), ast::ErrorKind::ClassUnclosed));
            }
        }
        // If `]` is the *first* char in a set, then interpret it as a literal
        // `]`. That is, an empty class is impossible to write.
        if union.items.is_empty() && self.char() == ']' {
            union.push(ClassSetItem::Literal(Literal {
                span: self.span_char(),
                kind: LiteralKind::Verbatim,
                c: ']',
            }));
            if !self.bump_and_bump_space() {
                return Err(self.error(Span::new(start, self.pos()), ast::ErrorKind::ClassUnclosed));
            }
        }
        let set = ClassBracketed {
            span: Span::new(start, self.pos()),
            negated,
            kind: ClassSet::union(ClassSetUnion {
                span: Span::new(union.span.start, union.span.start),
                items: vec![],
            }),
        };
        Ok((set, union))
    }

    #[inline(never)]
    fn maybe_parse_ascii_class(&self) -> Option<ClassAscii> {
        assert_eq!(self.char(), '[');
        // If parsing fails, then we back up the parser to this starting point.
        let start = self.pos();
        let mut negated = false;
        if !self.bump() || self.char() != ':' {
            self.parser().pos.set(start);
            return None;
        }
        if !self.bump() {
            self.parser().pos.set(start);
            return None;
        }
        if self.char() == '^' {
            negated = true;
            if !self.bump() {
                self.parser().pos.set(start);
                return None;
            }
        }
        let name_start = self.offset();
        while self.char() != ':' && self.bump() {}
        if self.is_eof() {
            self.parser().pos.set(start);
            return None;
        }
        let name = &self.pattern()[name_start..self.offset()];
        if !self.bump_if(":]") {
            self.parser().pos.set(start);
            return None;
        }
        let kind = match regex_syntax::ast::ClassAsciiKind::from_name(name) {
            Some(kind) => kind,
            None => {
                self.parser().pos.set(start);
                return None;
            }
        };
        Some(ClassAscii {
            span: Span::new(start, self.pos()),
            kind,
            negated,
        })
    }

    #[inline(never)]
    fn parse_unicode_class(&self) -> Result<ClassUnicode> {
        assert!(self.char() == 'p' || self.char() == 'P');

        let mut scratch = self.parser().scratch.borrow_mut();
        scratch.clear();

        let negated = self.char() == 'P';
        if !self.bump_and_bump_space() {
            return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
        }
        let (start, kind) = if self.char() == '{' {
            let start = self.span_char().end;
            while self.bump_and_bump_space() && self.char() != '}' {
                scratch.push(self.char());
            }
            if self.is_eof() {
                return Err(self.error(self.span(), ast::ErrorKind::EscapeUnexpectedEof));
            }
            assert_eq!(self.char(), '}');
            self.bump();

            let name = scratch.as_str();
            if let Some(i) = name.find("!=") {
                (
                    start,
                    ClassUnicodeKind::NamedValue {
                        op: ClassUnicodeOpKind::NotEqual,
                        name: name[..i].to_string(),
                        value: name[i + 2..].to_string(),
                    },
                )
            } else if let Some(i) = name.find(':') {
                (
                    start,
                    ClassUnicodeKind::NamedValue {
                        op: ClassUnicodeOpKind::Colon,
                        name: name[..i].to_string(),
                        value: name[i + 1..].to_string(),
                    },
                )
            } else if let Some(i) = name.find('=') {
                (
                    start,
                    ClassUnicodeKind::NamedValue {
                        op: ClassUnicodeOpKind::Equal,
                        name: name[..i].to_string(),
                        value: name[i + 1..].to_string(),
                    },
                )
            } else {
                (start, ClassUnicodeKind::Named(name.to_string()))
            }
        } else {
            let start = self.pos();
            let c = self.char();
            if c == '\\' {
                return Err(self.error(self.span_char(), ast::ErrorKind::UnicodeClassInvalid));
            }
            self.bump_and_bump_space();
            let kind = ClassUnicodeKind::OneLetter(c);
            (start, kind)
        };
        Ok(ClassUnicode {
            span: Span::new(start, self.pos()),
            negated,
            kind,
        })
    }

    #[inline(never)]
    fn parse_perl_class(&self) -> ClassPerl {
        let c = self.char();
        let span = self.span_char();
        self.bump();
        let (negated, kind) = match c {
            'd' => (false, regex_syntax::ast::ClassPerlKind::Digit),
            'D' => (true, regex_syntax::ast::ClassPerlKind::Digit),
            's' => (false, regex_syntax::ast::ClassPerlKind::Space),
            'S' => (true, regex_syntax::ast::ClassPerlKind::Space),
            'w' => (false, regex_syntax::ast::ClassPerlKind::Word),
            'W' => (true, regex_syntax::ast::ClassPerlKind::Word),
            c => panic!("expected valid Perl class but got '{}'", c),
        };
        ClassPerl {
            span,
            kind,
            negated,
        }
    }
}

pub fn parse_ast<'s>(
    tb: &mut TB<'s>,
    pattern: &'s str,
) -> std::result::Result<NodeId, ResharpError> {
    let mut p: ResharpParser<'s> = ResharpParser::new(pattern);
    p.parse(tb)
}

pub fn parse_ast_with<'s>(
    tb: &mut TB<'s>,
    pattern: &'s str,
    flags: &PatternFlags,
) -> std::result::Result<NodeId, ResharpError> {
    let mut p: ResharpParser<'s> = ResharpParser::with_flags(pattern, flags);
    p.parse(tb)
}
