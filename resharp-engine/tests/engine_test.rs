use resharp::{EngineOptions, Error, Regex};
use std::path::Path;

struct TestCase {
    name: String,
    pattern: String,
    input: String,
    matches: Vec<(usize, usize)>,
    ignore: bool,
    expect_error: bool,
    anchored: bool,
    vs_regex: bool,
}

fn load_tests(filename: &str) -> Vec<TestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| TestCase {
            name: t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            input: t
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            matches: t
                .get("matches")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|m| {
                            let a = m.as_array().unwrap();
                            (
                                a[0].as_integer().unwrap() as usize,
                                a[1].as_integer().unwrap() as usize,
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
            expect_error: t
                .get("expect_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            anchored: t.get("anchored").and_then(|v| v.as_bool()).unwrap_or(false),
            vs_regex: t.get("vs_regex").and_then(|v| v.as_bool()).unwrap_or(false),
        })
        .collect()
}

fn run_file(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore {
            continue;
        }
        if tc.vs_regex {
            check_vs_regex(&tc.pattern, tc.input.as_bytes());
            continue;
        }
        if tc.expect_error {
            // error may occur at compile time or during matching
            let re = match Regex::new(&tc.pattern) {
                Err(_) => continue,
                Ok(re) => re,
            };
            if !tc.input.is_empty() {
                let result = re.find_all(tc.input.as_bytes());
                assert!(
                    result.is_err(),
                    "file={}, name={:?}, pattern={:?}: expected error but got Ok",
                    filename,
                    tc.name,
                    tc.pattern
                );
            } else {
                panic!(
                    "file={}, name={:?}, pattern={:?}: expected error but compiled Ok (no input to test matching)",
                    filename, tc.name, tc.pattern
                );
            }
            continue;
        }
        let re = Regex::new(&tc.pattern).unwrap_or_else(|e| {
            panic!(
                "file={}, name={:?}, pattern={:?}: compile error: {}",
                filename, tc.name, tc.pattern, e
            )
        });
        if tc.anchored {
            let m = re.find_anchored(tc.input.as_bytes()).unwrap();
            let result: Vec<(usize, usize)> = m.iter().map(|m| (m.start, m.end)).collect();
            assert_eq!(
                result, tc.matches,
                "file={}, name={:?}, pattern={:?}, input={:?} (anchored)",
                filename, tc.name, tc.pattern, tc.input
            );
        } else {
            let matches = re.find_all(tc.input.as_bytes()).unwrap();
            let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
            assert_eq!(
                result, tc.matches,
                "file={}, name={:?}, pattern={:?}, input={:?}",
                filename, tc.name, tc.pattern, tc.input
            );
        }
    }
}

#[test]
fn basic() {
    run_file("basic.toml");
}

#[test]
fn normal_anchors() {
    run_file("anchors.toml");
}

#[test]
fn boolean() {
    run_file("boolean.toml");
}

#[test]
fn lookaround() {
    run_file("lookaround.toml");
}

#[test]
fn semantics() {
    run_file("semantics.toml");
}

#[test]
fn errors() {
    run_file("errors.toml");
}

#[test]
fn date_pattern() {
    run_file("date_pattern.toml");
}

#[test]
fn edge_cases() {
    run_file("edge_cases.toml");
}

#[test]
fn normal_cross_feature() {
    run_file("cross_feature.toml");
}

/// cross-validate resharp against regex crate
fn check_vs_regex(pattern: &str, input: &[u8]) {
    let re = Regex::new(pattern).unwrap();
    let matches = re.find_all(input).unwrap();
    let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();

    let rx = regex::bytes::Regex::new(pattern).unwrap();
    let expected: Vec<(usize, usize)> = rx.find_iter(input).map(|m| (m.start(), m.end())).collect();

    assert_eq!(
        result, expected,
        "resharp vs regex mismatch: pattern={:?}",
        pattern
    );
}

#[test]
fn literal_alt_is_match() {
    let re = Regex::new("cat|dog|bird").unwrap();
    assert!(re.is_match(b"I have a dog").unwrap());
    assert!(!re.is_match(b"I have a fish").unwrap());
}

#[test]
fn literal_alt_suffix_is_match() {
    let re = Regex::new("(cat|dog)\\d+").unwrap();
    assert!(re.is_match(b"cat123").unwrap());
    assert!(!re.is_match(b"cat!").unwrap());
}

fn _assert_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Regex>();
}

#[test]
fn precompiled_matches_lazy() {
    let pattern = "aa";
    let input = b"aaaa";
    let lazy_re = Regex::with_options(
        pattern,
        EngineOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        EngineOptions {
            dfa_threshold: 1000,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        lazy_re.find_all(input).unwrap(),
        precompiled_re.find_all(input).unwrap()
    );
}

#[test]
fn precompiled_complex() {
    let pattern = "[^F]+";
    let input = b"The Adventures of Huckleberry Finn', published in 1885.";
    let lazy_re = Regex::with_options(
        pattern,
        EngineOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        EngineOptions {
            dfa_threshold: 1000,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        lazy_re.find_all(input).unwrap(),
        precompiled_re.find_all(input).unwrap()
    );
}

#[test]
fn complement_bounded_repeat_inter_1() {
    let re = Regex::new("~(_*(\\n_*){2})&[a-z]_*").unwrap();
    let m = re.find_all(b"ab\ncd\nef").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r[0], (0, 5), "complement+alpha: got {:?}", r);
}

#[test]
fn complement_bounded_repeat_inter_2() {
    let re = Regex::new("~(_*(\\n_*){2})&_*d_*").unwrap();
    let m = re.find_all(b"ab\ncd\nef").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r[0], (0, 5), "complement+contains_d: got {:?}", r);
}

#[test]
fn complement_bounded_repeat_inter_3() {
    let re = Regex::new("~(_*(\\n_*){2})&[a-z]_*&_*d_*").unwrap();
    let m = re.find_all(b"ab\ncd\nef").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r[0], (0, 5), "complement+alpha+contains_d: got {:?}", r);
}

fn extract_prefix(pattern: &str) -> Vec<u8> {
    let mut b = resharp_algebra::RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.extract_literal_prefix(node).0
}

#[test]
fn literal_prefix_pure_literal() {
    assert_eq!(extract_prefix("Sherlock Holmes"), b"Sherlock Holmes");
}

#[test]
fn literal_prefix_with_wildcard() {
    assert_eq!(extract_prefix("https://.*"), b"https://");
}

#[test]
fn literal_prefix_alternation_at_root() {
    assert_eq!(extract_prefix("Sherlock|Holmes"), b"");
}

#[test]
fn literal_prefix_char_class_no_prefix() {
    assert_eq!(extract_prefix("[A-Z]herlock"), b"");
}

#[test]
fn literal_prefix_single_char_pattern() {
    assert_eq!(extract_prefix("a"), b"a");
}

fn check_literal_equiv(pattern: &str, input: &str) {
    let re_literal = Regex::new(pattern).unwrap();
    let mut b = resharp_algebra::RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let re_dfa = Regex::from_node(b, node, EngineOptions::default()).unwrap();
    let literal_matches = re_literal.find_all(input.as_bytes()).unwrap();
    let dfa_matches = re_dfa.find_all(input.as_bytes()).unwrap();
    assert_eq!(
        literal_matches, dfa_matches,
        "mismatch for pattern {:?} on input {:?}",
        pattern, input
    );
}

#[test]
fn literal_equiv_sherlock() {
    check_literal_equiv(
        "Sherlock Holmes",
        "Sherlock Holmes was a detective. Sherlock Holmes lived in London.",
    );
}

#[test]
fn literal_equiv_prefix_the() {
    check_literal_equiv("the ", "the cat sat on the mat");
}

#[test]
fn literal_equiv_no_prefix() {
    check_literal_equiv("[A-Z]herlock", "Sherlock and sherlock");
}

#[test]
fn literal_equiv_empty_input() {
    check_literal_equiv("Sherlock Holmes", "");
}

#[test]
fn literal_equiv_no_match() {
    check_literal_equiv("Sherlock Holmes", "Watson was here");
}

#[test]
fn capacity_exceeded_at_compile() {
    let result = Regex::with_options(
        "a.*b.*c",
        EngineOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 2,
            ..Default::default()
        },
    );
    assert!(
        matches!(result, Err(Error::CapacityExceeded)),
        "expected CapacityExceeded error"
    );
}

#[test]
fn dictionary_small() {
    let pattern = "accommodating|acknowledging|comprehensive|corresponding|disappointing";
    let input = b"a]comprehensive/disappointing;acknowledging";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(2, 15), (16, 29), (30, 43)]);
}

#[test]
fn dictionary_context_small() {
    let pattern = ".{0,10}(abc|def|ghi|jkl)";
    let input = b"def;jkl;ghi";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    assert!(!m.is_empty(), "should match");
}

#[test]
fn dictionary_context_small_both() {
    let pattern = ".{0,10}(abc|def|ghi|jkl).{0,10}";
    let input = b"def;jkl;ghi";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    assert!(!m.is_empty(), "should match with prefix+suffix");
}

#[test]
fn dictionary_context_small_suffix() {
    let pattern = "(abc|def|ghi|jkl).{0,10}";
    let input = b"def;jkl;ghi";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    assert!(!m.is_empty(), "should match");
}

#[test]
fn dictionary_context_medium() {
    let path = format!(
        "{}/../data/regexes/dictionary-fixed-context.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let pattern = std::fs::read_to_string(&path).unwrap();
    let pattern = pattern.trim()[7..].trim();
    let input = b"hello Zoroastrianism's world";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    assert!(!m.is_empty(), "should match");
}

#[test]
fn normal_paragraph() {
    run_file("paragraph.toml");
}

#[test]
fn find_anchored() {
    run_file("find_anchored.toml");
}

#[test]
fn ci() {
    run_file("ci.toml");
}

#[test]
fn word_boundary() {
    run_file("word_boundary.toml");
}

#[test]
fn literal_alt() {
    run_file("literal_alt.toml");
}

#[test]
fn capacity_exceeded_at_match() {
    let re = Regex::with_options(
        "a.*b.*c.*d",
        EngineOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let result = re.find_all(b"a___b___c___d");
    assert!(
        matches!(result, Err(Error::CapacityExceeded)),
        "expected CapacityExceeded error"
    );
}

#[test]
fn unanchored_search_false_positive() {
    let cases = [
        ("A00[12]", "A003"),
        ("A00[12]", "A004"),
        ("A00[12]", "sample_A003_chunk_001.txt"),
        ("A001|A002", "A003"),
        ("A001|A002", "A004"),
    ];

    for (pattern, input) in cases {
        let re = Regex::new(pattern).unwrap();

        assert_eq!(re.find_anchored(input.as_bytes()).unwrap(), None);

        let spans = re.find_all(input.as_bytes()).unwrap();
        assert_eq!(
            spans,
            [],
            "unanchored false positive for pattern={pattern:?}, input={input:?}, spans={spans:?}"
        );
    }
}

fn rev_nulls(pattern: &str, input: &[u8]) -> Vec<usize> {
    let re = Regex::new(pattern).unwrap();
    let nulls = re.collect_rev_nulls_debug(input);
    for i in 1..nulls.len() {
        assert!(
            nulls[i] <= nulls[i - 1],
            "rev nulls not sorted descending at [{}]: {} > {} (pattern={:?}, input={:?}, nulls={:?})",
            i, nulls[i], nulls[i - 1], pattern, std::str::from_utf8(input).unwrap_or("?"), nulls
        );
    }
    nulls
}

#[test]
fn collect_rev_nulls_sorted_descending() {
    rev_nulls(r" [A-Z][a-z]+ ", b" Hello World Foo ");
    rev_nulls(r"[A-Z][a-z]+", b" Hello World Foo ");
    rev_nulls(r"(?<=\s)[A-Z][a-z]+(?=\s)", b" Hello World Foo ");
}

#[test]
fn collect_rev_readme_lookahead_short() {
    let nulls = rev_nulls(r"(?<=\s)[A-Z][a-z]+(?=\s)", b" Hello World Foo ");
    eprintln!("readme short: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 3);
}

#[test]
fn collect_rev_readme_lookahead_scaling() {
    let pattern = r"(?<=\s)[A-Z][a-z]+(?=\s)";
    let re = Regex::new(pattern).unwrap();

    let n10 = re
        .collect_rev_nulls_debug(" Aaa ".repeat(10).as_bytes())
        .len();
    let n100 = re
        .collect_rev_nulls_debug(" Aaa ".repeat(100).as_bytes())
        .len();
    let n1000 = re
        .collect_rev_nulls_debug(" Aaa ".repeat(1000).as_bytes())
        .len();

    eprintln!(
        "collect_rev scaling: 10-rep={}, 100-rep={}, 1000-rep={}",
        n10, n100, n1000,
    );
    assert!(n1000 <= n100 * 12);
}

#[test]
fn collect_rev_lookahead_simple() {
    let nulls = rev_nulls(r"a(?=b)", b"_ab_ab_");
    assert_eq!(nulls.len(), 2);
}

#[test]
fn collect_rev_dotstar_lookahead() {
    let re = Regex::new(r".*(?=aaa)").unwrap();
    let n = re.collect_rev_nulls_debug(b"baaa");
    eprintln!(".*(?=aaa) on \"baaa\": {} nulls {:?}", n.len(), n);

    let short = "b".repeat(10) + "aaa";
    let long = "b".repeat(1000) + "aaa";
    let n_short = re.collect_rev_nulls_debug(short.as_bytes());
    let n_long = re.collect_rev_nulls_debug(long.as_bytes());
    eprintln!(
        ".*(?=aaa): short(13)={} nulls, long(1003)={} nulls",
        n_short.len(),
        n_long.len()
    );
}

#[test]
fn collect_rev_dotstar_lookahead_multiple() {
    let re = Regex::new(r".*(?=.*bbb)(?=.*ccc)").unwrap();
    let short = "aaa bbb ccc";
    let long = "a".repeat(500) + " bbb " + &"a".repeat(500) + " ccc";
    let n_short = re.collect_rev_nulls_debug(short.as_bytes());
    let n_long = re.collect_rev_nulls_debug(long.as_bytes());
    eprintln!(
        "chained: short({})={} nulls, long({})={} nulls",
        short.len(),
        n_short.len(),
        long.len(),
        n_long.len(),
    );
}

#[test]
fn collect_rev_lookahead_word_boundary() {
    let nulls = rev_nulls(r"a+\b(?=.*---)", b"aaa ---");
    eprintln!("wb: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 3);
}

#[test]
fn collect_rev_lookbehind_lookahead_combined() {
    let nulls = rev_nulls(r"(?<=a.*).(?=.*c)", b"a__c");
    eprintln!("lb+la: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 2);
}

#[test]
fn collect_rev_lookahead_class_repetition() {
    let nulls = rev_nulls(r"[a-z]+(?=[A-Z])", b"abcDefGhi");
    eprintln!("class rep: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 5);
}

#[test]
fn collect_rev_lookahead_time_pattern() {
    let nulls = rev_nulls(r"\d+(?=[aApP]\.?[mM]\.?)", b"10pm");
    eprintln!("time: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 2);
}

#[test]
fn literal_20_bytes() {
    let pattern = "ABCDEFGHIJKLMNOPQRST";
    let mut hay = vec![b'.'; 200];
    hay[100..120].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re
        .find_all(&hay)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(r, vec![(100, 120)]);
}

#[test]
fn literal_16_bytes() {
    let pattern = "ABCDEFGHIJKLMNOP";
    let mut hay = vec![b'.'; 100];
    hay[50..66].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re
        .find_all(&hay)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(r, vec![(50, 66)]);
}

#[test]
fn literal_17_bytes() {
    let pattern = "ABCDEFGHIJKLMNOPQ";
    let mut hay = vec![b'.'; 100];
    hay[40..57].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re
        .find_all(&hay)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(r, vec![(40, 57)]);
}
#[test]
fn dotstar_inner_literal_rev_midskip() {
    let re = Regex::new(".*=.*").unwrap();
    // exercise rev DFA with multiline input to trigger \n mid-skip on nullable state
    let nulls = re.collect_rev_nulls_debug(b"first\nsecond=line\nthird");
    let mut sorted = nulls.clone();
    sorted.sort();
    sorted.dedup();
    // match starts: every position from 6 ('s' in second) to 12 ('=')
    assert_eq!(sorted, vec![6, 7, 8, 9, 10, 11, 12]);
}

#[test]
fn dotstar_huck_stripped_prefix() {
    let re = Regex::new(".*Huck.*&~(.*F.*)").unwrap();
    let m = re
        .find_all(b"The Adventures of Huckleberry Finn', published in 1885.")
        .unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(0, 30)], ".*Huck.*&~(.*F.*)");
}

#[test]
fn nullable_head_correctness() {
    // non-Star nullable heads can't use backward scan (no self-loop)
    let re = Regex::new(r"\d?abc").unwrap();
    let check = |input: &[u8], expected: Vec<(usize, usize)>| {
        let m = re.find_all(input).unwrap();
        let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(
            r,
            expected,
            r"input={:?}",
            std::str::from_utf8(input).unwrap()
        );
    };
    check(b"abc", vec![(0, 3)]);
    check(b"1abc", vec![(0, 4)]);
    check(b"x1abcx", vec![(1, 5)]);
    check(b"xabcx", vec![(1, 4)]);
    check(b"11abc", vec![(1, 5)]); // \d? is at most one digit
    check(b"1abc2abc", vec![(0, 4), (4, 8)]);
}

#[test]
fn bounded_dfa_basic() {
    // intersection with complement: variable length, no prefix, bounded
    // _*c_*&[a-z]{2,4} = 2-4 lowercase letters containing 'c'
    let re = Regex::new("_*c_*&[a-z]{2,4}").unwrap();
    let m = re.find_all(b"xycdzz abcde fg").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, [(0, 4), (7, 11)]);
}

use resharp::{NodeId, RegexBuilder, BDFA};

fn chain_len(node: NodeId, b: &RegexBuilder) -> usize {
    let mut n = 0;
    let mut cur = node;
    while cur != NodeId::MISSING {
        n += 1;
        cur = cur.right(b);
    }
    n
}

fn chain_pp(node: NodeId, b: &RegexBuilder) -> Vec<String> {
    let mut result = Vec::new();
    let mut cur = node;
    while cur != NodeId::MISSING {
        result.push(b.pp(cur));
        cur = cur.right(b);
    }
    result
}

fn bdfa_state_pp(pattern: &str, input: &[u8]) -> Vec<String> {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let mut bdfa = BDFA::new(&mut b, node).unwrap();
    let mut state = bdfa.initial;
    let mut result = Vec::new();
    for pos in 0..input.len() {
        let mt = bdfa.minterms_lookup[input[pos] as usize] as usize;
        state = (bdfa.transition(&mut b, state, mt).unwrap() & 0xFFFF) as u16;
        let rel = bdfa.match_rel[state as usize];
        let entries = chain_pp(bdfa.states[state as usize], &b);
        result.push(format!(
            "pos={} '{}' s={} rel={} [{}]",
            pos,
            input[pos] as char,
            state,
            rel,
            entries.join(", ")
        ));
    }
    result
}

fn bdfa_step_trace(pattern: &str, input: &[u8]) -> Vec<(usize, u16, usize, u32)> {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let mut bdfa = BDFA::new(&mut b, node).unwrap();
    let mut state = bdfa.initial;
    let mut trace = Vec::new();
    for pos in 0..input.len() {
        let mt = bdfa.minterms_lookup[input[pos] as usize] as usize;
        state = (bdfa.transition(&mut b, state, mt).unwrap() & 0xFFFF) as u16;
        let rel = bdfa.match_rel[state as usize];
        let clen = chain_len(bdfa.states[state as usize], &b);
        trace.push((pos, state, clen, rel));
    }
    trace
}

fn bdfa_matches(pattern: &str, input: &[u8]) -> Vec<(usize, usize)> {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let mut bdfa = BDFA::new(&mut b, node).unwrap();
    let mut state = bdfa.initial;
    let mut matches = Vec::new();
    let mut pos = 0;
    while pos < input.len() {
        let mt = bdfa.minterms_lookup[input[pos] as usize] as usize;
        state = (bdfa.transition(&mut b, state, mt).unwrap() & 0xFFFF) as u16;
        let rel = bdfa.match_rel[state as usize];
        if rel > 0 {
            let end_off = bdfa.match_end_off[state as usize];
            let start = pos + 1 - rel as usize;
            let end = pos + 1 - end_off as usize;
            matches.push((start, end));
            state = bdfa.initial;
            continue;
        }
        pos += 1;
    }
    // flush
    if state != bdfa.initial {
        let node = bdfa.states[state as usize];
        if node != NodeId::MISSING {
            let best = BDFA::counted_best(node, &b);
            if best > 0 {
                let end = input.len();
                let start = end - best as usize;
                matches.push((start, end));
            }
        }
    }
    matches
}

#[test]
fn bdfa_literal_abc() {
    // simple literal: each step should accumulate one more candidate
    let trace = bdfa_step_trace("abc", b"xabcx");
    eprintln!("abc on 'xabcx':");
    for &(pos, s, vl, rel) in &trace {
        eprintln!("  pos={} state={} vec_len={} rel={}", pos, s, vl, rel);
    }
    // after 'x' at pos=4, body dies with step=4 (match is 3 bytes starting at pos+1-step=1)
    assert!(
        trace.iter().any(|&(_, _, _, rel)| rel == 4),
        "expected match with rel=4 (step)"
    );
}

#[test]
fn bdfa_alternation_ab_cd() {
    assert_bdfa_eq("ab|cd", b"xabcdx");
}

#[test]
fn bdfa_two_candidates() {
    assert_bdfa_eq("aa", b"aaa");
}

fn assert_bdfa_eq(pattern: &str, input: &[u8]) {
    let m = bdfa_matches(pattern, input);
    let re = Regex::new(pattern).unwrap();
    let std_m: Vec<_> = re
        .find_all(input)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(
        m,
        std_m,
        "pattern={:?} input={:?}",
        pattern,
        String::from_utf8_lossy(input)
    );
}

#[test]
fn bdfa_ambiguous_a_or_aa() {
    assert_bdfa_eq("a|aa", b"aab");
}

#[test]
fn bdfa_ambiguous_ab_or_a() {
    assert_bdfa_eq("ab|a", b"abab");
}

#[test]
fn bdfa_ambiguous_repeat_ab_1_3() {
    assert_bdfa_eq("(ab){1,3}", b"abababx");
}

#[test]
fn bdfa_ambiguous_overlap_abc_bcd() {
    assert_bdfa_eq("abc|bcd", b"abcde");
}

#[test]
fn bdfa_ambiguous_a_1_4_greedy() {
    assert_bdfa_eq("a{1,4}", b"aaaa");
}

#[test]
fn bdfa_ambiguous_nested_alt() {
    assert_bdfa_eq("(a|ab)(b|c)", b"abcx");
}

#[test]
fn bdfa_ambiguous_triple_overlap() {
    assert_bdfa_eq("a{2,4}", b"aaaaaa");
}

#[test]
fn bdfa_multi_match_overlap() {
    assert_bdfa_eq("a{2,4}", b"aaaaaaaaa");
    assert_bdfa_eq("ab|a", b"ababababab");
    assert_bdfa_eq("(ab){1,3}", b"ababababababab");
    assert_bdfa_eq("abc|bcd", b"xabcbcdabcdy");
    assert_bdfa_eq("[a-c]{2,3}", b"abcabcabc");
}

#[test]
fn bdfa_prefix_literal() {
    // "Twain.{0,5}" has literal prefix "Twain", BDFA should skip to it
    assert_bdfa_eq("Twain.{0,5}", b"xxxx Twain was here, Twainyyy end");
}

#[test]
fn bdfa_prefix_predicate() {
    // [A-Z][a-z]{1,3} has deterministic prefix [A-Z]
    assert_bdfa_eq("[A-Z][a-z]{1,3}", b"Hello World Foo B xy");
}

#[test]
fn bdfa_prefix_predicate_pp() {
    let pp = bdfa_state_pp("[A-Z][a-z]{1,3}", b"Hello World Foo B xy");
    for line in &pp {
        eprintln!("{}", line);
    }
    assert_eq!(pp, vec![
        "pos=0 'H' s=2 rel=0 [#([a-z]{1,3})s1b0]",
        "pos=1 'e' s=3 rel=0 [#((|(|[a-z])[a-z]))s2b2]",
        "pos=2 'l' s=4 rel=0 [#((|[a-z]))s3b3]",
        "pos=3 'l' s=5 rel=0 [#()s4b4]",
        "pos=4 'o' s=6 rel=5 [#(\u{22a5})s5b4]",
        "pos=5 ' ' s=6 rel=5 [#(\u{22a5})s5b4]",
        "pos=6 'W' s=7 rel=5 [#(\u{22a5})s5b4, #([a-z]{1,3})s1b0]",
        "pos=7 'o' s=8 rel=5 [#(\u{22a5})s5b4, #((|(|[a-z])[a-z]))s2b2]",
        "pos=8 'r' s=9 rel=5 [#(\u{22a5})s5b4, #((|[a-z]))s3b3]",
        "pos=9 'l' s=10 rel=5 [#(\u{22a5})s5b4, #()s4b4]",
        "pos=10 'd' s=11 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4]",
        "pos=11 ' ' s=11 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4]",
        "pos=12 'F' s=12 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #([a-z]{1,3})s1b0]",
        "pos=13 'o' s=13 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #((|(|[a-z])[a-z]))s2b2]",
        "pos=14 'o' s=14 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #((|[a-z]))s3b3]",
        "pos=15 ' ' s=15 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #(\u{22a5})s4b3]",
        "pos=16 'B' s=16 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #(\u{22a5})s4b3, #([a-z]{1,3})s1b0]",
        "pos=17 ' ' s=15 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #(\u{22a5})s4b3]",
        "pos=18 'x' s=15 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #(\u{22a5})s4b3]",
        "pos=19 'y' s=15 rel=5 [#(\u{22a5})s5b4, #(\u{22a5})s5b4, #(\u{22a5})s4b3]",
    ]);
}

#[test]
fn bdfa_prefix_has_prefix() {
    // verify the BDFA actually built a prefix for a literal pattern
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, "Twain.{0,5}").unwrap();
    let bdfa = BDFA::new(&mut b, node).unwrap();
    assert!(bdfa.prefix.is_some(), "expected prefix for Twain.{{0,5}}");
    assert!(
        bdfa.prefix_len >= 5,
        "expected prefix_len >= 5, got {}",
        bdfa.prefix_len
    );
}

#[test]
fn bdfa_aws_key() {
    assert_bdfa_eq(
        r"((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))",
        b"xxx AKIAIOSFODNN7EXAMPLE yyy",
    );
}

#[test]
fn bdfa_cyrillic_names() {
    assert_bdfa_eq(
        "Шерлок Холмс|Джон Уотсон|Ирен Адлер|инспектор Лестрейд|профессор Мориарти",
        "zzz Шерлок Холмс и Джон Уотсон zzz".as_bytes(),
    );
}

#[test]
fn opts_unicode_false() {
    let re = Regex::with_options(
        r"\w+",
        EngineOptions::default().unicode(resharp::UnicodeMode::Ascii),
    )
    .unwrap();
    let m = re.find_all("café".as_bytes()).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].start, m[0].end), (0, 3));
    let re_u = Regex::new(r"\w+").unwrap();
    let m_u = re_u.find_all("café".as_bytes()).unwrap();
    assert_eq!(m_u.len(), 1);
    assert!(m_u[0].end > 3);
}

#[test]
fn opts_case_insensitive() {
    let re = Regex::with_options("hello", EngineOptions::default().case_insensitive(true)).unwrap();
    let m = re.find_all(b"Hello HELLO hello").unwrap();
    assert_eq!(m.len(), 3);
}

#[test]
fn opts_dot_matches_new_line() {
    let re =
        Regex::with_options("a.b", EngineOptions::default().dot_matches_new_line(true)).unwrap();
    let m = re.find_all(b"a\nb").unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].start, m[0].end), (0, 3));

    let re2 = Regex::new("a.b").unwrap();
    let m2 = re2.find_all(b"a\nb").unwrap();
    assert_eq!(m2.len(), 0);
}

#[test]
fn opts_dot_all_inline_flag() {
    let re = Regex::new("(?s)a.b").unwrap();
    let m = re.find_all(b"a\nb").unwrap();
    assert_eq!(m.len(), 1);
}

#[test]
fn opts_dot_all_scoped_group() {
    let re = Regex::new("(?s:a.b).c").unwrap();
    let m = re.find_all(b"a\nbxc").unwrap();
    assert_eq!(m.len(), 1);

    let m2 = re.find_all(b"a\nb\nc").unwrap();
    assert_eq!(m2.len(), 0);
}

#[test]
fn opts_ignore_whitespace() {
    let re = Regex::with_options(
        r"hello \ world",
        EngineOptions::default().ignore_whitespace(true),
    )
    .unwrap();
    let m = re.find_all(b"hello world").unwrap();
    assert_eq!(m.len(), 1);
}

#[test]
fn word_match_lengths_en_sampled() {
    let path = format!(
        "{}/../data/haystacks/en-sampled.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let content = std::fs::read_to_string(&path).unwrap();
    let input: String = content.lines().take(2500).collect::<Vec<_>>().join("\n");
    let input = input.as_bytes();

    let pattern = r"\b[0-9A-Za-z_]+\b";
    let re = Regex::with_options(
        pattern,
        EngineOptions::default().unicode(resharp::UnicodeMode::Ascii),
    )
    .unwrap();
    let matches = re.find_all(input).unwrap();

    let rx = regex::bytes::RegexBuilder::new(pattern)
        .unicode(false)
        .build()
        .unwrap();
    let expected: Vec<(usize, usize)> = rx.find_iter(input).map(|m| (m.start(), m.end())).collect();

    let sum: usize = matches.iter().map(|m| m.end - m.start).sum();
    let expected_sum: usize = expected.iter().map(|(s, e)| e - s).sum();

    assert_eq!(
        expected_sum, 56_691,
        "regex crate baseline changed: expected 56691, got {}",
        expected_sum,
    );
    assert_eq!(
        sum, 56_691,
        "resharp total match length: expected 56691, got {}",
        sum,
    );
    assert_eq!(
        matches.len(),
        expected.len(),
        "match count mismatch: resharp={} regex={}",
        matches.len(),
        expected.len(),
    );
}

fn run_file_hardened(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore || tc.expect_error || tc.anchored {
            continue;
        }
        if tc.vs_regex {
            check_hardened_vs_normal(&tc.pattern, tc.input.as_bytes());
            continue;
        }
        let opts = EngineOptions::default().hardened(true);
        let re = match Regex::with_options(&tc.pattern, opts) {
            Ok(re) => re,
            Err(_) => continue,
        };
        let matches = re.find_all(tc.input.as_bytes()).unwrap();
        let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(
            result, tc.matches,
            "HARDENED file={}, name={:?}, pattern={:?}, input={:?}",
            filename, tc.name, tc.pattern, tc.input
        );
    }
}

#[test]
fn hardened_basic() {
    run_file_hardened("basic.toml");
}

#[test]
fn hardened_anchors() {
    run_file_hardened("anchors.toml");
}

#[test]
fn hardened_semantics() {
    run_file_hardened("semantics.toml");
}

#[test]
fn hardened_date_pattern() {
    run_file_hardened("date_pattern.toml");
}

#[test]
fn hardened_edge_cases() {
    run_file_hardened("edge_cases.toml");
}

#[test]
fn hardened_lookaround() {
    run_file_hardened("lookaround.toml");
}

#[test]
fn hardened_boolean() {
    run_file_hardened("boolean.toml");
}

#[test]
fn hardened_cross_feature() {
    run_file_hardened("cross_feature.toml");
}

#[test]
fn hardened_paragraph() {
    run_file_hardened("paragraph.toml");
}

#[test]
fn hardened_find_anchored() {
    run_file_hardened("find_anchored.toml");
}

#[test]
fn hardened_ci() {
    run_file_hardened("ci.toml");
}

#[test]
fn hardened_word_boundary() {
    run_file_hardened("word_boundary.toml");
}

#[test]
fn hardened_literal_alt() {
    run_file_hardened("literal_alt.toml");
}

#[test]
fn hardened_pathological() {
    let pattern = r".*[^A-Z]|[A-Z]";
    let input = "A".repeat(1000);
    let re_normal = Regex::new(pattern).unwrap();
    let re_hardened =
        Regex::with_options(pattern, EngineOptions::default().hardened(true)).unwrap();
    assert_eq!(
        re_normal.find_all(input.as_bytes()).unwrap(),
        re_hardened.find_all(input.as_bytes()).unwrap(),
        "pathological pattern mismatch"
    );
}

fn check_hardened_vs_normal(pattern: &str, input: &[u8]) {
    let opts = EngineOptions::default().hardened(true);
    let re_s = match Regex::with_options(pattern, opts) {
        Ok(re) => re,
        Err(_) => return, // skip patterns that fail in hardened mode (e.g. lookaround)
    };
    let re_n = Regex::new(pattern).unwrap();
    let normal = re_n.find_all(input).unwrap();
    let hardened = re_s.find_all(input).unwrap();
    assert_eq!(
        normal,
        hardened,
        "hardened vs normal mismatch: pattern={:?}, input={:?}",
        pattern,
        std::str::from_utf8(input).unwrap_or("<binary>")
    );
}

#[test]
fn hardened_cross_validate() {
    let en = std::fs::read_to_string(format!(
        "{}/../data/haystacks/en-sampled.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let input = &en.as_bytes()[..2000];
    let patterns = [
        r"\d+",
        r"[A-Z][a-z]+",
        r"\w{3,8}",
        r"[aeiou]+",
        r"the|and|for|that|with",
        r"[0-9]{1,3}\.[0-9]{1,3}",
        r"[A-Z]{2,}",
        r".*[^a-z]|[a-z]",
        r"\d{4}-\d{2}-\d{2}",
        r"[A-Za-z]{8,13}",
        r"(Sherlock|Holmes|Watson)[a-z]{0,5}",
    ];
    for p in &patterns {
        check_hardened_vs_normal(p, input);
    }
    // pathological: dense candidates with dotstar
    let aaaa = "A".repeat(500);
    check_hardened_vs_normal(r".*[^A-Z]|[A-Z]", aaaa.as_bytes());
    check_hardened_vs_normal(r"[A-Z]+", aaaa.as_bytes());
    check_hardened_vs_normal(r"A{1,3}", aaaa.as_bytes());
}

#[test]
fn hardened_bounded_repeat_tail() {
    let s8 = "A".repeat(8);
    let s500 = "A".repeat(500);
    let s7 = "A".repeat(7);
    let s10 = "A".repeat(10);
    let cases: Vec<(&str, &str)> = vec![
        (r"A{1,3}", &s8),
        (r"A{1,3}", &s500),
        (r"A{2,5}", &s7),
        (r"[A-Z]{1,3}", &s10),
    ];
    for (pattern, input) in &cases {
        let re_ref = regex::Regex::new(pattern).unwrap();
        let expected: Vec<(usize, usize)> = re_ref
            .find_iter(input)
            .map(|m| (m.start(), m.end()))
            .collect();

        let re_u = Regex::with_options(pattern, EngineOptions::default().hardened(true)).unwrap();
        let got: Vec<(usize, usize)> = re_u
            .find_all(input.as_bytes())
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();

        assert_eq!(
            expected,
            got,
            "BDFA bounded repeat mismatch: pattern={:?}, len={}",
            pattern,
            input.len()
        );
    }
}

#[test]
fn range_prefix_correctness() {
    let en = std::fs::read_to_string(format!(
        "{}/../data/haystacks/en-sampled.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let inputs: Vec<&[u8]> = vec![
        en.as_bytes(),
        b"hello world no caps here 123",
        b"ABCDEFGhijklmnop",
        b"aZbYcXdW",
        b"",
        b"Z",
        b"ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ", // > 32 bytes of matches
        &[0u8; 100],                               // no ASCII letters
    ];
    // patterns with >16 byte char classes that should use range prefix
    let patterns = [
        r"[A-Z]+",
        r"[A-Z][a-z]+",
        r"[A-Z]{2,}",
        r"[A-Za-z]+",
        r"[A-Za-z0-9]+",
        r"[A-Z][A-Z][a-z]",
    ];
    for p in &patterns {
        let re = Regex::new(p).unwrap();
        let re_hardened = Regex::with_options(p, EngineOptions::default().hardened(true)).unwrap();
        for input in &inputs {
            let normal = re.find_all(input).unwrap();
            let hardened = re_hardened.find_all(input).unwrap();
            assert_eq!(
                normal,
                hardened,
                "range prefix mismatch: pattern={:?}, input={:?}",
                p,
                std::str::from_utf8(input).unwrap_or("<binary>")
            );
        }
    }
}

#[test]
fn range_prefix_random_haystack() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let patterns = [r"[A-Z][a-z]+", r"[A-Z]{2,5}", r"[A-Za-z]{3,}"];
    for seed in 0u64..50 {
        let mut h = DefaultHasher::new();
        seed.hash(&mut h);
        let hash = h.finish();
        // generate pseudorandom haystack mixing ASCII ranges
        let input: Vec<u8> = (0..256)
            .map(|i| {
                let v = ((hash.wrapping_mul(i as u64 + 1).wrapping_add(seed)) >> 8) as u8;
                // bias toward printable ASCII
                32 + (v % 95)
            })
            .collect();
        for p in &patterns {
            let re = Regex::new(p).unwrap();
            let re_s = Regex::with_options(p, EngineOptions::default().hardened(true)).unwrap();
            let normal = re.find_all(&input).unwrap();
            let hardened = re_s.find_all(&input).unwrap();
            assert_eq!(
                normal, hardened,
                "random haystack mismatch: seed={}, pattern={:?}",
                seed, p
            );
        }
    }
}

#[test]
fn fwd_prefix_search_long_prefix_no_panic() {
    let re = Regex::new("[aA]bcdefghijklmnopqrs(x|xy)").unwrap();
    let m = re.find_all(b"abcdefghijklmnopqrsx").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(0, 20)]);
}

#[test]
fn hardened_nullable_empty_after_dedup() {
    let cases: Vec<(&str, &str)> = vec![
        (r".*(?=aaa)", "baaa"),
        (r".*(?=b_)", "_ab_ab_"),
        (r"a*", "bab"),
        (r"a*", "aab"),
        (r"[a-z]*", "1a2"),
        (r"_*", "ab"),
    ];
    for (pattern, input) in &cases {
        let re_normal = Regex::new(pattern).unwrap();
        let normal: Vec<(usize, usize)> = re_normal
            .find_all(input.as_bytes())
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();

        let opts = EngineOptions::default().hardened(true);
        let re_h = Regex::with_options(pattern, opts).unwrap();
        let hardened: Vec<(usize, usize)> = re_h
            .find_all(input.as_bytes())
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(
            hardened, normal,
            "hardened mismatch: pattern={:?} input={:?}\n  normal:   {:?}\n  hardened: {:?}",
            pattern, input, normal, hardened
        );
    }
}

#[test]
#[ignore = "takes a while"]
fn hardened_cross_validate_all_toml() {
    let files = [
        "basic.toml",
        "anchors.toml",
        "semantics.toml",
        "date_pattern.toml",
        "edge_cases.toml",
        "lookaround.toml",
        "boolean.toml",
        "cross_feature.toml",
        "paragraph.toml",
        "cloudflare_redos.toml",
        "find_anchored.toml",
        "accel_skip.toml",
        "ci.toml",
        "word_boundary.toml",
        "literal_alt.toml",
    ];
    let mut tested = 0;
    let mut activated = 0;
    for file in &files {
        let tests = load_tests(file);
        for tc in &tests {
            if tc.ignore || tc.expect_error || tc.anchored {
                continue;
            }
            let opts = EngineOptions::default().hardened(true);
            let re = match Regex::with_options(&tc.pattern, opts) {
                Ok(re) => re,
                Err(_) => continue,
            };
            tested += 1;
            if re.is_hardened() {
                activated += 1;
            }
            let matches = re.find_all(tc.input.as_bytes()).unwrap();
            let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
            assert_eq!(
                result,
                tc.matches,
                "HARDENED-XVAL file={}, name={:?}, pattern={:?}, input={:?}, is_hardened={}",
                file,
                tc.name,
                tc.pattern,
                tc.input,
                re.is_hardened()
            );
        }
    }
    eprintln!(
        "hardened_cross_validate_all_toml: {tested} tested, {activated} activated hardened mode"
    );
    assert!(
        activated >= 10,
        "expected at least 10 patterns to activate hardened, got {activated}"
    );
}

struct InternalTestCase {
    name: String,
    pattern: String,
    pp: String,
}

fn load_internal_tests(filename: &str) -> Vec<InternalTestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| InternalTestCase {
            name: t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            pp: t["pp"].as_str().unwrap().to_string(),
        })
        .collect()
}

fn run_file_internal(filename: &str) {
    let tests = load_internal_tests(filename);
    for tc in &tests {
        let mut b = resharp::RegexBuilder::new();
        let node = resharp_parser::parse_ast(&mut b, &tc.pattern).unwrap_or_else(|e| {
            panic!(
                "file={}, name={:?}, pattern={:?}: compile error: {}",
                filename, tc.name, tc.pattern, e
            )
        });
        let got = b.pp(node);
        assert_eq!(
            got, tc.pp,
            "file={}, name={:?}, pattern={:?}",
            filename, tc.name, tc.pattern
        );
    }
}

#[test]
fn internal() {
    run_file_internal("internal.toml");
}
