use resharp_algebra::{NodeId, RegexBuilder, ResharpError};

const MAX: usize = 100_000;

fn minimal(b: &mut RegexBuilder, n: NodeId) -> usize {
    b.minimize_dfa(n, MAX).expect("minimize").minimal()
}

fn dot(b: &mut RegexBuilder) -> NodeId {
    let lo = b.mk_range_u8(0, 9);
    let hi = b.mk_range_u8(11, 255);
    b.mk_union(lo, hi)
}

fn cat_all(b: &mut RegexBuilder, parts: &[NodeId]) -> NodeId {
    parts.iter().rev().fold(NodeId::EPS, |acc, &p| b.mk_concat(p, acc))
}

fn limited(b: &mut RegexBuilder, any: NodeId, rest: NodeId, k: u32) -> NodeId {
    let exact = b.mk_repeat(any, k, k);
    let longer = b.mk_concat(exact, NodeId::TS);
    let shorter = b.mk_compl(longer);
    let short = b.mk_inter(shorter, rest);
    b.mk_union(exact, short)
}

#[test]
fn star_and_literal() {
    let mut b = RegexBuilder::new();
    let a = b.mk_u8(b'a');
    let star = b.mk_star(a);
    assert_eq!(minimal(&mut b, star), 2);
    assert_eq!(minimal(&mut b, NodeId::EPS), 2);
    assert_eq!(minimal(&mut b, NodeId::TS), 1);
    assert_eq!(minimal(&mut b, NodeId::BOT), 1);
    let abc = b.mk_string("abc");
    assert_eq!(minimal(&mut b, abc), 5);
}

#[test]
fn counted_repeat_is_k_plus_2() {
    for k in [1u32, 4, 16, 64] {
        let mut b = RegexBuilder::new();
        let r = b.mk_repeat(NodeId::TOP, 0, k);
        assert_eq!(minimal(&mut b, r), k as usize + 2, "_{{0,{k}}}");
    }
}

#[test]
fn greedy_union_rewrite_equals_counted_prefix() {
    for k in [1u32, 4, 16, 64] {
        let mut b = RegexBuilder::new();
        let r = limited(&mut b, NodeId::TOP, NodeId::TS, k);
        let d = b.minimize_dfa(r, MAX).expect("minimize");
        assert_eq!(d.minimal(), k as usize + 2, "_{{{k}}}|~(_{{{k}}}_*)&_*");
        assert!(d.minimal() <= d.reachable);
    }
}

#[test]
fn language_star_collapses_counted_union() {
    for k in [1u32, 4, 16, 64] {
        let mut b = RegexBuilder::new();
        let d = dot(&mut b);
        let exact = b.mk_repeat(d, k, k);
        let star = b.mk_star(d);
        let u = b.mk_union(exact, star);
        assert_eq!(minimal(&mut b, u), 2, ".{{{k}}}|.*");
    }
}

#[test]
fn newline_branch_doubles_the_counter() {
    for k in [2u32, 5, 16] {
        let mut b = RegexBuilder::new();
        let d = dot(&mut b);
        let rest = b.mk_star(d);
        let r = limited(&mut b, NodeId::TOP, rest, k);
        assert_eq!(minimal(&mut b, r), 2 * k as usize + 1, "_{{{k}}}|~(_{{{k}}}_*)&.*");
    }
}

#[test]
fn nth_symbol_from_the_end_is_exponential() {
    for n in [2u32, 3, 5] {
        let mut b = RegexBuilder::new();
        let ab = b.mk_ranges_u8(&[(b'a', b'b')]);
        let bb = b.mk_u8(b'b');
        let star = b.mk_star(ab);
        let tail = b.mk_repeat(ab, n, n);
        let r = cat_all(&mut b, &[star, bb, tail]);
        assert_eq!(minimal(&mut b, r), (1 << (n + 1)) + 1, "(a|b)*b(a|b){{{n}}}");
    }
}

#[test]
fn equivalent_regexes_have_equal_minimal_size() {
    let mut b = RegexBuilder::new();
    let a = b.mk_u8(b'a');
    let bb = b.mk_u8(b'b');
    let ab = b.mk_concat(a, bb);
    let ab_star = b.mk_star(ab);
    let abab = b.mk_concat(ab_star, ab_star);
    let aab = b.mk_concat(ab, ab_star);
    let u = b.mk_union(ab_star, aab);
    let n1 = minimal(&mut b, ab_star);
    assert_eq!(n1, 3);
    assert_eq!(minimal(&mut b, abab), n1);
    assert_eq!(minimal(&mut b, u), n1);
}

#[test]
fn complement_and_intersection() {
    let mut b = RegexBuilder::new();
    let a = b.mk_u8(b'a');
    let star = b.mk_star(a);
    let not_star = b.mk_compl(star);
    assert_eq!(minimal(&mut b, not_star), 2);
    let a2 = b.mk_repeat(a, 2, 2);
    let a3 = b.mk_repeat(a, 3, 3);
    let both = b.mk_inter(a2, a3);
    assert_eq!(minimal(&mut b, both), 1);
}

#[test]
fn state_limit_and_unsupported() {
    let mut b = RegexBuilder::new();
    let r = b.mk_repeat(NodeId::TOP, 0, 50);
    assert_eq!(b.minimize_dfa(r, 10), Err(ResharpError::StateSpaceExplosion));
    let la = b.mk_lookahead(NodeId::TOP, NodeId::MISSING, 0);
    assert_eq!(b.minimize_dfa(la, MAX), Err(ResharpError::UnsupportedPattern));
}

#[test]
fn minimization_merges_unminimized_states() {
    let mut b = RegexBuilder::new();
    let d = dot(&mut b);
    let exact = b.mk_repeat(d, 16, 16);
    let star = b.mk_star(d);
    let u = b.mk_union(exact, star);
    let m = b.minimize_dfa(u, MAX).expect("minimize");
    assert_eq!(m.minimal(), 2);
    assert!(m.reachable > 16, "{m:?}");
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

fn nerode(alpha: &[u8], n: usize, member: impl Fn(&[u8]) -> bool) -> usize {
    let ws = words(alpha, n);
    let sigs: std::collections::HashSet<Vec<bool>> = ws
        .iter()
        .map(|p| {
            ws.iter()
                .map(|s| member(&[p.as_slice(), s.as_slice()].concat()))
                .collect()
        })
        .collect();
    sigs.len()
}

#[test]
fn agrees_with_brute_force_nerode_classes() {
    for k in 2..=6u32 {
        let mut b = RegexBuilder::new();
        let r = limited(&mut b, NodeId::TOP, NodeId::TS, k);
        let want = nerode(b"ab", k as usize + 3, |w| w.len() <= k as usize);
        assert_eq!(want, k as usize + 2);
        assert_eq!(minimal(&mut b, r), want, "len<=k, k={k}");
    }
    for k in 2..=4u32 {
        let mut b = RegexBuilder::new();
        let d = dot(&mut b);
        let rest = b.mk_star(d);
        let r = limited(&mut b, NodeId::TOP, rest, k);
        let kk = k as usize;
        let want = nerode(b"a\n", 2 * kk + 2, |w| w.len() == kk || (w.len() < kk && !w.contains(&b'\n')));
        assert_eq!(minimal(&mut b, r), want, "newline branch, k={k}");
    }
    for n in 1..=4u32 {
        let mut b = RegexBuilder::new();
        let ab = b.mk_ranges_u8(&[(b'a', b'b')]);
        let bb = b.mk_u8(b'b');
        let star = b.mk_star(ab);
        let tail = b.mk_repeat(ab, n, n);
        let r = cat_all(&mut b, &[star, bb, tail]);
        let nn = n as usize;
        let want = nerode(b"abc", nn + 2, |w| {
            w.iter().all(|&c| c == b'a' || c == b'b') && w.len() > nn && w[w.len() - nn - 1] == b'b'
        });
        assert_eq!(minimal(&mut b, r), want, "(a|b)*b(a|b){{{n}}}");
    }
    for k in 2..=6u32 {
        let mut b = RegexBuilder::new();
        let a = b.mk_u8(b'a');
        let exact = b.mk_repeat(a, k, k);
        let star = b.mk_star(a);
        let u = b.mk_union(exact, star);
        let want = nerode(b"ac", 4, |w| w.iter().all(|&c| c == b'a'));
        assert_eq!(minimal(&mut b, u), want, "a{{{k}}}|a*");
    }
}
