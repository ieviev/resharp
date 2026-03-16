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
fn date_pattern() {
    run_file("date_pattern.toml");
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
fn paragraph() {
    run_file("paragraph.toml");
}

#[test]
fn find_anchored() {
    run_file("find_anchored.toml");
}

#[test]
fn cloudflare_redos() {
    run_file("cloudflare_redos.toml");
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
// lookahead patterns may produce excess nulls (bounded, not quadratic).

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
    // 2 matches, ideal=2, BUG: 3 nulls [4, 1, 1] (dup at 1)
    let nulls = rev_nulls(r"a(?=b)", b"_ab_ab_");
    eprintln!("a(?=b): {} nulls {:?}", nulls.len(), nulls);
    assert!(nulls.len() <= 2);
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

#[test]
fn literal_20_bytes() {
    let pattern = "ABCDEFGHIJKLMNOPQRST";
    let mut hay = vec![b'.'; 200];
    hay[100..120].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re.find_all(&hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(100, 120)]);
}

#[test]
fn literal_16_bytes() {
    let pattern = "ABCDEFGHIJKLMNOP";
    let mut hay = vec![b'.'; 100];
    hay[50..66].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re.find_all(&hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(50, 66)]);
}

#[test]
fn literal_17_bytes() {
    let pattern = "ABCDEFGHIJKLMNOPQ";
    let mut hay = vec![b'.'; 100];
    hay[40..57].copy_from_slice(pattern.as_bytes());
    let re = Regex::new(pattern).unwrap();
    let r: Vec<_> = re.find_all(&hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(40, 57)]);
}

// -- case insensitivity cross-validated against regex crate --

#[test]
fn ci_literal_vs_regex() {
    check_vs_regex("(?i)abc", b"xAbCx");
}

#[test]
fn ci_literal_no_match() {
    check_vs_regex("(?i)abc", b"xyz");
}

#[test]
fn ci_alternation_vs_regex() {
    check_vs_regex("(?i)(foo|bar)", b"FOO and Bar and bAr");
}

#[test]
fn ci_class_range_vs_regex() {
    check_vs_regex("(?i)[a-f]+", b"xABCDEFx");
}

#[test]
fn ci_quantifier_vs_regex() {
    check_vs_regex("(?i)a+", b"aAaA");
}

#[test]
fn ci_bounded_repeat_vs_regex() {
    check_vs_regex("(?i)ab{2,4}", b"aBBBx");
}

#[test]
fn ci_dotstar_vs_regex() {
    check_vs_regex("(?i)a.*z", b"AbcZ");
}

#[test]
fn ci_mixed_case_literal() {
    check_vs_regex("(?i)HeLLo", b"hello HELLO HeLLo hElLo");
}

#[test]
fn ci_word_boundary() {
    check_vs_regex(r"(?i)\bhello\b", b"Hello HELLO hello");
}

#[test]
fn ci_digits_unaffected() {
    check_vs_regex("(?i)test123", b"TEST123 test123 TeSt123");
}

#[test]
fn ci_char_class_explicit() {
    check_vs_regex("(?i)[xyz]+", b"XyZxYz");
}

#[test]
fn ci_negated_class() {
    check_vs_regex("(?i)[^a-c]+", b"xABCxDEFx");
}

#[test]
fn ci_anchored_start() {
    check_vs_regex("(?i)^hello", b"Hello world");
}

#[test]
fn ci_anchored_end() {
    check_vs_regex("(?i)world$", b"hello WORLD");
}

#[test]
fn ci_anchored_full() {
    check_vs_regex("(?i)^hello world$", b"HELLO WORLD");
}

#[test]
fn ci_anchored_no_match() {
    check_vs_regex("(?i)^hello$", b"xhellox");
}

#[test]
fn ci_optional() {
    check_vs_regex("(?i)colou?r", b"Color COLOUR color colour");
}

#[test]
fn ci_plus_quantifier() {
    check_vs_regex("(?i)z+", b"zZzZZz");
}

#[test]
fn ci_star_quantifier() {
    check_vs_regex("(?i)ab*c", b"AC ABC ABBC ac abc");
}

#[test]
fn ci_escape_sequence() {
    check_vs_regex(r"(?i)\d+[a-f]+", b"123ABC 456def 789aF");
}

#[test]
fn ci_lookahead() {
    let re = Regex::new("(?i)foo(?=bar)").unwrap();
    let r: Vec<_> = re.find_all(b"FOOBAR foobar FoObAr foobaz").unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(0, 3), (7, 10), (14, 17)]);
}

#[test]
fn ci_lookbehind() {
    let re = Regex::new("(?i)(?<=foo)bar").unwrap();
    let r: Vec<_> = re.find_all(b"FOOBAR foobar FoObAr bazbar").unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(3, 6), (10, 13), (17, 20)]);
}

#[test]
fn ci_empty_input() {
    check_vs_regex("(?i)abc", b"");
}

#[test]
fn ci_single_char() {
    check_vs_regex("(?i)a", b"AaAa");
}

#[test]
fn ci_unicode_ascii() {
    check_vs_regex("(?i)caf", b"CAF caf Caf");
}

#[test]
fn ci_pipe_in_group() {
    check_vs_regex("(?i)(cat|dog|bird)", b"CAT Dog BIRD cat");
}

#[test]
fn ci_nested_groups() {
    check_vs_regex("(?i)(a(bc)d)", b"ABCD abcd AbCd");
}

#[test]
fn ci_exact_repeat() {
    check_vs_regex("(?i)a{3}", b"aAa AAA aaa");
}

#[test]
fn ci_range_repeat() {
    check_vs_regex("(?i)x{2,4}", b"xX XXx xXxX");
}

#[test]
fn ci_scoped_vs_regex() {
    check_vs_regex("(?i:abc)def", b"ABCdef abcDEF ABCDef abcdef");
}

#[test]
fn ci_scoped_no_leak() {
    check_vs_regex("(?i:abc)def", b"ABCDEF");
}

#[test]
fn ci_scoped_alternation() {
    check_vs_regex("(?i:foo|bar)baz", b"FOObaz BARbaz foobaz FOOBAZ");
}

#[test]
fn ci_scoped_class() {
    check_vs_regex("(?i:[a-f])+g", b"ABCDEFg abcdefg ABCDEfG");
}

#[test]
fn ci_scoped_nested() {
    check_vs_regex("(?i:a(?-i:b)c)", b"AbC ABC abc aBc");
}

// -- word boundary tests --

#[test]
fn wb_bare_11() {
    check_vs_regex(r"\b11\b", b"11");
}

#[test]
fn wb_leading_space() {
    check_vs_regex(r"\b11\b", b" 11");
}

#[test]
fn wb_trailing_space() {
    check_vs_regex(r"\b11\b", b"11 ");
}

#[test]
fn wb_both_spaces() {
    check_vs_regex(r"\b11\b", b" 11 ");
}

#[test]
fn wb_long_word_12plus() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"hello extraordinary world");
}

#[test]
fn wb_long_word_no_match() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"hello world foo");
}

#[test]
fn wb_long_word_multiple() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"understanding communication");
}

#[test]
fn wb_long_word_mixed_case() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"THE understanding OF communication HERE");
}

#[test]
fn wb_long_word_at_start() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"extraordinary!");
}

#[test]
fn wb_long_word_at_end() {
    check_vs_regex(r"\b[a-z]{12,}\b", b"!extraordinary");
}

#[test]
fn wb_exact_13() {
    check_vs_regex(r"\b[a-z]{13}\b", b"hello world extraordinary");
}

#[test]
fn wb_exact_12_no_match() {
    check_vs_regex(r"\b[a-z]{12}\b", b"hello world extraordinary");
}

#[test]
fn wb_word_plus() {
    check_vs_regex(r"\b\w+\b", b"hello world");
}

#[test]
fn wb_digits() {
    check_vs_regex(r"\b\d+\b", b"foo 123 bar");
}

#[test]
fn wb_lowercase_words() {
    check_vs_regex(r"\b[a-z]+\b", b"hello WORLD foo");
}

#[test]
fn wb_partial_leading() {
    check_vs_regex(r"\b11", b" 11");
}

#[test]
fn wb_partial_trailing() {
    check_vs_regex(r"11\b", b"11 ");
}

#[test]
fn wb_partial_trailing_bare() {
    check_vs_regex(r"11\b", b"11");
}

#[test]
fn wb_contains_a() {
    check_vs_regex(r"\b\w*a\w*\b", b"ffaff");
}

#[test]
fn wb_trailing_a() {
    check_vs_regex(r"a\b", b"a ");
}

#[test]
fn wb_dash_boundary() {
    check_vs_regex(r"\b-", b"1-2");
}

#[test]
fn wb_before_dash() {
    check_vs_regex(r"1\b-", b"1-2");
}

#[test]
fn wb_across_dash() {
    check_vs_regex(r"1\b-2", b"1-2");
}

#[test]
fn wb_no_match_embedded() {
    check_vs_regex(r"\b11\b", b"a11b");
}

#[test]
fn wb_adjacent_words() {
    check_vs_regex(r"\b[a-z]+\b", b"cat dog bird");
}

#[test]
fn wb_bounded_rep_at_boundary() {
    check_vs_regex(r"\b[a-z]{3,5}\b", b"cat extraordinary dog bird");
}

#[test]
fn wb_whitespace_neighbor() {
    check_vs_regex(r"\s\b[a-z]+\b\s", b" cat ");
}

#[test]
fn wb_after_whitespace_class() {
    check_vs_regex(r"[ \t]\b\w+", b" hello\tworld");
}

#[test]
fn wb_alpha_class_union() {
    check_vs_regex(r"\b[a-zA-Z]+\b", b"Hello WORLD foo 123");
}

#[test]
fn wb_alnum_class() {
    check_vs_regex(r"\b[a-zA-Z0-9]+\b", b"foo123 !bar! 42");
}


#[test]
fn dotstar_inner_literal_correctness() {
    check_vs_regex(".*=.*", b"key=value");
    check_vs_regex(".*=.*", b"no equals here");
    check_vs_regex(".*=.*", b"a=b c=d e=f");
    check_vs_regex(".*=.*", b"first line\nsecond=line\nthird");
    check_vs_regex(".*=.*", b"===");
    check_vs_regex(".*=.*", b"x=y\na=b\n");
}

#[test]
fn dotstar_inner_literal_accel() {
    let re = Regex::new(".*=.*").unwrap();
    let (fwd, rev) = re.has_accel();
    // BFS strips leading .* and extracts '=' as forward prefix
    // rev skip on '=' also active via reverse DFA
    assert!(fwd, ".*=.* should have forward prefix on '='");
    assert!(rev, ".*=.* should have reverse skip on '='");
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
    let m = re.find_all(b"The Adventures of Huckleberry Finn', published in 1885.").unwrap();
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
        assert_eq!(r, expected, r"input={:?}", std::str::from_utf8(input).unwrap());
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
    eprintln!("has_accel: {:?}", re.has_accel());
    let m = re.find_all(b"xycdzz abcde fg").unwrap();
    let r: Vec<_> = m.iter().map(|m| (m.start, m.end)).collect();
    eprintln!("bounded result: {:?}", r);
}

use resharp::{BDFA, NodeId, RegexBuilder};


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
            let end = pos;
            let start = end - rel as usize;
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
    // after 'c' at pos=3, should have a match with rel=3
    assert!(trace.iter().any(|&(_, _, _, rel)| rel == 3), "expected match with rel=3");
}

#[test]
fn bdfa_literal_abc_states() {
    let pp = bdfa_state_pp("abc", b"xabcx");
    for line in &pp {
        eprintln!("{}", line);
    }
}

#[test]
fn bdfa_alternation_ab_cd() {
    assert_bdfa_eq("ab|cd", b"xabcdx");
}

#[test]
fn bdfa_bounded_repeat() {
    // a{2,4}: variable length, bounded
    let pp = bdfa_state_pp("a{2,4}", b"xaaaaax");
    eprintln!("a{{2,4}} on 'xaaaaax':");
    for line in &pp {
        eprintln!("{}", line);
    }
    let trace = bdfa_step_trace("a{2,4}", b"xaaaaax");
    let match_positions: Vec<_> = trace.iter().filter(|t| t.3 > 0).map(|t| (t.0, t.3)).collect();
    eprintln!("matches: {:?}", match_positions);
}

#[test]
fn bdfa_two_candidates() {
    assert_bdfa_eq("aa", b"aaa");
}

// --- derivative traces for (curr, last_nullable) pairs ---

#[test]
fn bdfa_der_a_or_aa() {
    let pp = bdfa_state_pp("a|aa", b"aab");
    eprintln!("a|aa on 'aab':");
    for line in &pp { eprintln!("{}", line); }
}

#[test]
fn bdfa_der_a_1_4() {
    let pp = bdfa_state_pp("a{1,4}", b"aaaax");
    eprintln!("a{{1,4}} on 'aaaax':");
    for line in &pp { eprintln!("{}", line); }
}

#[test]
fn bdfa_der_ab_1_3() {
    let pp = bdfa_state_pp("(ab){1,3}", b"abababx");
    eprintln!("(ab){{1,3}} on 'abababx':");
    for line in &pp { eprintln!("{}", line); }
}

#[test]
fn bdfa_der_abc_bcd() {
    let pp = bdfa_state_pp("abc|bcd", b"abcde");
    eprintln!("abc|bcd on 'abcde':");
    for line in &pp { eprintln!("{}", line); }
}

#[test]
fn bdfa_der_nested_alt() {
    let pp = bdfa_state_pp("(a|ab)(b|c)", b"abcx");
    eprintln!("(a|ab)(b|c) on 'abcx':");
    for line in &pp { eprintln!("{}", line); }
}

// --- ambiguous cases: bdfa must match std engine (leftmost longest) ---

fn assert_bdfa_eq(pattern: &str, input: &[u8]) {
    let m = bdfa_matches(pattern, input);
    let re = Regex::new(pattern).unwrap();
    let std_m: Vec<_> = re.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(m, std_m, "pattern={:?} input={:?}", pattern, String::from_utf8_lossy(input));
}

#[test]
fn bdfa_ambiguous_a_or_aa() {
    // std: (0,2); bdfa currently gives (0,1),(1,2)
    assert_bdfa_eq("a|aa", b"aab");
}

#[test]
fn bdfa_ambiguous_ab_or_a() {
    // std: (0,2),(2,4); bdfa currently gives (0,1),(2,3)
    assert_bdfa_eq("ab|a", b"abab");
}

#[test]
fn bdfa_ambiguous_repeat_ab_1_3() {
    // std: (0,6); bdfa currently gives (0,2),(2,4),(4,6)
    assert_bdfa_eq("(ab){1,3}", b"abababx");
}

#[test]
fn bdfa_ambiguous_overlap_abc_bcd() {
    // std: (0,3); bdfa agrees
    assert_bdfa_eq("abc|bcd", b"abcde");
}

#[test]
fn bdfa_ambiguous_a_1_4_greedy() {
    // std: (0,4); bdfa currently gives (0,1),(1,2),(2,3),(3,4)
    assert_bdfa_eq("a{1,4}", b"aaaa");
}

#[test]
fn bdfa_ambiguous_nested_alt() {
    // std: (0,3); bdfa currently gives (0,2)
    assert_bdfa_eq("(a|ab)(b|c)", b"abcx");
}

#[test]
fn bdfa_ambiguous_triple_overlap() {
    // std: (0,4),(4,6); bdfa currently gives (0,2),(2,4),(4,6)
    assert_bdfa_eq("a{2,4}", b"aaaaaa");
}

// -- multi-match overlap tests

#[test]
fn bdfa_multi_match_overlap() {
    assert_bdfa_eq("a{2,4}", b"aaaaaaaaa");
    assert_bdfa_eq("ab|a", b"ababababab");
    assert_bdfa_eq("(ab){1,3}", b"ababababababab");
    assert_bdfa_eq("abc|bcd", b"xabcbcdabcdy");
    assert_bdfa_eq("[a-c]{2,3}", b"abcabcabc");
}

#[test]
fn bdfa_multi_match_traces() {
    let pp = bdfa_state_pp("a{2,4}", b"aaaaaaaaa");
    eprintln!("a{{2,4}} on 'aaaaaaaaa' (multi-match):");
    for line in &pp { eprintln!("  {}", line); }
    let m = bdfa_matches("a{2,4}", b"aaaaaaaaa");
    eprintln!("  matches: {:?}", m);
}

// -- prefix acceleration tests

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
        "pos=4 'o' s=6 rel=4 [#(\u{22a5})s5b4]",
        "pos=5 ' ' s=7 rel=4 [#(\u{22a5})s6b4]",
        "pos=6 'W' s=8 rel=4 [#(\u{22a5})s7b4, #([a-z]{1,3})s1b0]",
        "pos=7 'o' s=9 rel=4 [#(\u{22a5})s8b4, #((|(|[a-z])[a-z]))s2b2]",
        "pos=8 'r' s=10 rel=4 [#(\u{22a5})s9b4, #((|[a-z]))s3b3]",
        "pos=9 'l' s=11 rel=4 [#(\u{22a5})s10b4, #()s4b4]",
        "pos=10 'd' s=12 rel=4 [#(\u{22a5})s11b4, #(\u{22a5})s5b4]",
        "pos=11 ' ' s=13 rel=4 [#(\u{22a5})s12b4, #(\u{22a5})s6b4]",
        "pos=12 'F' s=14 rel=4 [#(\u{22a5})s13b4, #(\u{22a5})s7b4, #([a-z]{1,3})s1b0]",
        "pos=13 'o' s=15 rel=4 [#(\u{22a5})s14b4, #(\u{22a5})s8b4, #((|(|[a-z])[a-z]))s2b2]",
        "pos=14 'o' s=16 rel=4 [#(\u{22a5})s15b4, #(\u{22a5})s9b4, #((|[a-z]))s3b3]",
        "pos=15 ' ' s=17 rel=4 [#(\u{22a5})s16b4, #(\u{22a5})s10b4, #(\u{22a5})s4b3]",
        "pos=16 'B' s=18 rel=4 [#(\u{22a5})s17b4, #(\u{22a5})s11b4, #(\u{22a5})s5b3, #([a-z]{1,3})s1b0]",
        "pos=17 ' ' s=19 rel=4 [#(\u{22a5})s18b4, #(\u{22a5})s12b4, #(\u{22a5})s6b3]",
        "pos=18 'x' s=20 rel=4 [#(\u{22a5})s19b4, #(\u{22a5})s13b4, #(\u{22a5})s7b3]",
        "pos=19 'y' s=21 rel=4 [#(\u{22a5})s20b4, #(\u{22a5})s14b4, #(\u{22a5})s8b3]",
    ]);
}


#[test]
fn bdfa_prefix_has_prefix() {
    // verify the BDFA actually built a prefix for a literal pattern
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, "Twain.{0,5}").unwrap();
    let bdfa = BDFA::new(&mut b, node).unwrap();
    assert!(bdfa.prefix.is_some(), "expected prefix for Twain.{{0,5}}");
    assert!(bdfa.prefix_len >= 5, "expected prefix_len >= 5, got {}", bdfa.prefix_len);
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
        EngineOptions::default().unicode(false),
    ).unwrap();
    // ASCII-only: "café" → "caf" matches, é (0xC3 0xA9) does not
    let m = re.find_all("café".as_bytes()).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].start, m[0].end), (0, 3));

    // contrast: with unicode=true (default), the whole word matches
    let re_u = Regex::new(r"\w+").unwrap();
    let m_u = re_u.find_all("café".as_bytes()).unwrap();
    assert_eq!(m_u.len(), 1);
    assert!(m_u[0].end > 3); // includes the é bytes
}

#[test]
fn opts_case_insensitive() {
    let re = Regex::with_options(
        "hello",
        EngineOptions::default().case_insensitive(true),
    ).unwrap();
    let m = re.find_all(b"Hello HELLO hello").unwrap();
    assert_eq!(m.len(), 3);
}

#[test]
fn opts_dot_matches_new_line() {
    let re = Regex::with_options(
        "a.b",
        EngineOptions::default().dot_matches_new_line(true),
    ).unwrap();
    let m = re.find_all(b"a\nb").unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].start, m[0].end), (0, 3));

    // without the flag, should not match
    let re2 = Regex::new("a.b").unwrap();
    let m2 = re2.find_all(b"a\nb").unwrap();
    assert_eq!(m2.len(), 0);
}

#[test]
fn opts_dot_all_inline_flag() {
    // (?s) inline should also work
    let re = Regex::new("(?s)a.b").unwrap();
    let m = re.find_all(b"a\nb").unwrap();
    assert_eq!(m.len(), 1);
}

#[test]
fn opts_dot_all_scoped_group() {
    // (?s:.) scoped: dot inside matches newline, dot outside does not
    let re = Regex::new("(?s:a.b).c").unwrap();
    let m = re.find_all(b"a\nbxc").unwrap();
    assert_eq!(m.len(), 1);

    // dot outside group should NOT match newline
    let m2 = re.find_all(b"a\nb\nc").unwrap();
    assert_eq!(m2.len(), 0);
}

#[test]
fn opts_ignore_whitespace() {
    let re = Regex::with_options(
        r"hello \ world",
        EngineOptions::default().ignore_whitespace(true),
    ).unwrap();
    let m = re.find_all(b"hello world").unwrap();
    assert_eq!(m.len(), 1);
}

#[test]
fn word_match_lengths_en_sampled() {
    let path = format!("{}/../data/haystacks/en-sampled.txt", env!("CARGO_MANIFEST_DIR"));
    let content = std::fs::read_to_string(&path).unwrap();
    let input: String = content.lines().take(2500).collect::<Vec<_>>().join("\n");
    let input = input.as_bytes();

    let pattern = r"\b[0-9A-Za-z_]+\b";
    let re = Regex::with_options(pattern, EngineOptions::default().unicode(false)).unwrap();
    let matches = re.find_all(input).unwrap();

    let rx = regex::bytes::RegexBuilder::new(pattern).unicode(false).build().unwrap();
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
        matches.len(), expected.len(),
        "match count mismatch: resharp={} regex={}",
        matches.len(), expected.len(),
    );
}

fn run_file_untrusted(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore || tc.expect_error || tc.anchored {
            continue;
        }
        let opts = EngineOptions::default().untrusted(true);
        let re = match Regex::with_options(&tc.pattern, opts) {
            Ok(re) => re,
            Err(_) => continue,
        };
        let matches = re.find_all(tc.input.as_bytes()).unwrap();
        let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(
            result, tc.matches,
            "UNTRUSTED file={}, name={:?}, pattern={:?}, input={:?}",
            filename, tc.name, tc.pattern, tc.input
        );
    }
}

#[test]
fn untrusted_basic() {
    run_file_untrusted("basic.toml");
}

#[test]
fn untrusted_anchors() {
    run_file_untrusted("anchors.toml");
}

#[test]
fn untrusted_semantics() {
    run_file_untrusted("semantics.toml");
}

#[test]
fn untrusted_date_pattern() {
    run_file_untrusted("date_pattern.toml");
}

#[test]
fn untrusted_edge_cases() {
    run_file_untrusted("edge_cases.toml");
}

#[test]
fn untrusted_rejects_lookaround() {
    let s = || EngineOptions::default().untrusted(true);
    // lookaround patterns that survive algebra simplification are rejected
    assert!(Regex::with_options(r".*(?=aaa)", s()).is_err());
    assert!(Regex::with_options(r"(?<=__).*", s()).is_err());
    assert!(Regex::with_options(r"foo(?!bar).*", s()).is_err());
    assert!(Regex::with_options(r".*(?<!bar)foo", s()).is_err());
    // trivially simplified lookarounds (e.g. (?=foo)bar → BOT) are fine
    assert!(Regex::with_options(r"(?=foo)bar", s()).is_ok());
    // non-lookaround patterns compile fine
    assert!(Regex::with_options(r"foo|bar", s()).is_ok());
}

#[test]
fn untrusted_pathological() {
    let pattern = r".*[^A-Z]|[A-Z]";
    let input = "A".repeat(1000);
    let re_normal = Regex::new(pattern).unwrap();
    let re_untrusted = Regex::with_options(
        pattern,
        EngineOptions::default().untrusted(true),
    ).unwrap();
    assert_eq!(
        re_normal.find_all(input.as_bytes()).unwrap(),
        re_untrusted.find_all(input.as_bytes()).unwrap(),
        "pathological pattern mismatch"
    );
}

fn check_untrusted_vs_normal(pattern: &str, input: &[u8]) {
    let opts = EngineOptions::default().untrusted(true);
    let re_s = match Regex::with_options(pattern, opts) {
        Ok(re) => re,
        Err(_) => return, // skip patterns that fail in untrusted mode (e.g. lookaround)
    };
    let re_n = Regex::new(pattern).unwrap();
    let normal = re_n.find_all(input).unwrap();
    let untrusted = re_s.find_all(input).unwrap();
    assert_eq!(
        normal, untrusted,
        "untrusted vs normal mismatch: pattern={:?}, input={:?}",
        pattern,
        std::str::from_utf8(input).unwrap_or("<binary>")
    );
}

#[test]
fn untrusted_cross_validate() {
    let en = std::fs::read_to_string(
        format!("{}/../data/haystacks/en-sampled.txt", env!("CARGO_MANIFEST_DIR"))
    ).unwrap();
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
        check_untrusted_vs_normal(p, input);
    }
    // pathological: dense candidates with dotstar
    let aaaa = "A".repeat(500);
    check_untrusted_vs_normal(r".*[^A-Z]|[A-Z]", aaaa.as_bytes());
    check_untrusted_vs_normal(r"[A-Z]+", aaaa.as_bytes());
    check_untrusted_vs_normal(r"A{1,3}", aaaa.as_bytes());
}

#[test]
fn untrusted_bounded_repeat_tail() {
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

        let re_u = Regex::with_options(pattern, EngineOptions::default().untrusted(true)).unwrap();
        let got: Vec<(usize, usize)> = re_u
            .find_all(input.as_bytes())
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();

        assert_eq!(
            expected, got,
            "BDFA bounded repeat mismatch: pattern={:?}, len={}",
            pattern,
            input.len()
        );
    }
}

#[test]
fn range_prefix_correctness() {
    let en = std::fs::read_to_string(
        format!("{}/../data/haystacks/en-sampled.txt", env!("CARGO_MANIFEST_DIR"))
    ).unwrap();
    let inputs: Vec<&[u8]> = vec![
        en.as_bytes(),
        b"hello world no caps here 123",
        b"ABCDEFGhijklmnop",
        b"aZbYcXdW",
        b"",
        b"Z",
        b"ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ", // > 32 bytes of matches
        &[0u8; 100],                                // no ASCII letters
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
        let re_untrusted = Regex::with_options(p, EngineOptions::default().untrusted(true)).unwrap();
        for input in &inputs {
            let normal = re.find_all(input).unwrap();
            let untrusted = re_untrusted.find_all(input).unwrap();
            assert_eq!(
                normal, untrusted,
                "range prefix mismatch: pattern={:?}, input={:?}",
                p, std::str::from_utf8(input).unwrap_or("<binary>")
            );
        }
    }
}

#[test]
fn range_prefix_random_haystack() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let patterns = [
        r"[A-Z][a-z]+",
        r"[A-Z]{2,5}",
        r"[A-Za-z]{3,}",
    ];
    for seed in 0u64..50 {
        let mut h = DefaultHasher::new();
        seed.hash(&mut h);
        let hash = h.finish();
        // generate pseudorandom haystack mixing ASCII ranges
        let input: Vec<u8> = (0..256).map(|i| {
            let v = ((hash.wrapping_mul(i as u64 + 1).wrapping_add(seed)) >> 8) as u8;
            // bias toward printable ASCII
            32 + (v % 95)
        }).collect();
        for p in &patterns {
            let re = Regex::new(p).unwrap();
            let re_s = Regex::with_options(p, EngineOptions::default().untrusted(true)).unwrap();
            let normal = re.find_all(&input).unwrap();
            let untrusted = re_s.find_all(&input).unwrap();
            assert_eq!(
                normal, untrusted,
                "random haystack mismatch: seed={}, pattern={:?}",
                seed, p
            );
        }
    }
}

