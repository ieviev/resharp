mod common;

#[test]
fn consuming_alternation_fixed_lookbehind() {
    let cases: &[(&str, &str, &[&str])] = &[
        (r".|(?<=ab)y", "Xaby", &["X", "a", "b", "y"]),
        (r".|(?<=ab)y", "XXXXaby", &["X", "X", "X", "X", "a", "b", "y"]),
        (r"x|(?<=ab)y", "abxaby", &["x", "y"]),
        (r"x|(?<=\.)y", ".axy", &["x"]),
    ];
    for &(p, inp, want) in cases {
        let re = Regex::new(p).unwrap();
        let got: Vec<String> = re
            .find_all(inp.as_bytes())
            .unwrap()
            .iter()
            .map(|m| String::from_utf8_lossy(&inp.as_bytes()[m.start..m.end]).into_owned())
            .collect();
        assert_eq!(got, want, "{p} on {inp}");
    }
}

#[test]
fn consuming_alternation_variable_lookbehind_fails_loud() {
    for p in [
        r"x|(?<=a[^\n\r]*)y",
        r"a|(?<=a[^\n\r]*)b",
        r"(?<!a)b|b(?!a)",
        r"[^\d.]|((?<=\..*)\.)",
    ] {
        assert!(Regex::new(p).is_err(), "expected unsupported (variable lb): {p}");
    }
}

#[test]
fn alternation_branch_lengths_disambiguate_lookbehind() {
    assert!(
        Regex::new(r"(?<=A)abc|(?<=C)abcd").is_err(),
        "ambiguous lookbehind alternation (same start `a`, differing lookbehinds, differing \
         lengths) is unsupported: the forward pass returns only a length and cannot tell which \
         branch's lookbehind held; must be rejected"
    );
    assert!(
        Regex::new(r"(?<=A)abc|(?<=C)abz").is_ok(),
        "same length (3): forward length is unambiguous; the match span is correct regardless of \
         which branch matched, and the reverse pass rejects when neither lookbehind holds"
    );
    assert!(
        Regex::new(r"(?<=A)abc|(?<=C)abc").is_ok(),
        "same length: forward length is unambiguous regardless of branch lookbehind"
    );
    assert!(
        Regex::new(r"(?<=A)abc|(?<=C)xyzw").is_ok(),
        "disjoint starts (a vs x): forward pass selects the right branch"
    );
    assert!(
        Regex::new(r"(?<=A)abc|(?<=A)abcd").is_ok(),
        "same lookbehind = (?<=A)(abc|abcd): one held lookbehind, forward longest is valid"
    );
    assert!(
        Regex::new(r"^abc|^abcd").is_ok(),
        "same anchor ^ = ^(abc|abcd): differing length under one shared lookbehind is fine"
    );
    assert!(
        Regex::new(r"^a|cd|^b").is_ok(),
        "disjoint forward firsts a/c/b: distinguishable regardless of anchors"
    );
    assert!(
        Regex::new(r"(?<=A)ab|(?<=C)ab|(?<=E)abc").is_err(),
        "len-3 (?<=E)abc overlaps the len-2 groups on `a`: differing lb + differing length"
    );
}

#[test]
fn length_one_lookbehind_alternation_supported() {
    let re = Regex::new(r"x|(?<=\.)y").unwrap();
    assert_eq!(
        re.find_all(b".axy").unwrap(),
        vec![resharp::Match { start: 2, end: 3 }]
    );
    let re = Regex::new(r"\ba{0}\b").unwrap();
    assert_eq!(re.is_match(b"").unwrap(), false);
}

#[test]
fn bounded_repeat_lookahead_no_compile_blowup() {
    let pat = r"(?:#)([A-Za-z0-9_](?:(?:[A-Za-z0-9_]|(?:\.(?!\.))){0,28}(?:[A-Za-z0-9_]))?)";
    let t = std::time::Instant::now();
    let re = Regex::new(pat).expect("compile");
    assert!(
        t.elapsed() < std::time::Duration::from_secs(2),
        "compile of bounded-repeat-with-lookahead took {:?}, expected sub-second",
        t.elapsed()
    );
    let hay = b"#hello.world.foo bar #a.b..c #x";
    let m = re.find_all(hay).unwrap();
    let got: Vec<&str> = m
        .iter()
        .map(|x| std::str::from_utf8(&hay[x.start..x.end]).unwrap())
        .collect();
    assert_eq!(got, vec!["#hello.world.foo", "#a.b", "#x"]);
}

use common::schemas::{EngineCase, EngineFile, InternalFile};
use resharp::{Error, Regex, RegexOptions};
use std::path::Path;

fn load_tests(filename: &str) -> Vec<EngineCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let content = std::fs::read_to_string(&path).expect(&format!("not found {}", filename));
    let file: EngineFile = toml::from_str(&content).unwrap();
    file.test
}

fn compile_case(tc: &EngineCase) -> Result<Regex, Error> {
    if tc.ascii {
        let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Ascii);
        Regex::with_options(&tc.pattern, opts)
    } else {
        Regex::new(&tc.pattern)
    }
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
        let re = compile_case(tc).unwrap_or_else(|e| {
            panic!(
                "file={}, name={:?}, pattern={:?}: compile error: {}",
                filename, tc.name, tc.pattern, e
            )
        });
        if tc.anchored {
            let m = re.find_anchored(tc.input.as_bytes()).unwrap();
            let result: Vec<[usize; 2]> = m.iter().map(|m| [m.start, m.end]).collect();
            assert_eq!(
                result, tc.matches,
                "file={}, name={:?}, pattern={:?}, input={:?} (anchored)",
                filename, tc.name, tc.pattern, tc.input
            );
        } else {
            let matches = re.find_all(tc.input.as_bytes()).unwrap();
            let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
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
#[ignore = "takes a long time; run only for releases"]
fn is_match_and_find_anchored_agree_with_find_all() {
    let files = [
        "anchors.toml",
        "basic.toml",
        "boolean.toml",
        "cross_feature.toml",
        "date_pattern.toml",
        "edge_cases.toml",
        "literal_alt.toml",
        "lookaround.toml",
        "paragraph.toml",
        "semantics.toml",
        "word_boundary.toml",
    ];
    for filename in files {
        let tests = load_tests(filename);
        for tc in &tests {
            if tc.ignore || tc.expect_error || tc.vs_regex || tc.anchored {
                continue;
            }
            let re = Regex::new(&tc.pattern).unwrap_or_else(|e| {
                panic!(
                    "file={}, name={:?}, pattern={:?}: compile error: {}",
                    filename, tc.name, tc.pattern, e
                )
            });
            let found = re.is_match(tc.input.as_bytes()).unwrap();
            assert_eq!(
                found,
                !tc.matches.is_empty(),
                "file={}, name={:?}, pattern={:?}, input={:?}",
                filename,
                tc.name,
                tc.pattern,
                tc.input
            );

            match re.find_anchored(tc.input.as_bytes()) {
                Ok(anchored) => {
                    let expected = tc
                        .matches
                        .first()
                        .filter(|m| m[0] == 0)
                        .map(|m| resharp::Match { start: m[0], end: m[1] });
                    assert_eq!(
                        anchored, expected,
                        "find_anchored disagrees with find_all: file={}, name={:?}, pattern={:?}, input={:?}",
                        filename, tc.name, tc.pattern, tc.input
                    );
                }
                Err(resharp::Error::Algebra(
                    resharp_algebra::ResharpError::UnsupportedPattern,
                )) => {}
                Err(e) => panic!(
                    "find_anchored error: file={}, name={:?}, pattern={:?}: {e:?}",
                    filename, tc.name, tc.pattern
                ),
            }
        }
    }
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
        let re = match Regex::with_options(&tc.pattern, opts) {
            Err(_) if tc.expect_error => continue,
            Err(e) => panic!(
                "file={}, name={:?}, pattern={:?}: compile error: {}",
                filename, tc.name, tc.pattern, e
            ),
            Ok(_) if tc.expect_error => panic!(
                "file={}, name={:?}, pattern={:?}: expected error but compiled Ok",
                filename, tc.name, tc.pattern
            ),
            Ok(re) => re,
        };
        let matches = re.find_all(tc.input.as_bytes()).unwrap();
        let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
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
fn offset_skip_brace_colon_ws_matches_regex() {
    let pat = r"\{:\s([^}]+)\}";
    let adversarial: &[u8] = b"   {:x}  { : y}  {:\ta} \n}}} {: } {:  z} {:\n q } {:w{:e} } noise }} \t\t {:\rk}  ";
    check_vs_regex(pat, adversarial);
    let re = Regex::new(pat).unwrap();
    assert_eq!(re.prefix_kind_name(), Some("AnchoredRev"));
}

#[test]
fn offset_skip_differential_fuzz() {
    let patterns = [
        r"\{:\s([^}]+)\}",
        r"\{:\s+([^}]+)\}",
        r"\{: ([^}]+)\}",
        r"<([^>]+)>",
        r"\{x([^}]+)\}",
        r"a:b([^z]+)z",
        r"\{:\s([^}]*)\}",
        r"\[: ([^\]]+)\]",
        r"foo:([^;]+);",
        r"\{:\s(\S[^}]*)\}",
        r":-\)([^!]+)!",
        r"\{\{([^}]+)\}\}",
    ];
    let alphabet: &[&str] = &[
        "{", "}", ":", " ", "\t", "\n", "\r", "<", ">", "[", "]",
        "x", "z", "a", "b", "f", "o", ";", "!", "-", ")", "S", "e", "u",
        "\u{e9}", "\u{4e2d}", "\u{1f600}",
    ];
    for pat in [r"\{:\s([^}]+)\}", r"<([^>]+)>", r"\{x([^}]+)\}"] {
        assert_eq!(
            Regex::new(pat).unwrap().prefix_kind_name(),
            Some("AnchoredRev"),
            "pattern {pat:?} should use AnchoredRev (offset-skip path)"
        );
    }
    let mut state: u64 = 0x9e3779b97f4a7c15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut mismatches = 0usize;
    for pat in patterns {
        let re = Regex::new(pat).expect(pat);
        let rx = regex::bytes::Regex::new(pat).unwrap();
        for _ in 0..4000 {
            let len = (next() % 300) as usize;
            let mut s = String::new();
            for _ in 0..len {
                s.push_str(alphabet[(next() as usize) % alphabet.len()]);
            }
            let input = s.as_bytes();
            let got: Vec<(usize, usize)> = re
                .find_all(input)
                .unwrap()
                .iter()
                .map(|m| (m.start, m.end))
                .collect();
            let exp: Vec<(usize, usize)> =
                rx.find_iter(input).map(|m| (m.start(), m.end())).collect();
            if got != exp {
                mismatches += 1;
                eprintln!("MISMATCH pat={pat:?} input={input:?} got={got:?} exp={exp:?}");
            }
            if re.is_match(input).unwrap() != !exp.is_empty() {
                mismatches += 1;
                eprintln!("IS_MATCH MISMATCH pat={pat:?} input={input:?}");
            }
        }
    }
    assert_eq!(mismatches, 0, "offset-skip differential mismatches");
}

#[test]
fn offset_skip_multibyte_class_differential_fuzz() {
    use resharp::{RegexOptions, UnicodeMode};
    let patterns = [r"([aÀ]{2,})", r"x([^zÀ]+)z", r"(À[a-z]+)", r"([abÀé]{2,})", r"y([^ À]+) "];
    let alphabet = ["a", "b", "z", "x", "y", "À", "é", " ", "1", "_"];
    let mut state: u64 = 0x1234567;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut mismatches = 0usize;
    for pat in patterns {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(UnicodeMode::Javascript))
            .expect(pat);
        let fr = fancy_regex::Regex::new(pat).unwrap();
        for _ in 0..3000 {
            let len = (next() % 40) as usize;
            let mut s = String::new();
            for _ in 0..len {
                s.push_str(alphabet[(next() as usize) % alphabet.len()]);
            }
            let input = s.as_bytes();
            let got: Vec<(usize, usize)> =
                re.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
            let mut exp: Vec<(usize, usize)> = Vec::new();
            let mut pos = 0;
            while pos <= s.len() {
                match fr.find_from_pos(&s, pos).unwrap() {
                    Some(m) => {
                        exp.push((m.start(), m.end()));
                        pos = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                    }
                    None => break,
                }
            }
            if got != exp {
                mismatches += 1;
                eprintln!("MISMATCH pat={pat:?} input={s:?} got={got:?} exp={exp:?}");
            }
        }
    }
    assert_eq!(mismatches, 0, "multibyte-class offset-skip differential mismatches");
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
fn hardened_zero_width_interior_null_matches_default() {
    for (pat, hay) in [
        (r"~(\A|\n+){2}", &b"\n\n"[..]),
        (r"[\x00-\x10]*(Z){2,}|(?!_{0}\A{3} {0,2}){3}", &b"\n\n"[..]),
        (r"1?a~(~((1?){2,}\z+){2}){2}", &b"a"[..]),
        (r"^{3}([\w]{2,}0{3}|_?)", &b"\n\n"[..]),
        (r"^_?", &b"\nb"[..]),
        (r"^_?", &b"\n\n"[..]),
    ] {
        let def = Regex::new(pat).unwrap();
        let hard = Regex::with_options(pat, RegexOptions::default().hardened(true)).unwrap();
        assert_eq!(
            def.find_all(hay).unwrap(),
            hard.find_all(hay).unwrap(),
            "default vs hardened find_all diverge for {pat:?} on {hay:?}"
        );
    }
}

#[test]
fn fwd_lb_prefix_line_anchor_find_all_is_exact() {
    let m = |s: usize, e: usize| resharp::Match { start: s, end: e };
    for (pat, hay, expected) in [
        (r"^_?", &b"\nb"[..], vec![m(0, 1), m(1, 2)]),
        (r"^_?", &b"\n\n"[..], vec![m(0, 1), m(1, 2), m(2, 2)]),
        (r"^_?", &b"\n\nb"[..], vec![m(0, 1), m(1, 2), m(2, 3)]),
        (r"^{3}([\w]{2,}0{3}|_?)", &b"\n\n"[..], vec![m(0, 1), m(1, 2), m(2, 2)]),
        (r"(?<=\n)_?", &b"\n\n"[..], vec![m(1, 2), m(2, 2)]),
    ] {
        let re = Regex::new(pat).unwrap();
        assert_eq!(
            re.find_all(hay).unwrap(),
            expected,
            "find_all wrong for {pat:?} on {hay:?}"
        );
    }
}

#[test]
fn bounded_repeat_over_lookaround_alternation_compiles() {
    let pats = [
        r"\A[a-z0-9]([a-z0-9]|(-(?!-))){1,61}[a-z0-9]\z",
        r"\A[^-#\x00-/:-@\[-^`{-\u{10FFFF}]([a-z]|[-](?![-])){0,62}[^-#\x00-/:-@\[-^`{-\u{10FFFF}]\z",
        r"\A([a-z]|(\d(?!\d{0,2}\.\d{1,3}\.\d{1,3}\.\d{1,3})))([a-z0-9]|(\.(?!(\.|-)))|(-(?!\.))){1,61}[a-z0-9]\z",
        r"\A\_\_([a-zA-Z](?:[a-zA-Z0-9]|\.[a-zA-Z]|(\.\_id)|\_(?!\_)){0,100})\_\_\z",
    ];
    for pat in pats {
        let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
        let re = Regex::with_options(pat, opts)
            .unwrap_or_else(|e| panic!("compile failed for {pat:?}: {e:?}"));
        let _ = re.find_all(b"ahZ09_/. ").unwrap();
    }
}

#[test]
fn leading_word_boundary_uses_anchored_prefix_and_is_exact() {
    let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
    let re = Regex::with_options(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b", opts).unwrap();
    assert_eq!(
        re.prefix_kind_name(),
        Some("AnchoredRev"),
        "constant-offset literal should pick a reverse-anchored prefix"
    );
    let a20 = "A".repeat(20);
    let m = |s: usize, e: usize| resharp::Match { start: s, end: e };
    let cases: Vec<(String, Vec<resharp::Match>)> = vec![
        (format!("github_pat_{a20}"), vec![m(0, 31)]),
        (format!(" github_pat_{a20}"), vec![m(1, 32)]),
        (format!("xgithub_pat_{a20}"), vec![]),
        (format!("github_pat_{a20}!"), vec![m(0, 31)]),
        (
            format!("github_pat_{a20}X yo github_pat_{a20}"),
            vec![m(0, 32), m(36, 67)],
        ),
        (format!("aa github_pat_{}", "A".repeat(19)), vec![]),
    ];
    for (hay, expected) in cases {
        assert_eq!(
            re.find_all(hay.as_bytes()).unwrap(),
            expected,
            "find_all wrong on {hay:?}"
        );
    }
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
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        RegexOptions {
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
            max_dfa_capacity: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    let precompiled_re = Regex::with_options(
        pattern,
        RegexOptions {
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
fn normal_word_boundary() {
    run_file("word_boundary.toml");
}

#[test]
fn literal_alt() {
    run_file("literal_alt.toml");
}

#[test]
fn capacity_exceeded_at_match() {
    let result = Regex::with_options(
        "a.*b.*c.*d",
        RegexOptions {
            max_dfa_capacity: 4,
            ..Default::default()
        },
    )
    .and_then(|re| re.find_all(b"a___b___c___d"));
    assert!(
        matches!(result, Err(Error::CapacityExceeded)),
        "expected CapacityExceeded error, got {result:?}"
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
        let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
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
#[ignore = "takes a long time; run only for releases"]
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
        Err(_) => return,
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
        let input: Vec<u8> = (0..256)
            .map(|i| {
                let v = ((hash.wrapping_mul(i as u64 + 1).wrapping_add(seed)) >> 8) as u8;
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
        "find_anchored.toml",
        "accel_skip.toml",
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
            if tc.vs_regex {
                check_hardened_vs_normal(&tc.pattern, tc.input.as_bytes());
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
            let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
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

fn load_internal_tests(filename: &str) -> Vec<common::schemas::InternalCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let content = std::fs::read_to_string(&path).unwrap();
    let file: InternalFile = toml::from_str(&content).unwrap();
    file.test
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
        if let Some(expected_pp) = &tc.pp {
            assert_eq!(
                got,
                expected_pp.clone(),
                "file={}, name={:?}, pattern={:?}",
                filename,
                tc.name,
                tc.pattern
            );
        }

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
fn opt_quantified_multichar_lookahead_fails_loud() {
    assert!(Regex::new(r"(?:(?!bc)[^\n\r])+").is_err());
    assert!(Regex::new(r"((?:(?!bc)[^\n\r])+)?x").is_err());
    assert!(Regex::new(r"<%((?:(?!%>).)+)?%>").is_err());

    let ok = Regex::new(r"(?:(?!b)[^\n\r])+").unwrap();
    assert_eq!(ok.find_all(b"aab").unwrap(), vec![resharp::Match { start: 0, end: 2 }]);
}

#[test]
fn word_boundary_after_grouped_trailing_lookahead() {
    let exp = vec![resharp::Match { start: 0, end: 1 }];
    for p in [r"(a(?= ))\b", r"(?:a(?= ))\b", r"a(?= )\b"] {
        let re = Regex::new(p).unwrap();
        assert_eq!(re.find_all(b"a b").unwrap(), exp, "pattern {p}");
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

fn run_file_exotic(filename: &str) {
    let tests = load_tests(filename);
    for tc in &tests {
        if tc.ignore {
            continue;
        }
        let re = match compile_case(tc) {
            Err(e) if tc.supported == Some(true) => panic!(
                "file={}, name={:?}, pattern={:?}: expected supported but compile failed: {}",
                filename, tc.name, tc.pattern, e
            ),
            Err(_) => continue,
            Ok(_) if tc.expect_error => panic!(
                "file={}, name={:?}, pattern={:?}: expected error but compiled Ok",
                filename, tc.name, tc.pattern
            ),
            Ok(re) => re,
        };
        let matches = match re.find_all(tc.input.as_bytes()) {
            Ok(m) => m,
            Err(e) if tc.supported == Some(true) => panic!(
                "file={}, name={:?}, pattern={:?}, input={:?}: expected supported but matching failed: {}",
                filename, tc.name, tc.pattern, tc.input, e
            ),
            Err(_) => continue,
        };
        if tc.supported == Some(false) {
            panic!(
                "file={}, name={:?}, pattern={:?}, input={:?}: expected unsupported but matching succeeded",
                filename, tc.name, tc.pattern, tc.input
            );
        }
        let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(
            result, tc.matches,
            "file={}, name={:?}, pattern={:?}, input={:?}: silently returned wrong result",
            filename, tc.name, tc.pattern, tc.input
        );
    }
}

#[test]
fn rust_numeric_literal_suffix_limited_rejects_nonleading_lookbehind() {
    let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
    let pattern =
        r"((?:\.\.)?)(?:\b0b\.?|\b|\.)\d[\d_]*(?:(?!\.\.)\.[\d_]*)?(?:e[+-]?\d[\d_]*)?[ulfi]{0,4}";
    assert!(Regex::with_options(pattern, opts).is_err());
}

#[test]
fn exotic_toml() {
    run_file_exotic("exotic.toml");
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
fn reject_lookahead_in_loop() {
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
fn repeat_limit_rejects_large_count() {
    let result = Regex::new(r"(?:[\x20-\x7E\xA0-\xFF](?!\uFE0F)){1,1000}");
    assert!(result.is_err(), "expected error for repeat > 500");
}

#[test]
fn repeat_limit_unbounded_allows_large_count() {
    let opts = RegexOptions::default().unbounded_size(true);
    let result = Regex::with_options(r"a{1,1000}", opts);
    assert!(result.is_ok(), "unbounded_size should allow repeat > 500");
}

#[test]
fn is_match_negative_lookahead() {
    let re = Regex::new(r"foo(?!bar)").unwrap();
    assert!(!re.is_match(b"foobar").unwrap());
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
fn rev_bot_constant_time() {
    use std::time::{Duration, Instant};
    fn best(re: &Regex, hay: &[u8], expect: usize) -> Duration {
        let mut lo = Duration::MAX;
        for _ in 0..16 {
            let t = Instant::now();
            let ms = re.find_all(hay).unwrap();
            let e = t.elapsed();
            assert_eq!(ms.len(), expect);
            lo = lo.min(e);
        }
        lo
    }
    let small = vec![b'x'; 1 << 14];
    let big = vec![b'x'; 1 << 22];

    let z = Regex::new(r"\z").unwrap();
    let z_small = best(&z, &small, 1);
    let z_big = best(&z, &big, 1);
    let z_factor = z_big.as_secs_f64() / z_small.as_secs_f64();

    let lin = Regex::new(r"q").unwrap();
    let lin_small = best(&lin, &small, 0);
    let lin_big = best(&lin, &big, 0);
    let lin_factor = lin_big.as_secs_f64() / lin_small.as_secs_f64();

    println!("z_factor={z_factor:.2} lin_factor={lin_factor:.2}");
    assert!(
        z_factor * 8.0 < lin_factor,
        "`\\z` scaling ({z_factor:.1}x) not clearly sub-linear vs literal scan ({lin_factor:.1}x); \
         z_small={z_small:?} z_big={z_big:?} lin_small={lin_small:?} lin_big={lin_big:?}",
    );
}

#[test]
fn max_depth_rejects_deep_nesting() {
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let at_cap = format!("{}a{}", "(".repeat(999), ")".repeat(999));
            assert!(Regex::new(&at_cap).is_ok(), "depth 999 should compile");

            let too_deep = format!("{}a{}", "(".repeat(1001), ")".repeat(1001));
            assert!(
                Regex::new(&too_deep).is_err(),
                "depth 1001 should be rejected by max_depth"
            );

            let compl_too_deep = format!("{}a{}", "~(".repeat(1001), ")".repeat(1001));
            assert!(
                Regex::new(&compl_too_deep).is_err(),
                "complement depth 1001 should be rejected by max_depth"
            );

            let opts = RegexOptions::default().unbounded_size(true);
            assert!(
                Regex::with_options(&too_deep, opts).is_ok(),
                "unbounded_size should disable the depth limit"
            );
        })
        .unwrap();
    handle.join().unwrap();
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
        let fwd_full = calc_potential_start(&mut b, node, 16, 64, false).unwrap();
        let fwd_s = pp_sets(&mut b, &fwd_full);
        let rev_pot = calc_potential_start_prune(&mut b, ts_rev, 16, 64, true).unwrap();
        let rev_s = pp_sets(&mut b, &rev_pot);
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

    use super::common::schemas::PrefixFile;

    fn load_prefix_tests() -> Vec<super::common::schemas::PrefixCase> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("prefix.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let file: PrefixFile = toml::from_str(&content).unwrap();
        file.test
    }

    #[test]
    fn test_prefix_toml() {
        for tc in load_prefix_tests() {
            if tc.ignore {
                continue;
            }
            let needs_sets =
                tc.prefix_rev.is_some() || tc.potential_rev.is_some() || tc.potential_fwd.is_some();
            let re = resharp::Regex::new(&tc.pattern);
            if re.is_err() {
                continue;
            }
            let sets_pair = needs_sets.then(|| make_prefix_sets(&tc.pattern));
            let check = |kind: &str, expected: &str| {
                let result = match kind {
                    "kind" => resharp::Regex::new(&tc.pattern)
                        .unwrap()
                        .prefix_kind_name()
                        .unwrap_or("None")
                        .to_string(),
                    other => {
                        let (b, sets) = sets_pair.as_ref().unwrap();
                        match other {
                            "prefix_rev" => pp_sets(b, &sets.rev_anchored.sets),
                            "potential_rev" => pp_sets(b, &sets.rev_potential.sets),
                            "potential_fwd" => pp_sets(b, &sets.fwd_potential.sets),
                            k => panic!("unknown prefix test kind: {}", k),
                        }
                    }
                };
                assert_eq!(
                    result, expected,
                    "prefix test failed: name={}, kind={}",
                    tc.name, kind
                );
            };
            if let Some(e) = &tc.kind {
                check("kind", e);
            }
            if let Some(e) = &tc.prefix_rev {
                check("prefix_rev", e);
            }
            if let Some(e) = &tc.potential_rev {
                check("potential_rev", e);
            }
            if let Some(e) = &tc.potential_fwd {
                check("potential_fwd", e);
            }
            #[cfg(feature = "convergence_prefix")]
            if let Some(e) = &tc.conv_literal {
                let got = resharp::detect_inner_literal_bytes(&tc.pattern)
                    .map(|v| String::from_utf8_lossy(&v).into_owned())
                    .unwrap_or_else(|| "None".to_string());
                assert_eq!(&got, e, "conv_literal mismatch: name={}", tc.name);
            }
        }
    }
}

mod accel_skip {
    use super::common::schemas::EngineFile;
    use resharp::{Regex, RegexOptions};
    use std::path::Path;

    #[test]
    #[ignore = "slow in debug; run with --ignored or in release"]
    fn accel_skip_lazy() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("accel_skip.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let file: EngineFile = toml::from_str(&content).unwrap();
        for tc in file.test {
            let re = Regex::with_options(
                &tc.pattern,
                RegexOptions {
                    max_dfa_capacity: 10000,
                    ..Default::default()
                },
            )
            .unwrap();
            let matches = re.find_all(tc.input.as_bytes()).unwrap();
            let result: Vec<[usize; 2]> = matches.iter().map(|m| [m.start, m.end]).collect();
            assert_eq!(
                result, tc.matches,
                "lazy: pattern={:?}, input={:?}",
                tc.pattern, tc.input
            );
        }
    }
}

mod auto_harden {
    use super::common::schemas::AutoHardenFile;
    use resharp::{Regex, RegexOptions};
    use std::path::Path;

    #[test]
    fn auto_harden_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("auto_harden.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let file: AutoHardenFile = toml::from_str(&content).unwrap();
        for tc in file.test {
            let re = Regex::new(&tc.pattern).expect(&format!(
                "file={},  pattern={:?}: compile failed",
                path.display(),
                tc.pattern
            ));
            assert_eq!(
                re.is_hardened(),
                tc.hardened,
                "pattern={:?}: expected is_hardened={}, got {}",
                tc.pattern,
                tc.hardened,
                re.is_hardened()
            );
            if tc.hardened {
                let hardened =
                    Regex::with_options(&tc.pattern, RegexOptions::default().hardened(true))
                        .unwrap();
                let inputs: &[&[u8]] = &[b"", b"aaaaaaaa", b"abcdefg", b"|  |\n| a |\n|  |"];
                for input in inputs {
                    assert_eq!(
                        re.find_all(input).unwrap(),
                        hardened.find_all(input).unwrap(),
                        "pattern={:?} input={:?}",
                        tc.pattern,
                        input
                    );
                }
            }
        }
    }
}

mod quadratic {
    use super::common::schemas::{QuadKind, QuadraticFile};
    use resharp::{Regex, RegexOptions};
    use std::path::Path;

    fn find_all_ns(re: &Regex, hay: &[u8]) -> u128 {
        let t = std::time::Instant::now();
        let _ = re.find_all(hay).unwrap();
        t.elapsed().as_nanos().max(1)
    }

    #[test]
    fn quadratic_toml() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("quadratic.toml");
        let content = std::fs::read_to_string(&path).unwrap();
        let file: QuadraticFile = toml::from_str(&content).unwrap();
        assert!(!file.test.is_empty());
        for tc in file.test {
            assert!(
                !tc.unit.is_empty(),
                "{}: missing worst-case construction unit",
                tc.name
            );
            match tc.kind {
                QuadKind::Fwd => {
                    let re =
                        Regex::with_options(&tc.pattern, RegexOptions::default().hardened(true))
                            .unwrap_or_else(|e| panic!("{}: compile failed: {e:?}", tc.name));
                    assert!(
                        !re.has_fwd_prefix(),
                        "{}: pattern {:?} selected a forward prefix under hardening; \
                         AnchoredFwd verify is O(n^2) here (unit={:?})",
                        tc.name,
                        tc.pattern,
                        tc.unit
                    );
                    let def = Regex::new(&tc.pattern)
                        .unwrap_or_else(|e| panic!("{}: compile failed: {e:?}", tc.name));
                    assert!(
                        !def.has_fwd_prefix(),
                        "{}: pattern {:?} selected a forward prefix in default mode; \
                         the interior loop swallows the prefix so AnchoredFwd verify is \
                         O(n^2) (unit={:?})",
                        tc.name,
                        tc.pattern,
                        tc.unit
                    );
                }
                QuadKind::Dfa => {
                    let re = Regex::new(&tc.pattern)
                        .unwrap_or_else(|e| panic!("{}: compile failed: {e:?}", tc.name));
                    assert!(
                        re.is_hardened(),
                        "{}: pattern {:?} is O(n^2) in the generic Dfa path; auto_harden must \
                         classify it as hardened in default mode",
                        tc.name,
                        tc.pattern
                    );
                    let build = |reps: usize| tc.unit.as_bytes().iter().cloned().cycle().take(reps).collect::<Vec<u8>>();
                    find_all_ns(&re, &build(20_000));
                    let baseline = find_all_ns(&re, &build(80_000));
                    let scaled = find_all_ns(&re, &build(640_000));
                    let ratio = scaled as f64 / baseline as f64;
                    assert!(
                        ratio < 24.0,
                        "{}: 8x input grew time {ratio:.1}x (>= 24x => quadratic); hardening must \
                         keep this dfa-quadratic pattern linear: {baseline}ns -> {scaled}ns",
                        tc.name
                    );
                }
            }
        }
    }

    #[test]
    fn auto_harden_suppresses_fwd_prefix_in_default_mode() {
        let pat = r"(@[A-Za-z0-9_0-9\$\_]+)([^\n\r]+\))([^\s])";
        let re = Regex::new(pat).unwrap();
        assert!(!re.is_hardened());
        assert_eq!(re.prefix_kind_name(), None);
        assert!(
            !re.has_fwd_prefix(),
            "default mode selected a fwd prefix; the @ opener feeds the wide interior \
             loop [^\\n\\r]+ so AnchoredFwd verify is O(n^2). This requires auto_harden's \
             no_fwd_prefix flag to fire without hardened(true)."
        );

        let baseline = scan_ns(&re, 8_000);
        let scaled = scan_ns(&re, 64_000);
        let ratio = scaled as f64 / baseline as f64;
        assert!(
            ratio < 16.0,
            "8x input grew time {ratio:.1}x (>= 16x => quadratic): {baseline}ns -> {scaled}ns"
        );
    }

    fn scan_ns(re: &Regex, n: usize) -> u128 {
        let hay = "@x".repeat(n / 2);
        let t = std::time::Instant::now();
        let m = re.find_all(hay.as_bytes()).unwrap().len();
        assert_eq!(m, 0);
        t.elapsed().as_nanos().max(1)
    }

    #[test]
    fn offset_skip_no_quadratic_for_multibyte_class() {
        use resharp::UnicodeMode;
        let re = Regex::with_options(
            r"[\wÀ]{2,}",
            RegexOptions::default().unicode(UnicodeMode::Javascript),
        )
        .unwrap();
        let run = |reps: usize| -> u128 {
            let hay = "abz ".repeat(reps).into_bytes();
            let t = std::time::Instant::now();
            let _ = re.find_all(&hay).unwrap().len();
            t.elapsed().as_nanos().max(1)
        };
        run(50_000);
        let baseline = run(200_000);
        let scaled = run(1_600_000);
        let ratio = scaled as f64 / baseline as f64;
        assert!(
            ratio < 16.0,
            "8x input grew time {ratio:.1}x (>= 16x => quadratic); a multibyte class \
             member must not make the reverse offset-skip bound search unbounded: \
             {baseline}ns -> {scaled}ns"
        );
    }

    fn offset_skip_scaling(pat: &str, unit: &str) -> f64 {
        let re = Regex::new(pat).unwrap();
        let run = |reps: usize| -> u128 {
            let hay = unit.repeat(reps).into_bytes();
            let t = std::time::Instant::now();
            let _ = re.find_all(&hay).unwrap().len();
            t.elapsed().as_nanos().max(1)
        };
        run(20_000);
        let baseline = run(80_000);
        let scaled = run(640_000);
        scaled as f64 / baseline as f64
    }

    #[test]
    fn offset_skip_no_quadratic_on_absent_seq() {
        let cases: &[(&str, &str)] = &[
            (r"<([a-z][a-z0-9]*)\b[^>]*>", "-> DEF\n"),
            (r"<([A-Z][A-Z0-9]*)\b[^>]*>", "-> def\n"),
            (r"!\[#([^\s\]]+)(?:\s+([^\]]*))?\]((?:\([^\)]*\)|\[[^\]]*\])?)", "] gh ij\n"),
        ];
        for (pat, unit) in cases {
            let ratio = offset_skip_scaling(pat, unit);
            assert!(
                ratio < 16.0,
                "8x input grew time {ratio:.1}x (>= 16x => quadratic) for {pat:?} on {unit:?}; \
                 an absent offset-skip seq must not make the reverse skip scan the whole prefix"
            );
        }
    }
}

mod hardened_regressions {
    #[test]
    fn hardened_always_nullable_empty_matches() {
        use resharp::{Regex, RegexOptions, UnicodeMode};
        let mk = || {
            RegexOptions::default()
                .unicode(UnicodeMode::Javascript)
                .hardened(true)
        };
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
                got,
                *expected,
                "pattern={pat:?} input={:?}",
                std::str::from_utf8(input).unwrap()
            );
        }
    }
}

#[test]
fn anchored_rev_intersection_complement_missed_by_find_all() {
    use resharp::Regex;
    let cases: &[(&str, &[u8], (usize, usize))] = &[
        ("x(_*b&~(b_+))", b"xab", (0, 3)),
        ("foo(_*bar&~(_*bar_+))", b"foo123bar", (0, 9)),
    ];
    for (pat, hay, expected) in cases {
        let r = Regex::new(pat).unwrap();
        let anchored = r.find_anchored(hay).unwrap();

        assert_eq!(
            anchored.map(|m| (m.start, m.end)),
            Some(*expected),
            "find_anchored sanity for {pat}"
        );
        let all = r.find_all(hay).unwrap();

        println!("anchored: {:?}", anchored);
        println!("all: {:?}", all);

        let spans: Vec<_> = all.iter().map(|m| (m.start, m.end)).collect();
        assert!(
            spans.contains(expected),
            "find_all missed match {expected:?} that find_anchored accepts; got {spans:?} for pat={pat}"
        );
        assert!(
            r.is_match(hay).unwrap(),
            "is_match disagrees with find_anchored for {pat}"
        );
    }
}

#[test]
fn js_numeric_literals() {
    let bin = resharp::Regex::new(r"0b[01]+(?:\_[01]+)*\b").unwrap();
    let oct = resharp::Regex::new(r"0o[0-7]+(?:\_[0-7]+)*\b").unwrap();
    let hex = resharp::Regex::new(r"(?i)0x[0-9a-f]+(?:\_[0-9a-f]+)*\b").unwrap();

    let matches = |re: &resharp::Regex, input: &[u8]| -> Vec<String> {
        re.find_all(input)
            .unwrap()
            .iter()
            .map(|m| String::from_utf8(input[m.start..m.end].to_vec()).unwrap())
            .collect()
    };

    assert_eq!(
        matches(&bin, b"0b1010 0b10_01 0b2 x0b10"),
        &["0b1010", "0b10_01", "0b10"]
    );
    assert_eq!(
        matches(&oct, b"0o777 0o7_7 0o8 x0o77"),
        &["0o777", "0o7_7", "0o77"]
    );
    assert_eq!(
        matches(&hex, b"0xff 0xA_B 0xg x0x1"),
        &["0xff", "0xA_B", "0x1"]
    );
}

#[test]
fn test_word_boundary_group() {
    let ok = |pat: &str| {
        resharp::Regex::new(pat).map(|_| true).unwrap_or_else(|e| {
            println!("FAIL {:?}: {}", pat, e);
            false
        })
    };
    assert!(ok(r#"(\b[A-Z])"#));
    assert!(ok(r#"((\b)[A-Z])"#));
    assert!(ok(r"\b\w|\A\w"));
    assert!(ok(r"(\b|\A)\w"));
    assert!(ok(r"\b\w|\A\w"));
    assert!(ok(r"(\b|\A)\w"));
}

#[test]
fn prefix_calc_terminates_on_complement_intersection_quantified() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = resharp::Regex::new(r"abc~(\w)&(?:aaa)*");
        let _ = tx.send(r.is_ok());
    });
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(_) => {}
        Err(_) => panic!("Regex::new hung on `abc~(\\w)&(?:aaa)*`"),
    }
}

#[test]
fn lookahead_rel_saturates_with_end_anchor_intersection() {
    let _ = resharp::Regex::new(r"(?:\w|$)(?:(?![1g]\_X)& a)");
}

#[test]
fn lookahead_rel_saturates_with_nested_quantified_lookahead() {
    let _ = resharp::Regex::new(r"(?:(?=a){1,2}){2}");
}

#[test]
fn lookaround_exotic() {
    let re = Regex::new(r"((?<!b)(?=b)|-)b(?!b)");
    if re.is_err() {
        return;
    }
    let re = re.unwrap();
    let m: Vec<[usize; 2]> = re
        .find_all(b"bbb")
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert!(m.is_empty(), "expected no matches, got {:?}", m);
}

#[test]
fn lookahead_rel_max_preserves_multibranch_body() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let mk_opts = || RegexOptions::default().unicode(UnicodeMode::Javascript);
    let p2 = r"\b(?=[A-Za-z0-9_]*[A-Z])(?=[A-Za-z0-9_]*[a-z])(?=[A-Za-z0-9_]*\d)[A-Za-z_][A-Za-z0-9_]*\b";
    let r2 = Regex::with_options(p2, mk_opts()).unwrap();
    let ms = r2.find_all(b".eXT12\n").unwrap();
    assert_eq!(ms.len(), 1);
    assert_eq!((ms[0].start, ms[0].end), (1, 6));
}

#[test]
fn strip_lb_rejects_lookbehind_in_intersection() {
    match resharp::Regex::new("(?:(?=a)&(?<=_))") {
        Ok(re) => {
            let ms = re
                .find_all(b"________________________________________________________________")
                .unwrap();
            assert!(ms.is_empty(), "spurious matches: {:?}", ms);
            let ms = re.find_all(&[b'a'; 128]).unwrap();
            assert!(ms.is_empty(), "spurious matches on a's: {:?}", ms);
        }
        Err(_) => {}
    }
}
#[test]
fn dot_is_match_twice() {
    let r = Regex::new(".").unwrap();
    assert!(r.is_match(b"hello").unwrap());
    assert!(r.is_match(b"hello").unwrap());
}
#[test]
fn dotdot_is_match_twice() {
    let r = Regex::new("..").unwrap();
    assert!(r.is_match(b"hello").unwrap());
    assert!(r.is_match(b"hello").unwrap());
}
#[test]
fn suffix_anchored_is_match() {
    let re = Regex::new(r"\.(client|server)\z").unwrap();
    for (s, want) in [
        ("foo.client", true),
        ("foo.server", true),
        ("foo.clientx", false),
        ("client", false),
        (".client.", false),
        ("", false),
    ] {
        assert_eq!(re.is_match(s.as_bytes()).unwrap(), want, "input={:?}", s);
    }
    let mut big = vec![b'a'; 64 * 1024];
    let n = big.len();
    big[n - 7..].copy_from_slice(b".client");
    assert!(re.is_match(&big).unwrap());
    assert!(!re.is_match(&vec![b'a'; 64 * 1024]).unwrap());
    let re2 = Regex::new(r"a?\z").unwrap();
    assert!(re2.is_match(b"abc").unwrap());
    assert!(re2.is_match(b"xyz").unwrap());
}

#[test]
fn grouped_boundary_contradiction() {
    match Regex::new(r"(\b)(\B)") {
        Ok(re) => assert!(re.find_all(b"ab").unwrap().is_empty()),
        Err(_) => {}
    }
}

#[test]
fn counted_rev_skip_no_boundary_double_consume() {
    let re = Regex::new(r"[\t\n\r ]{2,}").unwrap();
    let input = b"\tstringReplaceAll,";
    assert!(re.find_all(input).unwrap().is_empty());
    assert!(!re.is_match(input).unwrap());

    let a = re.is_match(b"  indented").unwrap();
    let b = re.is_match(b" */").unwrap();
    assert!(a);
    assert!(!b);
    assert_eq!(
        b,
        Regex::new(r"[\t\n\r ]{2,}")
            .unwrap()
            .is_match(b" */")
            .unwrap()
    );
}

#[test]
fn long_union_missing_literal_suffix_has_no_match() {
    let pattern = "wwwwwwwwwwveeg|eggggeg|eeg|f|wveeg|eggggeg|eeg|f|eeeg|eeg|b|g|ee|te|zte|mte|zte|mje|.zt..rr...z.wwwwwwwwwwv|ee|te|zte|mte|zte|mje|.zt..rr...z..z..nj.ek";
    let haystack = "ezwwwwwwwwwwwwwwwwwwwwww";
    let regex = Regex::with_options(
        pattern,
        RegexOptions::default().unicode(resharp::UnicodeMode::Ascii),
    )
    .unwrap();
    let matches: Vec<[usize; 2]> = regex
        .find_all(haystack.as_bytes())
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(matches, Vec::<[usize; 2]>::new());
}

#[test]
fn long_dot_union_does_not_match_short_haystack() {
    let pattern = "............n.......n.n.t.t..t|ee";
    let haystack = "ennn";
    let regex = Regex::with_options(
        pattern,
        RegexOptions::default().unicode(resharp::UnicodeMode::Full),
    )
    .unwrap();
    assert!(!regex.is_match(haystack.as_bytes()).unwrap());
}

#[test]
fn wb_after_mixed_word_nonword_class_not_silently_wrong() {
    for p in [r"-?[A-z.\-]+\b", r"[a-z.]+\b", r"[A-z]+\b"] {
        if let Ok(re) = Regex::new(p) {
            assert!(
                re.is_match(b"    i = 0;").unwrap(),
                "{p:?} compiled but silently mis-matches"
            );
        }
    }
}

#[test]
fn end_anchor_word_boundary_rejected_not_wrong() {
    let p = r"\b(?:af|il)\z\b";
    if let Ok(re) = Regex::new(p) {
        assert_eq!(
            re.is_match(b"il").unwrap(),
            true,
            "{p:?} compiled but silently mis-matches"
        );
    }
}

#[test]
fn multichar_negative_lookbehind_matches_reference() {
    let cases: &[(&str, &str)] = &[(r"(?<!ab)x", "xabx")];
    for &(p, s) in cases {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: compile error: {e}"));
        let ours: Vec<[usize; 2]> = re
            .find_all(s.as_bytes())
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        let fr = fancy_regex::Regex::new(p).unwrap();
        let mut reference = vec![];
        let mut start = 0;
        while let Ok(Some(m)) = fr.find_from_pos(s, start) {
            reference.push([m.start(), m.end()]);
            start = if m.end() > m.start() {
                m.end()
            } else {
                m.end() + 1
            };
            if start > s.len() {
                break;
            }
        }
        assert_eq!(ours, reference, "{p:?} on {s:?}");
    }
}

#[test]
fn end_anchored_with_lookaround_matches_fancy_regex() {
    let pats = [
        r"\},(?!\x22)\z",
        r"(?<=:)[0-9]+\z",
        r"[a-z]+(?!x)\z",
        r"(?<=[#@])[a-z0-9]+\z",
        r"[}\],][a-z]*(?!\x22)\z",
    ];
    let alpha: &[u8] = b"ab12}],:@#\"xed\n";
    let mut state: u64 = 0x9e3779b97f4a7c15;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for p in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: compile error: {e}"));
        let fr = fancy_regex::Regex::new(p).unwrap();
        for _ in 0..20_000 {
            let len = (rng() % 12) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| alpha[(rng() as usize) % alpha.len()]).collect();
            let s = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[test]
fn end_anchored_always_wins_over_fwd_prefix() {
    let keep = [
        "<script[\\s\\S]*\\z",
        "abc[\\s\\S]*\\z",
        "a\\z|b\\z",
        ".com\\z|.net\\z|.org\\z",
        "[\\s\\S]*foo\\z",
        "\\w+\\z",
    ];
    for pat in keep {
        let re = Regex::with_options(pat, RegexOptions::default().multiline(false)).unwrap();
        assert_eq!(re.find_all_kind_name(), "EndAnchored", "pat={pat}");
    }
    let re = Regex::with_options("<script[\\s\\S]*\\z", RegexOptions::default().multiline(false)).unwrap();
    let fr = fancy_regex::Regex::new("<script[\\s\\S]*\\z").unwrap();
    for s in ["x <script>a</script> y", "no match", "<script", "a<script>\n<script>z"] {
        let ours: Vec<[usize; 2]> = re.find_all(s.as_bytes()).unwrap().iter().map(|m| [m.start, m.end]).collect();
        let mut reference = vec![];
        let mut start = 0;
        while let Ok(Some(m)) = fr.find_from_pos(s, start) {
            reference.push([m.start(), m.end()]);
            start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
            if start > s.len() { break; }
        }
        assert_eq!(ours, reference, "pat=<script...> on {s:?}");
    }
}

#[test]
fn begin_anchored_with_leading_lookbehind_matches_fancy_regex() {
    let pats = [
        (r"(?<!a)\A>", "Anchored"),
        (r"(?<!ab)\Ax", "Anchored"),
        (r"(?<!a)\A[a-z]+", "Anchored"),
        (r"(?<=ab)\Ax", "EmptyLang"),
        (r"(?<=a)\A>", "EmptyLang"),
    ];
    let alpha: &[u8] = b"abx>yz ";
    let mut state: u64 = 0x51ed270b;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for (p, want_kind) in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: compile error: {e}"));
        assert_eq!(re.find_all_kind_name(), want_kind, "{p:?}");
        let fr = fancy_regex::Regex::new(p).unwrap();
        for _ in 0..30_000 {
            let len = (rng() % 10) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| alpha[(rng() as usize) % alpha.len()]).collect();
            let s = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[test]
fn literal_prefix_with_following_lookahead_matches_fancy_regex() {
    let pats = [
        r"https://(?![^:@/\s]+:[^:@/\s]+@)[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
        r"foo(?=bar)[a-z]+",
        r"key=(?!secret)[a-z]+",
    ];
    let alpha: &[u8] = b"htps:/@.aZ09-x \nbcomfokeyrt";
    let mut state: u64 = 0xdeadbeefcafe;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for p in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: compile error: {e}"));
        assert!(re.has_prefix(), "{p:?}: expected a prefilter");
        let fr = fancy_regex::Regex::new(p).unwrap();
        for _ in 0..40_000 {
            let len = (rng() % 32) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| alpha[(rng() as usize) % alpha.len()]).collect();
            let s = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[test]
fn fixed_length_neg_lookbehind_prefix_matches_fancy_regex() {
    let pats = [
        r"(?<!]\()https://[a-zA-Z0-9./]+",
        r"(?<![\$.])foo[a-z]+",
        r"(?<!ab)xyz[0-9]*",
        r"(?<!x)key=[a-z]+",
        r"(?<![\$.])(?<![ab])foo[a-z]*",
        r"(?<!xy)(?<![ab])(?<!\.)key=[a-z]+",
    ];
    let alpha: &[u8] = b"htps:/].(ab xyz0123fokl= $.cmABZ9-";
    let mut state: u64 = 0x1234_5678_9abc;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for p in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: compile error: {e}"));
        assert!(re.has_prefix(), "{p:?}: expected a prefilter");
        let fr = fancy_regex::Regex::new(p).unwrap();
        for _ in 0..40_000 {
            let len = (rng() % 32) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| alpha[(rng() as usize) % alpha.len()]).collect();
            let s = match std::str::from_utf8(&bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[test]
fn lookahead_in_optional_with_surrounding_stars() {
    assert!(Regex::new(r"((?=(x|yy))x)? *\z").is_err());
    let cases: &[(&str, &[u8], &[[usize; 2]])] = &[(r"\A *((?=[^ ])[^ ])? *\z", b" x", &[[0, 2]])];
    for (pat, hay, expected) in cases {
        let re = Regex::new(pat);
        if re.is_err() {
            continue;
        }
        let re = re.unwrap();
        let got: Vec<[usize; 2]> = re
            .find_all(hay)
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        assert_eq!(&got[..], *expected, "pat={pat:?} hay={hay:?}");
    }
}

#[test]
fn hardened_word_boundary_non_utf8_findall() {
    assert!(Regex::with_options(r"\B|,", RegexOptions::default().hardened(true)).is_err());
}

#[test]
fn hardened_bare_lookahead_zero_width_dot_hash() {
    let opts = RegexOptions::default().hardened(true);
    let re = Regex::with_options("(?=[.#])", opts).unwrap();
    let result: Vec<[usize; 2]> = re
        .find_all(b"a.b#c")
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(result, vec![[1, 1], [3, 3]]);
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn leading_literal_prefers_fwd_over_convergence() {
    use resharp::UnicodeMode;
    let fwd: &[&str] = &[
        r"<([/]?)([^ >]+)",
        r"[^\x08]\x08",
    ];
    for p in fwd {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert_eq!(
            re.prefix_kind_name(),
            Some("AnchoredFwd"),
            "pat={p} kind={:?}",
            re.prefix_kind_name()
        );
        assert!(!re.uses_convergence_prefix(), "pat={p} still convergence");
    }
    let fwd_verify_quadratic: &[&str] = &[
        r"<(?:\w+:)?Compression\s+([^>]*)/?>",
        r"@([./][^\s\n]+\.[^\s\n]+)",
        r"</?([a-z]\w*)\b[^>]*>",
    ];
    for p in fwd_verify_quadratic {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert_ne!(
            re.prefix_kind_name(),
            Some("AnchoredFwd"),
            "pat={p}: AnchoredFwd verify is O(n^2) (interior loop swallows the prefix); \
             must pick a linear-safe prefix instead",
        );
    }
    let no_conv: &[&str] = &[
        r"[^.!?:]+[.!?:]+",
        "\\s*([^=]+)=\"([^\"]*)\",?",
    ];
    for p in no_conv {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(!re.uses_convergence_prefix(), "pat={p} still convergence");
    }
    let conv_ok: &[&str] = &[
        "((?:\\\\.|[^\"])*)\"",
    ];
    for p in conv_ok {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(re.uses_convergence_prefix(), "pat={p} should select convergence");
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_rejected_for_interior_unbounded_verify() {
    use resharp::UnicodeMode;
    let pats: &[&str] = &[
        r"([a-zA-Z0-9_\.]*\([^\)]+\)|[^\s]+)\s+\?\s*([^\:]+)\s+\:\s*([^\n]+)",
        r"(\([^\)]+\)|[^\s]+)\s*\?\s*([^\:]+)\s+\:\s*([^\n]+)",
        r"(@\S[^@]+)",
    ];
    for p in pats {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(
            !re.uses_convergence_prefix(),
            "pat={p} selected convergence; its right part is an unbounded interior \
             forward verify re-run per literal hit (quadratic), kind={:?}",
            re.prefix_kind_name()
        );
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_rejected_for_bounded_short_no_anchor() {
    use resharp::UnicodeMode;
    let no_conv: &[&str] = &[
        r"[^%]%[^%]",
        "([^\u{00A4}])\u{00A4}([^\u{00A4}])",
        r"0.5.0",
        r"([^\\])sinx",
        "[^\"](\"\")",
    ];
    for p in no_conv {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(
            !re.uses_convergence_prefix(),
            "pat={p} selected convergence; it is fully bounded and short with no \
             anchor/boundary, so llmatch's bounded matcher beats it, kind={:?}",
            re.prefix_kind_name()
        );
    }
    let conv_ok: &[&str] = &[r"\b\s?<\s?\b", r".cjs\b", r"([^\w]|^)tr\("];
    for p in conv_ok {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(
            re.uses_convergence_prefix(),
            "pat={p} should keep convergence; its anchor/boundary makes llmatch's \
             reverse pass costly, kind={:?}",
            re.prefix_kind_name()
        );
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn interior_slash_uses_convergence() {
    for p in [r"\S+/\S+", r"[^ ]+/[^ ]+", r"\d+/\d+"] {
        let re = Regex::new(p).unwrap();
        if p == r"\d+/\d+" {
            assert!(!re.uses_convergence_prefix(), "{p}");
        } else {
            assert!(re.uses_convergence_prefix(), "{p} should use convergence");
        }
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_giant_match_dense_literal_is_linear() {
    let p = r"([a-z0-9-]+)\s*:\s*([^;\s]+(?:\s*[^;\s]+)*);?";
    let re = Regex::new(p).unwrap();
    assert!(re.uses_convergence_prefix(), "{p} should use convergence");
    let mut hay = String::new();
    for i in 0..20_000 {
        hay.push_str(&format!("key{i}: value {i} here\n"));
    }
    let ms = re.find_all(hay.as_bytes()).unwrap();
    assert_eq!(ms.len(), 1, "one giant match");
    assert_eq!(ms[0].start, 0);
    assert_eq!(ms[0].end, hay.trim_end().len());
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_is_match_no_false_positive() {
    let cases: &[(&str, &str)] = &[
        (r"\S+/\S+", "ab/ "),
        (r"\S+/\S+", "ab/"),
        (r"\S+/\S+", "/cd"),
        (r"\S+@\S+", "a@ "),
        (r"\d+/\d+", "12/ "),
        (r"\S+/\S+", "a/b"),
        (r"\S+/\S+", "no slash"),
    ];
    for &(p, s) in cases {
        let re = Regex::new(p).unwrap();
        let im = re.is_match(s.as_bytes()).unwrap();
        let fa = !re.find_all(s.as_bytes()).unwrap().is_empty();
        assert_eq!(im, fa, "is_match/find_all disagree for {p:?} on {s:?}: is_match={im} find_all_nonempty={fa}");
    }
}

#[test]
fn bounded_matches_general_path_differential() {
    use resharp::RegexOptions;
    let pats = [
        "(?:a|c?|cac)",
        "(?:a|c?|[cb]ac)",
        "(?:b?|bab)",
        "(?:a|ab|abc)",
        "(?:xy|y?|x)",
        "(?:ab|b|)",
        "(?:a|aa|aaa)?",
        "ab|b|c?",
        "(?:a?b?|abc)",
        "(?:a|b|ab|ba|aba)",
        "(?:a{1,3}|aab)",
    ];
    let alphabet = b"abc";
    for pat in pats {
        let bounded = Regex::new(pat).unwrap();
        let general =
            Regex::with_options(pat, RegexOptions::default().hardened(true)).unwrap();
        for n in 0u32..=6 {
            for code in 0..3usize.pow(n) {
                let mut s = String::new();
                let mut c = code;
                for _ in 0..n {
                    s.push(alphabet[c % 3] as char);
                    c /= 3;
                }
                let o: Vec<_> = bounded.find_all(s.as_bytes()).unwrap();
                let r: Vec<_> = general.find_all(s.as_bytes()).unwrap();
                assert_eq!(o, r, "bounded != general for pat={pat:?} s={s:?}");
            }
        }
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn inner_literal_detection() {
    use resharp::detect_inner_literal_bytes;
    assert_eq!(detect_inner_literal_bytes(r"(\S+)\/(\S+)"), Some(vec![b'/']));
    assert_eq!(detect_inner_literal_bytes(r"(\d+)\/(\d+)"), Some(vec![b'/']));
    assert_eq!(detect_inner_literal_bytes(r"\S+@\S+"), Some(vec![b'@']));
    assert_eq!(detect_inner_literal_bytes(r"\w+@\w+"), Some(vec![b'@']));
    assert_eq!(detect_inner_literal_bytes(r".(?=a)"), Some(vec![b'a']));
    assert_eq!(detect_inner_literal_bytes(r".(?=a|$)"), Some(vec![b'\n', b'a']));
    assert_eq!(detect_inner_literal_bytes(r"\S+(/\S+)?"), None);
    assert_eq!(detect_inner_literal_bytes(r"\S+"), None);
    assert_eq!(detect_inner_literal_bytes(r"\S+://\S+"), Some(b"://".to_vec()));
    assert_eq!(detect_inner_literal_bytes(r"foo\S+bar"), Some(b"bar".to_vec()));
    assert_eq!(detect_inner_literal_bytes(r"\S+ <-> \S+"), Some(b" <-> ".to_vec()));
    assert_eq!(
        detect_inner_literal_bytes(r"<(g|mi) (xlink[^> ]+) (xml[^> ]+)"),
        Some(b" xml".to_vec())
    );
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn trailing_redundant_lookahead_keeps_convergence_skip() {
    let p = r"(?:^|\W)props\.(\w+)(?!\w)";
    let fr = fancy_regex::Regex::new(p).unwrap();
    let inputs: &[&str] = &[
        "x props.foo y props.barBaz! end .props.q123 props. props.a",
        " props.x props.y_z9 \nprops.AbC ",
        "no match here at all",
        "props.foo \u{e9}cole .props.bar99 caf\u{e9}.props.baz",
    ];
    for unicode in [resharp::UnicodeMode::Ascii, resharp::UnicodeMode::Javascript] {
        let opts = RegexOptions::default().unicode(unicode);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(
            re.prefix_kind_name().is_some(),
            "trailing redundant lookahead must not disable the prefix ({unicode:?})"
        );
        for s in inputs {
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?} ({unicode:?})");
        }
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_prefix_matches_fancy_regex() {
    let pats = [
        r"(\S+)/(\S+)",
        r"(\d+)/(\d+)",
        r"\S+@\S+",
        r"\w+@\w+",
        r".(?=a)",
        r".(?=a|$)",
        r"x.(?=y)",
        r"\S+/\S+(?= END)",
        r"([a-z0-9-]+)\s*:\s*([^;\s]+(?:\s*[^;\s]+)*);?",
        r"[\sa-z]+/[\sa-z]+",
    ];
    let inputs: &[&str] = &[
        "a/b foo/bar x//y /lead trail/ no_slash a/b/c",
        "  /  ab/cd  12/34  e@f  user@host.com  /// ",
        "\u{e9}x/\u{e9}y caf\u{e9}/th\u{e9} a/b",
        "nothing here at all",
        "banana xaxa zaq aq a",
        "abc xy xyz x.y end",
        "trailing a",
        "p/q END r/s notEND u/v END",
        "a/b END",
        "/",
        "a",
        "color: red; margin: 0 auto; key: a b c",
        "k:v",
    ];
    for p in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        let fr = fancy_regex::Regex::new(p).unwrap();
        for s in inputs {
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn multi_byte_run_convergence_matches_fancy_regex() {
    let pats = [
        r"\S+ - \S+",
        r"[\sa-z]+ :: [\sa-z]+",
        r"\S+ => \S+",
        r"\S+ <-> \S+",
        r"\S+ OR \S+",
    ];
    let inputs: &[&str] = &[
        "a - b  foo-bar  x -  - y lone - end - - -",
        "x :: y  a::b   c :: d :: e  no colons here",
        "a => b c=>d  e => f =>  => g end",
        "p <-> q  r<->s  t <-> u <-> v  end <-> ",
        "a OR b ORb aOR c OR  OR d xyz OR z",
        " :: ",
        " - ",
        "",
        "nothing",
    ];
    for p in pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert!(re.uses_convergence_prefix(), "{p:?} no longer uses convergence");
        let fr = fancy_regex::Regex::new(p).unwrap();
        for s in inputs {
            let ours: Vec<[usize; 2]> = re
                .find_all(s.as_bytes())
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let mut reference = vec![];
            let mut start = 0;
            while let Ok(Some(m)) = fr.find_from_pos(s, start) {
                reference.push([m.start(), m.end()]);
                start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
                if start > s.len() {
                    break;
                }
            }
            assert_eq!(ours, reference, "{p:?} on {s:?}");
        }
    }
}

#[test]
fn plus_of_end_anchored_alts_is_end_anchored() {
    let re = Regex::new(r"(/\z|\\\z)+").unwrap();
    assert_eq!(re.find_all_kind_name(), "EndAnchored");
    let cases: &[(&str, &[[usize; 2]])] = &[
        ("a/", &[[1, 2]]),
        ("a\\", &[[1, 2]]),
        ("abc", &[]),
        ("/", &[[0, 1]]),
        ("x//", &[[2, 3]]),
        ("//\\", &[[2, 3]]),
        ("", &[]),
    ];
    for (s, want) in cases {
        let got: Vec<[usize; 2]> = re
            .find_all(s.as_bytes())
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        assert_eq!(got, want.to_vec(), "find_all {s:?}");
        assert_eq!(re.is_match(s.as_bytes()).unwrap(), !want.is_empty(), "is_match {s:?}");
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn wide_unbounded_fwd_anchor_yields_to_convergence() {
    let pats = [
        "([A-Z0-9-]+)=((\"[^\"]*\")|([^\",]*))(?:,|\\z)",
        "([A-Z0-9-]+)=(?:\"([^\"]+)\"|([^,]+))",
        "([A-Z-]+)=(?:\"([^\"]+)\"|([^,]+))",
    ];
    let hay = "FOO=\"bar baz\",QUX=quux,A-B=1,lower=skip,X=";
    for p in pats {
        let re = Regex::new(p).unwrap();
        assert!(re.uses_convergence_prefix(), "pat={p} should use convergence");
        let ours: Vec<[usize; 2]> = re
            .find_all(hay.as_bytes())
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        let fr = fancy_regex::Regex::new(p).unwrap();
        let mut reference = vec![];
        let mut start = 0;
        while let Ok(Some(m)) = fr.find_from_pos(hay, start) {
            reference.push([m.start(), m.end()]);
            start = if m.end() > m.start() { m.end() } else { m.end() + 1 };
            if start > hay.len() {
                break;
            }
        }
        assert_eq!(ours, reference, "pat={p}");
    }
}

#[test]
fn wide_class_prefix_yields_to_rare_rev_literal() {
    let re = Regex::new(r"([A-Z\_][A-Z0-9\_]{2,})\s*=").unwrap();
    assert_eq!(re.prefix_kind_name(), Some("AnchoredRev"));
    assert_ne!(re.find_all_kind_name(), "FwdPrefix");
    let hay = b"x = 1; FOO_BAR = 2; lower = 3; ABC=4; Q=5";
    let got: Vec<&str> = re
        .find_all(hay)
        .unwrap()
        .iter()
        .map(|m| std::str::from_utf8(&hay[m.start..m.end]).unwrap())
        .collect();
    assert_eq!(got, vec!["FOO_BAR =", "ABC="]);
}

#[test]
fn convergence() {
    assert!(Regex::new(".*(.+)*.+").is_ok());
    assert!(Regex::new(r"a*&(b|^)").is_ok());
    assert!(Regex::new(r"(?iu)(?:@2222&(?:(?:(?:(?:(?:i22|222)|(?:222|^))|caf\u{e9})|caf\u{e9})|caf\u{e9}))").is_ok());
}

#[test]
fn bug3_is_match_vs_find_all_z_lookahead() {
    let re = Regex::new(r"(\z|(?=a)\w)").unwrap();
    let hay = b"0";
    let fa = re.find_all(hay).unwrap();
    let im = re.is_match(hay).unwrap();
    assert_eq!(im, !fa.is_empty(),
        "is_match={im} find_all.len()={} disagree on '0'", fa.len());
}

#[test]
fn bug21_bb_not_idempotent() {
    let re = Regex::new(r"\Bb").unwrap();
    let r1 = re.is_match(b"ba").unwrap();
    let r2 = re.is_match(b"ba").unwrap();
    assert_eq!(r1, r2, "is_match(ba) not idempotent: first={r1} second={r2}");
    assert!(!r1, "\\Bb on 'ba' must be false (no non-word-boundary before b)");
}

#[test]
fn bug3_is_match_vs_find_all_bu() {
    let re = Regex::new(r"\BU").unwrap();
    let hay = b"Ui";
    println!("{:?}","CALL 1");
    let fa1 = re.find_all(hay).unwrap();
    println!("{:?}","CALL 2");
    let fa2 = re.find_all(hay).unwrap();
    assert_eq!(fa1, fa2, "find_all not idempotent: first={fa1:?} second={fa2:?}");
    let im = re.is_match(hay).unwrap();
    assert_eq!(im, !fa1.is_empty(),
        "is_match={im} find_all.len()={} disagree on 'Uii\\\\'", fa1.len());
}

#[test]
fn bug3_is_match_vs_find_all_z_a_empty() {
    let re = Regex::new(r"\z\A(?:a){0,1}").unwrap();
    let hay = b"";
    let fa = re.find_all(hay).unwrap();
    let im = re.is_match(hay).unwrap();
    assert_eq!(im, !fa.is_empty(),
        "is_match={im} find_all.len()={} disagree on empty input", fa.len());
}

#[test]
fn bug3_is_match_vs_find_all_lookbehind() {
    let re = Regex::new(r"(?<=\D?[a-c]+0?)b").unwrap();
    let hay = b"ba";
    let fa = re.find_all(hay).unwrap();
    let im = re.is_match(hay).unwrap();
    assert_eq!(im, !fa.is_empty(),
        "is_match={im} find_all.len()={} disagree on 'ba'", fa.len());
}

#[test]
fn bug4_no_match_sentinel_not_leaked_as_match_end() {
    let check = |ms: Vec<resharp::Match>, hay: &[u8]| {
        for m in &ms {
            assert!(
                m.end <= hay.len(),
                "end={} > hay.len()={}: Match {{ start: {}, end: {} }}",
                m.end, hay.len(), m.start, m.end
            );
        }
    };

    let mk_flags = || {
        resharp::RegexOptions::default()
            .case_insensitive(true)
            .ignore_whitespace(true)
            .dot_matches_new_line(true)
            .multiline(false)
    };

    let re = resharp::Regex::with_options(r"~(_*$)", mk_flags()).unwrap();
    check(re.find_all(b"ab").unwrap(), b"ab");
    check(re.find_all(b"abc").unwrap(), b"abc");

    let re2 = resharp::Regex::with_options(r"~(_*\z)", mk_flags()).unwrap();
    check(re2.find_all(b"ab").unwrap(), b"ab");
    check(re2.find_all(b"abc").unwrap(), b"abc");

    let re3 = resharp::Regex::new(r"\Bb+").unwrap();
    check(re3.find_all(b"ba").unwrap(), b"ba");

    let re4 = resharp::Regex::new(r"(?<=[^a])b+").unwrap();
    check(re4.find_all(b"ba").unwrap(), b"ba");
}

#[test]
fn bug7_negated_perl_classes_not_nullable_in_ascii_mode() {
    macro_rules! mk {
        ($pat:expr) => {
            resharp::Regex::with_options(
                $pat,
                resharp::RegexOptions::default().unicode(resharp::UnicodeMode::Ascii),
            ).unwrap()
        };
    }

    assert!(!mk!(r"\D").is_match(b"").unwrap(), r"\D must not match empty");
    assert!(!mk!(r"\S").is_match(b"").unwrap(), r"\S must not match empty");
    assert!(!mk!(r"\W").is_match(b"").unwrap(), r"\W must not match empty");

    assert!(!mk!(r"\D").is_match(b"0").unwrap(), r"\D must not match '0'");
    assert!(!mk!(r"\S").is_match(b" ").unwrap(), r"\S must not match ' '");
    assert!(!mk!(r"\W").is_match(b"a").unwrap(), r"\W must not match 'a'");

    assert!(!mk!(r"a*\D").is_match(b"").unwrap(), r"a*\D must not match empty");
    assert!(!mk!(r"a*\D").is_match(b"0").unwrap(), r"a*\D must not match '0'");

    assert!(!mk!(r"[\D]").is_match(b"").unwrap(), r"[\D] must not match empty");
    assert!(!mk!(r"[^\d]").is_match(b"").unwrap(), r"[^\d] must not match empty");

    assert!(mk!(r"\d").is_match(b"5").unwrap(), r"\d must match '5'");
    assert!(mk!(r"\s").is_match(b" ").unwrap(), r"\s must match ' '");
    assert!(mk!(r"\w").is_match(b"_").unwrap(), r"\w must match '_'");
}

#[test]
fn bug8_default_and_hardened_find_all_agree() {
    let cases: &[(&str, &[u8])] = &[
        (r"~(_a+)", b"aaa"),
        (r"~(aa*a)", b"aaa"),
        (r"a~(a+)", b"aaa"),
    ];
    for (pat, hay) in cases {
        let def = resharp::Regex::new(pat).unwrap();
        let hard = resharp::Regex::with_options(
            pat,
            resharp::RegexOptions::default().hardened(true),
        ).unwrap();
        let def_ms = def.find_all(hay).unwrap();
        let hard_ms = hard.find_all(hay).unwrap();
        assert_eq!(
            def_ms, hard_ms,
            "pat={pat:?} hay={hay:?}: default={def_ms:?} hardened={hard_ms:?}"
        );
    }
}

#[test]
fn compile_wildcard_literal_wildcard_terminates() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let pat = ".................\x1a...............................";
        let _ = tx.send(resharp::Regex::new(pat).is_ok());
    });
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(ok) => assert!(ok, "compile failed"),
        Err(_) => panic!("Regex::new hung on wildcard-literal-wildcard pattern"),
    }
}

#[test]
fn bug10_default_and_hardened_find_all_agree() {
    let cases: &[(&str, &[u8])] = &[
        (r"(?<=^)~(0+)", b"\n"),
        (r"(?<=^)~(0+)", b"0\n"),
    ];
    for (pat, hay) in cases {
        let def = resharp::Regex::new(pat).unwrap();
        let hard = resharp::Regex::with_options(
            pat,
            resharp::RegexOptions::default().hardened(true),
        ).unwrap();
        let def_ms = def.find_all(hay).unwrap();
        let hard_ms = hard.find_all(hay).unwrap();
        assert_eq!(
            def_ms, hard_ms,
            "pat={pat:?} hay={hay:?}: default={def_ms:?} hardened={hard_ms:?}"
        );
    }
}

#[test]
fn find_all_lb_prefix_keeps_offset1_zero_width() {
    let hay: &[u8] = b"\n\n";
    let spans = |re: &resharp::Regex| -> Vec<(usize, usize)> {
        re.find_all(hay)
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect()
    };
    let def = resharp::Regex::new("^$").unwrap();
    let hard = resharp::Regex::with_options(
        "^$",
        resharp::RegexOptions::default().hardened(true),
    )
    .unwrap();
    let def_spans = spans(&def);
    let hard_spans = spans(&hard);
    assert_eq!(
        def_spans,
        vec![(0, 0), (1, 1), (2, 2)],
        "default find_all(^$, \"\\n\\n\")={def_spans:?}, want [0:0,1:1,2:2]"
    );
    assert_eq!(
        def_spans, hard_spans,
        "default={def_spans:?} hardened={hard_spans:?} must agree"
    );
}

#[test]
fn rev_boundary_prefix_keeps_trailing_word_boundary() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let re = Regex::with_options(r"[a-z]+assert\b(?!\$)", opts).unwrap();
    let spans = |h: &[u8]| {
        re.find_all(h)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect::<Vec<_>>()
    };
    assert_eq!(spans(b"xassert"), vec![(0, 7)]);
    assert_eq!(spans(b"xassert_eq"), vec![]);
    assert_eq!(spans(b"xassertx"), vec![]);
    assert_eq!(spans(b"xassert$"), vec![]);
    assert_eq!(spans(b"xassert yassert_eq zassertx wassert"), vec![(0, 7), (28, 35)]);
    assert_eq!(
        spans(b"fooassert barassert_eq bazassert; quxassertx"),
        vec![(0, 9), (23, 32)]
    );

    let mut long = Vec::new();
    long.extend_from_slice(b"fooassert ");
    long.extend(std::iter::repeat(b'q').take(8192));
    long.extend_from_slice(b" barassert_eq bazassertx quxassert");
    let base = 10 + 8192;
    assert_eq!(spans(&long), vec![(0, 9), (base + 25, base + 34)]);
}

#[test]
fn fullmode_dot_literal_concat_compile_bounded() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let t = std::time::Instant::now();
    let opts = RegexOptions::default().unicode(UnicodeMode::Full);
    let re = Regex::with_options(".n.................  n.", opts).unwrap();
    let el = t.elapsed();
    assert!(el.as_secs_f64() < 12.0, "compile blew up: {el:?}");
    assert_eq!(re.find_all(b"xn................. zn.").unwrap().len(), 0);
}

#[test]
fn inter_optional_lookahead_no_width_leak() {
    use resharp::{Regex, RegexOptions};
    let check = |pat: &str, hay: &[u8], want: &[(usize, usize)], anchored: Option<(usize, usize)>| {
        let re = Regex::with_options(pat, RegexOptions::default()).unwrap();
        let fa: Vec<(usize, usize)> =
            re.find_all(hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(fa, want, "find_all {pat} on {hay:?}");
        assert_eq!(
            re.find_anchored(hay).unwrap().map(|m| (m.start, m.end)),
            anchored,
            "find_anchored {pat} on {hay:?}"
        );
    };
    check(r"a?&(?=a)?", b"ab", &[(0, 0), (1, 1), (2, 2)], Some((0, 0)));
    check(r"a?&(?!b)?", b"ab", &[(0, 0), (1, 1), (2, 2)], Some((0, 0)));
    check(r"a?&(?=c)?", b"ab", &[(0, 0), (1, 1), (2, 2)], Some((0, 0)));
    check(r"(\W|(?!c))&a", b"a", &[], None);
    check(r"(\d|(?!c))&a", b"a", &[], None);
    check(r"(\W|(?=a))&a", b"a", &[], None);
}

#[test]
fn lookahead_union_inter_complement_no_crash() {
    use resharp::{Regex, RegexOptions};
    let re = Regex::with_options(r"((?!a)|b)&(~((c)))", RegexOptions::default()).unwrap();
    let cases: &[(&[u8], &[(usize, usize)])] = &[
        (b"ca", &[(0, 0), (2, 2)]),
        (b"c", &[(0, 0), (1, 1)]),
        (b"abca", &[(1, 2), (2, 2), (4, 4)]),
        (b"", &[(0, 0)]),
    ];
    for &(hay, want) in cases {
        let fa: Vec<(usize, usize)> =
            re.find_all(hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(fa, want, "find_all on {hay:?}");
        assert_eq!(
            re.is_match(hay).unwrap(),
            !fa.is_empty(),
            "is_match vs find_all on {hay:?}"
        );
    }
}

#[test]
fn is_match_findall_agree_complement_end_anchor() {
    let mk = |full: bool| -> resharp::RegexOptions {
        if full {
            resharp::RegexOptions::default()
                .case_insensitive(true)
                .ignore_whitespace(true)
                .dot_matches_new_line(true)
                .multiline(false)
        } else {
            resharp::RegexOptions::default()
        }
    };
    let cases: &[(&str, &[u8], bool)] = &[
        (r"[0-9]{2}~(\z{1,3}|^{2}\W{0})+", b"00", true),
        (r"a~(\z)", b"a", false),
        (r"a~(\z|b)", b"a", false),
        (r"a~(\z)", b"ab", false),
        (r"ab~(\z)c", b"abXc", false),
    ];
    for &(pat, hay, full) in cases {
        let re = resharp::Regex::with_options(pat, mk(full)).unwrap();
        let im = re.is_match(hay).unwrap();
        let fa = re.find_all(hay).unwrap();
        assert_eq!(
            im,
            !fa.is_empty(),
            "{pat} on {hay:?}: is_match={im} but find_all={fa:?} (must agree)"
        );
    }
    let re = resharp::Regex::with_options(r"a~(\z)", resharp::RegexOptions::default()).unwrap();
    assert_eq!(re.is_match(b"a").unwrap(), false, "a~(\\z) on a: end is in z, complement empty");
    assert_eq!(re.find_all(b"a").unwrap().len(), 0);
}

#[test]
fn find_all_anchor_in_consumed_region() {
    let spans = |pat: &str, hay: &[u8]| -> Vec<(usize, usize)> {
        let def = resharp::Regex::new(pat).unwrap();
        let hard = resharp::Regex::with_options(
            pat,
            resharp::RegexOptions::default().hardened(true),
        )
        .unwrap();
        let d: Vec<(usize, usize)> =
            def.find_all(hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
        let h: Vec<(usize, usize)> =
            hard.find_all(hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(d, h, "{pat}: default={d:?} hardened={h:?} must agree");
        d
    };
    assert_eq!(
        spans("^\n", b"\n\n\n"),
        vec![(0, 1), (1, 2), (2, 3)],
        "^\\n on \\n\\n\\n: ^ at offset 1 is anchored by the \\n the prior match consumed"
    );
    assert_eq!(
        spans("^a\n", b"a\na\n"),
        vec![(0, 2), (2, 4)],
        "^a\\n on a\\na\\n: ^ at offset 2 is anchored by the \\n the prior match consumed"
    );
    assert_eq!(
        spans("^a", b"a\na"),
        vec![(0, 1), (2, 3)],
        "control: anchoring \\n sits between matches, not inside one"
    );
}

#[test]
fn bug12_neg_lookahead_class_not_nullable() {
    use resharp::Regex;
    let cases: &[&str] = &[
        r"(?!\w)0+",
        r"(?!~(b{0}))[a-z]",
        r"(?!\D)()*\D{2,2}",
        r"(?!\D)(1&[a-c]+)",
        r"(?!\w)0+.{0,2}",
    ];
    for &pat in cases {
        let re = Regex::new(pat).unwrap();
        assert!(
            !re.is_match(b"").unwrap(),
            "pat={pat:?}: expected is_match(\"\")=false, got true"
        );
        let ms = re.find_all(b"").unwrap();
        assert!(
            ms.is_empty(),
            "pat={pat:?}: expected find_all(\"\")=[], got {ms:?}"
        );
    }
}

#[test]
fn bug14_nullable_sibling_drops_lookbehind_gate() {
    use resharp::Regex;
    let rejected: &[&str] = &[
        r"(|(?<=[a-z])b)",
        r"(a*|(?<=[a-z])b)",
        r"(a?|(?<=[a-z])b)",
        r"((?<=[a-z])b|)",
    ];
    for &pat in rejected {
        assert!(
            Regex::new(pat).is_err(),
            "pat={pat:?} should be rejected (nullable sibling + lookbehind union)"
        );
    }
}

#[test]
fn bug27_word_boundary_nullable_composition() {
    let re = resharp::Regex::new(r"\ba{0}\b").unwrap();
    assert_eq!(re.is_match(b"").unwrap(), false, r"\ba{{0}}\b on empty: expected false");
    let re = resharp::Regex::new(r"\Ba{0}\z").unwrap();
    assert_eq!(re.is_match(b"").unwrap(), true, r"\Ba{{0}}\z on empty: expected true");
}

#[test]
fn bug22_is_match_fwd_prefix_not_quadratic() {
    let re = Regex::new(r"(a+)+b").unwrap();
    assert_eq!(re.is_match(b"aaab").unwrap(), true);
    assert_eq!(re.is_match(b"ba").unwrap(), false);
    let hay = vec![b'a'; 65536];
    let t = std::time::Instant::now();
    let _ = re.is_match(&hay).unwrap();
    let elapsed = t.elapsed().as_secs_f64();
    assert!(
        elapsed < 1.0,
        "is_match (a+)+b on 64 KB all-a took {elapsed:.3}s (O(n^2) regression)"
    );
}

#[test]
fn always_nullable_greedy_fast_path_correct() {
    let cases: &[(&str, &[u8], &[[usize; 2]])] = &[
        (r"[^/]*", b"aa/bb", &[[0, 2], [2, 2], [3, 5], [5, 5]]),
        (r"[^/]{0,3}", b"aaaa/b", &[[0, 3], [3, 4], [4, 4], [5, 6], [6, 6]]),
        (r".*", b"ab\ncd", &[[0, 2], [2, 2], [3, 5], [5, 5]]),
        (r"\d{0,7}", b"x0y", &[[0, 0], [1, 2], [2, 2], [3, 3]]),
    ];
    for (pat, input, expected) in cases {
        let re = Regex::new(pat).unwrap();
        let got: Vec<[usize; 2]> =
            re.find_all(input).unwrap().iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(&got, expected, "pattern={pat:?} input={input:?}");
    }
    let re = Regex::new(r"[^/]*").unwrap();
    let hay = vec![b'a'; 4 * 1024 * 1024];
    let t = std::time::Instant::now();
    let n = re.find_all(&hay).unwrap().len();
    assert_eq!(n, 2);
    assert!(
        t.elapsed().as_secs_f64() < 0.2,
        "find_all [^/]* on 4 MB non-slash should be near-linear, took {:.3}s",
        t.elapsed().as_secs_f64()
    );
}

#[test]
fn concat_wide_star_middle_not_hardened_and_linear() {
    let per_byte = |re: &Regex, sz: usize| -> f64 {
        let hay = vec![b'a'; sz];
        let _ = re.find_all(&hay).unwrap();
        let mut best = f64::MAX;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let _ = re.find_all(&hay).unwrap();
            best = best.min(t.elapsed().as_secs_f64() / sz as f64);
        }
        best
    };
    let small = 64 * 1024usize;
    let big = 4 * 1024 * 1024usize;
    for pat in [r"[^\.]*[^\n\r][^\.]*", r"[^\.]*[^\n\r]"] {
        let re = Regex::new(pat).unwrap();
        assert!(!re.is_hardened(), "pattern {pat:?} should not be auto-hardened");
        let ns_small = per_byte(&re, small);
        let ns_big = per_byte(&re, big);
        assert!(
            ns_big < ns_small * 4.0,
            "pattern {pat:?} super-linear: per-byte {:.2}ns at {small} vs {:.2}ns at {big} ({}x size)",
            ns_small * 1e9,
            ns_big * 1e9,
            big / small
        );
    }
}

#[test]
fn bug18_find_all_not_quadratic_on_always_nullable() {
    let re = Regex::new("~(a+)").unwrap();
    let result = re.find_all(b"aaa").unwrap();
    assert_eq!(
        result,
        vec![
            resharp::Match { start: 0, end: 0 },
            resharp::Match { start: 1, end: 1 },
            resharp::Match { start: 2, end: 2 },
            resharp::Match { start: 3, end: 3 },
        ]
    );
    let hay = vec![b'a'; 65536];
    let t = std::time::Instant::now();
    let _ = re.find_all(&hay).unwrap();
    let elapsed = t.elapsed().as_secs_f64();
    assert!(
        elapsed < 1.0,
        "find_all ~(a+) on 64 KB all-a took {elapsed:.3}s (O(n^2) regression)"
    );
}

#[test]
fn bug16_lookahead_in_lookbehind_rejected() {
    let rejected = [
        "(?<=$)",
        "((?<=$))",
        "(?:(?<=$))",
        "(?<=(?= ))",
        "(?<=(?=z))",
        "(?<!(?=z))",
    ];
    for pat in &rejected {
        assert!(
            Regex::with_options(pat, resharp::RegexOptions::default()).is_err(),
            "expected {pat:?} to be rejected but it compiled"
        );
    }
    assert!(Regex::with_options("(?<=a)", resharp::RegexOptions::default()).is_ok());
    assert!(Regex::with_options("(?<=a*)b", resharp::RegexOptions::default()).is_ok());
    assert!(Regex::with_options("(?<!a)", resharp::RegexOptions::default()).is_ok());
}

#[test]
fn bug19_optional_anchor_before_class_same_matches() {
    let hay: Vec<u8> = (0..256u16).map(|i| i as u8).collect();
    let dflt = resharp::RegexOptions::default();
    let re_anchored = Regex::with_options(r"$?\w", dflt).unwrap();
    let dflt = resharp::RegexOptions::default();
    let re_bare = Regex::with_options(r"\w", dflt).unwrap();
    assert_eq!(
        re_anchored.find_all(&hay).unwrap(),
        re_bare.find_all(&hay).unwrap(),
        "$?\\w and \\w should produce identical matches"
    );
    let re_anchored_opt = Regex::with_options(r"(?=x)?y", resharp::RegexOptions::default()).unwrap();
    let re_bare_y = Regex::with_options(r"y", resharp::RegexOptions::default()).unwrap();
    assert_eq!(
        re_anchored_opt.find_all(b"xyz yyy").unwrap(),
        re_bare_y.find_all(b"xyz yyy").unwrap(),
        "(?=x)?y and y should produce identical matches"
    );
}

#[test]
fn bug20_find_anchored_respects_leading_assertion_at_begin() {
    let re = Regex::new(r"\B0").unwrap();
    let hay = b"00";
    assert_eq!(
        re.find_all(hay).unwrap(),
        vec![resharp::Match { start: 1, end: 2 }],
        "find_all should match at 1"
    );
    let no_match = |r: &Regex, h: &[u8]| match r.find_anchored(h) {
        Ok(None) => true,
        Err(resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)) => true,
        other => panic!("expected None or UnsupportedPattern, got {other:?}"),
    };
    assert!(
        no_match(&re, hay),
        "find_anchored should return None (\\B fails at offset 0)"
    );
    let re2 = Regex::new(r"(?<=0)0").unwrap();
    assert!(
        no_match(&re2, hay),
        "find_anchored should return None ((?<=0) fails at offset 0)"
    );
    let re3 = Regex::new(r"\b0").unwrap();
    match re3.find_anchored(hay) {
        Ok(m) => assert_eq!(
            m,
            Some(resharp::Match { start: 0, end: 1 }),
            "find_anchored should return Some(0..1) for \\b0"
        ),
        Err(resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)) => {}
        other => panic!("expected Some(0..1) or UnsupportedPattern, got {other:?}"),
    }
}

#[test]
fn bug26_end_before_begin_anchor_matches_empty_string() {
    let re = Regex::new(r"\z\A").unwrap();
    assert_eq!(re.is_match(b"").unwrap(), true, "\\z\\A must match empty string");
    assert_eq!(re.is_match(b"x").unwrap(), false, "\\z\\A must not match non-empty");
    assert_eq!(
        re.find_all(b"").unwrap(),
        vec![resharp::Match { start: 0, end: 0 }]
    );
    let re2 = Regex::new(r"\za*\A").unwrap();
    assert_eq!(re2.is_match(b"").unwrap(), true, "\\za*\\A must match empty string");
    assert_eq!(re2.is_match(b"a").unwrap(), false, "\\za*\\A must not match non-empty");
}

#[test]
fn end_before_begin_anchor_reverse_dead_skips() {
    let re = Regex::new(r"\z\A").unwrap();
    let hay = vec![b'a'; 200_000];
    assert_eq!(re.find_all(&hay).unwrap().len(), 0);
    assert!(
        re.has_accel().1,
        "\\z\\A reverse scan must enable dead-skip instead of self-looping over the whole input"
    );
}

#[test]
fn bug_hardened_complement_find_all_skips_longer_match() {
    check_hardened_vs_normal("~(.*and.*)", b"__A and B");
}

#[test]
fn bug25_mutex_poison_does_not_brick_regex() {
    use std::panic;
    let re = Regex::new(r"\w+b").unwrap();
    let _ = re.find_all(b"ab");
    let first = panic::catch_unwind(panic::AssertUnwindSafe(|| re.find_all(b"ba")));
    let bricked = panic::catch_unwind(panic::AssertUnwindSafe(|| re.is_match(b"z")));
    assert!(
        bricked.is_ok(),
        "Regex must survive a caught panic: is_match after poisoning must not re-panic (got {:?})",
        bricked
    );
    drop(first);
}

const NESTED_LOOKAROUND_PAT: &str = r"(?<!x.*),?(.+)";

fn basket_haystack() -> Vec<u8> {
    std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../data/haystacks/js-ts-html-basket.txt"
    ))
    .expect("haystack file")
}

#[test]
fn nested_unbounded_lookaround_anchor_limit() {
    let mut opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
    opts.lookahead_context_max = 40;
    let re = Regex::with_options(NESTED_LOOKAROUND_PAT, opts).expect("compile");
    let result = re.find_all(&basket_haystack());
    let err = result.expect_err("expected AnchorLimit error on large haystack");
    assert!(
        matches!(err, Error::Algebra(_)) && err.to_string().contains("anchor limit"),
        "expected anchor limit error, got: {err:?}"
    );
}

#[test]
fn begin_anchored_lookahead_short_circuits() {
    let pats = ["(?=^#{1,4}\\s)", "(?=^##\\s)", "(?=^---\\s+\\S)", "(?=^@@ )", "(?=^##? )"];
    let hay = "lorem ipsum dolor sit amet ".repeat(4000).into_bytes();
    for pat in pats {
        let re = Regex::with_options(pat, RegexOptions::default().multiline(false)).unwrap();
        assert!(re.is_fwd_begin_anchored(), "pat={pat} not begin-anchored");
        assert!(re.find_all(&hay).unwrap().is_empty(), "pat={pat} false match");
    }
    let re = Regex::with_options("(?=^##\\s)", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(
        re.find_all(b"## hi\nmore").unwrap(),
        vec![resharp::Match { start: 0, end: 0 }]
    );
    assert!(re.find_all(b"x## hi").unwrap().is_empty());
}

#[test]
fn end_anchored_short_circuits() {
    let pats = ["(c|a)\\z", "(e|en|es)\\z", "\\w+\\z", "[0-9]+\\z", "abc\\z"];
    let hay = "lorem ipsum dolor sit amet ".repeat(4000).into_bytes();
    for pat in pats {
        let re = Regex::with_options(pat, RegexOptions::default().multiline(false)).unwrap();
        assert!(
            re.find_all(&hay).unwrap().is_empty(),
            "pat={pat} false match on non-matching haystack"
        );
    }
    let re = Regex::with_options("(e|en|es)\\z", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(
        re.find_all(b"notes").unwrap(),
        vec![resharp::Match { start: 3, end: 5 }]
    );
}

#[test]
fn end_anchored_alternation_hoist() {
    for pat in ["es\\z|s\\z", ".com\\z|.net\\z|.org\\z", "a\\z|b\\z"] {
        let re = Regex::with_options(pat, RegexOptions::default().multiline(false)).unwrap();
        assert_eq!(re.find_all_kind_name(), "EndAnchored", "pattern {pat}");
    }
    let re = Regex::with_options("es\\z|s\\z", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(re.find_all(b"notes").unwrap(), vec![resharp::Match { start: 3, end: 5 }]);
}

#[test]
fn end_anchored_with_leading_lookbehind() {
    let re = Regex::with_options(r"\b(Ant[o\xc2\xba]?[.]?[o\xc2\xba]?)\z", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(re.find_all_kind_name(), "EndAnchored");
    assert_eq!(re.find_all(b"x Anto").unwrap(), vec![resharp::Match { start: 2, end: 6 }]);
    assert_eq!(re.find_all(b"xAnto").unwrap(), vec![]);
    assert_eq!(re.find_all(b"Ant.").unwrap(), vec![resharp::Match { start: 0, end: 4 }]);
    assert_eq!(re.find_all(b"foo Ant").unwrap(), vec![resharp::Match { start: 4, end: 7 }]);
    assert_eq!(re.find_all(b"foo Anto bar").unwrap(), vec![]);

    let wb = Regex::with_options(r"\bcat\z", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(wb.find_all_kind_name(), "EndAnchored");
    assert_eq!(wb.find_all(b"a cat").unwrap(), vec![resharp::Match { start: 2, end: 5 }]);
    assert_eq!(wb.find_all(b"scat").unwrap(), vec![]);
}

#[test]
fn bug28_not_word_boundary_drops_consecutive_matches() {
    for mode in [
        resharp::UnicodeMode::Ascii,
        resharp::UnicodeMode::Javascript,
        resharp::UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"\Bx", opts).unwrap();
        let ms: Vec<[usize; 2]> = re.find_all(b"axx").unwrap().iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(ms, vec![[1, 2], [2, 3]], "\\Bx on 'axx' mode={mode:?}");

        let re2 = Regex::with_options(r"\B[A-Z]", RegexOptions::default().unicode(mode)).unwrap();
        let ms2: Vec<[usize; 2]> = re2.find_all(b"README").unwrap().iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(ms2, vec![[1, 2], [2, 3], [3, 4], [4, 5], [5, 6]], "\\B[A-Z] on 'README' mode={mode:?}");
    }
}

#[test]
fn empty_match_byte_offsets_vs_utf8_intersection() {
    let body = r"((([A-Za-z]+(-[\dA-Za-z]+){0,2})|\*)(;q=[01](\.\d+)?)?)*";
    let inp = "Bootstrap\u{2019}s form".as_bytes();

    let raw = Regex::new(body).unwrap();
    let got: Vec<[usize;2]> = raw.find_all(inp).unwrap().iter().map(|m|[m.start,m.end]).collect();
    assert_eq!(got, vec![[0,9],[9,9],[10,10],[11,11],[12,13],[13,13],[14,18],[18,18]]);

    let aligned = Regex::new(&format!(r"({body})&\p{{utf8}}*")).unwrap();
    let got: Vec<[usize;2]> = aligned.find_all(inp).unwrap().iter().map(|m|[m.start,m.end]).collect();
    assert_eq!(got, vec![[0,9],[9,9],[10,10],[11,11],[12,13],[13,13],[14,18],[18,18]]);
}

#[test]
fn repro_bug04_reentrant_union_rewrite_panic() {
    for p in [
        r"(.*.+)*.+",
        r"(0*.{3}b{0,2})+",
        r"(.{0,2}.{2,}[a-c]{3}\W*)*\w{2}.*",
        r".*(.+)*.+",
        r"(.*.*)*.*",
        r"(.+.*)+.+",
        r".*|.*(.+)*.+",
    ] {
        if let Ok(re) = Regex::new(p) {
            let _ = re.find_all(b"aaa").unwrap();
        }
    }
    let re = Regex::new(r"(.*.+)*.+").unwrap();
    let got: Vec<[usize; 2]> = re
        .find_all(b"aaa")
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(got, vec![[0, 3]]);
}

#[test]
fn repro_armbug01_simd_findall_offset1_zerowidth() {
    for (p, inp, want) in [
        (r"^$", "\n\n", vec![[0,0],[1,1],[2,2]]),
        (r"^\x00?", "\n\x06.\n\x00", vec![[0,0],[1,1],[4,5]]),
    ] {
        let re = Regex::new(p).unwrap();
        let got: Vec<[usize;2]> = re.find_all(inp.as_bytes()).unwrap().iter().map(|m|[m.start,m.end]).collect();
        assert_eq!(got, want, "{p} on {inp:?}");
    }
}

#[test]
fn repro_bug05_rev_trivial_assert() {
    let m = |s: usize, e: usize| resharp::Match { start: s, end: e };
    let cases: &[(&str, &[u8], Vec<resharp::Match>)] = &[
        (r"_*$", b"\n\xfe*\xfe_*", vec![m(0, 6), m(6, 6)]),
        (r"_*$", b"abc", vec![m(0, 3), m(3, 3)]),
        (r"_*$", b"", vec![m(0, 0)]),
        (r"_*(?!_)", b"aa", vec![m(0, 2), m(2, 2)]),
    ];
    for (p, hay, want) in cases {
        let re = Regex::new(p).unwrap();
        assert_eq!(re.find_all_kind_name(), "Dfa", "pattern {p:?} routing changed");
        assert_eq!(
            &re.find_all(hay).unwrap(),
            want,
            "rev_trivial find_all wrong for {p:?} on {hay:?}"
        );
    }
}
#[test]
fn bug05_rev_trivial_vs_regex_crate_oracle() {
    let cases: &[(&str, &str)] = &[
        (r"_*$", r"(?s).*$"),
        (r".*$", r".*$"),
        (r"[a-z]*$", r"[a-z]*$"),
        (r"\w*$", r"\w*$"),
        (r"[0-9]*$", r"[0-9]*$"),
    ];
    let hays: &[&[u8]] = &[
        b"", b"a", b"abc", b"a\nb", b"\n\n", b"aXb\ncd", b"123\n456\n", b"\n",
        b"aaaa", b"a\nb\nc\n", b"zz\nzz", b"\xfe\x00\xff", b"abc\ndef",
        b"\n\xfe*\xfe_*", b"hello world\nfoo bar baz\n",
    ];
    for (p, rx) in cases {
        let re = Regex::new(p).unwrap();
        let oracle = regex::bytes::RegexBuilder::new(rx)
            .unicode(false)
            .multi_line(true)
            .build()
            .unwrap();
        for hay in hays {
            let got: Vec<[usize; 2]> = re
                .find_all(hay)
                .unwrap()
                .iter()
                .map(|m| [m.start, m.end])
                .collect();
            let want: Vec<[usize; 2]> =
                oracle.find_iter(hay).map(|m| [m.start(), m.end()]).collect();
            let mut prev_end: Option<usize> = None;
            let got_no_adj_empty: Vec<[usize; 2]> = got
                .iter()
                .copied()
                .filter(|m| {
                    let keep = !(m[0] == m[1] && Some(m[0]) == prev_end);
                    prev_end = Some(m[1]);
                    keep
                })
                .collect();
            assert_eq!(
                got_no_adj_empty, want,
                "rev_trivial find_all diverges from regex crate for {p:?} on {hay:?} \
                 (got={got:?}, kind={})",
                re.find_all_kind_name()
            );
        }
    }
}

#[test]
fn complement_z_active_set_no_end_phantom() {
    let cases: &[(&str, &str, [usize; 2])] = &[
        (r"~(.{1,3}\z)", "ab", [0, 1]),
        (r"~(.{1,3}\z){2,4}", "ab", [0, 1]),
        (r"~(.{1,3}\z){2,4}", "a", [0, 0]),
        (r"~(.{1,3}\z){2,4}", "abcdef", [0, 6]),
        (r"~(a_{0}(\z){2})+", "ab", [0, 2]),
        (r"~(\W{0,2}\z{2,})?", "ab", [0, 2]),
        (r"~([Z-a]*[^\w]+\z+)", "ab", [0, 2]),
        (r"~(.{2}\z)+", "abcde", [0, 5]),
        (r"~(\W{0,2}\z{2,})?", "  ", [0, 1]),
    ];
    for &(p, input, want) in cases {
        let re = resharp::Regex::new(p).unwrap();
        let inp = input.as_bytes();
        let fa = re.find_anchored(inp).unwrap().map(|m| [m.start, m.end]);
        assert_eq!(fa, Some(want), "find_anchored {p:?} on {input:?}");
        let all = re.find_all(inp).unwrap();
        assert_eq!(
            all.first().map(|m| [m.start, m.end]),
            Some(want),
            "find_all leftmost must match find_anchored (active-set END phantom) {p:?} on {input:?}: {all:?}"
        );
    }
}
#[test]
fn sanity_always_nullable_end_matches() {
    let cases: &[(&str, &str, &[[usize;2]])] = &[
        (r"a*", "aaa", &[[0,3],[3,3]]),
        (r"(a|b)*", "abab", &[[0,4],[4,4]]),
        (r"a*\z", "aaa", &[[0,3],[3,3]]),
        (r".*", "xy", &[[0,2],[2,2]]),
    ];
    for &(p, inp, want) in cases {
        let re = resharp::Regex::new(p).unwrap();
        let all: Vec<[usize;2]> = re.find_all(inp.as_bytes()).unwrap().iter().map(|m|[m.start,m.end]).collect();
        assert_eq!(all, want.to_vec(), "p={p:?} inp={inp:?}");
    }
}

#[test]
fn bounded_always_nullable_uses_bounded_path() {
    use resharp::{RegexOptions, UnicodeMode};
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let re = Regex::with_options(r"[^\n\r]{0,3}", opts).unwrap();
    assert_eq!(
        re.find_all_kind_name(),
        "Bounded",
        "bounded always-nullable pattern must route through the BDFA bounded path"
    );
    let all: Vec<[usize; 2]> = re
        .find_all(b"abcdef\ngh")
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(
        all,
        vec![[0, 3], [3, 6], [6, 6], [7, 9], [9, 9]],
        "leftmost-longest non-overlapping with zero-width fill at gaps"
    );
}

fn oracle_bounded_nonnewline(data: &[u8], bound: usize) -> Vec<[usize; 2]> {
    let len = data.len();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < len {
        let mut run = 0usize;
        while run < bound
            && cursor + run < len
            && data[cursor + run] != b'\n'
            && data[cursor + run] != b'\r'
        {
            run += 1;
        }
        if run > 0 {
            out.push([cursor, cursor + run]);
            cursor += run;
        } else {
            out.push([cursor, cursor]);
            cursor += 1;
        }
    }
    if out.last().map(|m| m[0]) != Some(len) {
        out.push([len, len]);
    }
    out
}

#[test]
fn bounded_always_nullable_matches_oracle() {
    use resharp::{RegexOptions, UnicodeMode};
    let mut data = Vec::new();
    for i in 0..3000u32 {
        let n = (i % 130) + 1;
        for j in 0..n {
            data.push(b'a' + (j % 26) as u8);
        }
        data.push(b'\n');
    }
    data.extend_from_slice(b"trailing no newline");
    for (pat, bound) in [(r"[^\n\r]{0,10}", 10), (r"[^\n\r]{0,40}", 40), (r"[^\n\r]{0,80}", 80)] {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(pat, opts).unwrap();
        assert_eq!(re.find_all_kind_name(), "Bounded", "pat={pat}");
        let got: Vec<[usize; 2]> = re
            .find_all(&data)
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        let want = oracle_bounded_nonnewline(&data, bound);
        assert_eq!(got, want, "pat={pat}");
    }
}

#[test]
fn bounded_range_with_nullable_alt_no_overrun() {
    use resharp::{RegexOptions, UnicodeMode};
    let cases: &[(&str, &str, &[(usize, usize)])] = &[
        ("c{2,3}ba|c?", "cccb", &[(0, 1), (1, 1), (2, 1), (3, 0), (4, 0)]),
        (
            "c{2,4}b(]|a)|(?:(?:c|))",
            "cccb",
            &[(0, 1), (1, 1), (2, 1), (3, 0), (4, 0)],
        ),
    ];
    for &(pat, input, expected) in cases {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(pat, opts).unwrap();
        assert_eq!(re.find_all_kind_name(), "Bounded", "pat={pat}");
        let got: Vec<(usize, usize)> = re
            .find_all(input.as_bytes())
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end - m.start))
            .collect();
        assert_eq!(got, expected, "pat={pat}");
    }
}

#[test]
fn zero_width_lookaround_alternation_no_double_count() {
    use resharp::{RegexOptions, UnicodeMode};
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let re = Regex::with_options("(?=[a-zA-Z])|(?<=[a-zA-Z])", opts).unwrap();
    let inp = b"ab cd";
    let ms = re.find_all(inp).unwrap();
    assert!(ms.iter().all(|m| m.start == m.end), "all zero-width");
    let positions: Vec<usize> = ms.iter().map(|m| m.start).collect();
    assert_eq!(positions, vec![0, 1, 2, 3, 4, 5], "one match per position");
}

#[test]
fn begin_anchor_after_nullable_quantifier_matches_empty_at_zero() {
    use resharp::{RegexOptions, UnicodeMode};
    let cases: &[(&str, &[u8])] = &[
        ("x*\\A", b"abc"),
        ("a*\\A", b"aaa"),
        ("[^\n\r]*\\A", b"abc"),
    ];
    for &(pat, hay) in cases {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(pat, opts).unwrap();
        let ms = re.find_all(hay).unwrap();
        assert_eq!(
            ms,
            vec![resharp::Match { start: 0, end: 0 }],
            "pat={pat} hay={:?}",
            std::str::from_utf8(hay).unwrap()
        );
    }
}

#[test]
fn lookbehind_kept_with_nullable_star_body_no_fwd_prefix() {
    let cases: &[(&str, &[u8], usize)] = &[
        ("(?<=Q)z[^\n\r]*z", b"zXz", 0),
        ("(?<=Q)z[^\n\r]*z", b"QzXz", 1),
        ("(?<=Q)z[^\n\r]*", b"zX", 0),
        ("(?<=Q)z[^\n\r]*", b"QzX", 1),
        ("(?<=@import )['\"].*['\"]", b"@import 'x'", 1),
        ("(?<=@import )['\"].*['\"]", b"import 'x'", 0),
    ];
    for &(pat, hay, want) in cases {
        let re = Regex::new(pat).unwrap();
        let got = re.find_all(hay).unwrap().len();
        assert_eq!(
            got,
            want,
            "pat={pat} hay={:?}",
            std::str::from_utf8(hay).unwrap()
        );
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn conv_forced_differential_vs_regex_crate() {
    let pats = [
        r"- ([^:]+): Rejected because ([^\n]+)",
        r"\[([a-z-]+)\s+([^\]]+)\]",
        r"foo([0-9]+)bar",
        r"a+X[bc]+",
        r"[a-z]+ foo [a-z]* QQ: [^\n]+",
        r"\{([a-zA-Z0-9_.]+), ([^}]+)\}",
        r"x[0-9]*Y[0-9]*z+",
        r#"\s*([^=]+)="([^"]*)",?"#,
        r#"((?:\\.|[^"])*)""#,
    ];
    let alphabet = b"- :RejctdbcausXabcfo0129[]{}|.QYz_ =\"\\\n\t";
    let mut state: u64 = 0x9e3779b97f4a7c15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for pat in pats {
        let rs = resharp::Regex::with_options(
            pat,
            resharp::RegexOptions::default()
                .unicode(resharp::UnicodeMode::Ascii)
                .force_convergence(true),
        )
        .unwrap();
        assert!(
            rs.uses_convergence_prefix(),
            "pat={pat:?} did not select convergence even when forced"
        );
        let re = regex::bytes::RegexBuilder::new(pat)
            .unicode(false)
            .build()
            .unwrap();
        let fr = fancy_regex::Regex::new(pat).unwrap();
        for _ in 0..20_000 {
            let len = (next() % 200) as usize;
            let hay: Vec<u8> = (0..len)
                .map(|_| alphabet[(next() as usize) % alphabet.len()])
                .collect();
            let rs_m = rs.is_match(&hay).unwrap();
            let re_m = re.is_match(&hay);
            let rs_n = rs.find_all(&hay).unwrap().len();
            let re_n = re.find_iter(&hay).count();
            if rs_m == re_m && rs_n.min(1) == re_n.min(1) {
                continue;
            }
            let hs = String::from_utf8_lossy(&hay);
            let fr_m = fr.is_match(&hs).unwrap();
            assert_eq!(
                rs_m, fr_m,
                "is_match divergence (resharp vs fancy-regex) pat={pat:?} hay={hs:?} regex-crate={re_m}"
            );
            assert_eq!(
                rs_n.min(1),
                fr_m as usize,
                "match-presence divergence (resharp vs fancy-regex) pat={pat:?} rs_n={rs_n} hay={hs:?}"
            );
        }
    }
}

#[test]
fn class_plus_fast_path() {
    let class_plus_pats = [r"\s+", r"[a-z]+"];
    for p in class_plus_pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert_eq!(re.find_all_kind_name(), "ClassPlus", "pat={p:?}");
    }
    let other_pats = [r".+", r"[\s\S]+", r"\d+", r"[-_.]+", r"a+", r"\s*", r"abc", r"\bx"];
    for p in other_pats {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
        assert_ne!(re.find_all_kind_name(), "ClassPlus", "pat={p:?}");
    }

    let mut state: u64 = 0x243f6a88;
    let mut rng = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let alpha: &[u8] = b"ab z\t\n9-_.XY";
    for p in [r"\s+", r".+", r"[a-z]+", r"[\s\S]+"] {
        let rs = Regex::new(p).unwrap();
        let rx = regex::bytes::Regex::new(p).unwrap();
        for _ in 0..20_000 {
            let len = (rng() % 60) as usize;
            let hay: Vec<u8> = (0..len).map(|_| alpha[(rng() as usize) % alpha.len()]).collect();
            let a: Vec<(usize, usize)> =
                rs.find_all(&hay).unwrap().iter().map(|m| (m.start, m.end)).collect();
            let b: Vec<(usize, usize)> =
                rx.find_iter(&hay).map(|m| (m.start(), m.end())).collect();
            assert_eq!(a, b, "pat={p:?} hay={:?}", String::from_utf8_lossy(&hay));
        }
    }
}

#[test]
fn bug07_lowerbound_1_repeat_after_overlapping_prefix() {
    let cases: &[(&str, &[u8], usize)] = &[
        (r"[ab]\n[b]+\n", b"a\nb\n", 1),
        (r"[ab]\n[b]{1,}\n", b"a\nb\n", 1),
        (r"[ab]\n[b][b]*\n", b"a\nb\n", 1),
        (r"[ab]\n[b]+\n", b"a\nbb\n", 1),
        (r"[ab]\n[b]*\n", b"a\nb\n", 1),
        (r"[ab]\n[b]{2,}\n", b"a\nbb\n", 1),
        (r"a\n[b]+\n", b"a\nb\n", 1),
        (r"(.+\r?\n)[-=]+\r?\n", b"title\n===\nx", 1),
    ];
    for &(pat, hay, want) in cases {
        let re = Regex::new(pat).unwrap();
        assert_eq!(
            re.is_match(hay).unwrap(),
            want > 0,
            "is_match pat={pat:?} hay={:?}",
            String::from_utf8_lossy(hay)
        );
        assert_eq!(
            re.find_all(hay).unwrap().len(),
            want,
            "find_all pat={pat:?} hay={:?}",
            String::from_utf8_lossy(hay)
        );
    }
}
