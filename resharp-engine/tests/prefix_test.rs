use resharp::{calc_potential_start, calc_prefix_sets, RegexBuilder};

/// helper: parse pattern, reverse it, compute linear prefix sets, return pp'd sets.
fn prefix_pp(pattern: &str) -> Vec<String> {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let sets = calc_prefix_sets(&mut b, rev).unwrap();
    sets.iter().map(|&set| b.solver_ref().pp(set)).collect()
}

/// helper: parse pattern, reverse it, BFS to first nullable, return pp'd sets joined by ";".
fn potential_start_pp(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let sets = calc_potential_start(&mut b, rev, 16, 64).unwrap();
    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

/// helper: forward BFS to first nullable, return pp'd sets joined by ";".
fn fwd_potential_start_pp(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let sets = calc_potential_start(&mut b, node, 16, 64).unwrap();
    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

#[test]
fn prefix_twain() {
    let p = prefix_pp("Twain");
    assert_eq!(p, vec!["n", "i", "a", "w", "T"]);
}

#[test]
fn prefix_intersection() {
    let p = prefix_pp("_*A_*&_*B");
    assert_eq!(p, vec!["B"]);
}

#[test]
fn prefix_huck() {
    let p = prefix_pp("_*Huck_*");
    assert_eq!(p, vec!["k", "c", "u", "H"]);
}

// -- prefix for a simple literal
#[test]
fn prefix_hello() {
    let p = prefix_pp("hello");
    assert_eq!(p, vec!["o", "l", "l", "e", "h"]);
}

#[test]
fn potential_start_alternation() {
    assert_eq!(
        potential_start_pp("Tom|Sawyer|Huckleberry|Finn"),
        "[mnry];[enor];[Tiry]"
    );
}

// -- prefix for lookahead: .*(?=aaa)
#[test]
fn prefix_lookahead() {
    let p = prefix_pp(".*(?=aaa)");
    assert_eq!(p, vec!["a", "a", "a"]);
}

// -- bounded repeat: ab{2,4}c reversed = cb{2,4}a
#[test]
fn prefix_bounded_repeat() {
    let p = prefix_pp("ab{2,4}c");
    assert!(
        p.len() <= 3 || p.iter().any(|s| s.len() > 1),
        "bounded repeat should not produce a long single-byte prefix: {:?}",
        p
    );
}

#[test]
fn potential_start_union_suffix() {
    assert_eq!(
        potential_start_pp("Huck[a-zA-Z]+|Saw[a-zA-Z]+"),
        "[A-Za-z];[kw];[ac];[Su]"
    );
}

#[test]
fn potential_start_long_union() {
    assert_eq!(
        potential_start_pp(
            "Sherlock Holmes|John Watson|Irene Adler|Inspector Lestrade|Professor Moriarty"
        ),
        "[enrsy];[deot];[almrs];[adlrt];[Aaiot];[ HWrs];[ eo];[LMkn];[ ceh];[or];[IJlo]"
    );
}

// -- forward BFS for Teddy prefix construction on literal union
#[test]
fn fwd_potential_start_literal_union() {
    assert_eq!(
        fwd_potential_start_pp("Sherlock|Holmes|Watson|Irene|Adler"),
        "[AHISW];[adhor];[elt];[emnrs];[elor]"
    );
}

#[test]
fn prefix_intersection_abc() {
    // .*a.*&.*b.*&.*c.* - must contain a, b, and c
    // linear prefix is empty (reversed intersection bifurcates immediately)
    let p = prefix_pp(".*a.*&.*b.*&.*c.*");
    assert!(p.is_empty());
    // BFS finds [a-c] at first position, then any char
    let s = potential_start_pp(".*a.*&.*b.*&.*c.*");
    assert_eq!(s, "[a-c];.;.");
}

#[test]
fn collect_rev_intersection_abc() {
    use resharp_algebra::TRegex;

    let pattern = ".*a.*&.*b.*&.*c.*";
    let mut b = resharp::RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let ts_rev = b.mk_concat(resharp::NodeId::TS, rev);

    eprintln!("rev node: {}", b.pp(rev));
    eprintln!("ts_rev node: {}", b.pp(ts_rev));
    eprintln!("ts_rev nullability: {:?}", b.nullability(ts_rev));

    fn resolve(
        b: &resharp::RegexBuilder,
        der_id: resharp_algebra::TRegexId,
        set: resharp::TSetId,
    ) -> resharp::NodeId {
        match b.get_tregex(der_id).clone() {
            TRegex::Leaf(n) => n,
            TRegex::ITE(cond, then_br, else_br) => {
                let s = b.solver_ref();
                let sat =
                    resharp_algebra::solver::Solver::is_sat(&s.get_set(set), &s.get_set(cond));
                if sat {
                    resolve(b, then_br, set)
                } else {
                    resolve(b, else_br, set)
                }
            }
        }
    }

    // walk "abc" in reverse: positions 2,1,0 reading c,b,a
    // first step uses BEGIN mask (like the DFA begin_table), rest use CENTER
    let input = b"abc";
    let mut current = ts_rev;
    for i in (0..input.len()).rev() {
        let byte = input[i];
        let byte_set = b.solver().u8_to_set_id(byte);
        let mask = if i == input.len() - 1 {
            resharp::Nullability::BEGIN
        } else {
            resharp::Nullability::CENTER
        };

        let der = b.der(current, mask).unwrap();
        let next = resolve(&b, der, byte_set);

        eprintln!(
            "pos={} byte='{}' mask={:?}: {} -> {}  nullable={:?}",
            i,
            byte as char,
            mask,
            b.pp(current),
            b.pp(next),
            b.nullability(next)
        );
        current = next;
    }

    // final node after walking c,b,a has nullability=7 (ALWAYS) and NullsId(1)
    eprintln!(
        "final node: {} nullability={:?} nulls_id={:?}",
        b.pp(current),
        b.nullability(current),
        b.get_nulls_id(current)
    );

    // prefix_transition is computed but unused; collect_rev_prefix walks
    // each byte individually through center_table, so intersections work.
    let re = resharp::Regex::new(pattern).unwrap();
    let nulls = re.collect_rev_nulls_debug(input);
    eprintln!("collect_rev nulls: {:?}", nulls);
    assert!(nulls.contains(&0), "expected 0 in nulls, got {:?}", nulls);
}

// -- literal with wildcard prefix: ..g
#[test]
fn prefix_dotdot_g() {
    let p = prefix_pp("..g");
    assert!(!p.is_empty(), "expected at least 1 prefix position");
    assert_eq!(p[0], "g");
}

// -- calc_potential_start for rev patterns with lookahead
#[test]
fn potential_start_rev_lookahead_word() {
    // rev of (?<=\s)[A-Z][a-z]+(?=\s): whitespace set first, then letters, then uppercase+letter, then whitespace
    assert_eq!(
        potential_start_pp(r"(?<=\s)[A-Z][a-z]+(?=\s)"),
        r"[\t-\r \x85\xA0];[a-z\xC2];[A-Za-z];[\t-\r A-Za-z\x85\xA0]"
    );
}

#[test]
fn potential_start_rev_simple_lookahead() {
    // rev of a(?=b): starts with 'b' lookahead, then 'a'
    assert_eq!(potential_start_pp(r"a(?=b)"), "b;a");
}

#[test]
fn potential_start_rev_lookbehind() {
    // rev of (?<=x)abc = cba(?=x): linear prefix c,b,a,x
    assert_eq!(potential_start_pp(r"(?<=x)abc"), "c;b;a;x");
}

#[test]
fn potential_start_rev_word_boundary() {
    let s = potential_start_pp(r"\b[A-Z][a-z]+\b");
    // neg lookahead/lookbehind for unicode \w makes prefix extraction harder
    assert_eq!(s, "");
}

#[test]
fn potential_start_rev_dotstar_suffix() {
    // rev of _*Huck = kcuH_*: linear prefix k,c,u,H
    assert_eq!(potential_start_pp(r"_*Huck"), "k;c;u;H");
}

#[test]
fn potential_start_rev_alternation_with_lookahead() {
    assert_eq!(
        potential_start_pp(r"(?<=\s)(Tom|Sawyer|Finn)(?=\s)"),
        r"[\t-\r \x85\xA0];[mnr\xC2];[em-or];[Teinoy];[\t-\r FTiwy\x85\xA0]"
    );
}

#[test]
fn potential_start_rev_char_class_plus() {
    // [0-9]+ reversed is [0-9]+, nullable after one digit step
    assert_eq!(potential_start_pp(r"[0-9]+"), "[0-9]");
}
