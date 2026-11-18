use resharp_algebra::{MinDfa, RegexBuilder};
use resharp_parser::{parse_ast_with, PatternFlags};

fn minimal(pattern: &str, single_line: bool) -> usize {
    dfa(pattern, single_line).1.minimal()
}

fn dfa(pattern: &str, single_line: bool) -> (RegexBuilder, MinDfa) {
    let mut b = RegexBuilder::new();
    let flags = PatternFlags {
        unicode: false,
        dot_matches_new_line: single_line,
        ..Default::default()
    };
    let n = parse_ast_with(&mut b, pattern, &flags).expect("parse");
    let d = b.minimize_dfa(n, 100_000).expect("minimize");
    (b, d)
}

fn accepts(b: &RegexBuilder, d: &MinDfa, w: &[u8], at_end: bool) -> bool {
    let mut s = 0;
    for &c in w {
        let hits: Vec<usize> = d.states[s]
            .edges
            .iter()
            .filter(|&&(_, set)| b.solver_ref().get_set(set).contains_byte(c))
            .map(|&(t, _)| t)
            .collect();
        assert_eq!(hits.len(), 1, "byte {c} must lead to exactly one state");
        s = hits[0];
    }
    if at_end { d.states[s].accepting_at_end } else { d.states[s].accepting }
}

#[test]
fn greedy_union_over_any_byte_is_k_plus_2() {
    for k in [4, 16, 100] {
        let rewrite = format!("_{{{k}}}|~(_{{{k}}}_*)&_*");
        assert_eq!(minimal(&rewrite, false), k + 2, "{rewrite}");
        assert_eq!(minimal(&format!("_{{0,{k}}}"), false), k + 2);
        assert_eq!(minimal(&format!("_{{{k}}}|_*"), false), 1);
    }
}

#[test]
fn single_line_dot_is_any_byte() {
    for k in [4, 16, 100] {
        let rewrite = format!(".{{{k}}}|~(.{{{k}}}_*)&.*");
        assert_eq!(minimal(&rewrite, true), k + 2, "{rewrite}");
        assert_eq!(minimal(&format!(".{{{k}}}|.*"), true), 1);
    }
}

#[test]
fn dot_without_newline_is_a_different_language() {
    for k in [4, 16] {
        assert_eq!(minimal(&format!(".{{{k}}}|.*"), false), 2);
        let same = format!(".{{{k}}}|~(.{{{k}}}_*)&.*");
        assert_eq!(minimal(&same, false), k + 2, "{same}");
        let mixed = format!("_{{{k}}}|~(_{{{k}}}_*)&.*");
        assert_eq!(minimal(&mixed, false), 2 * k + 1, "{mixed}");
    }
}

fn words(alpha: &[u8], max: usize) -> Vec<Vec<u8>> {
    let mut all = vec![Vec::new()];
    let mut layer = vec![Vec::new()];
    for _ in 0..max {
        let mut next = Vec::new();
        for w in &layer {
            for &c in alpha {
                let mut x = w.clone();
                x.push(c);
                next.push(x);
            }
        }
        all.extend(next.iter().cloned());
        layer = next;
    }
    all
}

fn nerode(n: usize, member: impl Fn(&[u8], bool) -> bool) -> usize {
    let ws = words(b"abc", n);
    let sigs: std::collections::HashSet<Vec<bool>> = ws
        .iter()
        .map(|p| {
            ws.iter()
                .flat_map(|s| {
                    let w = [p.as_slice(), s.as_slice()].concat();
                    [member(&w, false), member(&w, true)]
                })
                .collect()
        })
        .collect();
    sigs.len()
}

#[test]
fn anchors_split_states_by_location_kind() {
    let cases: Vec<(&str, Box<dyn Fn(&[u8], bool) -> bool>)> = vec![
        (r"a*\z", Box::new(|w, e| e && w.iter().all(|&c| c == b'a'))),
        (r"\Aa*", Box::new(|w, _| w.iter().all(|&c| c == b'a'))),
        (r"\Aab\z|b", Box::new(|w, e| (e && w == b"ab") || w == b"b")),
        (r"~(a\z)", Box::new(|w, e| !(e && w == b"a"))),
        (r"\A\z", Box::new(|w, e| e && w.is_empty())),
        (r"a\Ab|b", Box::new(|w, _| w == b"b")),
        (r"(a|b)*b\z", Box::new(|w, e| e && w.last() == Some(&b'b') && w.iter().all(|&c| c == b'a' || c == b'b'))),
    ];
    for (pattern, member) in cases {
        let want = nerode(5, &member);
        let (b, d) = dfa(pattern, false);
        assert_eq!(d.minimal(), want, "{pattern}");
        for w in words(b"abc", 5) {
            for at_end in [false, true] {
                assert_eq!(accepts(&b, &d, &w, at_end), member(&w, at_end), "{pattern} on {w:?}, end={at_end}");
            }
        }
    }
}
