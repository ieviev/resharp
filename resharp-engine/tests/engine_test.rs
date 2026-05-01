use resharp::{Error, Regex, RegexOptions};
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
fn normal_basic() {
    run_file("basic.toml");
}

#[test]
fn normal_anchors() {
    run_file("anchors.toml");
}

#[test]
fn normal_boolean() {
    run_file("boolean.toml");
}

#[test]
fn normal_lookaround() {
    run_file("lookaround.toml");
}

#[test]
#[ignore = "slow in debug; run with --ignored or in release"]
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

fn run_file_javascript(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore {
            continue;
        }
        let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
        let re = Regex::with_options(&tc.pattern, opts).unwrap_or_else(|e| {
            panic!(
                "file={}, name={:?}, pattern={:?}: compile error: {}",
                filename, tc.name, tc.pattern, e
            )
        });
        let matches = re.find_all(tc.input.as_bytes()).unwrap();
        let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(
            result, tc.matches,
            "JS file={}, name={:?}, pattern={:?}, input={:?}",
            filename, tc.name, tc.pattern, tc.input
        );
    }
}

#[test]
fn javascript() {
    run_file_javascript("javascript.toml");
}

/// cross-validate resharp against regex crate
fn check_vs_regex(pattern: &str, input: &[u8]) {
    let re = Regex::new(pattern).expect(&format!("failed compile {}", pattern));
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

#[test]
fn intersect_narrow_with_widened_term_is_sound() {
    for pat in ["foo&_*bar_*", "foo&.*bar.*"] {
        let re = Regex::with_options(pat, RegexOptions::default()).unwrap();
        for input in ["foo", "foo baz", "foo bar", "barfoo", "foobar"] {
            let ms = re.find_all(input.as_bytes()).unwrap();
            assert!(
                ms.is_empty(),
                "pat={pat:?} input={input:?} unexpectedly matched: {ms:?}"
            );
        }
    }
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
        RegexOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        RegexOptions {
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
        RegexOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        RegexOptions {
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
fn anchored_alt_star_rejected() {
    use resharp::{RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Default, UnicodeMode::Javascript] {
        let opts = RegexOptions::default().unicode(mode);
        let err = Regex::with_options("(^\\*|REMARK)*", opts).err();
        assert!(err.is_some(), "mode={:?} expected rejection, got ok", mode);
    }
}

#[test]
fn space_newline_space() {
    use resharp::{RegexOptions, UnicodeMode};
    let mk = || RegexOptions::default().unicode(UnicodeMode::Javascript);
    let line = "abcdefghij abcdefghij abcdefghij abcdefg ";
    let mut hay = String::new();
    while hay.len() < 1_000_000 {
        hay.push_str(line);
        hay.push('\n');
    }
    let bytes = hay.as_bytes();
    for pat in [" *\\n *", " *\\n", "\\n *", "\\n", " +\\n +"] {
        let re = Regex::with_options(pat, mk()).unwrap();
        let _ = re.find_all(bytes).unwrap();
        let t = std::time::Instant::now();
        let m = re.find_all(bytes).unwrap();
        let dt = t.elapsed();
        let mbps = (bytes.len() as f64 / 1e6) / dt.as_secs_f64();
        eprintln!(
            "pat={:?} matches={} dt={:?} MB/s={:.2}",
            pat,
            m.len(),
            dt,
            mbps
        );
    }
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
    let re_dfa = Regex::from_node(b, node, RegexOptions::default()).unwrap();
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
        RegexOptions {
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
#[ignore = "slow; run with --ignored"]
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
fn normal_word_boundary() {
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
        RegexOptions {
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

#[test]
fn opts_unicode_false() {
    let re = Regex::with_options(
        r"\w+",
        RegexOptions::default().unicode(resharp::UnicodeMode::Ascii),
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
    let re = Regex::with_options("hello", RegexOptions::default().case_insensitive(true)).unwrap();
    let m = re.find_all(b"Hello HELLO hello").unwrap();
    assert_eq!(m.len(), 3);
}

#[test]
fn opts_dot_matches_new_line() {
    let re =
        Regex::with_options("a.b", RegexOptions::default().dot_matches_new_line(true)).unwrap();
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
        RegexOptions::default().ignore_whitespace(true),
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
        RegexOptions::default().unicode(resharp::UnicodeMode::Ascii),
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
        let opts = RegexOptions::default().hardened(true);
        let re = match Regex::with_options(&tc.pattern, opts) {
            Ok(re) => re,
            Err(_) => continue,
        };
        let matches = re.find_all(tc.input.as_bytes()).unwrap_or_else(|e| {
            panic!(
                "err on file={} name={:?} pat={:?} inp={:?}: {:?}",
                filename, tc.name, tc.pattern, tc.input, e
            )
        });
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
#[ignore = "slow in debug; run with --ignored or in release"]
fn hardened_semantics() {
    run_file_hardened("semantics.toml");
}

#[test]
#[ignore = "slow; run with --ignored"]
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
#[ignore = "slow; run with --ignored"]
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
#[ignore = "slow; run with --ignored"]
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
    let re_hardened = Regex::with_options(pattern, RegexOptions::default().hardened(true)).unwrap();
    assert_eq!(
        re_normal.find_all(input.as_bytes()).unwrap(),
        re_hardened.find_all(input.as_bytes()).unwrap(),
        "pathological pattern mismatch"
    );
}

fn check_hardened_vs_normal(pattern: &str, input: &[u8]) {
    let opts = RegexOptions::default().hardened(true);
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

        let re_u = Regex::with_options(pattern, RegexOptions::default().hardened(true)).unwrap();
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
        b"ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        &[0u8; 100],                              
    ];
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
        let re_hardened = Regex::with_options(p, RegexOptions::default().hardened(true)).unwrap();
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
            let re_s = Regex::with_options(p, RegexOptions::default().hardened(true)).unwrap();
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

        let opts = RegexOptions::default().hardened(true);
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
            let opts = RegexOptions::default().hardened(true);
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
    ts_rev: Option<String>,
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
            ts_rev: t
                .get("ts_rev")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
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
        let node = b.simplify_fwd_initial(node);
        let got = b.pp(node);
        assert_eq!(
            got, tc.pp,
            "file={}, name={:?}, pattern={:?}",
            filename, tc.name, tc.pattern
        );
        if let Some(expected_ts_rev) = &tc.ts_rev {
            let ts_rev_start = b.ts_rev_start(node).unwrap();
            let got_ts_rev = b.pp(ts_rev_start);
            assert_eq!(
                got_ts_rev, *expected_ts_rev,
                "ts_rev mismatch: file={}, name={:?}, pattern={:?}",
                filename, tc.name, tc.pattern
            );
        }
    }
}

#[test]
fn internal() {
    run_file_internal("internal.toml");
}

#[test]
fn normalize_toml() {
    run_file_internal("normalize.toml");
}

#[test]
#[ignore = "unsupported"]
fn word_boundary_inference() {
    let re = Regex::new(r"<.*(?<=<)bg").unwrap();
    let input = b"<bg";
    let ms = re.find_all(input).unwrap();
    let actual: Vec<[usize; 2]> = ms.iter().map(|m| [m.start, m.end]).collect();
    assert_eq!(actual, &[[0, 3]]);
}

#[test]
fn alt_embedded_line_anchor_compiles_ok() {
    assert!(Regex::new(r"^a|^b").is_ok());
    assert!(Regex::new(r"^(ab)").is_ok());
}

#[test]
fn word_boundaries_loop() {
    let re = resharp::Regex::new(r"\(\?[:=!]|\)|\{\d+\b,?\d*\}|[+*]\?|[()$^+*?.]").unwrap();
    let _ = re.find_all(b"$").unwrap();
}

#[test]
fn fwd_la_1() {
    let pattern = r"(?:\[[^\]]*\]|[^\]]|\](?=[^\[]*\]))*";
    let ops = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
    match Regex::with_options(pattern, ops) {
        Err(resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)) => {}
        Err(e) => panic!("unexpected error: {:?}", e),
        Ok(_) => panic!("expected UnsupportedPattern"),
    }
}

#[test]
fn fwd_la_2() {
    let pattern = r"^((?=.*[0-9])(?=.*[a-z])(?=.*[A-Z])(?=.*[@#$%]).{6})";
    let hay = include_bytes!("../../data/haystacks/smallserver.txt");
    let ops = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
    let re = Regex::with_options(pattern, ops).unwrap();
    let _ = re.find_all(hay).unwrap();
}

#[test]
fn fwd_la_2_js() {
    let pattern = r"^(?=.{8,})(?=.*[A-Z])(?=.*[a-z])(?=.*[0-9])(?=.*[A-Za-z0-9]).*$";
    let hay = include_bytes!("../../data/haystacks/smallserver.txt");
    let ops = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
    let re = Regex::with_options(pattern, ops).unwrap();
    let _ = re.find_all(&hay[..50]).unwrap();
}

#[test]
fn fwd_la_3() {
    let pattern = "<(?:\\/?(?!(?:div|p|br|span)>)\\w+|(?:(?!(?:span style=\"white-space:\\s?pre;?\">)|br\\s?\\/>))\\w+\\s[^>]+)>";
    let hay = include_bytes!("../../data/haystacks/smallserver.txt");
    let ops = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
    let re = Regex::with_options(pattern, ops).unwrap();
    let _ = re.find_all(&hay[..2]).unwrap();
}

#[test]
fn repro_lookahead_in_loop() {
    let pattern = r"(.(?=.))+x";
    let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
    let result = Regex::with_options(pattern, opts);
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("pattern {:?} must be rejected", pattern),
    };
    assert!(
        matches!(
            err,
            resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)
        ),
        "expected UnsupportedPattern, got {:?}",
        err
    );
}
#[test]
fn hardened_long_word() {
    let p = r"\b[a-z]{12,}\b";
    let input = b"!extraordinary";
    let re_h = Regex::with_options(p, RegexOptions::default().hardened(true)).unwrap();
    let re_n = Regex::new(p).unwrap();
    let a = re_n.find_all(input).unwrap();
    let b = re_h.find_all(input).unwrap();
    assert_eq!(a, b);
}

#[test]
fn no_progress() {
    let re = Regex::new(r"ab|bcd*").unwrap();
    let hay = "abcdddxabxbcdddyabbcd".repeat(20);
    let ms = re.find_all(hay.as_bytes()).unwrap();
    assert!(!ms.is_empty());
}

#[test]
fn repro_is_match_negative_lookahead() {
    let re = Regex::new(r"foo(?!bar)").unwrap();
    assert!(!re.is_match(b"foobar").unwrap());
}

#[test]
fn light_depth_pass_bdfa_prefix_falls_through_to_potential() {
    let p = r"\s\!?LIGHT_DEPTH_PASS\s";
    for mode in [
        resharp::UnicodeMode::Ascii,
        resharp::UnicodeMode::Javascript,
        resharp::UnicodeMode::Full,
    ] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let hay = " LIGHT_DEPTH_PASS ".repeat(100);
        let ms = re.find_all(hay.as_bytes()).unwrap();
        assert_eq!(ms.len(), 100, "mode {:?}", mode);
    }
}

#[test]
fn assets_path_js_unicode_uses_rev_literal() {
    let p = r"..\/..\/Assets\/";
    for mode in [
        resharp::UnicodeMode::Ascii,
        resharp::UnicodeMode::Javascript,
        resharp::UnicodeMode::Full,
    ] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let hay = "xx/yy/Assets/file.cs\n".repeat(100);
        let ms = re.find_all(hay.as_bytes()).unwrap();
        assert_eq!(ms.len(), 100, "mode {:?}", mode);
    }
}

#[test]
fn lookahead_alternation_with_end_of_line() {
    let re = Regex::new(r"x(?=a|$)").unwrap();
    let input = b"xa xb x\nxc x";
    let positions: Vec<usize> = re
        .find_all(input)
        .unwrap()
        .iter()
        .map(|m| m.start)
        .collect();
    assert_eq!(positions, vec![0, 6, 11]);
}

#[test]
fn fwd_begin_anchor_short_circuits() {
    use std::time::Instant;
    let big = vec![b'x'; 1 << 22];
    for &(p, expect_match) in &[(r"\Afoo", false), (r"\Axxx", true), (r"\A", true)] {
        let re = Regex::new(p).unwrap();
        let t = Instant::now();
        let n = re.find_all(&big).unwrap().len();
        let elapsed = t.elapsed();
        assert_eq!(n > 0, expect_match, "pattern {:?}", p);
        assert!(
            elapsed.as_micros() < 1000,
            "pattern {:?} took {:?}, expected O(1)",
            p,
            elapsed
        );
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore)]
fn rev_bot_skip_terminates_fast() {
    use std::time::Instant;
    let big = vec![b'x'; 1 << 22];

    let re = Regex::new(r"\z").unwrap();
    let t = Instant::now();
    let ms = re.find_all(&big).unwrap();
    let elapsed = t.elapsed();
    assert_eq!(ms.len(), 1);
    assert_eq!(ms[0].start, big.len());
    assert_eq!(ms[0].end, big.len());
    assert!(
        elapsed.as_micros() < 500,
        "`\\z` on 4MB took {:?}, expected sub-ms (BOT skip regressed?)",
        elapsed
    );
}

#[test]
fn is_match_agrees_with_find_all_for_lookahead() {
    use resharp::UnicodeMode;
    let mk = |p: &str| {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        Regex::with_options(p, opts).unwrap()
    };
    let re = mk(r".(?=a|$)");
    let hay = b"xa xb x\nxc x";
    assert_eq!(
        re.is_match(hay).unwrap(),
        !re.find_all(hay).unwrap().is_empty()
    );
    for hay in [&b"\n"[..], b"\n\n", b"\n\n\n\n"] {
        assert_eq!(
            re.is_match(hay).unwrap(),
            !re.find_all(hay).unwrap().is_empty(),
            "is_match disagrees with find_all on hay={:?}",
            hay
        );
    }
}

#[test]
fn alternation_prefix_soundness_bulk() {
    use resharp::UnicodeMode;
    let mk = |p: &str| {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        Regex::with_options(p, opts).unwrap()
    };

    let re = mk(r"EMU-(?!CLAUSE|XREF|ANNEX|INTRO)|DFN");
    let mut hay = Vec::new();
    for _ in 0..500 {
        hay.extend_from_slice(b"zz EMU-FOO zz ");
    }
    assert!(!hay.windows(3).any(|w| w == b"DFN"));
    assert_eq!(re.find_all(&hay).unwrap().len(), 500);

    let re = mk(r"abcdef|xy");
    let mut hay = Vec::new();
    for _ in 0..200 {
        hay.extend_from_slice(b"_ abcdef _ ");
    }
    assert_eq!(re.find_all(&hay).unwrap().len(), 200);
}

#[test]
fn trailing_dollar_after_top_star_pruned() {
    use resharp::UnicodeMode;
    let mk = |p: &str| {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        Regex::with_options(p, opts).unwrap()
    };
    let with_dollar = mk(r"^((?!_\S+=)[^\s]+)\s?([\S\s]*)$");
    let without_dollar = mk(r"^((?!_\S+=)[^\s]+)\s?([\S\s]*)");
    let hay = b"hello world\nfoo bar baz";
    assert_eq!(
        with_dollar.find_all(hay).unwrap(),
        without_dollar.find_all(hay).unwrap()
    );
    // multi-line haystack: longest match runs to \z regardless
    let hay2 = b"abc def ghi\njkl mno\npqr";
    assert_eq!(
        with_dollar.find_all(hay2).unwrap(),
        without_dollar.find_all(hay2).unwrap()
    );
}

#[test]
fn empty_language_short_circuits() {
    let p = r"x+(?=aa(b+))z{2,}";
    let re = Regex::new(p).unwrap();
    let big = vec![b'x'; 1 << 20];
    assert_eq!(re.find_all(&big).unwrap(), vec![]);
    assert_eq!(re.is_match(&big).unwrap(), false);
    assert_eq!(re.find_anchored(&big).unwrap(), None);
    // Empty input path too.
    assert_eq!(re.find_all(b"").unwrap(), vec![]);
    assert_eq!(re.is_match(b"").unwrap(), false);
}


#[test]
fn trailing_star_yields_to_fwd_prefix_kind() {
    use resharp::UnicodeMode;
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let re = Regex::with_options(r"BREAKING CHANGE:([\s\S]*)", opts).unwrap();
    assert_eq!(re.prefix_kind_name(), Some("AnchoredFwd"));
}

#[test]
fn anchored_fwd_lb_selected_when_min_len_zero_kind() {
    use resharp::UnicodeMode;
    for pat in [r"^(?!\_\S+=)\S+", r"^((?!\_\S+=)[^\s]+)\s?([\S\s]*)$"] {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(pat, opts).unwrap();
        assert_eq!(
            re.prefix_kind_name(),
            Some("AnchoredFwdLb"),
            "expected AnchoredFwdLb for `{pat}`, got {:?}",
            re.prefix_kind_name()
        );
    }
}

mod probe_alt {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    #[test]
    fn probe_alt() {
        let p = r"2011|TL868|NETTV\/3.1\b";
        let mode = std::env::var("MODE").unwrap_or_else(|_| "js".into());
        let m = match mode.as_str() {
            "ascii" => UnicodeMode::Ascii,
            "full" => UnicodeMode::Full,
            _ => UnicodeMode::Javascript,
        };
        let re = Regex::with_options(p, RegexOptions::default().unicode(m)).unwrap();
        let hay = "User-Agent: Mozilla/5.0 NETTV/3.1 or 2011 or TL868 random text\n".repeat(50);
        let ms = re.find_all(hay.as_bytes()).unwrap();
        let mut counts = [0usize; 3];
        for m in &ms {
            let s = &hay.as_bytes()[m.start..m.end];
            if s.starts_with(b"2011") {
                counts[0] += 1;
            } else if s.starts_with(b"TL868") {
                counts[1] += 1;
            } else if s.starts_with(b"NETTV") {
                counts[2] += 1;
            }
        }
        println!(
            "matches: {} algo: {:?} 2011={} TL868={} NETTV={}",
            ms.len(),
            re.prefix_kind_name(),
            counts[0],
            counts[1],
            counts[2]
        );
    }
}

mod probe_nettv {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    #[test]
    fn probe_nettv() {
        let p = r"NETTV\/3.1\b";
        let re = Regex::with_options(p, RegexOptions::default().unicode(UnicodeMode::Javascript))
            .unwrap();
        let hay = "xyz NETTV/3.1 abc NETTV/3.1 end".as_bytes();
        let ms = re.find_all(hay).unwrap();
        println!("matches={} algo={:?}", ms.len(), re.prefix_kind_name());
        for m in &ms {
            println!(
                "  at {}..{} = {:?}",
                m.start,
                m.end,
                std::str::from_utf8(&hay[m.start..m.end]).unwrap()
            );
        }
    }
}

mod probe_nullable_prefix {
    use resharp::{calc_potential_start, calc_potential_start_prune};
    use resharp_algebra::RegexBuilder;

    fn pp_sets(b: &mut RegexBuilder, sets: &[resharp_algebra::solver::TSetId]) -> String {
        sets.iter()
            .map(|&s| b.solver().pp(s))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn probe_result(pat: &str) -> (String, String) {
        let mut b = RegexBuilder::new();
        let node = resharp_parser::parse_ast(&mut b, pat).unwrap();
        let ts_rev = b.ts_rev_start(node).unwrap();
        println!("--- {pat}");
        println!("  fwd pp:        {}", b.pp(node));
        println!("  ts_rev:        {}", b.pp(ts_rev));
        let fwd_full = calc_potential_start(&mut b, node, 16, 64, false).unwrap();
        let fwd_s = pp_sets(&mut b, &fwd_full);
        println!("  fwd_potential:    {}", fwd_s);
        let rev_pot = calc_potential_start_prune(&mut b, ts_rev, 16, 64, true).unwrap();
        let rev_s = pp_sets(&mut b, &rev_pot);
        println!("  rev_potential:    {}", rev_s);
        (fwd_s, rev_s)
    }

    #[test]
    fn probe_nullable_suffix() {
        assert_eq!(probe_result(r"a~(b_*)"), ("a".into(), "a".into()));
        assert_eq!(probe_result(r"a~(b_*)c"), ("a;[^b]".into(), "c;_".into()));
        assert_eq!(
            probe_result(r"_*\A~(_*b)c"),
            ("_;_;_;_;_;_;_;_;_;_;_;_;_;_;_;_".into(), "c".into())
        );
        assert_eq!(probe_result(r"_*[^b]c|\Ac"), ("_;_".into(), "c".into()));
        assert_eq!(
            probe_result(r"2011|TL868|NETTV\/3.1\b"),
            (
                "[2NT];[0EL];[18T];[16T]".into(),
                "[18];[16];[08];[2L]".into()
            )
        );
    }
}

mod parser_size {
    use resharp::Regex;

    #[test]
    fn huge_repetitions_are_rejected() {
        let reject = [
            "a{2001}",
            "a{1000000}",
            ".{1,8191}",
            ".{1,7168}",
            "a{2147483647,2147483647}",
            "a{2147483648,2147483648}",
            "([0-9]{1,9999}):([0-9]{1,9999})",
        ];
        let accept = ["a{500}", "a{0,500}", "a{1,499}"];
        for p in reject {
            assert!(Regex::new(p).is_err(), "expected error for {p:?}");
        }
        for p in accept {
            assert!(Regex::new(p).is_ok(), "expected ok for {p:?}");
        }
    }

    #[test]
    fn deeply_nested_repetitions_rejected() {
        let reject = [
            "(?:a(?:b(?:c(?:d(?:e(?:f(?:g(?:h(?:i(?:FooBar){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}",
            "(?:a(?:b(?:c(?:d(?:e(?:f(?:g(?:h(?:i(?:j(?:k(?:l(?:FooBar){2}){2}){2}){2}){2}){2}){2}){2}){2}){2}){2}){2}){2}",
        ];
        for p in reject {
            assert!(Regex::new(p).is_err(), "expected error for {p:?}");
        }
        let long_alt = format!("{}|{}", "a".repeat(5000), "b".repeat(5000));
        assert!(Regex::new(&long_alt).is_err());
        let accept = [
            "(?:a(?:b(?:c(?:FooBar){2}){2}){2}){2}",
            "a{100}",
            "[a-z]{50,200}",
        ];
        for p in accept {
            assert!(Regex::new(p).is_ok(), "expected ok for {p:?}");
        }
    }

    #[test]
    fn mixed_alt_and_intersection_top_level_does_not_panic() {
        let cases = ["^&|&$", r"\s|&nbsp;", "&|x", "&&|\\|\\|"];
        for p in cases {
            assert!(Regex::new(p).is_err(), "expected error for {p:?}");
        }
    }
}

mod prefix_toml {
    use resharp::{PrefixSets, RegexBuilder};
    use resharp_algebra::solver::TSetId;
    use std::path::Path;

    fn make_prefix_sets(pattern: &str) -> (RegexBuilder, PrefixSets) {
        let mut b = RegexBuilder::new();
        let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
        let rev = b.ts_rev_start(node).unwrap();
        let sets = PrefixSets::compute(&mut b, node, rev).unwrap();
        (b, sets)
    }

    fn pp_sets(b: &RegexBuilder, sets: &[TSetId]) -> String {
        sets.iter()
            .map(|&s| b.solver_ref().pp(s))
            .collect::<Vec<_>>()
            .join(";")
    }

    const KINDS: &[&str] = &[
        "prefix_rev",
        "prefix_fwd",
        "potential_rev",
        "potential_fwd",
        "kind",
    ];

    struct PrefixTestCase {
        name: String,
        pattern: String,
        ignore: bool,
        checks: Vec<(&'static str, String)>,
    }

    fn load_prefix_tests() -> Vec<PrefixTestCase> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("prefix.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let table: toml::Value = content.parse().unwrap();
        let tests = table["test"].as_array().unwrap();
        tests
            .iter()
            .map(|t| PrefixTestCase {
                name: t["name"].as_str().unwrap().to_string(),
                pattern: t["pattern"].as_str().unwrap().to_string(),
                ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
                checks: KINDS
                    .iter()
                    .filter_map(|&kind| {
                        t.get(kind)
                            .and_then(|v| v.as_str())
                            .map(|expected| (kind, expected.to_string()))
                    })
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn test_prefix_toml() {
        for tc in load_prefix_tests() {
            if tc.ignore {
                continue;
            }
            let needs_sets = tc.checks.iter().any(|(k, _)| *k != "kind");
            let sets_pair = needs_sets.then(|| make_prefix_sets(&tc.pattern));
            let kind_result = tc.checks.iter().find(|(k, _)| *k == "kind").map(|_| {
                resharp::Regex::new(&tc.pattern)
                    .unwrap()
                    .prefix_kind_name()
                    .unwrap_or("None")
                    .to_string()
            });
            for (kind, expected) in &tc.checks {
                let result = if *kind == "kind" {
                    kind_result.clone().unwrap()
                } else {
                    let (b, sets) = sets_pair.as_ref().unwrap();
                    match *kind {
                        "prefix_rev" => pp_sets(b, &sets.rev_anchored.sets),
                        "potential_rev" => pp_sets(b, &sets.rev_potential.sets),
                        "potential_fwd" => pp_sets(b, &sets.fwd_potential.sets),
                        k => panic!("unknown prefix test kind: {}", k),
                    }
                };
                assert_eq!(
                    result, *expected,
                    "prefix test failed: name={}, kind={}",
                    tc.name, kind
                );
            }
        }
    }
}

mod accel_skip {
    use resharp::{Regex, RegexOptions};
    use std::path::Path;

    fn load_tests(path: &str) -> Vec<(String, String, Vec<(usize, usize)>)> {
        let content = std::fs::read_to_string(path).unwrap();
        let table: toml::Value = content.parse().unwrap();
        let tests = table["test"].as_array().unwrap();
        tests
            .iter()
            .map(|t| {
                let pattern = t["pattern"].as_str().unwrap().to_string();
                let input = t["input"].as_str().unwrap().to_string();
                let matches: Vec<(usize, usize)> = t["matches"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| {
                        let arr = m.as_array().unwrap();
                        (
                            arr[0].as_integer().unwrap() as usize,
                            arr[1].as_integer().unwrap() as usize,
                        )
                    })
                    .collect();
                (pattern, input, matches)
            })
            .collect()
    }

    #[test]
    #[ignore = "slow in debug; run with --ignored or in release"]
    fn accel_skip_lazy() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("accel_skip.toml");
        let tests = load_tests(path.to_str().unwrap());
        for (pattern, input, expected) in &tests {
            let re = Regex::with_options(
                pattern,
                RegexOptions {
                    dfa_threshold: 0,
                    max_dfa_capacity: 10000,
                    ..Default::default()
                },
            )
            .unwrap();
            let matches = re.find_all(input.as_bytes()).unwrap();
            let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
            assert_eq!(
                result, *expected,
                "lazy: pattern={:?}, input={:?}",
                pattern, input
            );
        }
    }
}

mod auto_harden {
    use resharp::{Regex, RegexOptions};
    use std::path::Path;

    #[test]
    fn auto_harden_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("auto_harden.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let table: toml::Value = content.parse().unwrap();
        let tests = table["test"].as_array().unwrap();
        for t in tests {
            let pattern = t["pattern"].as_str().unwrap();
            let expected = t["hardened"].as_bool().unwrap();
            let re = Regex::new(pattern).expect("pattern compiles");
            assert_eq!(
                re.is_hardened(),
                expected,
                "pattern={:?}: expected is_hardened={}, got {}",
                pattern,
                expected,
                re.is_hardened()
            );
            if expected {
                let hardened =
                    Regex::with_options(pattern, RegexOptions::default().hardened(true)).unwrap();
                let inputs: &[&[u8]] = &[b"", b"aaaaaaaa", b"abcdefg", b"|  |\n| a |\n|  |"];
                for input in inputs {
                    assert_eq!(
                        re.find_all(input).unwrap(),
                        hardened.find_all(input).unwrap(),
                        "pattern={:?} input={:?}",
                        pattern,
                        input
                    );
                }
            }
        }
    }
}

mod deriv {
    use resharp::{NodeId, RegexBuilder};
    use resharp_algebra::nulls::Nullability;
    use std::path::Path;

    struct DerivTestCase {
        name: String,
        pattern: String,
        ignore: bool,
        input: String,
        rev: Vec<Option<String>>,
        fwd: Vec<Option<String>>,
        rev_nulls: Option<Vec<usize>>,
        fwd_nulls: Option<Vec<usize>>,
    }

    fn parse_null_positions(t: &toml::Value, key: &str) -> Option<Vec<usize>> {
        t.get(key).and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .map(|e| e.as_integer().expect("null pos must be integer") as usize)
                .collect()
        })
    }

    fn parse_expected(t: &toml::Value, key: &str) -> Vec<Option<String>> {
        t.get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|e| {
                        let s = e.as_str().unwrap();
                        if s == "?" {
                            None
                        } else {
                            Some(s.to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn load_tests() -> Vec<DerivTestCase> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("deriv.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let table: toml::Value = content.parse().unwrap();
        let tests = table["test"].as_array().unwrap();
        tests
            .iter()
            .map(|t| DerivTestCase {
                name: t["name"].as_str().unwrap().to_string(),
                pattern: t["pattern"].as_str().unwrap().to_string(),
                ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
                input: t
                    .get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                rev: parse_expected(t, "rev"),
                fwd: parse_expected(t, "fwd"),
                rev_nulls: parse_null_positions(t, "rev_nulls"),
                fwd_nulls: parse_null_positions(t, "fwd_nulls"),
            })
            .collect()
    }

    fn pos_mask(pos: usize, n: usize) -> Nullability {
        if n == 0 {
            Nullability::BEGIN.or(Nullability::END)
        } else if pos == 0 {
            Nullability::BEGIN
        } else if pos == n {
            Nullability::END
        } else {
            Nullability::CENTER
        }
    }

    fn walk_bytes(
        b: &mut RegexBuilder,
        mut node: NodeId,
        bytes: &[u8],
        expected: &[Option<String>],
        expected_nulls: Option<&[usize]>,
        dir: &str,
        name: &str,
    ) {
        assert_eq!(
            bytes.len(),
            expected.len(),
            "input length must match {dir} expected length for {name}"
        );
        let n = bytes.len();
        let report_null = |b: &mut RegexBuilder, node: NodeId, pos: usize, label: &str| -> bool {
            let mask = pos_mask(pos, n);
            let null = b.nullability(node).has(mask);
            eprintln!(
                "  [{}] {} pos={} mask={:?} nullable={}",
                dir, label, pos, mask, null
            );
            null
        };
        let mut got_nulls: Vec<usize> = Vec::new();
        if report_null(b, node, 0, "initial") {
            got_nulls.push(0);
        }
        for (i, byte) in bytes.iter().enumerate() {
            let der_mask = pos_mask(i, n);
            let tset = b.solver().u8_to_set_id(*byte);
            let tregex = b.der(node, der_mask).unwrap();
            let next = b.transition_term(tregex, tset);
            let pp = b.pp(next);
            eprintln!(
                "  [{}] step={} byte='{}' (0x{:02x}) der_mask={:?} node={:?} => {}",
                dir, i, *byte as char, byte, der_mask, next, pp
            );
            if let Some(exp) = &expected[i] {
                assert_eq!(
                    pp, *exp,
                    "deriv pp mismatch: name={} dir={} step={} byte='{}'",
                    name, dir, i, *byte as char
                );
            }
            node = next;
            if report_null(b, node, i + 1, "after") {
                got_nulls.push(i + 1);
            }
        }
        if let Some(exp) = expected_nulls {
            assert_eq!(
                got_nulls, exp,
                "nullability mismatch: name={} dir={}\n  got:      {:?}\n  expected: {:?}",
                name, dir, got_nulls, exp
            );
        }
    }

    /// Regression: `init_contributes_pos` must record an empty match `(pos, pos)`
    /// when the always-nullable initial state transitions to DEAD on the byte
    /// at `pos`. Without it, intermediate empty matches at positions where
    /// the initial-state transition dies are lost in the FAS hardened scan.
    /// Triggered by patterns that are simultaneously always-nullable AND have
    /// a non-nullable cycle (forcing hardened mode).
    #[test]
    fn hardened_always_nullable_empty_matches() {
        use resharp::{Regex, RegexOptions, UnicodeMode};
        let mk = || RegexOptions::default().unicode(UnicodeMode::Javascript).hardened(true);
        let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
            ("(?:b*c|)", b"yy", &[(0, 0), (1, 1), (2, 2)]),
            ("(?:[^<]*<[\\w\\W]+>[^>]*$|)", b"x", &[(0, 0), (1, 1)]),
            ("()|(a+b+)", b"x", &[(0, 0), (1, 1)]),
            ("(?:.*x|)", b"yy", &[(0, 0), (1, 1), (2, 2)]),
        ];
        for (pat, input, expected) in cases {
            let re = Regex::with_options(pat, mk()).unwrap();
            assert!(re.is_hardened(), "{pat:?} should be hardened");
            let got: Vec<(usize, usize)> = re
                .find_all(input)
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            assert_eq!(
                got, *expected,
                "pattern={pat:?} input={:?}",
                std::str::from_utf8(input).unwrap()
            );
        }
    }

    #[test]
    fn test_deriv_toml() {
        for tc in load_tests() {
            if tc.ignore {
                continue;
            }
            let mut b = RegexBuilder::new();
            let node = resharp_parser::parse_ast(&mut b, &tc.pattern).unwrap();

            if !tc.rev.is_empty() || tc.rev_nulls.is_some() {
                let rev = b.reverse(node).unwrap();
                let rev = b.normalize_rev(rev).unwrap();
                let rev = b.mk_concat(NodeId::TS, rev);

                eprintln!(
                    "\n[{}] rev initial: node={:?} pp={}",
                    tc.name,
                    rev,
                    b.pp(rev)
                );
                let bytes: Vec<u8> = tc.input.as_bytes().iter().rev().copied().collect();
                let empty_rev = vec![None; bytes.len()];
                let rev_pp = if tc.rev.is_empty() {
                    &empty_rev
                } else {
                    &tc.rev
                };
                walk_bytes(
                    &mut b,
                    rev,
                    &bytes,
                    rev_pp,
                    tc.rev_nulls.as_deref(),
                    "rev",
                    &tc.name,
                );
            }

            if !tc.fwd.is_empty() || tc.fwd_nulls.is_some() {
                eprintln!(
                    "\n[{}] fwd initial: node={:?} kind={:?} pp={}",
                    tc.name,
                    node,
                    b.get_kind(node),
                    b.pp(node)
                );
                let bytes: Vec<u8> = tc.input.as_bytes().to_vec();
                let empty_fwd = vec![None; bytes.len()];
                let fwd_pp = if tc.fwd.is_empty() {
                    &empty_fwd
                } else {
                    &tc.fwd
                };
                walk_bytes(
                    &mut b,
                    node,
                    &bytes,
                    fwd_pp,
                    tc.fwd_nulls.as_deref(),
                    "fwd",
                    &tc.name,
                );
            }
        }
    }
}


