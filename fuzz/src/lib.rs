//! shared helpers for the resharp fuzz targets.
//!
//! kept deliberately small: an option sweep used by the `compile` and
//! `match_invariants` targets, a restricted regex grammar used by the
//! differential `diff_regex` target, and a couple of formatting helpers so
//! crash messages carry a self-contained reproducer.

use arbitrary::{Arbitrary, Result, Unstructured};
use resharp::{RegexOptions, UnicodeMode};

/// Representative `RegexOptions` configs, each a distinct compile/match code
/// path (unicode class logic, hardened O(N*S) scan, `(?ismx)`-style flags).
pub fn option_sweep() -> [RegexOptions; 6] {
    [
        RegexOptions::default(),
        RegexOptions::default().hardened(true),
        RegexOptions::default().unicode(UnicodeMode::Ascii),
        RegexOptions::default().unicode(UnicodeMode::Full),
        RegexOptions::default().unicode(UnicodeMode::Javascript),
        RegexOptions::default()
            .case_insensitive(true)
            .ignore_whitespace(true)
            .dot_matches_new_line(true)
            .multiline(false),
    ]
}

/// Hex rendering of arbitrary bytes for crash reproducers (haystacks are raw
/// `&[u8]`; `{:?}` mangles non-utf8 input).
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

/// A regex from the subset where resharp and `regex` share `is_match`
/// semantics: ascii literals, `.`, classes, alternation, concat, grouping,
/// greedy quantifiers. Excludes anchors, `\b`, unicode-width-sensitive
/// `\w`/`\d`/`\s`, backreferences, and resharp-only operators -- everywhere
/// leftmost-longest vs leftmost-greedy can only change match length, so
/// `is_match` must agree byte-for-byte.
#[derive(Debug)]
pub struct DiffPattern(pub String);

impl<'a> Arbitrary<'a> for DiffPattern {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let mut out = String::new();
        // depth budget bounds both the generator recursion (no stack overflow
        // while *building* the pattern) and the compiled automaton size.
        gen_node(u, &mut out, 4)?;
        if out.is_empty() {
            out.push('a');
        }
        Ok(DiffPattern(out))
    }
}

/// emit one regex node into `out`, recursing until `depth` is exhausted.
fn gen_node(u: &mut Unstructured<'_>, out: &mut String, depth: u8) -> Result<()> {
    if depth == 0 || u.is_empty() {
        return gen_leaf(u, out);
    }
    match u.int_in_range(0u8..=5)? {
        // concatenation of 2..=3 children.
        0 => {
            let n = u.int_in_range(2u8..=3)?;
            for _ in 0..n {
                gen_node(u, out, depth - 1)?;
            }
        }
        // alternation, wrapped so the `|` binds where intended.
        1 => {
            out.push_str("(?:");
            gen_node(u, out, depth - 1)?;
            out.push('|');
            gen_node(u, out, depth - 1)?;
            out.push(')');
        }
        // plain group.
        2 => {
            out.push_str("(?:");
            gen_node(u, out, depth - 1)?;
            out.push(')');
        }
        // `*` / `+` / `?` on a grouped child.
        3 => {
            out.push_str("(?:");
            gen_node(u, out, depth - 1)?;
            out.push(')');
            out.push(*u.choose(&['*', '+', '?'])?);
        }
        // bounded repeat `{lo,hi}` with small counts so the automaton stays small.
        4 => {
            out.push_str("(?:");
            gen_node(u, out, depth - 1)?;
            out.push(')');
            let lo = u.int_in_range(0u8..=3)?;
            let hi = u.int_in_range(lo..=3)?;
            out.push('{');
            out.push_str(&lo.to_string());
            out.push(',');
            out.push_str(&hi.to_string());
            out.push('}');
        }
        _ => gen_leaf(u, out)?,
    }
    Ok(())
}

/// emit a terminal: a literal, `.`, or a (possibly negated) character class.
fn gen_leaf(u: &mut Unstructured<'_>, out: &mut String) -> Result<()> {
    match u.int_in_range(0u8..=3)? {
        0 => out.push(*u.choose(b"abcABC012")? as char),
        1 => out.push('.'),
        2 => {
            out.push('[');
            push_class_items(u, out)?;
            out.push(']');
        }
        _ => {
            out.push_str("[^");
            push_class_items(u, out)?;
            out.push(']');
        }
    }
    Ok(())
}

/// emit 1..=3 class members; ranges stay within one ascending ascii block so
/// they are valid and identically interpreted by both engines.
fn push_class_items(u: &mut Unstructured<'_>, out: &mut String) -> Result<()> {
    const ITEMS: &[&str] = &["a", "b", "c", "A", "B", "C", "0", "1", "2", "a-c", "A-C", "0-9"];
    let n = u.int_in_range(1u8..=3)?;
    for _ in 0..n {
        out.push_str(u.choose(ITEMS)?);
    }
    Ok(())
}
