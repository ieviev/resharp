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
        })
        .collect()
}

fn run_file(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore {
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
        let re = Regex::new(&tc.pattern).unwrap();
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
fn anchors() {
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
fn edge_cases() {
    run_file("edge_cases.toml");
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

// -- literal alternation with DFA state jumping --

#[test]
fn literal_alt_pure() {
    check_vs_regex("cat|dog|bird", b"I have a cat and a dog and a bird");
}

#[test]
fn literal_alt_pure_no_match() {
    check_vs_regex("cat|dog|bird", b"I have a fish");
}

#[test]
fn literal_alt_pure_adjacent() {
    check_vs_regex("aa|bb|cc", b"aabbcc");
}

#[test]
fn literal_alt_pure_overlapping_prefix() {
    // bar|baz gets factored to ba(r|z) by algebra
    check_vs_regex("bar|baz", b"bar baz bar");
}

#[test]
fn literal_alt_factored_three() {
    // algebra factors shared prefix
    check_vs_regex("fooX|fooY|fooZ", b"__fooX__fooZ__fooY__");
}

#[test]
fn literal_alt_with_suffix_digits() {
    check_vs_regex("(cat|dog)\\d+", b"cat123 dog45 cat bird99");
}

#[test]
fn literal_alt_with_suffix_no_match() {
    // suffix doesn't match
    check_vs_regex("(cat|dog)\\d+", b"cat! dog? catdog");
}

#[test]
fn literal_alt_with_suffix_at_end() {
    check_vs_regex("(foo|bar)\\d{1,3}", b"foo99");
}

#[test]
fn literal_alt_with_suffix_zero_width() {
    // [a-z]{0,3} can match zero chars
    check_vs_regex("(cat|dog)[a-z]{0,3}", b"cat dog123 catfish dogs");
}

#[test]
fn literal_alt_factored_with_suffix() {
    // bar|baz → ba(r|z), then suffix
    check_vs_regex("(bar|baz)\\d+", b"bar99 baz1 ba! bar");
}

#[test]
fn literal_alt_single() {
    check_vs_regex("hello|world", b"hello world");
}

#[test]
fn literal_alt_at_boundaries() {
    check_vs_regex("cat|dog", b"catdog");
}

#[test]
fn literal_alt_repeated_input() {
    check_vs_regex(
        "Sherlock|Holmes|Watson",
        b"Sherlock Holmes and Watson met Sherlock",
    );
}

#[test]
fn literal_prefix_with_suffix_dfa() {
    // tests prefix_fwd_state path: literal prefix + non-fixed-length suffix
    check_vs_regex("hello\\w+", b"helloworld hello123 hello");
}

#[test]
fn literal_prefix_suffix_at_end() {
    check_vs_regex("http\\w*", b"http https httpd");
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
    assert!(r[0].1 <= 5, "complement+alpha: got {:?}", r);
}

#[test]
fn complement_bounded_repeat_inter_2() {
    let re = Regex::new("~(_*(\\n_*){2})&_*d_*").unwrap();
    let m = re.find_all(b"ab\ncd\nef").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert!(r[0].1 <= 5, "complement+contains_d: got {:?}", r);
}

#[test]
fn complement_bounded_repeat_inter_3() {
    let re = Regex::new("~(_*(\\n_*){2})&[a-z]_*&_*d_*").unwrap();
    let m = re.find_all(b"ab\ncd\nef").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    assert!(r[0].1 <= 5, "complement+alpha+contains_d: got {:?}", r);
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
    // let pattern = ".{0,10}(abc|def|ghi|jkl).{0,10}";
    let pattern = ".{0,10}(abc|def|ghi|jkl)";
    // let input = b"def;jkl;ghi";
    let input = b"def;jkl;ghi";
    let re = Regex::new(pattern).unwrap();
    let m = re.find_all(input).unwrap();
    assert!(!m.is_empty(), "should match");
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
fn find_anchored() {
    run_file("find_anchored.toml");
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

// -- collect_rev null count tests --
// BUG: collect_rev produces duplicate/excess nulls for lookahead
// patterns, causing quadratic behavior in scan_fwd_all.
// each test documents current (buggy) count vs ideal count.

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
    // 3 matches, ideal=3, BUG: 9 nulls [13, 7, 1, 1, 7, 1, 1, 1, 1]
    let nulls = rev_nulls(r"(?<=\s)[A-Z][a-z]+(?=\s)", b" Hello World Foo ");
    eprintln!("readme short: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 9); // TODO: fix to <= 3
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
    // BUG: currently quadratic (65 -> 5150 -> ~500k)
    // TODO: fix to assert n1000 <= n100 * 12
}

#[test]
fn collect_rev_lookahead_simple() {
    // 2 matches, ideal=2, BUG: 3 nulls [4, 1, 1] (dup at 1)
    let nulls = rev_nulls(r"a(?=b)", b"_ab_ab_");
    eprintln!("a(?=b): {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 3); // TODO: fix to <= 2
}

#[test]
fn collect_rev_dotstar_lookahead() {
    // .* can legitimately match at every position before "aaa"
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
    // 1 match, ideal=1, BUG: 4 nulls [2, 1, 0, 0]
    let nulls = rev_nulls(r"a+\b(?=.*---)", b"aaa ---");
    eprintln!("wb: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 4); // TODO: fix to <= 1
}

#[test]
fn collect_rev_lookbehind_lookahead_combined() {
    // 2 matches, ideal=2, BUG: 4 nulls [2, 1, 2, 1] (dups)
    let nulls = rev_nulls(r"(?<=a.*).(?=.*c)", b"a__c");
    eprintln!("lb+la: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 4); // TODO: fix to <= 2
}

#[test]
fn collect_rev_lookahead_class_repetition() {
    // 2 matches, ideal=2, BUG: 6 nulls [5, 4, 2, 1, 0, 0]
    let nulls = rev_nulls(r"[a-z]+(?=[A-Z])", b"abcDefGhi");
    eprintln!("class rep: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 6); // TODO: fix to <= 2
}

#[test]
fn collect_rev_lookahead_time_pattern() {
    // 1 match, ideal=1, BUG: 3 nulls [1, 0, 0]
    let nulls = rev_nulls(r"\d+(?=[aApP]\.?[mM]\.?)", b"10pm");
    eprintln!("time: {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 3); // TODO: fix to <= 1
}

#[test]
fn collect_rev_lookahead_scaling_stress() {
    let re = Regex::new(r"[a-z]+(?=[A-Z])").unwrap();

    let mk_input = |n: usize| -> Vec<u8> {
        let mut v = Vec::new();
        for _ in 0..n {
            v.extend_from_slice(b"abcD");
        }
        v
    };

    let n10 = re.collect_rev_nulls_debug(&mk_input(10)).len();
    let n100 = re.collect_rev_nulls_debug(&mk_input(100)).len();
    let n1000 = re.collect_rev_nulls_debug(&mk_input(1000)).len();

    eprintln!(
        "[a-z]+(?=[A-Z]) nulls: 10-rep={}, 100-rep={}, 1000-rep={}",
        n10, n100, n1000,
    );

    // this pattern scales linearly (good)
    assert!(
        n1000 <= n100 * 12,
        "1000-rep nulls ({}) grew more than 12x vs 100-rep ({})",
        n1000,
        n100,
    );
}
