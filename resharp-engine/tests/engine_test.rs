mod common;

#[test]
fn consuming_alternation_fixed_lookbehind() {
    let cases: &[(&str, &str, &[&str])] = &[
        (r".|(?<=ab)y", "Xaby", &["X", "a", "b", "y"]),
        (
            r".|(?<=ab)y",
            "XXXXaby",
            &["X", "X", "X", "X", "a", "b", "y"],
        ),
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
        r"[^\d.]|((?<=\..*)\.)",
    ] {
        assert!(
            Regex::new(p).is_err(),
            "expected unsupported (variable lb): {p}"
        );
    }
}

#[test]
fn fixed_length_alternation_of_bounded_lookbehind_and_lookahead_is_supported() {
    // both branches are fixed-length-1, so this must compile and match.
    let re = Regex::new(r"(?<!a)b|b(?!a)").unwrap();
    assert_eq!(
        re.find_all(b"ab").unwrap(),
        vec![resharp::Match { start: 1, end: 2 }]
    );
    assert_eq!(
        re.find_all(b"ba").unwrap(),
        vec![resharp::Match { start: 0, end: 1 }]
    );
    assert_eq!(
        re.find_all(b"bab").unwrap(),
        vec![resharp::Match { start: 0, end: 1 }, resharp::Match { start: 2, end: 3 }]
    );
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
    assert_unique_names(filename, file.test.iter().map(|tc| tc.name.as_str()));
    file.test
}

fn assert_unique_names<'a>(filename: &str, names: impl Iterator<Item = &'a str>) {
    let mut seen = std::collections::HashSet::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        assert!(seen.insert(name), "file={filename}: duplicate case name {name:?}");
    }
}

fn case_options(tc: &EngineCase) -> RegexOptions {
    assert!(
        (tc.ascii as u8 + tc.javascript as u8 + tc.full as u8) <= 1,
        "case {:?}: ascii, javascript, and full are mutually exclusive",
        tc.name
    );
    let mut opts = RegexOptions::default();
    if tc.javascript {
        opts = opts.unicode(resharp::UnicodeMode::Javascript);
    } else if tc.ascii {
        opts = opts.unicode(resharp::UnicodeMode::Ascii);
    } else if tc.full {
        opts = opts.unicode(resharp::UnicodeMode::Full);
    }
    if let Some(multiline) = tc.multiline {
        opts = opts.multiline(multiline);
    }
    opts
}

fn compile_case(tc: &EngineCase) -> Result<Regex, Error> {
    Regex::with_options(&tc.pattern, case_options(tc))
}

fn check_prefix_kind(tc: &EngineCase, re: &Regex, filename: &str) {
    if let Some(want) = &tc.prefix_kind {
        assert_eq!(
            re.prefix_kind_name(),
            Some(want.as_str()),
            "file={}, name={:?}, pattern={:?}: prefix_kind",
            filename,
            tc.name,
            tc.pattern
        );
    }
    if let Some(forbidden) = &tc.not_prefix_kind {
        assert_ne!(
            re.prefix_kind_name(),
            Some(forbidden.as_str()),
            "file={}, name={:?}, pattern={:?}: not_prefix_kind",
            filename,
            tc.name,
            tc.pattern
        );
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
            let re = match compile_case(tc) {
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
        check_prefix_kind(tc, &re, filename);
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
            assert_eq!(
                re.is_match(tc.input.as_bytes()).unwrap(),
                !result.is_empty(),
                "file={}, name={:?}, pattern={:?}, input={:?}: is_match disagrees with find_all",
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
            let re = compile_case(tc).unwrap_or_else(|e| {
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
                    let expected =
                        tc.matches
                            .first()
                            .filter(|m| m[0] == 0)
                            .map(|m| resharp::Match {
                                start: m[0],
                                end: m[1],
                            });
                    assert_eq!(
                        anchored, expected,
                        "find_anchored disagrees with find_all: file={}, name={:?}, pattern={:?}, input={:?}",
                        filename, tc.name, tc.pattern, tc.input
                    );
                }
                Err(resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)) => {
                }
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

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_regressions() {
    run_file("convergence.toml");
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
    let adversarial: &[u8] =
        b"   {:x}  { : y}  {:\ta} \n}}} {: } {:  z} {:\n q } {:w{:e} } noise }} \t\t {:\rk}  ";
    check_vs_regex(pat, adversarial);
    let re = Regex::new(pat).unwrap();
    assert_eq!(re.prefix_kind_name(), Some("AnchoredRev"));
}

#[test]
#[ignore = "slow; run with --ignored"]
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
        "{",
        "}",
        ":",
        " ",
        "\t",
        "\n",
        "\r",
        "<",
        ">",
        "[",
        "]",
        "x",
        "z",
        "a",
        "b",
        "f",
        "o",
        ";",
        "!",
        "-",
        ")",
        "S",
        "e",
        "u",
        "\u{e9}",
        "\u{4e2d}",
        "\u{1f600}",
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
#[ignore = "slow; run with --ignored"]
fn offset_skip_multibyte_class_differential_fuzz() {
    use resharp::{RegexOptions, UnicodeMode};
    let patterns = [
        r"([aÀ]{2,})",
        r"x([^zÀ]+)z",
        r"(À[a-z]+)",
        r"([abÀé]{2,})",
        r"y([^ À]+) ",
    ];
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
        let re = Regex::with_options(
            pat,
            RegexOptions::default().unicode(UnicodeMode::Javascript),
        )
        .expect(pat);
        let fr = fancy_regex::Regex::new(pat).unwrap();
        for _ in 0..3000 {
            let len = (next() % 40) as usize;
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
            let mut exp: Vec<(usize, usize)> = Vec::new();
            let mut pos = 0;
            while pos <= s.len() {
                match fr.find_from_pos(&s, pos).unwrap() {
                    Some(m) => {
                        exp.push((m.start(), m.end()));
                        pos = if m.end() > m.start() {
                            m.end()
                        } else {
                            m.end() + 1
                        };
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
    assert_eq!(
        mismatches, 0,
        "multibyte-class offset-skip differential mismatches"
    );
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
        let opts = case_options(tc).hardened(true);
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
    assert_unique_names(filename, file.test.iter().map(|tc| tc.name.as_str()));
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
fn deep_concat_chain_compiles_on_a_small_stack() {
    let done = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let opts = RegexOptions::default().unbounded_size(true);
            let re = Regex::with_options(r"a{1,2000}", opts).unwrap();
            assert_eq!(re.find_all(b"aaa").unwrap().len(), 1);
        })
        .unwrap()
        .join();
    assert!(done.is_ok(), "deep concat chains must not overflow the stack");
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
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let pat = r"^(?!\_\S+=)\S+";
    let re = Regex::with_options(pat, opts).unwrap();
    assert_eq!(
        re.prefix_kind_name(),
        Some("AnchoredFwdLb"),
        "expected AnchoredFwdLb for `{pat}`, got {:?}",
        re.prefix_kind_name()
    );
}

#[test]
fn anchored_fwd_lb_declined_when_fused_lookahead_tail_has_interior_unbounded_loop() {
    // The lookahead's fused tail (`[^\s]+` then the unbounded `[\S\s]*`)
    // carries real forward-interior-quadratic risk, so this pattern must
    // not qualify for `AnchoredFwdLb`; `mml_min` must fold in the fused
    // tail's own minimum length rather than reporting 0.
    use resharp::UnicodeMode;
    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let pat = r"^((?!\_\S+=)[^\s]+)\s?([\S\s]*)$";
    let re = Regex::with_options(pat, opts).unwrap();
    assert_eq!(
        re.prefix_kind_name(),
        None,
        "expected no accelerated prefix kind for `{pat}`, got {:?}",
        re.prefix_kind_name()
    );
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
            #[cfg(not(feature = "convergence_prefix"))]
            if tc.kind.as_deref() == Some("Convergence") || tc.conv_literal.is_some() {
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
                    let build = |reps: usize| {
                        tc.unit
                            .as_bytes()
                            .iter()
                            .cloned()
                            .cycle()
                            .take(reps)
                            .collect::<Vec<u8>>()
                    };
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
    }

    #[test]
    #[ignore = "time based test"]
    fn auto_harden_suppressed_fwd_prefix_stays_linear() {
        let pat = r"(@[A-Za-z0-9_0-9\$\_]+)([^\n\r]+\))([^\s])";
        let re = Regex::new(pat).unwrap();
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
            (
                r"!\[#([^\s\]]+)(?:\s+([^\]]*))?\]((?:\([^\)]*\)|\[[^\]]*\])?)",
                "] gh ij\n",
            ),
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
#[ignore = "slow; run with --ignored"]
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
            let bytes: Vec<u8> = (0..len)
                .map(|_| alpha[(rng() as usize) % alpha.len()])
                .collect();
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
    let re = Regex::with_options(
        "<script[\\s\\S]*\\z",
        RegexOptions::default().multiline(false),
    )
    .unwrap();
    let fr = fancy_regex::Regex::new("<script[\\s\\S]*\\z").unwrap();
    for s in [
        "x <script>a</script> y",
        "no match",
        "<script",
        "a<script>\n<script>z",
    ] {
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
            start = if m.end() > m.start() {
                m.end()
            } else {
                m.end() + 1
            };
            if start > s.len() {
                break;
            }
        }
        assert_eq!(ours, reference, "pat=<script...> on {s:?}");
    }
}

#[test]
#[ignore = "slow; run with --ignored"]
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
            let bytes: Vec<u8> = (0..len)
                .map(|_| alpha[(rng() as usize) % alpha.len()])
                .collect();
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
}

#[test]
#[ignore = "slow; run with --ignored"]
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
            let bytes: Vec<u8> = (0..len)
                .map(|_| alpha[(rng() as usize) % alpha.len()])
                .collect();
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
}

#[test]
#[ignore = "slow; run with --ignored"]
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
            let bytes: Vec<u8> = (0..len)
                .map(|_| alpha[(rng() as usize) % alpha.len()])
                .collect();
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
}

#[test]
fn lookahead_in_optional_with_surrounding_stars() {
    // (?=(x|yy))x reduces to plain `x`: the lookahead only ever needs its
    // "x" arm, since whenever the literal x after it actually matches, that
    // same byte already satisfies the (x|yy) lookahead on its own.
    let re = Regex::new(r"((?=(x|yy))x)? *\z").unwrap();
    let got: Vec<[usize; 2]> = re
        .find_all(b"xx")
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(got, [[1, 2], [2, 2]]);

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
    let fwd: &[&str] = &[r"<([/]?)([^ >]+)", r"[^\x08]\x08"];
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
    let no_conv: &[&str] = &[r"[^.!?:]+[.!?:]+", "\\s*([^=]+)=\"([^\"]*)\",?"];
    for p in no_conv {
        let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
        let re = Regex::with_options(p, opts).unwrap();
        assert!(!re.uses_convergence_prefix(), "pat={p} still convergence");
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
fn convergence_unbounded_all_adjacent_literals_to_pos_one() {
    let re = Regex::new(r"(\S+)/(\S+)").unwrap();
    assert!(re.uses_convergence_prefix());
    let cases: &[(&[u8], &[(usize, usize)])] = &[
        (b"///", &[(0, 3)]),
        (b"/// ", &[(0, 3)]),
        (b"////", &[(0, 4)]),
    ];
    for (hay, want) in cases {
        let got: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, *want, "hay={:?}", std::str::from_utf8(hay).unwrap());
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_multibyte_class_variable_bounded_right() {
    let re = Regex::new(r"(\S):(\S{1,3})").unwrap();
    let cases: &[(&[u8], &[(usize, usize)])] = &[
        (b"x:19 ", &[(0, 4)]),
        (b"x:1 ", &[(0, 3)]),
        (b"a:b ", &[(0, 3)]),
        (b":::", &[(0, 3)]),
        (b"a:bc", &[(0, 4)]),
        ("é:ab ".as_bytes(), &[(0, 5)]),
        ("café:x t".as_bytes(), &[(3, 7)]),
        (b"  a:bb  z:9", &[(2, 6), (8, 11)]),
    ];
    for (hay, want) in cases {
        let got: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, *want, "hay={:?}", std::str::from_utf8(hay).unwrap());
    }
}

#[cfg(feature = "convergence_prefix")]
#[test]
fn convergence_adjacent_literal_overlap_seeds_all_starts() {
    let re = Regex::new("a?+:..").unwrap();
    let cases: &[(&[u8], &[(usize, usize)])] = &[
        (b"::xy", &[(0, 3)]),
        (b"z::xy", &[(1, 4)]),
        (b"zz:xy:xy", &[(2, 5), (5, 8)]),
        (b":.:.:.", &[(0, 3)]),
        (b"z:w:xy", &[(1, 4)]),
    ];
    for (hay, want) in cases {
        let got: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(
            got,
            *want,
            "pat=a?+:.. hay={:?}: convergence skip dropped an overlapping start",
            std::str::from_utf8(hay).unwrap()
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
        r"\b\s?<\s?\b",
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
        assert_eq!(
            im, fa,
            "is_match/find_all disagree for {p:?} on {s:?}: is_match={im} find_all_nonempty={fa}"
        );
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
        let general = Regex::with_options(pat, RegexOptions::default().hardened(true)).unwrap();
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
    assert_eq!(
        detect_inner_literal_bytes(r"(\S+)\/(\S+)"),
        Some(vec![b'/'])
    );
    assert_eq!(
        detect_inner_literal_bytes(r"(\d+)\/(\d+)"),
        Some(vec![b'/'])
    );
    assert_eq!(detect_inner_literal_bytes(r"\S+@\S+"), Some(vec![b'@']));
    assert_eq!(detect_inner_literal_bytes(r"\w+@\w+"), Some(vec![b'@']));
    assert_eq!(detect_inner_literal_bytes(r".(?=a)"), Some(vec![b'a']));
    assert_eq!(
        detect_inner_literal_bytes(r".(?=a|$)"),
        Some(vec![b'\n', b'a'])
    );
    assert_eq!(detect_inner_literal_bytes(r"\S+(/\S+)?"), None);
    assert_eq!(detect_inner_literal_bytes(r"\S+"), None);
    assert_eq!(
        detect_inner_literal_bytes(r"\S+://\S+"),
        Some(b"://".to_vec())
    );
    assert_eq!(
        detect_inner_literal_bytes(r"foo\S+bar"),
        Some(b"bar".to_vec())
    );
    assert_eq!(
        detect_inner_literal_bytes(r"\S+ <-> \S+"),
        Some(b" <-> ".to_vec())
    );
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
    for unicode in [
        resharp::UnicodeMode::Ascii,
        resharp::UnicodeMode::Javascript,
    ] {
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
                start = if m.end() > m.start() {
                    m.end()
                } else {
                    m.end() + 1
                };
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
        assert!(
            re.uses_convergence_prefix(),
            "{p:?} no longer uses convergence"
        );
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
        assert_eq!(
            re.is_match(s.as_bytes()).unwrap(),
            !want.is_empty(),
            "is_match {s:?}"
        );
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
            start = if m.end() > m.start() {
                m.end()
            } else {
                m.end() + 1
            };
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
    assert!(Regex::new(
        r"(?iu)(?:@2222&(?:(?:(?:(?:(?:i22|222)|(?:222|^))|caf\u{e9})|caf\u{e9})|caf\u{e9}))"
    )
    .is_ok());
}

#[test]
fn double_negation_not_idempotent() {
    let re = Regex::new(r"\Bb").unwrap();
    let r1 = re.is_match(b"ba").unwrap();
    let r2 = re.is_match(b"ba").unwrap();
    assert_eq!(
        r1, r2,
        "is_match(ba) not idempotent: first={r1} second={r2}"
    );
    assert!(
        !r1,
        "\\Bb on 'ba' must be false (no non-word-boundary before b)"
    );
}

#[test]
fn is_match_vs_find_all_agree_short_literal() {
    let re = Regex::new(r"\BU").unwrap();
    let hay = b"Ui";
    println!("{:?}", "CALL 1");
    let fa1 = re.find_all(hay).unwrap();
    println!("{:?}", "CALL 2");
    let fa2 = re.find_all(hay).unwrap();
    assert_eq!(
        fa1, fa2,
        "find_all not idempotent: first={fa1:?} second={fa2:?}"
    );
    let im = re.is_match(hay).unwrap();
    assert_eq!(
        im,
        !fa1.is_empty(),
        "is_match={im} find_all.len()={} disagree on 'Uii\\\\'",
        fa1.len()
    );
}

#[test]
fn no_match_sentinel_not_leaked_as_match_end() {
    let check = |ms: Vec<resharp::Match>, hay: &[u8]| {
        for m in &ms {
            assert!(
                m.end <= hay.len(),
                "end={} > hay.len()={}: Match {{ start: {}, end: {} }}",
                m.end,
                hay.len(),
                m.start,
                m.end
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
fn negated_perl_classes_not_nullable_in_ascii_mode() {
    macro_rules! mk {
        ($pat:expr) => {
            resharp::Regex::with_options(
                $pat,
                resharp::RegexOptions::default().unicode(resharp::UnicodeMode::Ascii),
            )
            .unwrap()
        };
    }

    assert!(
        !mk!(r"\D").is_match(b"").unwrap(),
        r"\D must not match empty"
    );
    assert!(
        !mk!(r"\S").is_match(b"").unwrap(),
        r"\S must not match empty"
    );
    assert!(
        !mk!(r"\W").is_match(b"").unwrap(),
        r"\W must not match empty"
    );

    assert!(
        !mk!(r"\D").is_match(b"0").unwrap(),
        r"\D must not match '0'"
    );
    assert!(
        !mk!(r"\S").is_match(b" ").unwrap(),
        r"\S must not match ' '"
    );
    assert!(
        !mk!(r"\W").is_match(b"a").unwrap(),
        r"\W must not match 'a'"
    );

    assert!(
        !mk!(r"a*\D").is_match(b"").unwrap(),
        r"a*\D must not match empty"
    );
    assert!(
        !mk!(r"a*\D").is_match(b"0").unwrap(),
        r"a*\D must not match '0'"
    );

    assert!(
        !mk!(r"[\D]").is_match(b"").unwrap(),
        r"[\D] must not match empty"
    );
    assert!(
        !mk!(r"[^\d]").is_match(b"").unwrap(),
        r"[^\d] must not match empty"
    );

    assert!(mk!(r"\d").is_match(b"5").unwrap(), r"\d must match '5'");
    assert!(mk!(r"\s").is_match(b" ").unwrap(), r"\s must match ' '");
    assert!(mk!(r"\w").is_match(b"_").unwrap(), r"\w must match '_'");
}

#[test]
fn default_and_hardened_find_all_agree_lookaround() {
    let cases: &[(&str, &[u8])] = &[
        (r"~(_a+)", b"aaa"),
        (r"~(aa*a)", b"aaa"),
        (r"a~(a+)", b"aaa"),
    ];
    for (pat, hay) in cases {
        let def = resharp::Regex::new(pat).unwrap();
        let hard =
            resharp::Regex::with_options(pat, resharp::RegexOptions::default().hardened(true))
                .unwrap();
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
fn default_and_hardened_find_all_agree_alternation() {
    let cases: &[(&str, &[u8])] = &[(r"(?<=^)~(0+)", b"\n"), (r"(?<=^)~(0+)", b"0\n")];
    for (pat, hay) in cases {
        let def = resharp::Regex::new(pat).unwrap();
        let hard =
            resharp::Regex::with_options(pat, resharp::RegexOptions::default().hardened(true))
                .unwrap();
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
    let hard = resharp::Regex::with_options("^$", resharp::RegexOptions::default().hardened(true))
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
    assert_eq!(
        spans(b"xassert yassert_eq zassertx wassert"),
        vec![(0, 7), (28, 35)]
    );
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
    let check =
        |pat: &str, hay: &[u8], want: &[(usize, usize)], anchored: Option<(usize, usize)>| {
            let re = Regex::with_options(pat, RegexOptions::default()).unwrap();
            let fa: Vec<(usize, usize)> = re
                .find_all(hay)
                .unwrap()
                .iter()
                .map(|m| (m.start, m.end))
                .collect();
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
        let fa: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();
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
    assert_eq!(
        re.is_match(b"a").unwrap(),
        false,
        "a~(\\z) on a: end is in z, complement empty"
    );
    assert_eq!(re.find_all(b"a").unwrap().len(), 0);
}

#[test]
fn find_all_anchor_in_consumed_region() {
    let spans = |pat: &str, hay: &[u8]| -> Vec<(usize, usize)> {
        let def = resharp::Regex::new(pat).unwrap();
        let hard =
            resharp::Regex::with_options(pat, resharp::RegexOptions::default().hardened(true))
                .unwrap();
        let d: Vec<(usize, usize)> = def
            .find_all(hay)
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();
        let h: Vec<(usize, usize)> = hard
            .find_all(hay)
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect();
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
fn nullable_sibling_drops_lookbehind_gate() {
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
fn word_boundary_nullable_composition() {
    let re = resharp::Regex::new(r"\ba{0}\b").unwrap();
    assert_eq!(
        re.is_match(b"").unwrap(),
        false,
        r"\ba{{0}}\b on empty: expected false"
    );
    let re = resharp::Regex::new(r"\Ba{0}\z").unwrap();
    assert_eq!(
        re.is_match(b"").unwrap(),
        true,
        r"\Ba{{0}}\z on empty: expected true"
    );
}

#[test]
fn is_match_fwd_prefix_not_quadratic() {
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
fn always_nullable_greedy_fast_path_linear_on_large_input() {
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
        assert!(
            !re.is_hardened(),
            "pattern {pat:?} should not be auto-hardened"
        );
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
fn find_all_not_quadratic_on_always_nullable() {
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
fn lookahead_in_lookbehind_rejected() {
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
fn optional_anchor_before_class_same_matches() {
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
    let re_anchored_opt =
        Regex::with_options(r"(?=x)?y", resharp::RegexOptions::default()).unwrap();
    let re_bare_y = Regex::with_options(r"y", resharp::RegexOptions::default()).unwrap();
    assert_eq!(
        re_anchored_opt.find_all(b"xyz yyy").unwrap(),
        re_bare_y.find_all(b"xyz yyy").unwrap(),
        "(?=x)?y and y should produce identical matches"
    );
}

#[test]
fn universal_class_matches_full_codepoint_in_unicode_modes() {
    use resharp::UnicodeMode;
    let ms = |p: &str, h: &[u8], mode: UnicodeMode| -> Vec<(usize, usize)> {
        Regex::with_options(p, resharp::RegexOptions::default().unicode(mode))
            .unwrap()
            .find_all(h)
            .unwrap()
            .iter()
            .map(|m| (m.start, m.end))
            .collect()
    };
    let euro = "\u{20AC}".as_bytes();
    assert_eq!(euro.len(), 3);
    for mode in [UnicodeMode::Javascript, UnicodeMode::Full] {
        assert_eq!(
            ms(r"[\s\S]", euro, mode),
            vec![(0, 3)],
            "[\\s\\S] must consume one codepoint in {mode:?}"
        );
        assert_eq!(
            ms(r"[\s\S]", euro, mode),
            ms(r".", euro, mode),
            "[\\s\\S] must agree with . in {mode:?}"
        );
        assert_eq!(ms(r"%([\dA-F]{2})|[\s\S]", euro, mode), vec![(0, 3)]);
        assert_eq!(
            ms(r"[\s\S]{2}", "\u{20AC}\u{20AC}".as_bytes(), mode),
            vec![(0, 6)]
        );
        assert_eq!(ms(r"[\s\S]*", euro, mode), vec![(0, 3), (3, 3)]);
        assert_eq!(
            ms(r"[\s\S]*", &[0xFFu8, 0x80, b'a'], mode),
            vec![(0, 0), (1, 1), (2, 3), (3, 3)],
            "[\\s\\S]* is valid-UTF-8 constrained (over-approx), not byte-universal, in {mode:?}"
        );
    }
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default] {
        assert_eq!(
            ms(r"[\s\S]", euro, mode),
            vec![(0, 1), (1, 2), (2, 3)],
            "byte modes unchanged: {mode:?}"
        );
    }
}

#[test]
fn find_anchored_respects_leading_assertion_at_begin() {
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
fn end_before_begin_anchor_matches_empty_string() {
    let re = Regex::new(r"\z\A").unwrap();
    assert_eq!(
        re.is_match(b"").unwrap(),
        true,
        "\\z\\A must match empty string"
    );
    assert_eq!(
        re.is_match(b"x").unwrap(),
        false,
        "\\z\\A must not match non-empty"
    );
    assert_eq!(
        re.find_all(b"").unwrap(),
        vec![resharp::Match { start: 0, end: 0 }]
    );
    let re2 = Regex::new(r"\za*\A").unwrap();
    assert_eq!(
        re2.is_match(b"").unwrap(),
        true,
        "\\za*\\A must match empty string"
    );
    assert_eq!(
        re2.is_match(b"a").unwrap(),
        false,
        "\\za*\\A must not match non-empty"
    );
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
fn hardened_complement_find_all_skips_longer_match() {
    check_hardened_vs_normal("~(.*and.*)", b"__A and B");
}

#[test]
fn mutex_poison_does_not_brick_regex() {
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
    let pats = [
        "(?=^#{1,4}\\s)",
        "(?=^##\\s)",
        "(?=^---\\s+\\S)",
        "(?=^@@ )",
        "(?=^##? )",
    ];
    let hay = "lorem ipsum dolor sit amet ".repeat(4000).into_bytes();
    for pat in pats {
        let re = Regex::with_options(pat, RegexOptions::default().multiline(false)).unwrap();
        assert!(re.is_fwd_begin_anchored(), "pat={pat} not begin-anchored");
        assert!(
            re.find_all(&hay).unwrap().is_empty(),
            "pat={pat} false match"
        );
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
    assert_eq!(
        re.find_all(b"notes").unwrap(),
        vec![resharp::Match { start: 3, end: 5 }]
    );
}

#[test]
fn end_anchored_with_leading_lookbehind() {
    let re = Regex::with_options(
        r"\b(Ant[o\xc2\xba]?[.]?[o\xc2\xba]?)\z",
        RegexOptions::default().multiline(false),
    )
    .unwrap();
    assert_eq!(re.find_all_kind_name(), "EndAnchored");
    assert_eq!(
        re.find_all(b"x Anto").unwrap(),
        vec![resharp::Match { start: 2, end: 6 }]
    );
    assert_eq!(re.find_all(b"xAnto").unwrap(), vec![]);
    assert_eq!(
        re.find_all(b"Ant.").unwrap(),
        vec![resharp::Match { start: 0, end: 4 }]
    );
    assert_eq!(
        re.find_all(b"foo Ant").unwrap(),
        vec![resharp::Match { start: 4, end: 7 }]
    );
    assert_eq!(re.find_all(b"foo Anto bar").unwrap(), vec![]);

    let wb = Regex::with_options(r"\bcat\z", RegexOptions::default().multiline(false)).unwrap();
    assert_eq!(wb.find_all_kind_name(), "EndAnchored");
    assert_eq!(
        wb.find_all(b"a cat").unwrap(),
        vec![resharp::Match { start: 2, end: 5 }]
    );
    assert_eq!(wb.find_all(b"scat").unwrap(), vec![]);
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn not_word_boundary_drops_consecutive_matches() {
    for mode in [
        resharp::UnicodeMode::Ascii,
        resharp::UnicodeMode::Javascript,
        resharp::UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"\Bx", opts).unwrap();
        let ms: Vec<[usize; 2]> = re
            .find_all(b"axx")
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        assert_eq!(ms, vec![[1, 2], [2, 3]], "\\Bx on 'axx' mode={mode:?}");

        let re2 = Regex::with_options(r"\B[A-Z]", RegexOptions::default().unicode(mode)).unwrap();
        let ms2: Vec<[usize; 2]> = re2
            .find_all(b"README")
            .unwrap()
            .iter()
            .map(|m| [m.start, m.end])
            .collect();
        assert_eq!(
            ms2,
            vec![[1, 2], [2, 3], [3, 4], [4, 5], [5, 6]],
            "\\B[A-Z] on 'README' mode={mode:?}"
        );
    }
}

#[test]
fn empty_match_byte_offsets_vs_utf8_intersection() {
    let body = r"((([A-Za-z]+(-[\dA-Za-z]+){0,2})|\*)(;q=[01](\.\d+)?)?)*";
    let inp = "Bootstrap\u{2019}s form".as_bytes();

    let raw = Regex::new(body).unwrap();
    let got: Vec<[usize; 2]> = raw
        .find_all(inp)
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(
        got,
        vec![
            [0, 9],
            [9, 9],
            [10, 10],
            [11, 11],
            [12, 13],
            [13, 13],
            [14, 18],
            [18, 18]
        ]
    );

    let aligned = Regex::new(&format!(r"({body})&\p{{utf8}}*")).unwrap();
    let got: Vec<[usize; 2]> = aligned
        .find_all(inp)
        .unwrap()
        .iter()
        .map(|m| [m.start, m.end])
        .collect();
    assert_eq!(
        got,
        vec![
            [0, 9],
            [9, 9],
            [10, 10],
            [11, 11],
            [12, 13],
            [13, 13],
            [14, 18],
            [18, 18]
        ]
    );
}

#[test]
fn reentrant_union_rewrite_does_not_panic() {
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
fn rev_trivial_assert_routes_through_dfa() {
    let m = |s: usize, e: usize| resharp::Match { start: s, end: e };
    let cases: &[(&str, &[u8], Vec<resharp::Match>)] = &[
        (r"_*$", b"\n\xfe*\xfe_*", vec![m(0, 6), m(6, 6)]),
        (r"_*$", b"abc", vec![m(0, 3), m(3, 3)]),
        (r"_*$", b"", vec![m(0, 0)]),
        (r"_*(?!_)", b"aa", vec![m(0, 2), m(2, 2)]),
    ];
    for (p, hay, want) in cases {
        let re = Regex::new(p).unwrap();
        assert_eq!(
            re.find_all_kind_name(),
            "Dfa",
            "pattern {p:?} routing changed"
        );
        assert_eq!(
            &re.find_all(hay).unwrap(),
            want,
            "rev_trivial find_all wrong for {p:?} on {hay:?}"
        );
    }
}
#[test]
fn rev_trivial_vs_regex_crate_oracle() {
    let cases: &[(&str, &str)] = &[
        (r"_*$", r"(?s).*$"),
        (r".*$", r".*$"),
        (r"[a-z]*$", r"[a-z]*$"),
        (r"\w*$", r"\w*$"),
        (r"[0-9]*$", r"[0-9]*$"),
    ];
    let hays: &[&[u8]] = &[
        b"",
        b"a",
        b"abc",
        b"a\nb",
        b"\n\n",
        b"aXb\ncd",
        b"123\n456\n",
        b"\n",
        b"aaaa",
        b"a\nb\nc\n",
        b"zz\nzz",
        b"\xfe\x00\xff",
        b"abc\ndef",
        b"\n\xfe*\xfe_*",
        b"hello world\nfoo bar baz\n",
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
            let want: Vec<[usize; 2]> = oracle
                .find_iter(hay)
                .map(|m| [m.start(), m.end()])
                .collect();
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
                got_no_adj_empty,
                want,
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
    for (pat, bound) in [
        (r"[^\n\r]{0,10}", 10),
        (r"[^\n\r]{0,40}", 40),
        (r"[^\n\r]{0,80}", 80),
    ] {
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
fn disable_prefixes_also_disables_bounded() {
    let pat = r"[^\n\r]{0,10}";
    let normal = Regex::with_options(pat, RegexOptions::default()).unwrap();
    assert_eq!(normal.find_all_kind_name(), "Bounded", "pat={pat}");
    let opts = RegexOptions { disable_prefixes: true, ..RegexOptions::default() };
    let disabled = Regex::with_options(pat, opts).unwrap();
    assert_ne!(disabled.find_all_kind_name(), "Bounded", "pat={pat}");
    let hay = b"abcdefghijklmnop";
    assert_eq!(
        normal.find_all(hay).unwrap(),
        disabled.find_all(hay).unwrap(),
        "pat={pat}"
    );
}

#[test]
fn bounded_range_with_nullable_alt_no_overrun() {
    use resharp::{RegexOptions, UnicodeMode};
    let cases: &[(&str, &str, &[(usize, usize)])] = &[
        (
            "c{2,3}ba|c?",
            "cccb",
            &[(0, 1), (1, 1), (2, 1), (3, 0), (4, 0)],
        ),
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
#[ignore = "slow; run with --ignored"]
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
    let other_pats = [
        r".+", r"[\s\S]+", r"\d+", r"[-_.]+", r"a+", r"\s*", r"abc", r"\bx",
    ];
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
            let hay: Vec<u8> = (0..len)
                .map(|_| alpha[(rng() as usize) % alpha.len()])
                .collect();
            let a: Vec<(usize, usize)> = rs
                .find_all(&hay)
                .unwrap()
                .iter()
                .map(|m| (m.start, m.end))
                .collect();
            let b: Vec<(usize, usize)> = rx.find_iter(&hay).map(|m| (m.start(), m.end())).collect();
            assert_eq!(a, b, "pat={p:?} hay={:?}", String::from_utf8_lossy(&hay));
        }
    }
}

#[test]
fn lowerbound_1_repeat_after_overlapping_prefix() {
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

#[test]
fn regex_instance_not_poisoned_after_match() {
    let re = Regex::new(r"([\(,])\s+|\s+([\),])").unwrap();
    assert!(re.is_match(b"a, b").unwrap());
    for _ in 0..6 {
        assert_eq!(re.find_all(b"a, b").unwrap().len(), 1);
    }
    let re2 = Regex::new(r"([\(,])\s+|\s+([\),])").unwrap();
    assert_eq!(re2.find_all(b"a, b").unwrap().len(), 1);
    assert_eq!(re2.find_all(b"zzzz").unwrap().len(), 0);
    assert_eq!(re2.find_all(b"a, b").unwrap().len(), 1);
}

#[test]
fn two_branch_lookbehind_no_superlinear_blowup() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    use std::time::Instant;

    let opts = RegexOptions::default().unicode(UnicodeMode::Javascript);
    let re = Regex::with_options(r"(?<!(</?[^>]*|\&[^;]*))([^\s<]+)", opts).unwrap();
    let hay: &[u8] = b"the quick brown fox jumps over the lazy dog while a small server \
runs inside an emulator or on a remote test device using the client program \
which connects to this server over a socket and performs various tasks such \
as reading writing files copying binaries running commands checking status \
waiting";

    let expected: &[(usize, usize)] = &[
        (0, 3),
        (4, 9),
        (10, 15),
        (16, 19),
        (20, 25),
        (26, 30),
        (31, 34),
        (35, 39),
        (40, 43),
        (44, 49),
        (50, 51),
        (52, 57),
        (58, 64),
        (65, 69),
        (70, 76),
        (77, 79),
        (80, 88),
        (89, 91),
        (92, 94),
        (95, 96),
        (97, 103),
        (104, 108),
        (109, 115),
        (116, 121),
        (122, 125),
        (126, 132),
        (133, 140),
        (141, 146),
        (147, 155),
        (156, 158),
        (159, 163),
        (164, 170),
        (171, 175),
        (176, 177),
        (178, 184),
        (185, 188),
        (189, 197),
        (198, 205),
        (206, 211),
        (212, 216),
        (217, 219),
        (220, 227),
        (228, 235),
        (236, 241),
        (242, 249),
        (250, 258),
        (259, 266),
        (267, 275),
        (276, 284),
        (285, 291),
        (292, 299),
    ];

    let ms = re.find_all(hay).unwrap();
    let spans: Vec<(usize, usize)> = ms.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(spans, expected);

    let t = Instant::now();
    re.find_all(hay).unwrap();
    let elapsed = t.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "find_all on a {}-byte haystack took {elapsed:?}",
        hay.len()
    );

    let long_hay = hay.repeat(20);
    let long_hay = &long_hay[..700.min(long_hay.len())];
    let t2 = Instant::now();
    re.find_all(long_hay).unwrap();
    let elapsed2 = t2.elapsed();
    assert!(
        elapsed2.as_millis() < 3000,
        "find_all on a {}-byte haystack took {elapsed2:?}",
        long_hay.len()
    );
}

#[test]
fn lookbehind_optional_atom_overlapping_run() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?<=.b)x?", opts).unwrap();
        let ms = re.find_all(b"abb").unwrap();
        let spans: Vec<(usize, usize)> = ms.into_iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(spans, vec![(2, 2), (3, 3)], "mode={mode:?}");

        let re2 = Regex::with_options(r"(?<=.b)-?", RegexOptions::default().unicode(mode)).unwrap();
        let spans2: Vec<(usize, usize)> = re2
            .find_all(b"abbbc")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans2, vec![(2, 2), (3, 3), (4, 4)], "mode={mode:?}");
    }
}
#[test]
fn nested_lookahead_in_neg_lookbehind_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?<!(?=a)x)", opts).unwrap();
        let ms = re.find_all(b"a").unwrap();
        let spans: Vec<(usize, usize)> = ms.into_iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(spans, vec![(0, 0), (1, 1)], "mode={mode:?}");

        let re2 = Regex::with_options(r"(?<!ab)", RegexOptions::default().unicode(mode)).unwrap();
        let spans2: Vec<(usize, usize)> = re2
            .find_all(b"c")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans2, vec![(0, 0), (1, 1)], "mode={mode:?}");
    }
}
#[test]
fn double_negated_lookahead_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?!(?=b))", opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"ab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 0), (2, 2)], "mode={mode:?}");

        let opts = RegexOptions::default().unicode(mode);
        match Regex::with_options(r"(?!(?!(?=b)))", opts) {
            Err(resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)) => {}
            other => panic!("mode={mode:?} unexpected result: {}", other.is_ok()),
        }
    }
}
#[test]
fn negated_lookahead_nested_in_lookahead_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?=.(?!(?=a)))", opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"aa")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(1, 1)], "mode={mode:?}");
    }
}
#[test]
fn bounded_repeat_of_failing_lookbehind_group_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?:(?<=.))?-", opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"b-")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(
            spans,
            vec![(1, 2)],
            "mode={mode:?} single-optional must stay supported"
        );

        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"((?<=.)){2,2}-", opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"bb-")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(
            spans,
            vec![(2, 3)],
            "mode={mode:?} exact-repeat (no optionality) must stay supported"
        );

        for pat in [r"((?<=.)){0,2}-", r"((?<=.)){0,3}-"] {
            let opts = RegexOptions::default().unicode(mode);
            match Regex::with_options(pat, opts) {
                Err(resharp::Error::Parse(_)) => {}
                other => panic!(
                    "mode={mode:?} pat={pat:?} unexpected result: {}",
                    other.is_ok()
                ),
            }
        }
    }
}

#[test]
fn lookahead_plus_star_plus_fixed_tail_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!:.)b*...";
        let re = Regex::with_options(pat, opts).unwrap();
        let fr = fancy_regex::Regex::new(pat).unwrap();
        for input in ["a:b:", "abcde", "abc", "a:b:c:d"] {
            let spans: Vec<(usize, usize)> = re
                .find_all(input.as_bytes())
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            let expected: Vec<(usize, usize)> = fr
                .find_iter(input)
                .map(|m| {
                    let m = m.unwrap();
                    (m.start(), m.end())
                })
                .collect();
            assert_eq!(spans, expected, "mode={mode:?} input={input:?}");
        }
        let spans: Vec<(usize, usize)> = re
            .find_all(b"a:b:")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 3)], "mode={mode:?}");
    }
}

#[test]
fn unbounded_star_repeated_literal_optional_atom_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r".*b.?a";
        let re = Regex::with_options(pat, opts).unwrap();
        let rr = regex::Regex::new(pat).unwrap();
        for input in ["bab", "bba", "xbabx", "bbbabab"] {
            let spans: Vec<(usize, usize)> = re
                .find_all(input.as_bytes())
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            let expected: Vec<(usize, usize)> =
                rr.find_iter(input).map(|m| (m.start(), m.end())).collect();
            assert_eq!(spans, expected, "mode={mode:?} input={input:?}");
        }
        let spans: Vec<(usize, usize)> = re
            .find_all(b"bab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 2)], "mode={mode:?}");
    }
}

#[test]
fn optional_atom_after_negative_lookahead_backoff_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"b?(?!a).?", opts).unwrap();
        for (input, expected) in [
            ("ba", vec![(0, 1), (2, 2)]),
            ("b", vec![(0, 1), (1, 1)]),
            ("bc", vec![(0, 2), (2, 2)]),
            ("bba", vec![(0, 2), (3, 3)]),
        ] {
            let spans: Vec<(usize, usize)> = re
                .find_all(input.as_bytes())
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            assert_eq!(spans, expected, "mode={mode:?} input={input:?}");
        }
    }
}

#[test]
fn optional_prefix_before_lookahead_with_nested_negative_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"x?(?=y(?!a))";
        let re = Regex::with_options(pat, opts).unwrap();
        let ms = re.find_all(b"cyba").unwrap();
        for m in &ms {
            assert!(
                m.start <= m.end,
                "mode={mode:?} invalid span {:?}",
                (m.start, m.end)
            );
        }
        let spans: Vec<(usize, usize)> = ms.into_iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(spans[0], (1, 1), "mode={mode:?}");
    }
}

#[test]
fn optional_atom_before_optional_negative_lookahead_group() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"b?((?!c))?";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"bc")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans[0], (0, 1), "mode={mode:?}");
    }
}

#[test]
fn leading_negative_lookahead_optional_literal_negative_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!a)\.?(?!.a).{0,2}";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"x.aa")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 2), (4, 4)], "mode={mode:?}");
    }
}

#[test]
fn bounded_quantifier_optional_atom_lookahead_nested_negative() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r".{2,3}x?(?=(?!zy)a)";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"baa-b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans[0], (0, 2), "mode={mode:?}");
    }
}

#[test]
fn leading_negative_lookahead_optional_dot_optional_positive_lookahead_group() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!a).?((?=b))?";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"ba")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 1), (2, 2)], "mode={mode:?}");
    }
}

#[test]
fn two_char_negative_lookahead_before_unbounded_atom_trailing_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!c.).+(?=a)";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"-ab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 1)], "mode={mode:?}");
    }
}

#[test]
fn optional_atom_literal_dot_plus_correct_leftmost_start() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r".?:..+";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"x::yz")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 5)], "mode={mode:?}");
    }
}

#[test]
fn unbounded_left_overlapping_single_char_literal_still_matches() {
    let pat = r"[a-z]+=[^\s]\S+";
    let opts = RegexOptions::default().unicode(resharp::UnicodeMode::Javascript);
    let re = Regex::with_options(pat, opts).unwrap();
    let got = re.find_all(b"x==aa").unwrap();
    assert_eq!(got, vec![resharp::Match { start: 0, end: 5 }]);
}

#[test]
fn negative_lookahead_star_full_backoff_to_empty() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!x).*(?=aa)";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"aab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 0)], "mode={mode:?}");
    }
}

#[test]
fn negative_lookahead_star_lookahead_star_drops_leftmost_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?!a):*(?=b)b*";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"b:b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 1), (1, 3)], "mode={mode:?}");
    }
}

#[test]
fn nested_positive_lookahead_inside_negative_lookbehind_false_negative() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?<!(?=y)b):";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"x:")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(1, 2)], "mode={mode:?}");
    }
}

#[test]
fn tautological_lookbehind_literal_start_dropped() {
    use resharp::Regex;
    let re = Regex::new(r"(?<=\A_*):").unwrap();
    assert_eq!(re.collect_rev_nulls_debug(b"x:"), vec![1]);
}

#[test]
fn nested_positive_lookahead_inside_lookbehind_drops_second_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?<!(?=a):):";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"x:y:")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(1, 2), (3, 4)], "mode={mode:?}");
    }
}

#[test]
fn bounded_quantifier_prefix_literal_optional_dot_plus_suffix_correct_leftmost_start() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r".{1,3}b.?a+";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"xxxbba")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 6)], "mode={mode:?}");
    }
}

#[test]
fn neg_lookahead_two_byte_body_optional_atom_trailing_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?!ba).?c*(?=.)", opts).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"b..")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 1), (1, 2), (2, 2)], "mode={mode:?}");
    }
}

#[test]
fn optional_lookbehind_group_after_unrelated_lookbehind_matches_correct_literal_byte() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"(?<!-)((?<=:))?b";
        let re = Regex::with_options(pat, opts).unwrap();
        let hay: &[u8] = b":b";
        let spans: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(1, 2)], "mode={mode:?}");
    }
}

#[test]
fn variable_range_quantified_positive_lookahead_group_consumes_a_character() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r"((?!b))a+((?=.)){1,2}";
        let re = Regex::with_options(pat, opts).unwrap();
        let hay: &[u8] = b"acx";
        let spans: Vec<(usize, usize)> = re
            .find_all(hay)
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 1)], "mode={mode:?}");
    }
}

#[test]
fn optional_atom_flanked_by_neg_lookahead_and_pos_lookahead_total_false_negative() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?!x)a?(?=-)", opts).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"-")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 0)], "mode={mode:?}");
    }
}

#[test]
fn optional_atom_flanked_by_two_neg_lookaheads_drops_leftmost_zero_width_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(r"(?!x)a?(?!x)", opts).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 0), (1, 1)], "mode={mode:?}");

        let opts2 = RegexOptions::default().unicode(mode);
        let re2 = Regex::with_options(r"(?!x)a?(?!x)", opts2).unwrap();
        let got2: Vec<(usize, usize)> = re2
            .find_all(b"bb")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got2, vec![(0, 0), (1, 1), (2, 2)], "mode={mode:?}");
    }
}

#[test]
fn interior_neg_lookahead_between_two_optional_dots_correct_leftmost_start() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options(".?(?!x).?(?!.)", opts).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"ab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 2), (2, 2)], "mode={mode:?}");
    }
}

#[test]
fn double_negated_lookahead_plus_star_plus_trailing_lookahead_total_false_negative() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default] {
        let opts = RegexOptions::default().unicode(mode);
        let re = Regex::with_options("(?!(?!.).)a*(?=b)", opts).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"b.")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 0)], "mode={mode:?}");
    }
    for mode in [UnicodeMode::Javascript, UnicodeMode::Full] {
        let opts = RegexOptions::default().unicode(mode);
        let err = Regex::with_options("(?!(?!.).)a*(?=b)", opts);
        assert!(err.is_err(), "mode={mode:?}");
    }
}

#[test]
fn optional_atom_literal_two_char_lookahead_total_false_negative_ascii_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let opts = RegexOptions::default().unicode(mode);
        let pat = r".?:(?=..).";
        let re = Regex::with_options(pat, opts).unwrap();
        let spans: Vec<(usize, usize)> = re
            .find_all(b"a:bb")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(spans, vec![(0, 3)], "mode={mode:?}");
    }
}

#[test]
fn quantified_prefix_lookahead_optional_atom_trailing_neg_lookahead_total_false_negative() {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re =
            Regex::with_options("a*(?=aa)-?(?!xy)", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"aaa")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 1), (1, 1)], "mode={mode:?}");
    }
}

#[test]
fn quantified_prefix_lookahead_then_star_lookahead_over_extends_past_failed_tail_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re =
            Regex::with_options("a+(?=a.)a*(?=a)", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"aa.")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 1)], "mode={mode:?}");
    }
}

#[test]
fn optional_trailing_lookahead_after_fixed_lookahead_and_optional_atom_does_not_infinite_recurse_at_compile_time(
) {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re =
            Regex::with_options("(?=aa)a?(?=a)?", RegexOptions::default().unicode(mode)).unwrap();
        let get = |hay: &[u8]| -> Vec<(usize, usize)> {
            re.find_all(hay)
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect()
        };
        assert_eq!(get(b"aaa"), vec![(0, 1), (1, 2)], "mode={mode:?} hay=aaa");
        assert_eq!(get(b"aa"), vec![(0, 1)], "mode={mode:?} hay=aa");
        assert_eq!(get(b"a"), vec![], "mode={mode:?} hay=a");
        assert_eq!(get(b""), vec![], "mode={mode:?} hay=empty");
        assert_eq!(get(b"-"), vec![], "mode={mode:?} hay=-");
    }
}

#[test]
fn optional_trailing_lookahead_after_two_char_group_lookahead_and_dotstar_does_not_infinite_recurse_at_compile_time(
) {
    use resharp::{Regex, RegexOptions, UnicodeMode};

    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options("(?=(.)(-)).*(?=:)?", RegexOptions::default().unicode(mode))
            .unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"b-:acc:ba:b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 11)], "mode={mode:?}");
    }
}

#[test]
fn negative_lookbehind_followed_by_optional_positive_lookbehind_does_not_lose_exclusion() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re =
            Regex::with_options("(?<!c)(?<=c)?a", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"ca")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, Vec::<(usize, usize)>::new(), "mode={mode:?}");

        let re2 =
            Regex::with_options("(?<!c)(?<=c)?a", RegexOptions::default().unicode(mode)).unwrap();
        let got2: Vec<(usize, usize)> = re2
            .find_all(b"xa")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got2, vec![(1, 2)], "mode={mode:?}");
    }

    for mode in [UnicodeMode::Ascii, UnicodeMode::Default] {
        let re =
            Regex::with_options("(?<!.)((?<=.))?.", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"ba")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 1)], "mode={mode:?}");
    }
}

#[test]
fn convergence_resume_boundary_finds_earlier_literal_occurrence() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r":[^c:].+(?=[^ac])", RegexOptions::default().unicode(mode))
            .unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b":-:-c:")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 5)], "mode={mode:?}");
    }
}
#[test]
fn optional_fixed_repeat_prefix_then_class_literal_plus_dotstar_finds_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r"(a{3})?[^a:]ba+.+", RegexOptions::default().unicode(mode))
            .unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"b-bab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(1, 5)], "mode={mode:?}");
    }
}
#[test]
fn optional_prefix_class_then_bounded_repeat_with_gap_and_trailing_star_finds_leftmost() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(
            r"[^bc]?-*b{2}.?b*[c:]",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"bbbc")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 4)], "mode={mode:?}");
    }
}

#[test]
#[cfg(feature = "convergence_prefix")]
fn convergence_prefix_no_open_bracket_does_not_hang_or_false_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let re = Regex::with_options(
        r"\[([a-z-]+)\s+([^\]]+)\]",
        RegexOptions::default()
            .unicode(UnicodeMode::Ascii)
            .force_convergence(true),
    )
    .unwrap();
    assert!(re.uses_convergence_prefix());
    let got: Vec<(usize, usize)> = re
        .find_all(b"]]")
        .unwrap()
        .into_iter()
        .map(|m| (m.start, m.end))
        .collect();
    assert_eq!(got, Vec::<(usize, usize)>::new());
}

#[test]
fn nullable_colon_star_prefix_then_bounded_repeat_plus_optional_atom_finds_leftmost_in_ascii() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r":*b{3}[^a]?[^b:]+", RegexOptions::default().unicode(mode))
            .unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"bbbbc")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 5)], "mode={mode:?}");
    }
}

#[test]
fn negative_lookahead_then_optional_atom_then_lookahead_prefers_longer_leftmost_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re =
            Regex::with_options(r"^(?!aa).?(?=.)", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"ab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 1)], "mode={mode:?}");

        let re2 =
            Regex::with_options(r"(?!aa).?(?=.)", RegexOptions::default().unicode(mode)).unwrap();
        let got2: Vec<(usize, usize)> = re2
            .find_all(b"ab")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got2, vec![(0, 1), (1, 1)], "mode={mode:?}");
    }
}

#[test]
#[ignore = "time based test"]
fn nested_bounded_repeat_of_bounded_repeat_compiles_in_bounded_time() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        for n in [1, 4] {
            let pat = format!("((b+(.+.+){{2,2}}){{1,{n}}}(.+){{2,2}})?x");
            let t0 = std::time::Instant::now();
            let re = Regex::with_options(&pat, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("mode={mode:?} n={n}: compile failed: {e:?}"));
            let elapsed = t0.elapsed();
            assert!(
                elapsed.as_secs() < 5,
                "mode={mode:?} n={n}: compile took {elapsed:?}; nested bounded repeats \
                 must not cause exponential compile-time blowup"
            );
            let _ = re.find_all(b"x").unwrap();
        }
    }
}

#[test]
#[ignore = "time based test"]
fn lookahead_plus_star_wrapped_nested_bounded_repeat_compiles_in_bounded_time() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        for n in [1, 4] {
            let pat = format!("(?=b)(((.*.+a?){{0,2}}bb){{1,{n}}})*");
            let t0 = std::time::Instant::now();
            let re = Regex::with_options(&pat, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("mode={mode:?} n={n}: compile failed: {e:?}"));
            let elapsed = t0.elapsed();
            assert!(
                elapsed.as_secs() < 5,
                "mode={mode:?} n={n}: compile took {elapsed:?}; a leading lookahead plus an \
                 unbounded-star-wrapped nested bounded repeat must not cause exponential \
                 compile-time blowup"
            );
            let _ = re.find_all(b"bbb").unwrap();
        }
    }
}

#[test]
fn doubly_nested_lookahead_with_trivially_satisfied_lookbehind_finds_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r".(?=.(?=(?<=.)b)):", RegexOptions::default().unicode(mode))
            .unwrap_or_else(|e| panic!("mode={mode:?}: compile failed: {e:?}"));
        let got: Vec<(usize, usize)> = re
            .find_all(b"b:b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 2)], "mode={mode:?}");
    }
}

#[test]
fn bounded_repeat_with_variance_of_a_lookahead_fused_with_a_trailing_optional_atom_is_rejected() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        for pat in [
            r".(?=((?=.+)[a.]?){1,2})",
            r".(?=((?=.+)[a.]?){2,3})",
            r"(?=((?!aab)[^b]){1,2}.)",
        ] {
            let err = match Regex::with_options(pat, RegexOptions::default().unicode(mode)) {
                Ok(_) => panic!("mode={mode:?} pat={pat:?}: expected a compile-time rejection, not a silent wrong match"),
                Err(e) => e,
            };
            assert!(
                format!("{err:?}").contains("UnsupportedResharpRegex"),
                "mode={mode:?} pat={pat:?}: got {err:?}"
            );
        }
    }
}

#[test]
fn bounded_repeat_variance_on_a_plain_lookahead_or_exact_count_on_a_fused_lookahead_still_matches()
{
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        for (pat, hay, expected) in [
            (r".(?=.+){1,2}", b"cb".as_slice(), vec![(0usize, 1usize)]),
            (r".(?=((?=.+)[a.]?){2,2})", b"cb".as_slice(), vec![(0, 1)]),
            (r".(?=((?=.+)[a.]?){2})", b"cb".as_slice(), vec![(0, 1)]),
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("mode={mode:?} pat={pat:?}: compile failed: {e:?}"));
            let got: Vec<(usize, usize)> = re
                .find_all(hay)
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            assert_eq!(got, expected, "mode={mode:?} pat={pat:?}");
        }
    }
}

#[test]
fn lookahead_containing_lookbehind_at_its_own_start_still_matches() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r"(?=(?<=.)b)", RegexOptions::default().unicode(mode))
            .unwrap_or_else(|e| panic!("mode={mode:?}: compile failed: {e:?}"));
        let got: Vec<(usize, usize)> = re
            .find_all(b"b:b")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(2, 2)], "mode={mode:?}");
    }
}

#[test]
fn convergence_prefix_seeds_window_past_trailing_lookahead_content() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r".*:[^.](?=.[^b])", RegexOptions::default().unicode(mode))
            .unwrap_or_else(|e| panic!("mode={mode:?}: compile failed: {e:?}"));
        let got: Vec<(usize, usize)> = re
            .find_all(b":a.a")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 2)], "mode={mode:?}");
    }
}

#[test]
fn nested_bounded_repeat_of_negated_class_lookahead_finds_leftmost() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        for (pat, hay, expected) in [
            (
                r"(?:.+(?::?(?!:)){2}){2}b",
                b"aab".as_slice(),
                vec![(0usize, 3usize)],
            ),
            (
                r"(?:[^b](?:a?(?!:)){2}){2}b",
                b"aab".as_slice(),
                vec![(0, 3)],
            ),
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("mode={mode:?} pat={pat:?}: compile failed: {e:?}"));
            let got: Vec<(usize, usize)> = re
                .find_all(hay)
                .unwrap()
                .into_iter()
                .map(|m| (m.start, m.end))
                .collect();
            assert_eq!(got, expected, "mode={mode:?} pat={pat:?}");
        }
    }
}

#[test]
fn optional_atom_then_two_unbounded_plus_finds_leftmost_via_backoff() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(r"ba?c+.+", RegexOptions::default().unicode(mode)).unwrap();
        let got: Vec<(usize, usize)> = re
            .find_all(b"bacc")
            .unwrap()
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect();
        assert_eq!(got, vec![(0, 4)], "mode={mode:?}");
    }
}

#[test]
fn convergence_prefix_hardened_no_quadratic() {
    use resharp::{Regex, RegexOptions};
    let re = Regex::with_options("x[ax]*c", RegexOptions::default().hardened(true)).unwrap();
    assert!(re.is_hardened());

    let small = "x".repeat(64_000).into_bytes();
    let large = "x".repeat(1_024_000).into_bytes();

    let time_it = |input: &[u8]| -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            re.find_all(input).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
        }
        best.max(1e-9)
    };

    let small_elapsed = time_it(&small);
    let large_elapsed = time_it(&large);

    let ratio = large_elapsed / small_elapsed;
    assert!(
        ratio < 40.0,
        "expected roughly linear scaling, got {ratio}x for 16x input (small={small_elapsed}s large={large_elapsed}s)"
    );
}

#[test]
fn hardened_trailing_dotstar_after_dangerous_prefix_no_quadratic() {
    use resharp::{Regex, RegexOptions};
    let re = Regex::with_options("x[ax]*c(.|\n)*", RegexOptions::default().hardened(true)).unwrap();
    assert!(re.is_hardened());

    let small = "x".repeat(64_000).into_bytes();
    let large = "x".repeat(1_024_000).into_bytes();

    let time_it = |input: &[u8]| -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            re.find_all(input).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
        }
        best.max(1e-9)
    };

    let small_elapsed = time_it(&small);
    let large_elapsed = time_it(&large);

    let ratio = large_elapsed / small_elapsed;
    assert!(
        ratio < 40.0,
        "expected roughly linear scaling, got {ratio}x for 16x input (small={small_elapsed}s large={large_elapsed}s)"
    );
}

#[test]
#[cfg(feature = "convergence_prefix")]
fn convergence_reverse_pass_no_quadratic_full_unicode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let re = Regex::with_options(
        r"[^a]*a.{1,4}b.+",
        RegexOptions::default().hardened(true).unicode(UnicodeMode::Full),
    )
    .unwrap();
    assert_eq!(re.prefix_kind_name(), Some("Convergence"));

    let small = vec![b'b'; 5_000];
    let large = vec![b'b'; 40_000];

    let time_it = |input: &[u8]| -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            re.find_all(input).unwrap();
            best = best.min(t.elapsed().as_secs_f64());
        }
        best.max(1e-9)
    };

    let small_elapsed = time_it(&small);
    let large_elapsed = time_it(&large);

    let ratio = large_elapsed / small_elapsed;
    assert!(
        ratio < 16.0,
        "expected roughly linear scaling, got {ratio}x for 8x input (small={small_elapsed}s large={large_elapsed}s)"
    );
}

#[test]
fn caret_literal_inside_lookbehind_matches_via_begin_path_every_letter() {
    use resharp::UnicodeMode;
    let opts = || RegexOptions::default().unicode(UnicodeMode::Ascii);
    for c in b'a'..=b'z' {
        let pat = format!("(?<=^{}).+", c as char);
        let hay = format!("{}yz", c as char);
        let re = Regex::with_options(&pat, opts()).unwrap();
        assert_eq!(
            re.find_all(hay.as_bytes()).unwrap(),
            vec![resharp::Match { start: 1, end: 3 }],
            "c={:?}",
            c as char
        );
    }
}

#[test]
fn bounded_repeat_of_nullable_group_compiles_in_linear_time() {
    use resharp::UnicodeMode;
    use std::time::Instant;
    let pat = "(?:a?){60}";
    let t0 = Instant::now();
    let re = Regex::with_options(pat, RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    assert!(
        t0.elapsed().as_millis() < 200,
        "compiling {pat} took {:?}",
        t0.elapsed()
    );
    assert!(re.is_match(b"aaaa").unwrap());
}

#[test]
fn named_capture_around_lookbehind_before_a_caret_false_match() {
    use resharp::Regex;
    let cases: &[(&str, &[u8])] = &[
        (r"(?P<g0>(?<=b))\A^b", b"b"),
        (r"(?P<g0>(?<=b))\A^.", b"b"),
        (r"(?P<g0>(?<=b))\A^b*", b"b"),
        (r"(?P<g0>(?<=b))\A^b", b"bb"),
        (r"(?P<g0>(?<=b))\A(?P<g1>^.*)", b"b"),
        (r"(?P<g0>(?<=b))\A(?P<g1>^b)", b"b"),
        (
            r"(?P<g0>(?<=:+[-]{2,2}))\A\A(?P<g1>^.*)",
            b"bccc-cbb:..",
        ),
        (r"(?P<g0>(?<=:--))\A(?P<g1>^.*)", b"bccc-cbb:.."),
    ];
    for &(p, inp) in cases {
        let re = Regex::new(p).unwrap_or_else(|e| panic!("compile {p}: {e}"));
        assert!(!re.is_match(inp).unwrap(), "is_match {p} on {inp:?}");
        assert_eq!(
            re.find_all(inp).unwrap(),
            Vec::new(),
            "find_all {p} on {inp:?}"
        );
    }
}

#[test]
fn unicode_mode_must_not_change_capture_spans_on_ascii_input() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8])] = &[
        (r"(?:.|..)(?P<g1>.*)", b"abcd"),
        (r"(?:.|..){2}(?P<g1>.*)", b"abcd"),
        (
            r"(?:.|.{1,4}.{1}){2,3}(?P<g1>.*(?P<g0>a{1})?)",
            b"acac-aa--aba",
        ),
        (r"(?P<g0>(?:.?|.(?:.*|a*.?)*){1})b{0,2}.*-*", b":c.::::-b-:"),
    ];
    for &(p, inp) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            let spans = caps.first().map(|c| c.spans().to_vec());
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}

#[test]
fn outer_lookahead_wrapping_literal_then_nested_lookahead_alt_z_matches_in_all_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let p = r"(?=x(?:(?=.)a|\z))";
    let inp: &[u8] = b"x";
    for mode in modes {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
            .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
        let matches = re
            .find_all(inp)
            .unwrap_or_else(|e| panic!("find_all {p} ({mode:?}) on {inp:?}: {e}"));
        assert_eq!(
            matches.len(),
            1,
            "{p} on {inp:?} in {mode:?}: expected one zero-width match at 0, got {matches:?}"
        );
        assert_eq!(matches[0].start, 0);
        assert_eq!(matches[0].end, 0);
    }
}

#[test]
fn implicit_captures_rejects_same_patterns_as_explicit_named_captures() {
    use resharp::{Regex, RegexOptions};
    let unsupported_bodies = [r"(?:(?<y>a))*"];
    for body in unsupported_bodies {
        let named = format!("(?P<g0>{body})");
        assert!(
            Regex::new(&named).is_err(),
            "expected {named:?} to be rejected at compile time (sanity check on the test itself)"
        );
        let implicit = format!("({body})");
        let re = Regex::with_options(&implicit, RegexOptions::default().implicit_captures(true));
        assert!(
            re.is_err(),
            "{implicit:?} under implicit_captures(true) must be rejected at compile time just like {named:?} is, not compile then fail at match time"
        );
    }
}


#[test]
fn alternation_branch_credit_with_disjoint_tags_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8])] = &[
        (r"(?:(?=.)(?P<g1>(?!b))|(?P<g2>(?![^a]+)))a", b"a"),
        (
            r"(?P<g0>-.+a{1,1})?(?:(?=.+\z){2}(?P<g1>(?!b))|b?(?P<g2>(?![^a.]+))(?P<g3>b?))(?:(?:.+|\z.*)+a|.*.{0}\.+)",
            b"a.::a.babaa",
        ),
        (r"(?:(?=.)(?P<g1>(?!b))|(?P<g2>(?![^a]{1,3})))a", b"a"),
        (r"(?:(?=.)(?P<g1>(?!b))|(?P<g2>(?![^a]{2,})))a", b"a"),
        (r"(?:(?P<g1>(?!b))|(?P<g2>(?![^ab]+))|(?P<g3>(?!c)))a", b"a"),
        (r"x?(?:(?=.)(?P<g1>(?!b))|(?P<g2>(?![^a]+)))a", b"xa"),
    ];
    for (p, inp) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(inp).unwrap();
            let spans: Vec<Vec<Option<(usize, usize)>>> =
                caps.iter().map(|c| c.spans().to_vec()).collect();
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}

#[test]
fn repro_capture_loses_trailing_atom_before_neg_lookahead() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[
        (r"(?P<g0>(?:b|a{0,2}).)(?!x)", b"bc"),
        (r"(?P<g0>(?:b|a*).)(?!x)", b"bc"),
        (r"(?P<g0>(?:b|a{0,3}).)(?!x)", b"bc"),
        (r"(?P<g0>(?:bb|a{0,2}).)(?!x)", b"bbc"),
    ];
    for (p, inp) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let all = re.find_all(inp).unwrap();
            let caps = re.captures_all(inp).unwrap();
            assert_eq!(all[0].end, inp.len(), "{p} on {inp:?} ({mode:?}): whole match must span the input");
            assert_eq!(
                caps[0].spans()[1],
                Some((0, inp.len())),
                "{p} on {inp:?} ({mode:?}): g0 must span the whole match like find_all does: {caps:?}"
            );
        }
    }
}

#[test]
fn isolation_boundary_cases_still_correct() {
    use resharp::Regex;
    let re = Regex::new(r"(?P<g0>(?:b|a?).)(?!x)").unwrap();
    let caps = re.captures_all(b"bc").unwrap();
    assert_eq!(caps[0].spans()[1], Some((0, 2)));

    let re = Regex::new(r"(?P<g0>(?:b|a{0,2})c)(?!x)").unwrap();
    let caps = re.captures_all(b"bc").unwrap();
    assert_eq!(caps[0].spans()[1], Some((0, 2)));

    let re = Regex::new(r"(?P<g0>(?:b|a{0,2}).)(?=y)").unwrap();
    let caps = re.captures_all(b"bxy").unwrap();
    assert_eq!(caps[0].spans()[1], Some((0, 2)));
}

#[test]
fn unsatisfiable_lookahead_branch_is_pruned_regardless_of_unicode_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
        (r".?(?=(?:\z|(?=b)[^b]))", b"b", &[(0, 1), (1, 1)]),
        (r".?(?=\z|(?=b)[^b])", b"b", &[(0, 1), (1, 1)]),
        (r".?(?:\z|(?=b)[^b])", b"b", &[(0, 1), (1, 1)]),
        (r".?(?=(?:\z|(?=a)[^a]))", b"b", &[(0, 1), (1, 1)]),
        (r"(?=(?:\z|(?=b)[^b]))", b"b", &[(1, 1)]),
    ];
    for &(p, inp, expected) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let m = re.find_all(inp).unwrap();
            let spans: Vec<(usize, usize)> = m.iter().map(|x| (x.start, x.end)).collect();
            assert_eq!(
                spans, expected,
                "{p:?} on {inp:?} ({mode:?}): expected {expected:?}, got {spans:?}"
            );
        }
    }
}

#[test]
fn zero_width_lookahead_alternative_participates_past_an_unrelated_dead_sibling_branch() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[
        (r"(?:.|(?:[^cb]{2}|(?P<g1>(?=a+))))?.+", b"ac"),
        (r"(?:.|[^cb]{2}|(?P<g1>(?=a+)))?.+", b"ac"),
        (r"(?:[^cb]{2}|(?P<g1>(?=a+)))?.+", b"ac"),
        (r"(?:[^c]{2}|(?P<g1>(?=a+)))?.+", b"ac"),
        (r"(?:.|(?P<g1>(?=a+)))?a", b"a"),
        (r"(?:.|(?P<g1>(?=a+)))?.", b"a"),
    ];
    for (p, inp) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(inp).unwrap();
            assert_eq!(
                caps[0].spans()[1],
                Some((0, 0)),
                "{p} on {inp:?} ({mode:?}): g1 must participate - the `.`/`[^cb]{{2}}` sibling \
                 arm either is dead on this input or ties on total length only via \
                 arm-order-dependent backtracking (glibc/fancy-regex flip their answer under \
                 arm-swap for this exact shape, disqualifying them as oracles here); the original \
                 expectation (decline) was based on V8's zero-width-iteration guard; got {:?}",
                caps[0].spans()
            );
        }
    }
}

#[test]
fn zero_width_lookahead_capture_tied_against_class_branch_participates_regardless_of_unicode_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], (usize, usize), (usize, usize))] = &[
        (r"-+(?:(?P<g0>(?!b{3}))|[^:a]*.[^b.]+-+)?.", b"-ba", (0, 2), (1, 1)),
        (r"(?P<g1>(?![^a].))?[^a]", b"b", (0, 1), (0, 0)),
    ];
    for &(p, inp, overall, group) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(inp).unwrap();
            assert_eq!(
                caps[0].spans(),
                &[Some(overall), Some(group)],
                "{p:?} on {inp:?} ({mode:?}): expected group to participate consistently across \
                 modes (the sibling class-branch arm is either dead on this input or a bare \
                 `X?`-wrapped zero-width group with no viable sibling at all, so participation \
                 wins; the original expectation, decline, was based on V8's zero-width-iteration \
                 guard, not POSIX)"
            );
        }
    }
}

#[test]
fn optional_prefix_before_negative_lookahead_capture_participates_consistently_across_unicode_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[&str] = &[
        r"(?:.(?=[^b])|a?(?P<g1>(?!.a)))?bb",
        r"(?:.(?=[^b])|(?P<g1>(?!.a)))?bb",
    ];
    for p in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"bb").unwrap();
            assert_eq!(
                caps[0].spans(),
                &[Some((0, 2)), Some((0, 0))],
                "{p:?} on \"bb\" ({mode:?}): expected g1 to participate consistently across modes \
                 (corrected in an earlier fix: glibc regexec and fancy-regex both agree \
                 that entering an optional wrapper to match zero-width, capturing a tag, beats \
                 declining the wrapper entirely - the original expectation here was based on V8, \
                 which has an ECMAScript-specific RepeatMatcher zero-width-iteration guard not \
                 shared by POSIX or other backtracking engines)"
            );
        }
    }
}

#[test]
fn unsatisfiable_lookbehind_with_a_anchor_never_matches_regardless_of_unicode_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let p = r"(?P<g0>(?<=.\A))(?<!\Bc)\.a";
    for mode in modes {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let m = re.find_all(b".a").unwrap();
        assert!(
            m.is_empty(),
            "{p:?} on \".a\" ({mode:?}): (?<=.\\A) is unsatisfiable (\\A only holds at position 0, but a preceding byte requires position >= 1), so there should be no match; got {m:?}"
        );
    }
}

#[test]
fn negative_lookahead_capture_at_non_zero_search_start_participates_regardless_of_unicode_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let p = r"[^.a](?:(?P<g1>\..{2}-?)?(?P<g2>(?!b+c))b?|.{1})?.{2}";
    for mode in modes {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"b-.ccbbbb").unwrap();
        assert_eq!(
            caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>(),
            vec![vec![Some((0, 4)), None, Some((1, 1))], vec![Some((4, 8)), None, Some((5, 5))]],
            "{p:?} on \"b-.ccbbbb\" ({mode:?}): g2 must participate at the same offset in BOTH \
             matches - (1,1) and (5,5) - consistently across modes. g1's arm cannot match at \
             either offset, so g1 stays None."
        );
    }
}

#[test]
fn negative_lookahead_capture_at_nonzero_search_start_participates_consistently() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let p = r"(?P<g2>(?:a?(?P<g0>(?=.))|(?P<g1>(?![^b])))?).{2,3}(?P<g3>ca*)?";
    for mode in modes {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a-.abb:a").unwrap();
        assert_eq!(
            caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>(),
            vec![
                vec![Some((0, 4)), Some((0, 1)), Some((1, 1)), None, None],
                vec![Some((4, 7)), Some((4, 4)), Some((4, 4)), Some((4, 4)), None],
            ],
            "{p:?} on \"a-.abb:a\" ({mode:?}): all modes must agree. For the second match g0's and g1's \
             arms both zero-width-participate at 4, an exact tie, so under UNION semantics both \
             report (4,4) - no arm is picked. The FIRST match is not a tie: that arm consumes a byte \
             before g0, so the longer end wins on position (rule 6a) and g1 stays out. Verified \
             invariant by swapping the two inner arms. The original expectation (only g0) came from \
             fancy-regex, a backtracking engine that rule 6 disqualifies for these ties."
        );
    }
}

#[test]
fn declined_leading_optional_does_not_poison_a_later_optional_capture() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: [(&str, &[u8], usize, &[Option<(usize, usize)>]); 2] = [
        (r"b?a(?P<g0>.)?b?", b"ab", 0, &[Some((0, 2)), Some((1, 2))]),
        (r"a?(?P<g0>[.a])?.?[^.]*", b".a.-c.", 0, &[Some((0, 2)), Some((0, 1))]),
    ];
    for (p, hay, idx, expected) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(
                caps[idx].spans(),
                expected,
                "{p:?} on {:?} ({mode:?}): a declined leading optional atom must not make a later \
                 optional capturing group wrongly decline too",
                String::from_utf8_lossy(hay)
            );
        }
    }
}

#[test]
fn directly_adjacent_bounded_optional_claims_its_own_byte() {
    use resharp::Regex;
    let re = Regex::new(r":?(?P<g0>:)?").unwrap();
    let caps = re.captures_all(b"-:").unwrap();
    assert_eq!(
        caps[1].spans(),
        &[Some((1, 2)), None],
        "the leading `:?` should claim the tied byte, not the trailing optional capture"
    );
}

#[test]
fn optional_capture_does_not_lose_a_tied_byte_to_a_later_bounded_optional_atom() {
    use resharp::Regex;
    let re = Regex::new(r"b*(?P<g0>.{1})?\.*:?").unwrap();
    let caps = re.captures_all(b"bba-bb:acb..").unwrap();
    assert_eq!(
        caps[2].spans(),
        &[Some((4, 7)), Some((6, 7))],
        "g0 should claim the tied ':' byte rather than the trailing bare `:?`"
    );
}

#[test]
fn optional_capture_with_bounded_range_body_after_mandatory_or_declined_quantifier() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(r"a+(?P<g0>.)?.{2,5}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abbb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 2))], "repro1 ({mode:?})");

        let re = Regex::with_options(r"a+(?P<g0>.{1,3})?.{2,5}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abbb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 2))], "repro1b ({mode:?})");

        let re = Regex::with_options(r"[^ca]*(?P<g1>.{2,3})?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b":ccc").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 3))], "repro2 ({mode:?})");
    }
}

#[test]
fn bounded_range_quantifiers_own_optional_tail_competes_with_a_following_optional_capture() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re =
            Regex::with_options(r"[a-c]{2,3}(?P<g0>[a-c])?[a-c]?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ccc:ca").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None], "repro1 ({mode:?})");

        let re =
            Regex::with_options(r".{1,2}(?P<g0>.)?[a-c]{0,2}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b".ca..").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), Some((2, 3))], "repro2 ({mode:?})");
    }
}

#[test]
fn unbounded_predecessor_risk_check_covers_multi_atom_optional_capture_bodies() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[
        (r"\.+(?P<g0>[^ab].)?.*", b"..babb"),
        (r"b+(?P<g0>.a)?.*", b"bbacc:"),
        (r":+(?P<g0>:a)?[ab]*", b"c:::ac"),
        (r"[^ab]+(?P<g0>[^ab]b)?.{0,2}", b"b::bb"),
    ];
    for mode in modes {
        for &(pat, hay) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}

#[test]
fn old_wide_predecessor_with_a_uniquely_placed_mandatory_atom_still_lets_the_following_optional_participate() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[Option<(usize, usize)>])] = &[
        (r".+-(?P<g0>c)?c*", b":-ca", &[Some((0, 3)), Some((2, 3))]),
        (r".+:(?P<g0>c{2})?(?P<g3>.{0,2})[^-c]", b"a:cc.c", &[Some((0, 5)), Some((2, 4)), Some((4, 4))]),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(
                caps[0].spans(),
                expected,
                "{pat:?} on {hay:?} ({mode:?}): the mandatory atom between the wide `.+` \
                 predecessor and the optional group occurs only once in this input, so there \
                 is no real donation ambiguity and the optional group must participate"
            );
        }
    }
}

#[test]
fn mandatory_class_atom_between_unbounded_leading_star_and_optional_capture_declines() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[(r".*.(?P<g0>.+)?", b"x:y"), (r".*.(?P<g0>.+)?.{0,2}", b"x:y")];
    for mode in modes {
        for &(pat, hay) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}

#[test]
fn residual_class_intervening_atom_between_unbounded_leading_star_and_optional_capture_declines() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[(r".*[^a](?P<g0>.+)?.{0,2}", b"x:y"), (r".*[^b](?P<g0>.+)?.{0,2}", b"x:y")];
    for mode in modes {
        for &(pat, hay) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}

#[test]
fn mandatory_atom_after_maximized_leading_star_declines_shorter_disjoint_split() {
    // `.*` (leftmost) maximizes to "x:", leaving g0 unset; rule 6a forbids
    // shortening it so g0 can participate. Verified against V8 and glibc.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[(r".*[^:](?P<g0>.+)?.{0,2}", b"x:y")];
    for mode in modes {
        for &(pat, hay) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}

#[test]
fn adjacent_tie_between_unbounded_leading_star_and_optional_capture_participates() {
    // A genuine two-way tie among *directly reachable* landing spots for
    // the leading `.*` (no disjoint occurrence, no flexible tail needed to
    // make the shift reachable). Verified against V8 in all 4 UnicodeModes.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], Option<(usize, usize)>)] = &[
        (r".*[^a](?P<g0>[a-z]+)?.{0,3}", b"zzaaa", Some((2, 5))),
        (r".*[^a](?P<g0>[a-z]+)?.{0,3}", b"zzzaaaa", Some((3, 7))),
        (r".*[^a](?P<g0>[a-z]+)?.{0,3}", b"zaaaaaa", Some((1, 7))),
        (r".*[^b](?P<g0>[a-z]+)?.{0,3}", b"bbaaa", None),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], expected, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}

#[test]
fn negative_lookahead_capture_participation_matches_across_unicode_modes() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let pat = r"(?:(?!a.)(?P<g0>c*))?.?..";
    let hay: &[u8] = b":ax";
    for mode in modes {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        assert_eq!(
            caps[0].spans()[1],
            Some((0, 0)),
            "pat={pat} hay={hay:?} mode={mode:?}: g0 must consistently participate across all \
             UnicodeModes (corrected in an earlier fix: the original expectation \
             here, None, was based on V8's ECMAScript-specific zero-width-iteration guard, not \
             true POSIX - glibc and fancy-regex both confirm participation wins)"
        );
    }
}

#[test]
fn exact_count_repeat_of_a_nullable_alternation_is_arm_order_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let pats = [r"(?:|:){2}(?P<g0>[^b])?", r"(?::|){2}(?P<g0>[^b])?"];
    let hay: &[u8] = b":b";
    let mut results = Vec::new();
    for mode in modes {
        for pat in pats {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            results.push(caps[0].spans().to_vec());
        }
    }
    let first = &results[0];
    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r, first,
            "result must not depend on UnicodeMode or on which alternation arm is written first \
             (RE#'s Union is unordered) - mismatch at index {i}: {results:?}"
        );
    }
}

#[test]
fn unrelated_disjoint_class_leading_star_does_not_stale_intervening_track_a_later_star() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8])] = &[
        (r".+(?P<g0>.)?", b"ac"),
        (r"x*.+(?P<g0>.)?", b"ac"),
        (r"[-]*.+(?P<g0>.)?", b"ac"),
        (r"a*.+(?P<g0>.)?", b"aac"),
    ];
    for mode in modes {
        for &(pat, hay) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} hay={hay:?} mode={mode:?}");
        }
    }
}


#[test]
fn shorter_union_arm_that_really_matches_contributes_its_captures() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let cases: &[(&str, &[u8])] = &[
        (r"....|(?P<g0>...)?(?P<g1>(?=.))", b"abcd"),
        (r"(?P<g0>...)?(?P<g1>(?=.))|....", b"abcd"),
        (r"xxxxx|....|(?P<g0>...)?(?P<g1>(?=.))", b"abcd"),
    ];
    for &(p, hay) in cases {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            let spans = caps[0].spans();
            assert_eq!(
                (spans[1], spans[2]),
                (Some((0, 3)), Some((3, 3))),
                "pattern={p} mode={mode:?} got {spans:?}: `....` sets the span (0,4), but the \
                 `(?P<g0>...)?(?P<g1>(?=.))` arm genuinely matches \"abc\" at 0 - it is a real \
                 accepting run of the whole pattern - so its groups participate. `|` is UNION: \
                 there is no losing branch to suppress. All three arm orderings must agree."
            );
        }
    }
}

#[test]
fn z_anchored_optional_capture_arm_vs_dash_literal_arm_matches_the_dot_dot_control() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[Option<(usize, usize)>])] = &[
        (r".+(?:(?P<g0>.?\z)|-.)", b"c-a", &[Some((0, 3)), Some((3, 3))]),
        (r".+(?:(?P<g0>.?\z)|x)", b"c-a", &[Some((0, 3)), Some((3, 3))]),
        (r".+(?P<g0>.?\z)", b"c-a", &[Some((0, 3)), Some((3, 3))]),
        (r".+(?:(?P<g0>.?\z)|-.)", b"cba", &[Some((0, 3)), Some((3, 3))]),
        (r".+(?:(?P<g0>.?\z)|..)", b"cba", &[Some((0, 3)), Some((3, 3))]),
        (
            r".+(?:(?P<g0>(?:[^:]{1}b?|a*)?\z)|-.*)",
            b"b--:.--.bc-.a",
            &[Some((0, 13)), Some((13, 13))],
        ),
    ];
    for (p, input, expected) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(input).unwrap();
            assert_eq!(
                caps[0].spans(),
                *expected,
                "pattern={p} mode={mode:?}: the leading `.+` is the leftmost element and is \
                 maximized first (rule 6(b)), so the trailing arm always wins the split and g0 \
                 participates zero-width at the end. glibc is NOT the oracle for the `-.`/`..` \
                 rows: on \"c-a\" it reports g1 (3,3) for `.+(.?|-.)$` and `.+(.?$|-.$)` but \
                 (1,3) for `.+(.?$|-.)` - all three have the same language and the same parse \
                 set, so merely factoring `$` in or out of the arms flips it, which rule 6 \
                 disqualifies. See scripts/posix_oracle.c"
            );
        }
    }
}

#[test]
fn dominant_star_wrapped_in_a_union_still_declines_the_trailing_optional_capture() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[Option<(usize, usize)>])] = &[
        (r"(?:.*|.)(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|.{1})(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|.{1,5})(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|x)(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|[a-z])(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|.?)(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:x|.*)(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:.*|.|.)(?P<g1>.)?a", b"ca", &[Some((0, 2)), None]),
        (r"(?:[^b]*|x)(?P<g0>a{2})?$", b"a--.aa", &[Some((0, 6)), None]),
    ];
    for (p, input, expected) in cases {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(input).unwrap();
            assert_eq!(
                caps[0].spans(),
                *expected,
                "pattern={p} mode={mode:?}: the weaker union arm can never reach a split point the \
                 dominant unbounded arm can't also reach, so the union is equivalent to the bare \
                 unbounded quantifier and the trailing optional capture must decline exactly as it \
                 does without the union wrapper"
            );
        }
    }
}

#[test]
fn star_of_union_body_capture_span_is_arm_order_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let patterns = [
        r"(?P<g1>(?:c?|.?)*a?).+",
        r"(?P<g1>(?:.?|c?)*a?).+",
        r"(?P<g1>(?:x?|.?)*a?).+",
        r"(?P<g1>(?:.?|x?)*a?).+",
    ];
    for p in patterns {
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"aab").unwrap();
            assert_eq!(
                caps[0].spans(),
                &[Some((0, 3)), Some((0, 2))],
                "pattern={p} mode={mode:?}: a star whose body is a 2-arm union must give the same g1 \
                 span regardless of which arm is written first (genuine set union) - glibc regexec \
                 was originally used as oracle here without checking this arm-swap invariance; \
                 glibc's own answer flips between (0,1) and (0,2) purely from swapping `c?`/`.?`'s \
                 textual order, disqualifying it as the oracle for this tie"
            );
        }
    }
}

#[test]
fn tied_union_arm_choice_declines_trailing_optional_capture_arm_order_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        for pat in [r"(?:.|.a)(?P<g0>.?)", r"(?:.a|.)(?P<g0>.?)"] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b":a").unwrap();
            assert_eq!(
                caps[0].spans(),
                &[Some((0, 2)), Some((2, 2))],
                "mode={mode:?} pattern={pat}: not a bug - the union's leading choice \
                 (whether to also consume the trailing 'a') is textually earlier than g0, so per rule \
                 6(b) (leftmost subexpression maximized first) it is settled before g0 gets a say, \
                 forcing g0 to decline; confirmed via glibc regexec on the arm-order-invariant \
                 common-prefix-factored form `.( a?|a|(|a) )(.?)` (all three phrasings agree: the \
                 earlier optional wins, g0 declines), which also matches resharp being arm-order \
                 independent here (unlike glibc/V8 on the raw un-factored `(.|.a)` form, which flips \
                 with arm order and is therefore not a valid oracle for the raw form)"
            );
        }
    }
}

#[test]
fn three_way_disjoint_tag_union_lets_every_tied_arm_participate() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    // A 3-way alternation chain parses as Union(A, Union(B, C)); all five
    // phrasings below must report the same participation regardless of how
    // the chain nests.
    for mode in modes {
        for pat in [
            r"(?:(?:(?:\-+(?:(?<g0>[^c.]))?)|(?<g1>[bc]))|(?<g2>(?:c)+))",
            r"(?:(?<g2>(?:c)+)|(?:(?:\-+(?:(?<g0>[^c.]))?)|(?<g1>[bc])))",
            r"(?:(?:\-+(?:(?<g0>[^c.]))?)|(?:(?<g1>[bc])|(?<g2>(?:c)+)))",
            r"(?:(?<g1>[bc])|(?:(?<g2>(?:c)+)|(?:\-+(?:(?<g0>[^c.]))?)))",
            r"(?:(?<g2>(?:c)+)|(?<g1>[bc])|(?:\-+(?:(?<g0>[^c.]))?))",
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"c").unwrap();
            assert_eq!(
                (caps[0].name("g1").map(|m| (m.start, m.end)), caps[0].name("g2").map(|m| (m.start, m.end))),
                (Some((0, 1)), Some((0, 1))),
                "mode={mode:?} pattern={pat}: on input \"c\" the g1 ([bc]) and g2 ((?:c)+) arms tie \
                 exactly - both match \"c\", same span (0,1). `|` is UNION, so there is no arm to \
                 pick: BOTH arms are live and BOTH groups participate. All five phrasings of this \
                 3-way chain must agree, which is automatic because merging is commutative and \
                 associative. Earlier revisions of this test asserted a single winner (g2 from the \
                 deleted compute_tag_rank, then g1 from a canonical surface key); both were wrong \
                 for the same reason - they read `|` as an ordered alternation."
            );
        }
    }
}


#[test]
fn tagged_zero_width_arm_wins_nullable_tie_against_untagged_arm_arm_order_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        for pat in [
            r"(?:.?)+(?:(?<g1>\z)|(?:z*|y))",
            r"(?:.?)+(?:(?:z*|y)|(?<g1>\z))",
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"").unwrap();
            assert_eq!(
                caps[0].name("g1").map(|m| (m.start, m.end)),
                Some((0, 0)),
                "mode={mode:?} pattern={pat}: on empty input, g1 (\\z) ties with the untagged\
                 3-way alternation's other two arms (z*|y, both reachable with zero-width via\
                 the shared nullable tie) and must win per participation-beats-non-participation,\
                 regardless of which of the tail union's three arms is written first"
            );
        }
    }
}

#[test]
fn equal_length_alternation_capturing_arm_wins_arm_order_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[Option<(usize, usize)>])] = &[
        (r".|(??.)", b"a", &[Some((0, 1)), Some((0, 1))]),
        (r"(??.)|.", b"a", &[Some((0, 1)), Some((0, 1))]),
        (r".|.|(??.)", b"a", &[Some((0, 1)), Some((0, 1))]),
        (r"(??.)|.|.", b"a", &[Some((0, 1)), Some((0, 1))]),
    ];
    for mode in modes {
        for (pat, input, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(input).unwrap();
            assert_eq!(
                caps[0].spans(),
                *expected,
                "mode={mode:?} pattern={pat}: on a genuine equal-length alternation tie (every arm \
                 matches the identical span), resharp deliberately prefers the arm that captures, \
                 regardless of its textual position - a principled, deterministic, arm-order-independent \
                 generalization of participation beats non-participation; glibc's answer for this shape \
                 (first alternative wins, capture-blind) is NOT used as an oracle here since it is \
                 itself arm-order-dependent (`.|(.)` vs `(.)|.` in ERE disagree)"
            );
        }
    }
}

#[test]
fn a_subset_arm_union_wrapping_a_dominant_star_is_equivalent_to_the_bare_star_not_a_real_bug() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let bare = Regex::with_options(r".*(?P<g0>.)?.{3}", RegexOptions::default().unicode(mode)).unwrap();
        let wrapped = Regex::with_options(r"(?:.?|.*)(?P<g0>.)?.{3}", RegexOptions::default().unicode(mode)).unwrap();
        let bare_caps = bare.captures_all(b"abba-").unwrap();
        let wrapped_caps = wrapped.captures_all(b"abba-").unwrap();
        assert_eq!(
            bare_caps[0].spans(),
            wrapped_caps[0].spans(),
            "mode={mode:?}: `.?` can never reach a split point `.*` can't also reach, so \
             `(?:.?|.*)` must behave exactly like bare `.*` - a \
             textual-form-dependent oracle disagreement here (V8) is invalid, not a bug"
        );
        assert_eq!(wrapped_caps[0].spans(), &[Some((0, 5)), None], "mode={mode:?}");
    }
}

#[test]
fn an_exact_count_repeated_union_before_a_hard_end_anchor_declines_the_trailing_capture_order_invariant_not_a_bug() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let a = Regex::with_options(r":(?:.?|.?.{2}){2}(?P<g0>a.*)?\z", RegexOptions::default().unicode(mode)).unwrap();
        let b = Regex::with_options(r":(?:.?.{2}|.?){2}(?P<g0>a.*)?\z", RegexOptions::default().unicode(mode)).unwrap();
        let caps_a = a.captures_all(b":a.b:b").unwrap();
        let caps_b = b.captures_all(b":a.b:b").unwrap();
        assert_eq!(
            caps_a[0].spans(),
            caps_b[0].spans(),
            "mode={mode:?}: swapping the anonymous union's arms must not change `g0`'s \
             participation (V8 and fancy-regex both flip on this swap, disqualifying \
             them as oracles here) - `g0` must decline in both \
             orders, since the textually-earlier anonymous union is maximized first \
             (leftmost-construct-first, rule 6b)"
        );
        assert_eq!(caps_a[0].spans(), &[Some((0, 6)), None], "mode={mode:?}");
    }
}

#[test]
fn negated_ascii_class_in_optional_group_unicode_mode_divergence() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = ".+(?P<g0>[^c])?(?P<g1>b.+)?";
    let hay = b"xxbxx";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, vec![vec![Some((0, 5)), None, None]], "mode={mode:?}");
    }
}

#[test]
fn negated_ascii_class_in_optional_group_unicode_mode_divergence_minimal() {
    // A mandatory unbounded `.+` directly followed by two optional captures,
    // the first a negated ASCII class. Under a unicode-aware mode, `[^c]`'s
    // automaton has a variable byte length (1-4 bytes, to cover any non-`c`
    // codepoint); on pure-ASCII input, where `[^c]` only ever takes its
    // 1-byte path, `g1` must still decline.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = ".+(?P<g0>[^c])?(?P<g1>b)?";
    let hay = b"xb";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, vec![vec![Some((0, 2)), None, None]], "mode={mode:?}");
    }
}
#[test]
fn quantified_negative_lookahead_bounded_class_corrupts_match_end() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = r"(?=(?![^a]{2})+)a?..";
    let hay = b"ca.a";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, vec![vec![Some((0, 2))], vec![Some((2, 4))]], "mode={mode:?}");
    }
}

#[test]
fn quantified_negative_lookahead_capture_panic() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = r"(?P<g0>(?=(?![^a]{2})+))a?..";
    let hay = b"ca.a";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(
            spans,
            vec![
                vec![Some((0, 2)), Some((0, 0))],
                vec![Some((2, 4)), Some((2, 2))]
            ],
            "mode={mode:?}"
        );
    }
}

#[test]
fn repeated_lookahead_all_repeat_kinds() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pats = [
        r"(?=(?![^a]{2}){3})a?..",
        r"(?=(?![^a]{2}){0,})a?..",
        r"(?=(?![^a]{2}){2})a?..",
        r"(?=(?![^a]{2}){2,3})a?..",
        r"(?=(?![^a]{2}){2,})a?..",
    ];
    for pat in pats {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Javascript, UnicodeMode::Full] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"ca.a").unwrap();
            let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
            assert_eq!(
                spans,
                vec![vec![Some((0, 2))], vec![Some((2, 4))]],
                "pat={pat:?} mode={mode:?}"
            );
        }
    }
}

#[test]
fn leading_star_before_tied_capture_arm_priority_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8])] = &[
        (r"(?:.*(?P<g0>\z)|x*(?P<g1>.))", b"b"),
        (r"[a]?(?:.*(?P<g0>\z)|-*(?P<g1>.+)b?){1}", b"bbabb:cb.cb"),
    ];
    for &(p, inp) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            let spans = caps.first().map(|c| c.spans().to_vec());
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}


#[test]
fn optional_dot_prefix_before_zero_width_tied_capture_arm_priority_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8])] = &[
        (r".(?:.?(?P<g0>(?=))|a?(?P<g1>))", b"b"),
        (r".(?:.?(?P<g0>(?=b{0}))|[a-]?(?P<g1>.{0,3})){1}", b"bc--c"),
        (r".?\.?(?:(?P<g0>(?!.+.{2,2}){3})|(?P<g1>(?=(?:.?|\.*.[cb]?)*(?!-?-+){3}))b?\A)", b""),
    ];
    for &(p, inp) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}

#[test]
fn dot_star_vs_bare_dot_alternation_tie_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8])] = &[
        (r"a+(?:(?P<g0>.*)|.)", b"aaa"),
        (r".+(?:(?P<g0>(?:b*|.)*)|.)", b"b::-"),
    ];
    for &(p, inp) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}

#[test]
fn bare_dot_first_arm_before_mandatory_capture_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Full,
        UnicodeMode::Javascript,
    ];
    let cases: &[(&str, &[u8], Option<(usize, usize)>)] = &[
        (r"a+(?:.|(?P<g0>.)).+", b"aabc", Some((2, 3))),
        (r"(?:\.?:{0,1}b+|a?)*(?:.|(?P<g0>.)?-{0,1}\.*).+", b"aac.:", Some((2, 3))),
    ];
    for &(p, inp, expected_g0) in cases {
        let mut results = Vec::new();
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
            assert_eq!(spans[0][1], expected_g0, "{p} on {inp:?}: {mode:?}");
            results.push((mode, spans));
        }
        let (first_mode, first_spans) = &results[0];
        for (mode, spans) in &results[1..] {
            assert_eq!(
                spans, first_spans,
                "{p} on {inp:?}: {mode:?} disagrees with {first_mode:?} ({spans:?} vs {first_spans:?})"
            );
        }
    }
}

#[test]
fn lookahead_fused_optional_group_before_alternation_declines_in_every_mode() {
    // V8 (`/^.+(?:(?<g0>(?=b{1,1})b{0}.{3})?a{1,2}|b+)[^b-]+.?/` on
    // `":.caba:ac:"`) also gives `g0=undefined`.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"^.+(?:(?P<g0>(?=b{1,1})b{0}.{3})?a{1,2}|b+)[^b-]+.?";
    let inp = b":.caba:ac:";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(inp).unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 10)), None], "{mode:?}");
    }
}

#[test]
fn word_boundary_in_optional_leading_group_participates_in_every_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    // Every case verified against the glibc `regexec` (POSIX ERE) oracle.
    // An optional group's own leading atom coinciding with the mandatory
    // tail's leading atom must not be mistaken for a genuine external
    // predecessor; the last two cases have a genuinely external predecessor
    // and must still decline.
    let cases: &[(&str, &[u8], &[Option<(usize, usize)>])] = &[
        (r"(?:.+(?P<g2>c\b))?.+\z", b"bac:a", &[Some((0, 5)), Some((2, 3))]),
        (r"(?:(?P<g0>.*)c)?.*d\z", b"acbbd", &[Some((0, 5)), Some((0, 1))]),
        (
            r"[^c]+(?P<g2>(?P<g1>(?!.+))[^bc]*b{1,4})?$",
            b"ab\nb",
            &[Some((0, 4)), None, None],
        ),
        (r"[^d]+(?P<g0>c)?.*d\z", b"abcbbd", &[Some((0, 6)), None]),
    ];
    for (p, inp, expected) in cases {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("compile {p} ({mode:?}): {e}"));
            let caps = re
                .captures_all(inp)
                .unwrap_or_else(|e| panic!("captures_all {p} ({mode:?}) on {inp:?}: {e}"));
            assert_eq!(caps[0].spans(), *expected, "{p} on {inp:?}: {mode:?}");
        }
    }
}

#[test]
fn zero_width_capture_in_optional_tail_prefers_participation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for p in [r".+(?:(?!.)|(?P<g0>$).?)?", r".+(?:(?P<g0>$).?|(?!.))?"] {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"aa").unwrap();
            assert_eq!(caps[0].spans(), &[Some((0, 2)), Some((2, 2))], "{p} {mode:?}");
        }
    }
}

#[test]
fn full_javascript_mode_does_not_overrun_past_the_input_length() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let cases: &[(&str, &[u8], &[[usize; 2]])] = &[
        (r"..(?:(?!b)|\z){1,2}", b"abc", &[[0, 2]]),
        (r"..(?:(?!b)|\z){1,2}", b"abcd", &[[0, 2], [2, 4]]),
        (
            r"(?P<g0>(?<=(?:(?<=[a.]*a{2,4})+(?:-?b|a+.)+|^){0})).{2}(?:(?!b{3}){2,2}|\z){1,2}",
            b"ab.b",
            &[[0, 2], [2, 4]],
        ),
    ];
    for (p, inp, expected_spans) in cases {
        for mode in [
            UnicodeMode::Ascii,
            UnicodeMode::Default,
            UnicodeMode::Full,
            UnicodeMode::Javascript,
        ] {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(inp).unwrap();
            let got: Vec<[usize; 2]> = caps
                .iter()
                .map(|c| {
                    let (lo, hi) = c.spans()[0].unwrap();
                    [lo, hi]
                })
                .collect();
            assert_eq!(got, *expected_spans, "{p} {:?} {mode:?}", String::from_utf8_lossy(inp));
        }
    }
}

#[test]
fn whole_match_must_equal_capture_wrapping_entire_pattern() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"(?P<g1>.(?:\z|(?=.)))";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aa").unwrap();
        for c in &caps {
            assert_eq!(c.spans()[0], c.spans()[1], "{mode:?} whole must equal g1");
        }
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn ascii_mode_agrees_with_default_full_javascript_on_optional_zero_width_alternation_tail() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"a\B.+(?:(?P<g1>(?!:))(?P<g2>(?=.?))|aa)?";
    let mut prev: Option<Vec<Option<(usize, usize)>>> = None;
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ab").unwrap();
        let got = caps[0].spans().to_vec();
        if let Some(p) = &prev {
            assert_eq!(&got, p, "{mode:?}");
        }
        prev = Some(got);
    }
}

#[test]
fn multiline_dollar_in_tail_alternation_does_not_flip_sibling_lookaround_participation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"b+(?:$.{2}|(?P<g0>(?!:)))?";
    let expected: Vec<Vec<Option<(usize, usize)>>> = vec![
        vec![Some((1, 2)), Some((2, 2))],
        vec![Some((3, 4)), None],
    ];
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b".b.b:c").unwrap();
        let got: Vec<Vec<Option<(usize, usize)>>> =
            caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(got, expected, "{mode:?}");
    }
}

#[test]
fn uncaptured_leading_plus_maximizes_before_empty_capture() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"b+(?:(?P<g0>b*)|.)";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bbb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), Some((3, 3))], "{mode:?}");
    }
}

#[test]
fn optional_dot_competing_with_capture_prefers_participation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"a*(?:.?|(?P<g0>[ab]*))";
    let expected = vec![
        vec![Some((0, 3)), Some((2, 3))],
        vec![Some((3, 3)), Some((3, 3))],
    ];
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aab").unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, expected, "{mode:?}");
    }
}

#[test]
fn tied_zero_width_lookahead_alternation_arms_pick_the_same_winner_in_every_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let cases: &[(&str, &[u8], &[Vec<Option<(usize, usize)>>])] = &[
        (
            r"(?P<g0>(?=a))|(?P<g1>(?=.))",
            b"a",
            &[vec![Some((0, 0)), Some((0, 0)), Some((0, 0))]],
        ),
        (
            r"(?:(?P<g0>(?=[b]?)).|(?P<g1>(?<=x?)).+)?.+",
            b"aa",
            &[vec![Some((0, 2)), Some((0, 0)), Some((0, 0))]],
        ),
    ];
    for (p, input, expected) in cases {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(input).unwrap();
            let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
            assert_eq!(
                spans, *expected,
                "{p} {mode:?}: the arms tie zero-width, and `|` is UNION - both arms are live, so \
                 BOTH groups participate. Identical in every mode and under swapping the two arms. \
                 Earlier revisions asserted a single winner (first the textually-first arm, then a \
                 canonical-key winner); both read `|` as an ordered alternation."
            );
        }
    }
}

#[test]
fn addendum_explicit_union_zero_width_arm_participates_in_every_mode() {
    // `g0` participates via a real accepting run at start 0: `.+` matches
    // "a" at position 1, leaving `g0`'s zero-width arm to match at 2. Matches
    // `fancy-regex`; V8's `null` is its own JS-specific rule, not POSIX.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r".+(?:(?P<g0>(?!.))|.+)?";
    let expected = vec![vec![Some((0, 2)), Some((2, 2))]];
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aa").unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, expected, "{mode:?}");
    }
}

#[test]
fn bare_lookahead_fused_with_begin_anchor_tail_corrupts_match_end() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = r"(?=(?![cb]{2})\A)[^c]{2}";
    let hay = b"bxx";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, vec![vec![Some((0, 2))]], "mode={mode:?}");
    }
}

#[test]
fn word_boundary_donation_to_trailing_optional_capture_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = r"a\b.+x?(?P<g0>.+)?";
    let hay = b"a:b";
    for mode in [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
        UnicodeMode::Javascript,
        UnicodeMode::Full,
    ] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        assert_eq!(spans, vec![vec![Some((0, 3)), None]], "mode={mode:?}");
    }
}

#[test]
fn word_boundary_donation_original_fuzzer_repro_is_unicode_mode_independent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pat = r"a+\b(?=.).+[ac]*(?P<g0>.*.+)?";
    let hay = b"..--aa.-aab";
    let mut results = Vec::new();
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Javascript, UnicodeMode::Full] {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        let spans: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
        results.push((mode, spans));
    }
    let first = &results[0].1;
    for (mode, spans) in &results[1..] {
        assert_eq!(spans, first, "mode={mode:?}");
    }
}

#[test]
fn grouped_multi_atom_negative_lookahead_before_begin_anchor_never_matches() {
    // `(?:(?!xy).)` must consume one byte, moving past `\A`'s only valid
    // position, so this can never match - but must compile.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"(?:(?!xy).)\A";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
            .unwrap_or_else(|e| panic!("{mode:?}: expected Ok, got {e:?}"));
        for hay in [&b""[..], &b"a"[..], &b"xy"[..], &b"ab"[..], &b"xyz"[..]] {
            assert_eq!(re.find_all(hay).unwrap(), vec![], "{mode:?} hay={hay:?}");
        }
    }
}

#[test]
fn bounded_optional_quantifier_does_not_donate_a_byte_to_trailing_optional_group() {
    // `a?` is greedy and can take the one available byte; if it does, the
    // following `(?P<g0>a{1,3})?` has nothing left and correctly declines.
    // V8/fancy-regex both agree: g0 = None. Same donation-decline mechanism
    // as the bare unbounded-quantifier case, but for a *bounded* (not
    // unbounded) preceding optional quantifier.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"a?(?P<g0>a{1,3})?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[0], Some((0, 1)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
    }

    // Any bounded (min=0) preceding quantifier reproduces (`a?`, `a{0,1}`,
    // `a{0,2}`, `a{0,3}`); an *unbounded* preceding quantifier is unaffected,
    // and the specific `.{1,2}`-style "own internal optional tail" shape
    // (mandatory rep immediately followed by an optional rep of the SAME
    // atom, e.g. inside `a{1,3}`'s own desugaring) is a distinct shape and
    // must not be conflated with the standalone bounded-optional case here.
    let also_decline: &[(&str, &[u8])] = &[
        (r"a{0,1}(?P<g0>a{1,3})?", b"a"),
        (r"a{0,2}(?P<g0>a{1,3})?", b"a"),
        (r"a{0,3}(?P<g0>a{1,3})?", b"a"),
    ];
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        for &(pat, hay) in also_decline {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(hay).unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} {mode:?}: {caps:?}");
        }
        // Unbounded preceding quantifiers: already correctly `None`,
        // must remain so.
        let re = Regex::with_options(r"a*(?P<g0>a{1,4})?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"a").unwrap()[0].spans()[1], None);
        let re = Regex::with_options(r"a+(?P<g0>a{1,4})?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"aa").unwrap()[0].spans()[1], None);
    }
}

#[test]
fn unbounded_quantifier_donates_a_byte_through_an_intervening_mandatory_atom() {
    // `c*` is greedy/unbounded but can only match `c`; on "ca" it naturally
    // stops after consuming the one `c`, leaving `.` to consume `a` and `g0`
    // nothing to claim. V8/fancy-regex both agree: g0 = None. Same donation
    // mechanism as the bounded case above, but with a mandatory, unrestricted
    // atom (`.`) sitting between the declining quantifier and the optional
    // capture, which a direct-adjacency-only guard would miss.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"c*.(?P<g0>a)?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ca").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
    }

    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        // Bounded (min=0) preceding quantifiers reproduce too.
        for pat in [r"c?.(?P<g0>a)?", r"c{0,3}.(?P<g0>a)?"] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b"ca").unwrap();
            assert_eq!(caps[0].spans()[1], None, "pat={pat} {mode:?}: {caps:?}");
        }
        // min>=1 preceding quantifiers must already correctly decline.
        for pat in [r"c+.(?P<g0>a)?", r"c{2,}.(?P<g0>a)?"] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode));
            if let Ok(re) = re {
                if let Ok(caps) = re.captures_all(b"cca") {
                    if !caps.is_empty() {
                        assert_eq!(caps[0].spans()[1], None, "pat={pat} {mode:?}: {caps:?}");
                    }
                }
            }
        }
        // An optional middle atom removes the ambiguity: no bug.
        let re = Regex::with_options(r"c*.?(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"ca").unwrap()[0].spans()[1], None);
        // A middle atom restricted away from the preceding quantifier's class: no bug.
        let re = Regex::with_options(r"c*[a]?(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"ca").unwrap()[0].spans()[1], None);
        // No middle atom at all (the original bare shape): g0 correctly participates,
        // since c* can never consume the 'a' anyway.
        let re = Regex::with_options(r"c*(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"ca").unwrap()[0].spans()[1], Some((1, 2)));
    }
}

#[test]
fn optional_capture_participates_via_a_shorter_accepting_run() {
    // `a*` (leftmost) maximizes to "a"; rule 6a forbids shortening it so
    // g2 can participate at 1. Verified against V8.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"a*.(?P<g2>(?![^b]+))?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ab-").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g2 participates via the shorter accepting run, got {caps:?}");
    }

    let p2 = r"a*.(?:(?::*|(?=[:c]{3}.*)(?P<g0>a{1}.?)?)|(?P<g2>(?P<g1>(?![^cb]+))a{0,3}))";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p2, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bab-bbacc..b").unwrap();
        assert_eq!(caps[1].spans()[0], Some((1, 3)), "{mode:?}: {caps:?}");
        assert_eq!(
            (caps[1].spans()[2], caps[1].spans()[3]),
            (None, None),
            "{mode:?}: `a*` is the leftmost subexpression and is already maximized (1 char) by \
             the winning (1,3) decomposition; shortening it to 0 to let g1/g2 zero-width-participate \
             at 2 is forbidden by rule 6a, same as `.+(?P<g0>.)?` on \"ac\". Verified against V8. \
             got {caps:?}"
        );
    }
}

#[test]
fn unicode_mode_must_not_change_capture_participation_for_pure_ascii_pattern() {
    // Pure-ASCII pattern and input: `UnicodeMode` must never change the
    // result. `Ascii`/`Default` (and V8) agree g0 must decline; `Full`/
    // `Javascript` wrongly let it participate.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r".{1,4}.{3}(?P<g0>.{2,5})?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a:.b.-").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 6)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
    }

    // Second, smaller repro: leading atom optional too, tie sits mid-pattern.
    let p2 = r".?(?P<g0>.{1,3})?.{1,3}";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p2, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bb").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
    }
}

#[test]
fn nested_optional_group_participates_consistently_across_unicode_modes() {
    // `.?(?P<g1>(?P<g0>b+.?)?[bc]+)?` on `"-bc"`: `g0` sits behind `g1`'s own
    // independent `?`, nested inside `g1`'s body. `g1`'s body can be split
    // either as `g0="b"` then `[bc]+="c"`, or `g0` declining and
    // `[bc]+="bc"` alone - a genuine tie, so `g0` participates. V8 agrees
    // (`Some((1, 2))`). Must hold in all four `UnicodeMode`s on this
    // pure-ASCII pattern+input.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r".?(?P<g1>(?P<g0>b+.?)?[bc]+)?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"-bc").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 3)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], Some((1, 3)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[2], Some((1, 2)), "{mode:?}: g0 should participate, got {caps:?}");
    }
}

#[test]
fn unbounded_predecessor_donation_survives_an_intervening_optional_star_of_a_different_class() {
    // `.+a*(?P<g0>.a*)?` on `"b:"`: `.+` greedily consumes both bytes; `a*`
    // (a different, unrelated class) can only ever match zero here, so it is
    // fully transparent - `g0` must decline through it too. V8/fancy-regex
    // agree. Shape here: a same-class `X*` sitting BOTH immediately after
    // the donor quantifier AND as the tail of the capture's own body.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r".+a*(?P<g0>.a*)?";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"b:").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
    }

    // Isolation matrix: an intervening OPTIONAL star that genuinely does
    // consume real content right before the capture's tag must NOT be
    // treated as transparent - the fallback only fires when the star
    // matched vacuously in this decomposition.
    let re = Regex::with_options(r"b*\.*(?P<g0>..)?.*", RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    let caps = re.captures_all(b".bc").unwrap();
    assert_eq!(caps[0].spans()[1], Some((1, 3)), "g0 should participate, got {caps:?}");

    // A farther unbounded predecessor of a different class must still be
    // able to donate through a nearer mandatory-then-unbounded atom that
    // ends in the same class as itself (the existing relocation model
    // must keep working unchanged alongside the new transparent-star path).
    let re = Regex::with_options(r".+[^b]+(?P<g1>.+.+)?", RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    let caps = re.captures_all(b"a-b:").unwrap();
    assert_eq!(caps[0].spans()[1], None, "g1 should decline, got {caps:?}");
}

#[test]
fn bounded_optional_before_optional_capture_does_not_falsely_compete_after_it_already_consumed() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r".?(?P<g0>.)?x*";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bx").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], Some((1, 2)), "{mode:?}: g0 should participate, got {caps:?}");
    }

    // Must generalize across a `{m,n}`-desugared chain of stacked bounded
    // optionals sharing one synthetic tag pair, and across a chain of
    // several DIFFERENT-class bounded predecessors in front of the capture.
    let re = Regex::with_options(r".{0,3}(?P<g0>.+)?[^a]+", RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    let caps = re.captures_all(b"abcdef").unwrap();
    assert_eq!(caps[0].spans()[1], Some((3, 5)), "g0 should participate, got {caps:?}");

    let re = Regex::with_options(r"[^b]*a{0,2}.?(?P<g0>:)?[^bb]{2,4}b+.+", RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    let caps = re.captures_all(b"ccab:-:b-").unwrap();
    assert_eq!(caps[0].spans()[1], Some((4, 5)), "g0 should participate, got {caps:?}");
}

#[test]
fn all_or_nothing_leading_optional_group_does_not_falsely_compete_with_a_following_optional_capture() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        // `(?:aa)?` has no partial value - it either consumes exactly
        // "aa" or exactly nothing. It must decline entirely for the
        // overall match to succeed here, and once declined it is fully
        // transparent: `g1` should get first crack at the freed byte,
        // leaving `a+` the second one, not the reverse.
        let re = Regex::with_options(r"(?:aa)?(?P<g1>a)?a+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aa").unwrap();
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}: {caps:?}");
        assert_eq!(caps[0].spans()[1], Some((0, 1)), "{mode:?}: g1 should participate, got {caps:?}");

        // Same shape, but the leading group is itself capturing - must
        // decline (no partial value forces it to give up entirely) while
        // `g1` still participates.
        let re = Regex::with_options(r"(?P<g0>aa)?(?P<g1>a)?a+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aa").unwrap();
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g0 should decline, got {caps:?}");
        assert_eq!(caps[0].spans()[2], Some((0, 1)), "{mode:?}: g1 should participate, got {caps:?}");

        // A *genuine* single-byte-granularity bounded optional predecessor
        // still wins the tie over the following optional capture, unlike the
        // all-or-nothing multi-byte case above.
        for pat in [r"a{0,1}(?P<g0>a{1,3})?", r"a{0,2}(?P<g0>a{1,3})?", r"a{0,3}(?P<g0>a{1,3})?", r"a?(?P<g0>a{1,3})?"] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            assert_eq!(re.captures_all(b"a").unwrap()[0].spans()[1], None, "{mode:?} pat={pat}");
        }
    }
}

#[test]
fn hash_consed_synthetic_quantifier_tag_not_misattributed_across_group_boundary() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        // `g1`'s body can only hold at end-of-string (`$`), which is
        // impossible here (a mandatory `.+` must follow) - it is forced
        // to always decline. `g2` must still correctly decline too, since
        // the first `.+` should greedily claim both `a` and `c`, leaving
        // only `b` for the second `.+`.
        let re = Regex::with_options(r"(?P<g1>$.*)?.+(?P<g2>c+)?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"acb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None, None], "{mode:?}: {caps:?}");

        // A non-trivial body inside the always-declining leading group
        // (still anchor-gated to end-of-string) reproduces identically.
        let re = Regex::with_options(r"(?P<g1>a*$.*)?.+(?P<g2>c+)?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"acb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None, None], "{mode:?}: {caps:?}");

        // A leading group anchored to \A instead of $ CAN meaningfully
        // compete at position 0 and correctly should participate.
        let re = Regex::with_options(r"(?P<g1>\A.*)?.+(?P<g2>c+)?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"acb").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), Some((0, 1)), None], "{mode:?}: {caps:?}");

        // The original already-fixed bare shape (no leading group at
        // all) must remain correct.
        let re = Regex::with_options(r".+(?P<g2>c+)?.+", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"acb").unwrap()[0].spans(), &[Some((0, 3)), None], "{mode:?}");

        // An unrelated forced-decline family repro must remain fixed.
        let re = Regex::with_options(r"a*(?P<g0>b)?b+", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"bb").unwrap()[0].spans(), &[Some((0, 2)), Some((0, 1))], "{mode:?}");
    }
}

#[test]
fn bounded_optional_predecessor_through_a_multi_atom_same_class_chain_still_competes() {
    // `[^b]?` (a genuine bounded, byte-granularity optional predecessor)
    // is separated from `g0` by `.{2}` - a CHAIN of two width-1 `.`
    // `Concat` steps, not a single atom. `[^b]?` claiming byte 0 and
    // `.{2}` claiming bytes 1-2 reaches the same total match length as
    // `[^b]?` declining and `.{2}` claiming bytes 0-1, leaving byte 2 for
    // `g0` - a genuine tie the earlier (leading) atom must win.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"[^b]?.{2}(?P<g0>.)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abc").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None], "{mode:?}: {caps:?}");

        // A longer same-class chain (three width-1 atoms, `.{3}`) must
        // extend the same way.
        let re = Regex::with_options(r"[^b]?.{3}(?P<g0>.)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abcd").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), None], "{mode:?}: {caps:?}");

        // The single-atom (chain length 1) shape must remain correct too.
        let re = Regex::with_options(r"[^c]?.(?P<g0>.)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // An UNBOUNDED-predecessor repro (through the same
        // `.{2}` chain shape, tied against a trailing unbounded absorber)
        // must still let `g0` participate.
        let re = Regex::with_options(r"a+.{2}(?P<g0>a)?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a:aaa").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 5)), Some((3, 4))], "{mode:?}: {caps:?}");
    }
}

#[test]
fn hash_consed_synthetic_quantifier_tag_shared_across_unrelated_concat_scopes_not_conflated() {
    // `.*` (top-level) and `g0`'s own leading-then-nested `[^.].*` share the
    // identical hash-consed `Star(dot)` `NodeId` for their own trailing
    // `.*`, but the two occurrences sit in unrelated `Concat` scopes
    // (top-level sibling vs. deep inside a completely different optional
    // capture's own body) and must not be conflated into shared runtime
    // bookkeeping. The leading `.*` is unbounded and greedy; it can (and
    // must) consume the whole `"bb"` prefix on its own, leaving nothing for
    // `g0` to claim.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r".*(?P<g0>[^.].*)?a", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bba").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None], "{mode:?}: {caps:?}");

        // `.+` in place of the leading `.*` reproduces identically.
        let re = Regex::with_options(r".+(?P<g0>[^.].*)?a", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bba").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None], "{mode:?}: {caps:?}");

        // `g0`'s own nested star as `.+` instead of `.*` reproduces too.
        let re = Regex::with_options(r".*(?P<g0>[^.].+)?a", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bbca").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), None], "{mode:?}: {caps:?}");

        // A mandatory leading `[^.]` ahead of the outer `.*` (so the shared
        // `Star(dot)` node's OUTER occurrence is not the very first atom in
        // the pattern) still must not leak the conflated tag.
        let re = Regex::with_options(r"[^.].*(?P<g0>[^.].*)?a", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bcbba").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 5)), None], "{mode:?}: {caps:?}");

        // The always-declining `g0` nested one level deeper inside another
        // capture (`g1`) must still resolve correctly - `g1` itself
        // genuinely participates (it wraps the mandatory trailing `a`).
        let re = Regex::with_options(r".*(?P<g1>(?P<g0>[^.].*)?a)", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"bba").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), Some((2, 3)), None], "{mode:?}: {caps:?}");
    }
}

#[test]
fn addendum_fix_must_not_break_legitimate_reuse_between_sibling_union_arms() {
    // Two arms of the SAME union carrying their own identical copy of a
    // hash-cons-shared tail (e.g. after a distributivity rewrite) have
    // IDENTICAL ancestry and are mutually exclusive, so aliasing their tag
    // registers is harmless - required, in fact, for the union's arm order
    // to stay irrelevant to which capture spans get reported. Full/
    // Javascript reject this shape outright (unrelated, pre-existing
    // `UnsupportedPattern` limitation) - only Ascii/Default accept it.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default] {
        for pat in [
            r"(?:.|(?=[^c.]))(?:[^c.][a-z])*(?P<g0>.)?",
            r"(?:(?=[^c.])|.)(?:[^c.][a-z])*(?P<g0>.)?",
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(b":").unwrap();
            assert_eq!(caps[0].spans(), &[Some((0, 1)), None], "{mode:?} {pat}: {caps:?}");
        }
    }
}

#[test]
fn trailing_quantifier_sharing_a_class_with_an_intervening_mandatory_atom_declines_when_ambiguous() {
    // `.+` (leftmost) maximizes to 4 chars, leaving nothing for `g0`; a
    // shorter `.+` giving g0 a span is forbidden by rule 6a. Same for `c*`
    // below. Verified against V8 and glibc.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r".+[bc](?P<g0>a{2})?b{0,2}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abaab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 5)), None], "{mode:?}: {caps:?}");

        // `?` (not just `{0,2}`) on the shared-literal trailing target
        // reproduces identically.
        let re = Regex::with_options(r".+[bc](?P<g0>a{2})?b?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abaab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 5)), None], "{mode:?}: {caps:?}");

        // The trailing quantifier's class being DISJOINT from the
        // intervening atom does NOT change anything: `c*` still maximizes
        // first and `g0` still declines.
        let re = Regex::with_options(r"c*.(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ca").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // Here there is no genuine ambiguity: `[bc]` has no other byte to
        // bind to (no more b/c in the input), so `.+`=1 is the only valid
        // decomposition and `g0` legitimately participates.
        let re = Regex::with_options(r".+[bc](?P<g0>a{2})?x{0,2}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abaax").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 5)), Some((2, 4))], "{mode:?}: {caps:?}");
    }
}

#[test]
fn negative_lookahead_with_a_single_byte_body_stays_transparent_to_predecessor_risk() {
    // `mk_neg_lookahead` compiles `(?!X)` into a complement-based rewrite
    // whose own min length is always 0 regardless of `X`'s actual width,
    // unlike a positive lookahead `(?=X)`. `[^b]+(?!z)(?P<g0>a)?` on
    // `"aab"`: `[^b]+` is unbounded and greedy and must claim both `a`s
    // itself, leaving nothing for `g0`, even though the zero-width `(?!z)`
    // in between looks like it could reset that.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"[^b]+(?!z)(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // A bounded-repeat body on `g0` reproduces identically.
        let re = Regex::with_options(r"[^b]+(?!z)(?P<g0>a{1,3})?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // A leading optional atom ahead of the unbounded quantifier must not
        // change anything.
        let re = Regex::with_options(r"\.?[^b]+(?!z)(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aab").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // The pre-existing positive-lookahead passthrough must
        // remain unaffected by this fix.
        let re = Regex::with_options(r".+(?=.).(?P<g0>.)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"aaa").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), None], "{mode:?}: {caps:?}");
    }
}

#[test]
fn optional_lookahead_before_unsatisfiable_width_lookbehind_never_matches() {
    // The lookbehind needs a 6-byte run (`a{3}.{3}`) that never occurs in
    // this 5-byte haystack, so no position can ever satisfy it - correct
    // answer (V8, and resharp's own Full/Javascript modes) is no match
    // anywhere. Ascii/Default instead reported a spurious 0-width match
    // at byte offset 8, past the end of the 5-byte haystack.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let p = r"(?=c)?(?<=a{3}.{3})";
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(p, RegexOptions::default().unicode(mode)).unwrap();
        let got = re.find_all(b"-.aaa").unwrap();
        assert!(got.is_empty(), "{mode:?}: {got:?}");
    }
}

#[test]
fn leading_bounded_optional_donation_risk_must_not_force_a_wide_range_optional_capture_to_fully_decline() {
    // `.?(?P<g0>.{2,4})?.+` on `"abcd"`: `.?` can donate at most ONE extra
    // byte of growth, while `g0` would need to give up TWO to reach a valid
    // decomposition if `.?` declined entirely - the resolution must reason
    // about the actual amount each side can give up, not just whether they
    // share a byte class, or it wrongly forces `g0` to fully decline and
    // throws away its otherwise-correct truncated width. V8 agrees `g0`
    // should participate with `Some((1, 3))`.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r".?(?P<g0>.{2,4})?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abcd").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 3))], "{mode:?}: {caps:?}");

        // A fixed-width body (range width 0) and a narrower range (width 1)
        // must give the identical answer.
        let re = Regex::with_options(r".?(?P<g0>.{2,2})?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abcd").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 3))], "{mode:?}: {caps:?}");

        let re = Regex::with_options(r".?(?P<g0>.{1,2})?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abcd").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((1, 3))], "{mode:?}: {caps:?}");

        // The shape above (where full decline really is correct, since
        // there's nothing left for `g0` at all once `a?` takes the one
        // available byte) must stay unaffected.
        let re = Regex::with_options(r"a?(?P<g0>a{1,3})?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 1)), None], "{mode:?}: {caps:?}");

        // A shape with a leading quantifier that has its OWN
        // flexibility, legitimately outranking `g0` and forcing a genuine
        // full decline) must also stay unaffected.
        let re = Regex::with_options(r".{1,4}.{3}(?P<g0>.{2,5})?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a:.b.-").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 6)), None], "{mode:?}: {caps:?}");
    }
}

#[test]
fn optional_single_char_capture_declines_when_a_trailing_bounded_repeat_follows_it() {
    // `.?.(?P<g0>:)?[^a-]{1,2}` on `"c::c"`: the mandatory `.` sitting
    // between the leading bounded-optional `.?` and `g0` is a genuine fixed
    // obstacle, not "more of the same optional unit" as `.?`. `.?` is
    // capped at exactly one extra byte of growth, ever - it must not be
    // treated as able to "relocate" an unlimited number of times the way a
    // real unbounded star legitimately can, inventing an unreachable
    // competing decomposition (`.?` growing to 2 bytes, which it can never
    // do) and wrongly forcing `g0` to fully decline. V8 agrees `g0` should
    // participate as `Some((2, 3))`.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r".?.(?P<g0>:)?[^a-]{1,2}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"c::c").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((2, 3))], "{mode:?}: {caps:?}");

        // Range width 1 must stay unaffected.
        let re = Regex::with_options(r".?.(?P<g0>:)?[^a-]{1,1}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"c::c").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 4)), Some((2, 3))], "{mode:?}: {caps:?}");

        // A wider corpus-shaped repro: same mechanism, a longer leading
        // chain (`b+.{0,3}[^.]`) and a wider trailing range (`{1,4}`).
        let re = Regex::with_options(r"b+.{0,3}[^.](?P<g0>:)?[^a-]{1,4}", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ab.caa::c-c").unwrap();
        assert_eq!(caps[0].spans(), &[Some((1, 9)), Some((6, 7))], "{mode:?}: {caps:?}");

        // The no-intervening-mandatory-atom shape (`a?`
        // directly beside `g0`) must stay unaffected: `a?` genuinely has
        // nothing left to donate once it takes the one available byte.
        let re = Regex::with_options(r"a?(?P<g0>a{1,3})?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 1)), None], "{mode:?}: {caps:?}");

        // The bounded-predecessor shape (a mandatory intervening
        // atom, but the predecessor genuinely has room to grow in the
        // decomposition being tested) must still correctly decline.
        let re = Regex::with_options(r"c?.(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ca").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        let re = Regex::with_options(r"c{0,3}.(?P<g0>a)?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ca").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), None], "{mode:?}: {caps:?}");

        // A bounded predecessor's own capacity can be a CHAIN of several
        // same-class optional units, not just one (`.{1,4}` desugars to a
        // mandatory rep plus 3 further chained optional reps); `.{1,4}`'s
        // own flexibility legitimately maximizes ahead of `g0` here, forcing
        // a genuine full decline: `.{1,4}.{3}` together must consume all 6
        // bytes leaving `g0` no room, same as V8 and fancy-regex.
        let re = Regex::with_options(r".{1,4}.{3}(?P<g0>.{2,5})?", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a:.b.-").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 6)), None], "{mode:?}: {caps:?}");
    }
}

#[test]
fn mandatory_capture_with_wildcard_then_distinct_literal_body_blocks_a_following_optional_capture() {
    // `(?P<g0>.?b?)(?P<g1>.)?[^a]?a` on `"a.a"`: `g0` (mandatory, but its
    // OWN internal choice between `.?` and `b?` is free) correctly claims
    // `(0,1)` (`'a'` via `.?`, `b?` contributing nothing). That leaves
    // `".a"` for `g1` (optional single char), `[^a]?` (optional), and the
    // final mandatory `a`. `g1` claiming `'.'` and `[^a]?` declining is a
    // valid tied-length decomposition (as is the reverse) - V8 and
    // fancy-regex agree the textually-first optional element (`g1`) should
    // claim it.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?P<g0>.?b?)(?P<g1>.)?[^a]?a", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"a.a").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 3)), Some((0, 1)), Some((1, 2))], "{mode:?}: {caps:?}");
    }
}

#[test]
fn top_level_union_arm_order_between_differently_tagged_arms() {
    use resharp::{Regex, RegexOptions};
    let a = r"(?:\z)+(?:(?<g0>(?:c|a)))?(?:(?<g1>(?:[bc])*)|(?:c)*(?:(?<g2>\:))?)";
    let b = r"(?:\z)+(?:(?<g0>(?:c|a)))?(?:(?:c)*(?:(?<g2>\:))?|(?<g1>(?:[bc])*))";
    for input in [b"".as_slice(), b":".as_slice(), b"-:".as_slice(), b":bb".as_slice(), b"-b::".as_slice(), b"cc:-:a".as_slice()] {
        let ra = Regex::with_options(a, RegexOptions::default()).unwrap();
        let rb = Regex::with_options(b, RegexOptions::default()).unwrap();
        let ca = ra.captures_all(input).unwrap();
        let cb = rb.captures_all(input).unwrap();
        let g1a = ca[0].name("g1").map(|m| (m.start, m.end));
        let g1b = cb[0].name("g1").map(|m| (m.start, m.end));
        assert_eq!(g1a, g1b, "input={input:?} a={ca:?} b={cb:?}");
    }
}

#[test]
fn lookahead_arm_order_inside_nested_union_before_optional_capture() {
    // The leading prefix union is the leftmost element and `(?:(?:a)?)*`
    // lets it reach (0,1), so it is maximized before g0 gets a say, forcing
    // g0 to decline regardless of the union's own arm order. glibc is not
    // the oracle here: `((c?)|(a?)*)(.)?[a-z]` on "ac" gives glibc g4=(0,1)
    // but the arm-reordered `(((a?)*)|(c?))(.)?[a-z]` gives g3=(0,1)/g4=None
    // - it flips the prefix/g0 split purely on arm order, which this
    // engine's arm-order invariance forbids.
    use resharp::{Regex, RegexOptions};
    let a = r"(?:(?:(?:c)?|(?!b))|(?:(?:a)?)*)(?:(?<g0>.))?[a-z]";
    let b = r"(?:(?:(?!b)|(?:c)?)|(?:(?:a)?)*)(?:(?<g0>.))?[a-z]";
    for input in [b"ac".as_slice(), b"ab-".as_slice()] {
        let ra = Regex::with_options(a, RegexOptions::default()).unwrap();
        let rb = Regex::with_options(b, RegexOptions::default()).unwrap();
        let ca = ra.captures_all(input).unwrap();
        let cb = rb.captures_all(input).unwrap();
        let g0a = ca[0].name("g0").map(|m| (m.start, m.end));
        let g0b = cb[0].name("g0").map(|m| (m.start, m.end));
        assert_eq!(g0a, g0b, "input={input:?} a={ca:?} b={cb:?}");
        assert_eq!(g0a, None, "input={input:?} a={ca:?}");
    }
}

#[test]
fn captures_all_no_exponential_blowup_with_unrelated_optional_group_before_bounded_repeat_anchor() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?P<g0>a)?.{0,30}\z", RegexOptions::default().unicode(mode)).unwrap();
        // warm up the capture DFA (pattern-dependent, not input-length-dependent
        // compile cost) before timing, so the assertion below isolates growth
        // with INPUT length specifically - the actual DoS-relevant dimension.
        let warm_hay = vec![b'x'; 40];
        re.captures_all(&warm_hay).unwrap();
        let hay = vec![b'x'; 5000];
        let t = std::time::Instant::now();
        let caps = re.captures_all(&hay).unwrap();
        assert!(
            t.elapsed() < std::time::Duration::from_secs(2),
            "{mode:?}: captures_all on a warmed-up DFA took {:?}, expected sub-2s",
            t.elapsed()
        );
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans(), &[Some((4970, 5000)), None], "{mode:?}");
        assert_eq!(caps[1].spans(), &[Some((5000, 5000)), None], "{mode:?}");
    }

    let re = Regex::new(r"(?P<g0>a)?.{0,30}\z").unwrap();
    let caps = re.captures_all(b"//! T").unwrap();
    assert_eq!(caps.len(), 2);
    assert_eq!(caps[0].spans(), &[Some((0, 5)), None]);
    assert_eq!(caps[1].spans(), &[Some((5, 5)), None]);
}

#[test]
fn captures_all_bound_driven_blowup_stays_near_quadratic_not_higher_order() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    // DFA transition work must not grow with the quantifier's declared
    // upper bound independent of input length. Asserts growth from bound=60
    // to bound=100 (~1.67x) stays well under quartic, with a generous
    // margin so this isn't flaky on slower CI hardware. bound=100 (not
    // higher): the recursive node-tree processing needs more stack than the
    // default `cargo test` worker-thread stack provides above that.
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let hay = vec![b'a'; 20];
        let small = Regex::with_options(r"(?P<g0>x)?a{0,60}", RegexOptions::default().unicode(mode)).unwrap();
        let t0 = std::time::Instant::now();
        small.captures_all(&hay).unwrap();
        let small_elapsed = t0.elapsed();

        let large = Regex::with_options(r"(?P<g0>x)?a{0,100}", RegexOptions::default().unicode(mode)).unwrap();
        let t1 = std::time::Instant::now();
        large.captures_all(&hay).unwrap();
        let large_elapsed = t1.elapsed();

        assert!(
            large_elapsed < std::time::Duration::from_secs(5),
            "{mode:?}: bound=100 took {large_elapsed:?}, expected sub-5s"
        );
        let ratio = large_elapsed.as_secs_f64() / small_elapsed.as_secs_f64().max(1e-6);
        assert!(
            ratio < 15.0,
            "{mode:?}: bound 60->100 blew up {ratio:.1}x, expected well under \
             the much larger blow-up an O(bound^4-5) growth would produce (small={small_elapsed:?}, large={large_elapsed:?})"
        );
    }
}

#[test]
fn lookahead_wrapping_a_bare_unfused_line_start_anchor_compiles_and_matches() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?=^)abc", RegexOptions::default().unicode(mode)).unwrap();
        let got = re.find_all(b"abc\nabc").unwrap();
        assert_eq!(
            got,
            vec![
                resharp::Match { start: 0, end: 3 },
                resharp::Match { start: 4, end: 7 },
            ],
            "{mode:?}"
        );

        let re2 = Regex::with_options(r"a(?=$)", RegexOptions::default().unicode(mode)).unwrap();
        let got2 = re2.find_all(b"ab\na").unwrap();
        assert_eq!(got2, vec![resharp::Match { start: 3, end: 4 }], "{mode:?}");
    }
}

#[test]
fn lookahead_wrapping_line_start_anchor_after_a_star_is_still_correctly_rejected() {
    // `a*(?=^)` reduces to the same shape as bare `a*^`: a lookbehind-derived
    // anchor that is not at a constant offset from a star. That is a
    // deliberate engine limitation (`ResharpError::UnsupportedPattern`) -
    // both forms must be rejected identically, not silently accepted with a
    // wrong/incomplete result.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let opts = RegexOptions::default().unicode(mode);
        let err_star_caret = match Regex::with_options("a*^", opts) {
            Err(e) => e,
            Ok(_) => panic!("{mode:?}: expected a*^ to be rejected"),
        };
        let opts = RegexOptions::default().unicode(mode);
        let err_star_la = match Regex::with_options("a*(?=^)", opts) {
            Err(e) => e,
            Ok(_) => panic!("{mode:?}: expected a*(?=^) to be rejected"),
        };
        assert!(
            matches!(
                err_star_caret,
                resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)
            ),
            "{mode:?}: {err_star_caret:?}"
        );
        assert!(
            matches!(
                err_star_la,
                resharp::Error::Algebra(resharp_algebra::ResharpError::UnsupportedPattern)
            ),
            "{mode:?}: {err_star_la:?}"
        );
    }
}

#[test]
fn star_of_alternation_maximizes_its_own_reachable_extent_across_all_iteration_counts() {
    // `(?:[^:b]b*|.*[^b].)*` on `":-b.bb:"` can reach position 6 (not just 5)
    // via a 2-iteration decomposition ([0,3) then [3,6)), so under this
    // project's POSIX tie-break rule (the leftmost subexpression is
    // maximized in length first, recursively, over the full space of
    // decompositions - not just a single greedy-per-iteration walk), the
    // star - being leftmost - must be maximized to 6 before `g3`
    // is even considered, handing `g3` only `(6,7)`. glibc `regexec`, V8 and
    // Rust `regex` all report `(5,7)` instead, but none of them perform this
    // exhaustive-decomposition maximization for nested unbounded repeats;
    // they commit greedily per iteration and never backtrack an already-
    // maximal iteration to enable a later one. `(6,7)` is reachable by the
    // star's own grammar and is the correct answer here.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?:[^:b]b*|.*[^b].)*\.?(?P<g3>.+)", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b":-b.bb:").unwrap();
        assert_eq!(caps[0].spans(), vec![Some((0, 7)), Some((6, 7))], "{mode:?}");
    }
}

#[test]
fn unbounded_star_before_a_tied_alternation_containing_a_capture_behind_a_consuming_atom_declines() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for pat in [r".+(?:b|.(?P<g0>))?", r".+(?:.(?P<g0>)|b)?"] {
        for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            assert_eq!(re.captures_all(b"ab").unwrap()[0].spans(), vec![Some((0, 2)), None], "{pat} {mode:?}");
        }
    }
}

#[test]
fn self_referential_star_of_lookahead_does_not_corrupt_match_end_or_panic() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"x?(?=b|(?=.)(?=.)*)", RegexOptions::default().unicode(mode)).unwrap();
        let ms = re.find_all(b"aa").unwrap();
        for m in &ms {
            assert!(m.start <= m.end, "{mode:?}: inverted match {m:?}");
        }
        assert_eq!(
            ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
            vec![(0, 0), (1, 1)],
            "{mode:?}"
        );

        let re2 = Regex::with_options(r"x?(?P<g0>(?=b|(?=.)(?=.)*))", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re2.captures_all(b"aa").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}");
        assert_eq!(caps[0].spans(), vec![Some((0, 0)), Some((0, 0))], "{mode:?}");
        assert_eq!(caps[1].spans(), vec![Some((1, 1)), Some((1, 1))], "{mode:?}");

        // Original fuzzer-discovered pattern this was reduced from - just
        // needs to compile and run to completion without panicking.
        let pat = r"(?P<g0>[-.]?[b:]{0}.{0,2}).(?P<g3>(?P<g2>(?:.[^bb]*|b+[^:][^bb]?){1}(?P<g1>.{2,5})?))";
        let re3 = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        re3.captures_all(b"c:.:-aca.--aa").unwrap();
    }
}

#[test]
fn g1_does_not_participate_on_homogeneous_run() {
    // `(?:a+|b+)` has two unbounded union arms; the byte class each arm
    // can donate through must be the union of BOTH arms' leading classes,
    // not just the first one found. Verified against V8/fancy-regex/the
    // Rust `regex` crate/glibc `regexec`.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?:a+|b+)(?P<g1>.)?", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"aa").unwrap()[0].spans(), vec![Some((0, 2)), None], "{mode:?}");
        assert_eq!(re.captures_all(b"bb").unwrap()[0].spans(), vec![Some((0, 2)), None], "{mode:?}");
    }
}

#[test]
fn mixed_boundary_run_already_correctly_lets_g1_participate() {
    // On a MIXED-boundary input, the union's leading arm cannot extend into
    // the second byte at all (that would require switching from the `a+`
    // arm to the `b+` arm mid-run), so `g1` must claim the second byte -
    // merging the arms' leading classes (previous test) must not regress
    // this.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let re = Regex::with_options(r"(?:a+|b+)(?P<g1>.)?", RegexOptions::default().unicode(UnicodeMode::Ascii)).unwrap();
    assert_eq!(re.captures_all(b"ab").unwrap()[0].spans(), vec![Some((0, 2)), Some((1, 2))]);
    assert_eq!(re.captures_all(b"ba").unwrap()[0].spans(), vec![Some((0, 2)), Some((1, 2))]);
}

#[test]
fn bounded_repeat_of_lookahead_plus_union_does_not_underflow_match_end() {
    // Previously, der()'s lookahead-split recursion could re-wrap an
    // already-resolved `Lookahead(EPS-or-always-nullable, tail, embedded_rel)`
    // marker as a further outer lookahead's own `la_body`. The marker's
    // embedded rel and the outer node's own rel count the same der() steps
    // (both advance in lockstep from the same position), so init_metadata's
    // Kind::Lookahead nulls computation must not add them; doing so double-
    // counted the byte distance and underflowed `pos - rel` in scan.rs's
    // collect_max, producing a structurally-invalid Match (verified against
    // both fancy-regex and V8 for the minimized repro: expected (0, 1)).
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"\.+(?=(?:(?=.{1,4}[^b])+a?|$){1,2})", RegexOptions::default().unicode(mode)).unwrap();
        let ms = re.find_all(b".cba").unwrap();
        assert_eq!(ms.len(), 1, "{mode:?}: {ms:?}");
        assert_eq!((ms[0].start, ms[0].end), (0, 1), "{mode:?}: {ms:?}");
    }
}

#[test]
fn star_of_lookahead_after_a_union_containing_a_different_lookahead_arm_does_not_underflow_match_end() {
    // An independently-discovered trigger for the same init_metadata
    // double-counting defect fixed above - a union
    // with a lookahead arm (`(?:x|(?=a))`) immediately followed by an
    // unbounded `Star` of an unrelated lookahead (`(?=b*)*`). Verified
    // against V8: matches at (0, 1) and (1, 1).
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"a?(?=(?:x|(?=a))(?=b*)*)", RegexOptions::default().unicode(mode)).unwrap();
        let ms = re.find_all(b"aa").unwrap();
        assert_eq!(
            ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
            vec![(0, 1), (1, 1)],
            "{mode:?}: {ms:?}"
        );
    }
}

#[test]
fn union_with_lookahead_arm_followed_by_two_optional_atoms_does_not_produce_malformed_match() {
    // A fourth independently-discovered trigger for the same
    // init_metadata double-counting defect fixed above -
    // no `*`/`+` on any lookahead needed at all here, just a union with a
    // lookahead arm followed by two nested optional atoms. Verified against
    // both fancy-regex and V8: matches at (0, 0) and (1, 1).
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?=(?:x|(?=.))y?)[-]?", RegexOptions::default().unicode(mode)).unwrap();
        let ms = re.find_all(b"cb").unwrap();
        assert_eq!(
            ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
            vec![(0, 0), (1, 1)],
            "{mode:?}: {ms:?}"
        );
    }
}

#[test]
fn g1_is_maximized_over_the_full_space_of_decompositions() {
    // Expectation CORRECTED from (0,3) to (0,4). (0,3) is exactly
    // glibc `regexec`'s answer (verified: `((.{2,3}|c)+).{2,4}` on
    // "aaaaaa" -> group1 (0,3), stable under arm reorder), i.e. the
    // per-iteration convention where the mandatory first copy of the body
    // is maximized to 3 and an already-maximal iteration is never
    // backtracked to enable a later one. The star-maximization tests above
    // explicitly reject that convention as narrower than the rule this
    // project has adopted, and this pattern asks the identical question, so it cannot
    // answer it differently. Under rule 6(b) the leftmost subexpression
    // `g1` is maximized in LENGTH over the full space of decompositions:
    // g1=(0,4) via two copies of `.{2,3}` each taking 2, leaving exactly 2
    // for the trailing `.{2,4}`. Total-extent maximization also depends
    // only on the repeat's language, never on its body's internal parse
    // structure, so it is order-invariant by construction.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?P<g1>(?:.{2,3}|c)+).{2,4}", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(re.captures_all(b"aaaaaa").unwrap()[0].spans(), vec![Some((0, 6)), Some((0, 4))], "{mode:?}");
    }
}

// `g2`'s own extent (the actual bug) must be maximized to (0,3) - `g1`
// ("c" right after `.+`) cannot participate once `g2` is fixed there, since
// `input[2]` is "a", not "c". The trailing union then operates on the empty
// remainder, a genuine zero-width tie between the tagged `g3` arm and the
// untagged `.*` arm matching empty - resolved by "participation wins a
// genuine tie" (verified arm-order-independent, unlike V8/fancy-regex,
// which flip their answer for `g3` depending on textual alternative order -
// not usable as an oracle for this specific tie).
#[test]
fn trailing_alternation_with_a_trivial_lookahead_branch_corrupts_a_preceding_captures_own_greedy_extent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        let re = Regex::with_options(r"(?P<g2>.+(?P<g1>c)?)(?:.*|(?P<g3>(?=\z)))", RegexOptions::default().unicode(mode)).unwrap();
        assert_eq!(
            re.captures_all(b"cca").unwrap()[0].spans(),
            vec![Some((0, 3)), Some((0, 3)), None, Some((3, 3))],
            "{mode:?}"
        );
    }
}

#[test]
fn leading_nullable_bounded_predecessor_already_exhausted_still_flagged_risky() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:[^b]*).?(?P<g2>[.:])?(?P<g5>[.:]{0,2}(?P<g4>b)?)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"b:.b").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 4)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], Some((1, 2)), "{mode:?}: g2 must participate, tied byte has no other claimant");
        assert_eq!(caps[0].spans()[2], Some((2, 4)), "{mode:?}");
        assert_eq!(caps[0].spans()[3], Some((3, 4)), "{mode:?}");
    }
}

#[test]
fn leading_star_with_a_later_landing_point_makes_an_optional_capture_decline() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".*[b](?P<g0>.)?.+[c:]",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bab:.").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 4)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], Some((1, 2)), "{mode:?}: g0 must participate per rule 6(a)+(b)");
    }
}

#[test]
fn trailing_optional_capture_in_complex_pattern_reports_zero_width() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>b*-?)(?:.?.{2}|[:][.-]{1,4})*.(?P<g1>.?)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"c--ca--a:-b").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 11)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], Some((0, 0)), "{mode:?}: g0 zero-width is acceptable (b* can match empty)");
        // CORRECTED to (11,11). `.?` always participates (it can match the
        // empty string), so the only question is where the starred element
        // stops. Its body `.?.{2}` consumes 2 or 3, so it reaches 10, leaving
        // `.`=(10,11) and g1 empty at 11. glibc answers (0,9)/(10,11) here, but
        // that is the per-iteration greed convention (9 = 3+3+3, refusing to
        // shorten an iteration to reach a longer total) which this corpus has
        // already rejected elsewhere in this file. It is structure-dependent,
        // which rule 6 bans: `((..)*).(.?)` on 11 chars gives glibc the
        // MAXIMIZED (0,10), and only changing the body to the equally-long-
        // reaching `.?.{2}` makes it drop to (0,9).
        assert_eq!(caps[0].spans()[2], Some((11, 11)), "{mode:?}: starred element is maximized by TOTAL extent");
    }
}

#[test]
fn optional_capture_after_greedy_prefix_reports_zero_width() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".*a{1,3}(?P<g0>a*.{1})?.+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"a:.:.a:c:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 10)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], Some((6, 7)), "{mode:?}: g0 must participate per rule 6(a)");
    }
}

#[test]
fn optional_capture_maximizes_leftmost_before_trailing_lookahead_dot() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>a(?:(?:.+.?[-c]|[^:]*)|b+))?.+(?P<g1>(?!-{2}))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"a-a-aca-.b").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 10)), "{mode:?}");
        // Not a bug: the overall match length (0,10) does
        // not depend on where g0 stops - whatever it doesn't consume, the
        // following mandatory `.+` consumes instead. `g0`'s body genuinely
        // CAN reach (0,9) (`a` then `[^:]*` over "-a-aca-."), leaving
        // exactly one byte for `.+` and the zero-width lookahead `g1` at
        // end-of-string - a real derivation, not an invented one. Being
        // leftmost, rule 6(b) requires maximizing g0 there. Confirmed
        // arm-order-invariant across all 4 UnicodeModes (see bug file).
        assert_eq!(caps[0].spans()[1], Some((0, 9)), "{mode:?}: g0 span is wrong");
    }
}

#[test]
fn optional_capture_maximizes_leftmost_before_trailing_lookahead_dot_star() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>b*(?:.|:{3}b*.{1})(?:-*|.{2,3}))(?P<g1>(?=.?)).*",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"b::c").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 4)), "{mode:?}");
        // Not a bug: the trailing `.*` after g1 can absorb
        // whatever g0 doesn't consume, so the overall match length (0,4)
        // does not depend on where g0 stops. g0's body genuinely CAN reach
        // the full input (`b*`="b", `.`-arm=":", `.{2,3}`-arm=":c") - a
        // real derivation. Being leftmost, rule 6(b) requires maximizing
        // g0 there, forcing g1 (and `.*`) to zero-width at the end.
        // Confirmed arm-order-invariant across all 4 UnicodeModes (see bug
        // file) - unlike glibc/V8, which flip with textual arm order.
        assert_eq!(caps[0].spans()[1], Some((0, 4)), "{mode:?}: g0 span is wrong");
        assert_eq!(caps[0].spans()[2], Some((4, 4)), "{mode:?}: g1 should be zero-width at the end");
    }
}

#[test]
fn lookahead_group_participates_in_symmetric_union_tie() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".{1,1}(?::?(?P<g0>-?[^.-]+)?(?P<g1>(?=b{0,2}[:]*))|(?P<g3>(?P<g2>.*-{0,2}))[^ac]{0}:*)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"b:c:b:").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 6)), "{mode:?}");
        // Not a bug: both union arms can validly reach
        // the same overall length here (both g0/g1's branch and g2/g3's
        // branch can consume the entire remainder), so this is a genuine
        // tie, not a case where only one branch is reachable. Confirmed
        // resharp is arm-order-invariant (compared by stable name identity,
        // not positional index) for this pattern - unlike glibc `regexec`,
        // which flips to whichever arm is written first (same category as
        // other closed not-a-bug cases in this file). Because both arms match, both arms' groups
        // participate: `|` is UNION, so no arm is preferred at all.
        assert_eq!(caps[0].spans()[1], Some((2, 6)), "{mode:?}: g0 participates - its arm matches too");
        assert_eq!(caps[0].spans()[2], Some((6, 6)), "{mode:?}: g1 span");
        assert_eq!(caps[0].spans()[3], Some((1, 6)), "{mode:?}: g3 span");
        assert_eq!(caps[0].spans()[4], Some((1, 6)), "{mode:?}: g2 span");
    }
}

#[test]
fn optional_capture_with_complex_body_maximizes_leftmost_repetition_first() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[b]?[a]*(?P<g0>.{1}[bc](?:[^-]{3}(?:-+|.{2})+-?|.*$)?)?.+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bacbcaa-:aa").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 11)), "{mode:?}");
        // Corrected from the originally filed (2,11) [mathematically
        // impossible, leaves 0 bytes for the mandatory trailing `.+`], then
        // to (2,9), and now back to (2,10). (2,9) came from maximizing the
        // `+`'s mandatory FIRST copy before the repeat as a whole - the
        // per-iteration convention that other tests in this file explicitly
        // reject, in favor of full total-extent maximization.
        // Rule 6(b) maximizes the leftmost subexpression `g0` in LENGTH
        // over the full space of decompositions, giving (2,10). glibc and
        // V8 also report (2,10) here but flip it under arm reorder, so they
        // are not the oracle for this tie; total-extent maximization
        // reaches the same value while depending only on the repeat's
        // language, never on its body's parse structure, hence is
        // order-invariant by construction.
        assert_eq!(caps[0].spans()[1], Some((2, 10)), "{mode:?}: g0 span");
    }
}

#[test]
fn minimized_two_stacked_leading_optionals_before_a_dollar_anchored_alternative_arm() {
    // Minimized from the case above: needs TWO distinct leading
    // optional/star atoms (`b?` then `a*`) before g0, and a `.*$` sibling
    // arm (a plain literal sibling like `x` does not trigger it) - the
    // `.*$` arm is never actually used by the winning decomposition but
    // still corrupts g0's donation-risk tracking, forcing it to wrongly
    // decline entirely. glibc `regexec` confirms `g0 = (2,4)` is reachable
    // and correct, with or without the `.*$` sibling arm present.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(r"b?a*(?P<g0>.{2}|.*$)?.+", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"ba::aa").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 6)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], Some((2, 4)), "{mode:?}: g0 span is wrong");
    }
}

#[test]
fn optional_capture_with_alternation_body_reports_longer_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>(?:(?:.b*.|[^a-][.]{1}){1}|(?:b{0}[^a].*|..?[^b]+)(?:[^:.]{1,1}a?-|a[-:]*.?){2,2}b)*)a?",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bcab.aca").unwrap();
        // Not a bug: the overall pattern `(?P<g0>...)*a?`
        // is fully nullable, so `captures_all` correctly reports a second,
        // trailing zero-width match at position 8 after the main match
        // consumes the whole input - this is standard "global find"
        // behavior for any nullable pattern (verified against `a*` and
        // `a*b?` elsewhere), not specific to this pattern's alternation body.
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 8)), "{mode:?}");
        // g0 (the leftmost subexpression, before the trailing `a?`) is
        // maximized per rule 6(b): both g0=(0,8)/a?="" and g0=(0,7)/a?="a"
        // are valid, equal-length decompositions of the same overall match;
        // resharp picks the longer g0, matching the star-maximization precedent for
        // leftmost-subexpression maximization (glibc/V8 instead commit
        // greedily to a narrower per-iteration convention that doesn't
        // maximize the star as a whole - not a valid oracle for this tie).
        assert_eq!(caps[0].spans()[1], Some((0, 8)), "{mode:?}: g0 span");
        assert_eq!(caps[1].spans()[0], Some((8, 8)), "{mode:?}");
    }
}

#[test]
fn lookahead_group_participates_rule6a_zero_width() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:c*|.?.?(?P<g0>(?=.)))[^b].+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":.-bb:a---ba").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 12)), "{mode:?}");
        // Not a bug: branch 1 (`c*`) and branch 2
        // (`.?.?` + a zero-width lookahead g0) both reach the same overall
        // match length. Per POSIX rule 6(a) ("participating with a
        // null/zero-width match beats not participating at all"), g0 MUST
        // participate here since it validly can - this doesn't even need a
        // tie-break oracle, it's the unconditional first rule. The
        // originally-filed expectation (g0 = None) violates rule 6(a).
        assert_eq!(caps[0].spans()[1], Some((2, 2)), "{mode:?}: g0 must participate (rule 6a)");
    }
}

#[test]
fn optional_capture_in_alternation_reports_zero_width() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"c{1}b{0,2}(?:(?:(?P<g0>[a]+)?|(?P<g1>[-]{0}[^b]{0,2}).{1,1}a)|b{2,5}c)[^-b]*",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bca..aab").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((1, 7)), "{mode:?}");
        // Not a bug: g0's branch (`[a]+`) and g1's branch
        // (`[-]{0}[^b]{0,2}` + `.{1,1}a`) both reach the same overall match
        // length via different decompositions (g0 at (2,3) plus the trailing
        // `[^-b]*` absorbing the rest, vs g1 at (2,4) plus its own `.{1,1}a`).
        // `|` is UNION: both decompositions are real, so BOTH groups
        // participate. glibc reports only one and flips with arm order, so it
        // is not a valid oracle here.
        assert_eq!(caps[0].spans()[1], Some((2, 3)), "{mode:?}: g0 participates - its branch matches");
        assert_eq!(caps[0].spans()[2], Some((2, 4)), "{mode:?}: g1 span");
    }
}

#[test]
fn optional_capture_in_alternation_reports_longer_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>(?:-?|(?:[^-]{0,1}.+.+|b{2,3}.{2,2}[^a]{0,3})*){2,2}[a]*):*.+-+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":babbb-::bccc").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 7)), "{mode:?}");
        // Not a bug: g0 (the leftmost subexpression before
        // `:*.+-+`) can validly reach either (0,0) (declining, letting the
        // trailing `.+` absorb more) or (0,5) (consuming more itself) -
        // both are reachable decompositions of the same overall match.
        // Rule 6(b) maximizes the leftmost subexpression first, so g0
        // should be as long as possible: (0,5), matching resharp's actual
        // (and order-invariant) answer. Same category as the other total-extent-maximization cases in this file.
        assert_eq!(caps[0].spans()[1], Some((0, 5)), "{mode:?}: g0 maximized per rule 6(b)");
    }
}

#[test]
fn lookahead_group_declines_when_arm_is_not_actually_tied() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"a{2,2}.+(?:(?:.{2}|(?:.+[.b]{1,4}[^.c]*|a{0,1}-*.)*)|(?P<g0>(?=:{0}(?:-*[^b]?|[^.]*.?a{0}))).?[^a]+){1}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b".abbbaaab.b:").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((5, 12)), "{mode:?}");
        // Expectation CORRECTED from Some((11,11)) to None. The old comment
        // called this "a genuine tie between the two union arms" and applied
        // rule 6(a), but the two arms are NOT tied: branch 1 is nullable, so
        // it lets the leading `.+` reach (7,12), whereas g0's branch requires
        // `.+` to stop at 11. The decomposition therefore differs and rule
        // 6(b) decides it, maximizing `.+` and leaving g0 out. Rule 6(a) is
        // subordinate to 6(b): it breaks ties only within an otherwise-fixed
        // decomposition. glibc confirms, arm-order invariantly: `(.+)(y*|()z)`
        // and `(.+)(()z|y*)` on "xz" both give g1=(0,2) with the zero-width
        // group NOT participating, while `(.+)(x?)` on "ab" (decomposition
        // fixed) does report g2=(2,2). See scripts/posix_oracle.c.
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: `.+` is maximized first (rule 6b)");
    }
}

#[test]
fn lookahead_group_participates_others_decline_when_unreachable() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"^(?:(?P<g0>(?!a+\.{1,3}))-?|(?:[^c:]*(?P<g1>(?=a?))|(?P<g2>(?=b{3}:?))))b+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bb:bab--:ab:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}");
        // Not a bug: g0's branch (`(?!a+\.{1,3})-?`) and
        // g1's branch (`[^c:]*(?=a?)`) both validly reach the same overall
        // match, so under UNION semantics both participate. g2's branch
        // (`(?=b{3}:?)`) genuinely cannot match here (`b{3}` doesn't hold at
        // this position), so g2 stays None - the merge only reports groups
        // that really can participate, which is what makes this test useful.
        assert_eq!(caps[0].spans()[1], Some((0, 0)), "{mode:?}: g0 participates - its branch matches");
        assert_eq!(caps[0].spans()[2], Some((1, 1)), "{mode:?}: g1 span");
        assert_eq!(caps[0].spans()[3], None, "{mode:?}: g2 correctly declines");
    }
}

#[test]
fn lookahead_group_reports_zero_width() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"-?(?:.?b*|[a:]{2,5}.{1,2}.*)?(?P<g0>(?!b+)).+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"::b.:".as_slice()).unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 5)), "{mode:?}");
        // Not a bug: the leading optional group before g0
        // can validly consume up to position 1 or up to position 4 (both
        // satisfy g0's `(?!b+)` condition, since neither position is
        // immediately followed by a `b`) - a genuine leftmost-
        // subexpression-width tie, same shape as the leftmost-subexpression-width tie above. Rule 6(b)
        // maximizes the leftmost subexpression (the optional prefix) first,
        // so it should consume as much as possible, putting g0 at (4,4).
        // Confirmed resharp is arm-order-invariant here.
        assert_eq!(caps[0].spans()[1], Some((4, 4)), "{mode:?}: g0 maximized per rule 6(b)");
    }
}



#[test]
fn optional_capture_in_complex_alternation_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"b*a?[ca]{1,1}[^b]+[^b]+(?P<g2>(?:(?:.+c[b]|.*-{0,2}){3}|:*(?P<g0>.)?.{0,1}).+(?P<g1>(?=[b]*)))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"aa-a-bcb").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 8)), "{mode:?}");
        // CORRECTED to (5,6). The old "branch 2 doesn't match" is false. Inside
        // g2 both arms of the union reach exactly (5,7): arm 1 via `.*-{0,2}`
        // (its `.*` absorbs "bc") and arm 2 via `:*`="" g0="b" `.{0,1}`="c".
        // The decomposition is therefore identical and rule 6(b) applies: the
        // arm in which g0 participates wins.
        assert_eq!(caps[0].name("g0").map(|m| (m.start, m.end)), Some((5, 6)), "{mode:?}: tied arms, g0 participates (rule 6b)");
    }
}

#[test]
fn bounded_repeat_capture_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[^a]+.*(?P<g2>(?P<g1>(?:[-]+|.*){1,1}(?P<g0>.{0,0})[^c]{2,5})?)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":baa").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 4)), "{mode:?}");
        // g0 is None because g1 declines: `[^c]{2,5}` needs at least 2 bytes,
        // and after `[^a]+`=(0,2) and `.*`=(2,4) are maximized there are none
        // left, so g2 takes the null match (4,4) and nothing inside it
        // participates.
        assert_eq!(caps[0].name("g0").map(|m| (m.start, m.end)), None, "{mode:?}: g1 declines, so g0 cannot participate");
    }
}

#[test]
fn optional_capture_at_end_of_string_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"$(?:$|(?P<g0>(?!-{1})))?",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"-.cba:b..").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((9, 9)), "{mode:?}");
        // CORRECTED to (9,9). The old "branch 2 doesn't match" is false: at EOF
        // the negative lookahead `(?!-{1})` holds, so both arms are zero-width
        // at 9 and the decomposition is tied. Rule 6(b) makes g0 participate,
        // matching the passing precedent
        // `tagged_zero_width_arm_wins_nullable_tie_against_untagged_arm_arm_order_independent`.
        // glibc is disqualified here: `$($|())?` gives the group None but the
        // arm-reordered `$(()|$)?` gives (9,9).
        assert_eq!(caps[0].spans()[1], Some((9, 9)), "{mode:?}: tied arms, g0 participates (rule 6b)");
    }
}

#[test]
fn optional_capture_in_alternation_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:.{2,2}|.(?P<g0>.?[^a])?)[^.]{1,4}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":-b:ac.ca-bca").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 6)), "{mode:?}");
        // CORRECTED to (1,3). The leading union is the leftmost element; arm 1
        // `.{2,2}` reaches 2 but arm 2 `.(?P<g0>.?[^a])?` reaches 3 ("-b"), and
        // `[^.]{1,4}` still covers (3,6). Rule 6(a) maximizes the element, so
        // the longer arm wins and g0=(1,3).
        assert_eq!(caps[0].spans()[1], Some((1, 3)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn lookahead_capture_correct_in_complex_pattern() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?::+|.?b{0})+(?P<g0>(?=[^a]?))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"::ab-b-").unwrap();
        // Count CORRECTED from 1 to 2. The pattern is nullable, so the trailing
        // empty match at len is engine-wide find semantics, not a capture bug:
        // `find_all` reports the same [(0,7), (7,7)], and the `a*` on "aa"
        // control gives [(0,2), (2,2)] exactly as rust-regex does.
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 7)), "{mode:?}");
        // g0's lookahead `(?=[^a]?)` is nullable, so it holds wherever the
        // maximized leading `+` stops, which is 7.
        assert_eq!(caps[0].spans()[1], Some((7, 7)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn lookahead_capture_in_complex_nested_pattern_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:\\.{2,2}c{2}|.+(?:.+(?P<g0>.{1,2})?(?P<g1>b?)|(?:c.*:*|.*.{0}))c+)?.{0,3}[^ba]{1}[^:a]*(?P<g2>(?!a{3}[a]{2}))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":cabcbbcb--b:").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 12)), "{mode:?}");
        assert_eq!(caps[0].spans()[1], None, "{mode:?}: g1 span");
    }
}

#[test]
fn bounded_repeat_capture_maximizes_leftmost_element_g4() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?::*(?P<g2>(?P<g0>a{0}.?)\\z(?P<g1>:?.*))|(?:(?:[ab]?[^aa]{0,1}|:.?)*(?P<g3>[^-].*)|(?P<g4>.{0}b{1})))?-*",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bbb..baa-").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 9)), "{mode:?}");
        // g4 should be (7,9) not (1,9)
        assert_eq!(caps[0].spans()[4], Some((7, 9)), "{mode:?}: g4 span");
    }
}

#[test]
fn optional_capture_with_complex_body_reports_span() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[^c]?(?:(?:(?:[c]{0}.{2}|.{2}.){2,3}(?P<g0>.{3})|\\.)(?P<g1>.?[bb]{2,4})?(?P<g3>(?P<g2>.{2,4}))?|[b]){1}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b".ab:ccb:cab:.").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 13)), "{mode:?}");
        // CORRECTED to None. `[^c]?` is the leftmost element and takes (0,1),
        // then `(?:...){2,3}` maximizes to (1,10) and g0=`.{3}`=(10,13), which
        // leaves nothing for g1 (needs >=2) or g3. g1=(7,10) requires `[^c]?`
        // to decline, which rule 6(a) forbids.
        assert_eq!(caps[0].name("g1").map(|m| (m.start, m.end)), None, "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn bounded_repeat_capture_maximizes_leftmost_element_g0() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g1>b?(?P<g0>(?:[^b]?|[^.:]*.?)?)a{1,3}).{2,4}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"aba:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 5)), "{mode:?}");
        // CORRECTED to (0,2). g1 is the leftmost element and maximizes to
        // (0,3), which forces `a{1,3}`="a"=(2,3) and hence g0=(0,2) (matched by
        // `[^.:]*.?`). The old (0,0) belongs to the strictly shorter g1=(0,1)
        // parse that rule 6(a) rejects.
        assert_eq!(caps[0].name("g0").map(|m| (m.start, m.end)), Some((0, 2)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn bounded_repeat_capture_correct_in_complex_alternation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:a{2}[-]*|(?:(?:[^b]{2,4}|.{3})(?:[bb]{2,5}.:|b)|[^.-]+c?c{2,2}){1,3}[a]*(?P<g0>.{1,4})?)?[^a]+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b".-bb:.ac.").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 9)), "{mode:?}");
        // CORRECTED to (4,8). The leading big optional is the leftmost element
        // and can reach (0,8) via `.{3}`=".-b" then `b`=(3,4) then `[a]*`=""
        // then g0=`.{1,4}`=(4,8), leaving `[^a]+`=(8,9). Rule 6(a) maximizes it,
        // so g0=(4,8); (3,7) is the strictly shorter E1=(0,7) parse.
        assert_eq!(caps[0].spans()[1], Some((4, 8)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

// A captured group wrapping an optional captured group whose body is a pure
// assertion, e.g. `(?P<g2>(?P<g1>(?=a))?)`. This used to be rejected with
// Algebra(UnsupportedPattern) in every mode except Ascii: the reverse pass
// turns the lookahead into a lookbehind, so `reverse_concat` hit a
// `(union containing a lookbehind) . R` shape whose non-TS-left case was
// unimplemented. TWO INDEPENDENT defects had to be fixed to support it.
//
// 1. FORWARD, and it was MASKED by the reverse gate. Fusing a tail into a
//    lookahead that carries a pending `rel` dropped that rel unless the BODY
//    was always-nullable. `rel` is POSITIONAL bookkeeping -- it pins a null
//    retroactively N bytes back -- and a ZERO-WIDTH tail cannot move that
//    position, so the condition belonged on the tail, not the body. With the
//    rel dropped, `(?P<g2>(?P<g1>(?!a))?)` -- entirely zero-width -- reported
//    span (1,2) on "ab", and find_anchored on "b" gave (0,1), i.e. the FORWARD
//    path. Two further details: a lookahead's `extra` is a NullsId holding a
//    rel RANGE, so the whole set must survive (mk_lookahead_nid), not just
//    get_lookahead_rel's single value -- collapsing it broke the multi-byte
//    bodies `(?!ab)`/`(?!abc)`; and the `[la split]` branch of `der` built the
//    split-off lookahead from the TAIL's nulls alone, ignoring the node's own
//    rel, so it now cross-shifts by `cur_rels` as the metadata path already did.
// 2. The reverse gate, now widened to distribute `X.(A|B)` into `X.A|X.B` when
//    X is zero-width, lookaround-free AND ANCHOR-free (in practice a Tag), and
//    EVERY arm of the union is itself zero-width.
//
// Both extra conditions in 2 are load-bearing and were each forced by a fuzz
// counterexample from captures_arm_order_fuzz; do not relax either.
//   - Allowing an ANCHOR in X (\A, \z are zero-width but context-sensitive)
//     breaks `(?:(?!(?:\:)+)|(?:(?:b)*|(?:[a-z]|[a-z])))(?:(?:\z)+)+`, which
//     then trips the internal "forward scan found no end for reverse-proposed
//     start" assertion in ldfa.rs.
//   - Allowing a CONSUMING union arm breaks arm-order invariance, e.g.
//     `(?<g0>(?:(?:(?!\-))+|(?:..|[a-z])))` vs the same with the arms swapped
//     disagree on "-b:" ((2,2) vs (2,3)); every counterexample found had a
//     consuming arm, and restricting to all-zero-width arms excludes them all.
// This is why the large pattern in `is_correct_where_supported_and_its_minimal_family_works_in_every_mode`
// is still Ascii-only: its union has a consuming arm. The minimal family below
// is supported in every mode.
//
// Widening the gate ALONE is NOT a fix and must not be retried on its own: it
// only exposes defect 1 as silent wrong matches. Validate any change here with
// the tag-free equivalent as the oracle (see the test below it): wrapping a
// group in captures must never change find_all spans, AND re-run the fuzz gate
// (RESHARP_FUZZ_SEED=6/23/47 caught the two over-wide versions; the plain test
// suite did NOT).
#[test]
fn is_correct_where_supported_and_its_minimal_family_works_in_every_mode() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let p = r".+(?P<g2>(?P<g1>(?P<g0>(?![^a])).{1}(?:.*|.{3}.{0,0}){1,1})?)";
    let mut accepted = 0;
    for mode in modes {
        match Regex::with_options(p, RegexOptions::default().unicode(mode)) {
            Err(e) => assert!(
                format!("{e:?}").contains("UnsupportedPattern"),
                "{mode:?}: rejected with the wrong error: {e:?}"
            ),
            Ok(re) => match re.captures_all(b"-a") {
                Err(e) => assert!(
                    format!("{e:?}").contains("UnsupportedPattern"),
                    "{mode:?}: failed with the wrong error: {e:?}"
                ),
                Ok(caps) => {
                    accepted += 1;
                    let got: Vec<_> = caps.iter().map(|c| c.spans().to_vec()).collect();
                    assert_eq!(
                        got,
                        vec![vec![Some((0, 2)), Some((2, 2)), None, None]],
                        "{mode:?}: g0 must be None, the leading `.+` is maximized (rule 6a)"
                    );
                }
            },
        }
    }
    assert!(
        accepted > 0,
        "no mode supports the pattern, so nothing was actually checked"
    );

    let want_pos = vec![
        vec![Some((0, 0)), Some((0, 0)), Some((0, 0))],
        vec![Some((1, 1)), Some((1, 1)), None],
        vec![Some((2, 2)), Some((2, 2)), None],
    ];
    let want_neg = vec![
        vec![Some((0, 0)), Some((0, 0)), None],
        vec![Some((1, 1)), Some((1, 1)), Some((1, 1))],
        vec![Some((2, 2)), Some((2, 2)), Some((2, 2))],
    ];
    for mode in modes {
        for (q, want) in [
            (r"(?P<g2>(?P<g1>(?=a))?)", &want_pos),
            (r"(?P<g2>(?P<g1>(?!a))?)", &want_neg),
        ] {
            let re = Regex::with_options(q, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("{mode:?}: {q} must compile now: {e:?}"));
            let got: Vec<_> = re
                .captures_all(b"ab")
                .unwrap()
                .iter()
                .map(|c| c.spans().to_vec())
                .collect();
            assert_eq!(&got, want, "{mode:?}: {q} on \"ab\"");
        }
    }
    for mode in modes {
        let re = Regex::with_options(r"(?P<g1>(?!a))?", RegexOptions::default().unicode(mode)).unwrap();
        let spans: Vec<(usize, usize)> =
            re.find_all(b"ab").unwrap().iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(spans, vec![(0, 0), (1, 1), (2, 2)], "{mode:?}: zero-width pattern matched a byte");
    }
}

#[test]
fn optional_lookahead_capture_not_zero_width_when_declining() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[^a]+(?P<g1>(?:(?P<g0>(?=.?))[^c]+(?:.+[c:]?|:.[::]+)?|(?:.{3}[ca]?.|[:c]{3})*))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"acc.").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((1, 4)), "{mode:?}");
        // g0 should be None not (3,3)
        assert_eq!(caps[0].name("g0").map(|m| (m.start, m.end)), None, "{mode:?}: g0 span");
    }
}

#[test]
fn lookahead_capture_ties_and_maximizes_leftmost_element() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:c*|(?:\\z(?:[.]{1}|.{2,4})?(?::+b*.{0,1}|\\.+)|[^-]?(?:b{0,0}.+|.?){2})?)?(?P<g0>(?=[^.]{2,3}))[^b:]+b{0}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"aa:.b.").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}");
        // CORRECTED to (1,1). The leading optional can reach (0,1) via
        // `[^-]?`="a", and g0's lookahead `(?=[^.]{2,3})` holds at 1 ("a:") but
        // NOT at 2 (":."), so (0,1) is the longest completable leftmost element.
        // Rule 6(a) takes it; (0,0) is the strictly shorter parse.
        assert_eq!(caps[0].spans()[1], Some((1, 1)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn nested_alternation_lookahead_capture_g4_declines() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"a{0,1}.+.+(?:(?P<g0>-{0,2}.(?:c{0}|:?.+[^cb]?)+)(?:\\.?[^
</textarea>][^c.]{0}.?|[c]+)(?:(?:[:a]{2}.*b+|[^.b]{3}[^.]{1,2}[^:a]*){1}(?P<g1>c{2,4}b{1}[^ca]*)|(?P<g2>.{2,3}b*[^b]*)?\\.?$)|.?(?P<g3>(?=.{2})))?.*.{1}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bbaa:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 6)), "{mode:?}");
        // g4 can only participate by shortening the leading mandatory
        // atoms below their maximized extent; rule 6a forbids that.
        assert_eq!(caps[0].spans()[4], None, "{mode:?}: g4 declines");
    }
}

#[test]
fn negative_lookahead_capture_ties_zero_width_and_participates() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".{0}.?(?:a*a?|(?P<g1>(?P<g0>(?![^b]+.{0,2}))))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"ba::bbb:").unwrap();
        assert_eq!(caps.len(), 8, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // Check match at (3,4)
        assert_eq!(caps[2].spans()[0], Some((3, 4)), "{mode:?}");
        // CORRECTED to (4,4). At this match `.?`=":"=(3,4) and the trailing
        // union must be empty at 4. `[^b]+` cannot match at 4 (input[4]='b'),
        // so the negative lookahead `(?![^b]+.{0,2})` HOLDS and the g1 arm is
        // zero-width, exactly like `a*a?`. Tied decomposition, so rule 6(b)
        // makes g1 participate.
        assert_eq!(caps[2].spans()[1], Some((4, 4)), "{mode:?}: tied arms, g1 participates (rule 6b)");
    }
}

#[test]
fn optional_capture_correct_in_complex_alternation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:(?P<g1>(?:[^a-]?[^--]?|.*a+)(?P<g0>[^-a]*.{1,3})?[^.]*)|.*.*(?:a?b+(?P<g2>(?=[^b]?))|(?P<g3>a?)(?:[^.c]*|c{2,2}.+a){1}a?)){1}b{0,3}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"a..-").unwrap();
        // Count CORRECTED from 1 to 2: nullable pattern, trailing empty match
        // at len, same as `find_all` and as rust-regex on `a*`/"aa".
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 4)), "{mode:?}");
        // g1 should be (0,4) not None
        assert_eq!(caps[0].spans()[1], Some((0, 4)), "{mode:?}: g1 span");
    }
}

#[test]
fn lookahead_capture_ties_with_trailing_class_rule6b() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[^b]+(?:.?|(?:(?P<g0>(?=.?.+))|(?:[.a]?-{2,4}|:{0,1})?(?P<g1>(?=b.*)))a*)a?[^b.]{2,4}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bababbb.:-").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((7, 10)), "{mode:?}");
        // g0 CORRECTED to (8,8). `[^b]+` maximizes to (7,8) - it cannot go
        // further because the trailing `[^b.]{2,4}` needs ":-"=(8,10) - so the
        // middle union must be empty at 8. Both `.?`="" and the g0 arm are
        // zero-width there (`(?=.?.+)` holds: ":-" follows), so the
        // decomposition is tied and rule 6(b) makes g0 participate.
        assert_eq!(caps[0].spans()[1], Some((8, 8)), "{mode:?}: tied arms, g0 participates (rule 6b)");
        // g1 stays None: its lookahead `(?=b.*)` fails at 8 (input[8] is ':').
        assert_eq!(caps[0].spans()[2], None, "{mode:?}: g1's lookahead genuinely fails at 8");
    }
}

#[test]
fn bounded_repeat_capture_after_star_of_alternation() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"b*(?:.?|a+[^ba]+)*(?P<g0>[^.a]+.{3})",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":ab:-abcbba").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 11)), "{mode:?}");
        // g0 should be (7,11) not (6,11)
        assert_eq!(caps[0].spans()[1], Some((7, 11)), "{mode:?}: g0 span");
    }
}

#[test]
fn lookahead_capture_arm_maximized_over_optional_sibling() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"a+(?P<g0>(?=c{0}))(?:a?|(?P<g2>:?(?P<g1>[^bb].{2,5}))?){1}.+.{0,2}",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"a:aa-cc").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 7)), "{mode:?}");
        // CORRECTED to (1,6). After `a+`=(0,1) and g0=(1,1), the `{1}` union is
        // the next element and is maximized: arm 1 `a?` is empty at 1 (input[1]
        // is ':'), while the g2 arm reaches (1,6) via `:?`=":", `[^bb]`="a",
        // `.{2,5}`="a-c", leaving `.+`=(6,7). Rule 6(a) takes the longer arm.
        assert_eq!(caps[0].spans()[2], Some((1, 6)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn optional_capture_cannot_start_before_mandatory_prefix() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".+(?P<g2>(?P<g1>(?:[^-]+|[^aa]{0}.?)+(?P<g0>[^..]{1,3}.?).?)?a{0,3}.{0,1}).+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"ac.:bb:c:b").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 10)), "{mode:?}");
        // CORRECTED to None. The old expectation (0,9) is impossible: g1 is
        // preceded by `.+`, which must consume at least one byte, so g1 cannot
        // start at 0. `.+` maximizes to (0,9), g2 is nullable so it takes
        // (9,9), the trailing `.+` takes (9,10), and g1 declines.
        assert_eq!(caps[0].name("g1").map(|m| (m.start, m.end)), None, "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn bounded_repeat_capture_second_match_after_optional_star() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"[^b]+(?:[^b]?|[b]{0})+(?P<g0>a?)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"aa:.c.a.b:.ab").unwrap();
        assert_eq!(caps.len(), 2, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // Second match
        assert_eq!(caps[1].spans()[0], Some((9, 12)), "{mode:?}");
        // g0 should be (12,12) not (11,12)
        assert_eq!(caps[1].spans()[1], Some((12, 12)), "{mode:?}: g0 span");
    }
}

#[test]
fn optional_capture_leftmost_repeat_body_maximized() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?!a+)(?P<g0>(?:b?|:*(?:a+[a]?-?|[^.b]?.?[^bb]+){1})+).+.$",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b":a.:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 5)), "{mode:?}");
        // CORRECTED to (0,3). g0 is the leftmost element and reaches (0,3) via
        // `:*`=":", `[^.b]?`="a", `.?`="", `[^bb]+`=".", leaving `.+`=(3,4) and
        // `.`=(4,5). glibc's (0,0) here is its known nullable-repeat-body
        // structure dependence and is disqualified: `((b?|a)+)(.+)$` on "aab"
        // gives glibc g1=(0,0) but the arm-reordered `((a|b?)+)(.+)$` gives
        // g1=(0,2), as does the equivalent `((a|b)*)(.+)$`. Rule 6(a) is
        // order-invariant and maximizes g0.
        assert_eq!(caps[0].spans()[1], Some((0, 3)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn optional_capture_declines_when_leading_repeat_can_reach_further() {
    // the leading repeat's TOTAL extent must be maximized (rule 6a), not
    // just one iteration's span; it reaches 10, leaving g0 unset. V8/glibc
    // resolve `+` by per-iteration greed instead, so they disagree here.
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?:.a|a?.{2})+(?P<g0>[^b])?(?P<g1>(?=c*)).+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"ca:c.aaba:a").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 11)), "{mode:?}");
        assert_eq!(caps[0].name("g0").map(|m| (m.start, m.end)), None, "{mode:?}: g0 declines, leading repeat maximizes to reach 10");
    }
}

#[test]
fn tied_zero_width_union_arms_all_participate() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>(?=b?))(?:c*c*(?:[-]*(?P<g1>(?![:a]{2}))|a{1,4})|(?P<g3>(?P<g2>(?<=[..]?.{0,0})))){1}.+",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"ac").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        assert_eq!(caps[0].spans()[0], Some((0, 2)), "{mode:?}");
        assert_eq!(
            (caps[0].spans()[2], caps[0].spans()[3], caps[0].spans()[4]),
            (Some((0, 0)), Some((0, 0)), Some((0, 0))),
            "{mode:?}: the two outer arms tie zero-width at 0, and `|` is UNION, so every group that \
             can participate does - g1, g3 and g2 all report (0,0). Verified invariant by writing \
             the g3/g2 arm first. Earlier revisions picked one arm (g1, then g3/g2)."
        );
    }
}

#[test]
fn bounded_repeat_capture_forbids_donating_to_trailing_star() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r"(?P<g0>(?:b{0}(?:b{2,4}[aa]{2,4}|[a]{2,3})*|b?)*c{0})(?:(?P<g1>(?=a{2,4}\\B))|(?!:{1,3})b*)?c(?P<g2>(?=b{0}))",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"bbb:.abbc:").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((6, 9)), "{mode:?}");
        // CORRECTED to (6,8). g0 is the leftmost element and reaches (6,8)
        // ("bb" via `b?` twice), after which `(?!:{1,3})b*` matches empty at 8,
        // `c`=(8,9) and g2 is nullable. (6,6) only arises from donating the
        // "bb" to the later `b*`, which rule 6(a) forbids.
        assert_eq!(caps[0].spans()[1], Some((6, 8)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn lookahead_capture_correct_span_after_greedy_prefix() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".?(?:a{1,2}c?|[^b]+)(?P<g0>(?=(?:(?:-{2}.+.{2,5}|[-]+){3}(?:[-a]+[b]?|b{0,1})?b*|a*\\A\\.{3})*))b{0,2}.{0,1}$",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"-a-").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?}: {:?}", caps.iter().map(|c| c.spans().to_vec()).collect::<Vec<_>>());
        // First match
        assert_eq!(caps[0].spans()[0], Some((0, 3)), "{mode:?}");
        // CORRECTED to (3,3). `.?`=(0,1) then `[^b]+`="a-"=(1,3) is the
        // leftmost-maximal parse, and g0's lookahead body is a star (nullable
        // everywhere), so g0=(3,3). glibc is disqualified here: it flips with
        // arm order - `.?(a{1,2}c?|[^b]+)()b{0,2}.{0,1}$` on "-a-" gives
        // g1=(1,2)/g2=(2,2) but `.?([^b]+|a{1,2}c?)()...` gives (1,3)/(3,3).
        assert_eq!(caps[0].spans()[1], Some((3, 3)), "{mode:?}: leftmost element is maximized (rule 6a)");
    }
}

#[test]
fn union_arm_tie_lets_both_arms_contribute_and_ignores_arm_order() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let named = |pat: &str, hay: &[u8], mode: UnicodeMode, names: &[&str]| {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(hay).unwrap();
        assert_eq!(caps.len(), 1, "{mode:?} {pat}");
        names.iter().map(|n| caps[0].name(n).map(|m| (m.start, m.end))).collect::<Vec<_>>()
    };
    let names = ["g0", "g1", "g2", "g3"];
    let want = [Some((2, 6)), Some((6, 6)), Some((1, 6)), Some((1, 6))];
    for mode in modes {
        let a = named(
            r".{1,1}(?::?(?P<g0>-?[^.-]+)?(?P<g1>(?=b{0,2}[:]*))|(?P<g3>(?P<g2>.*-{0,2}))[^ac]{0}:*)",
            b"b:c:b:",
            mode,
            &names,
        );
        let b = named(
            r".{1,1}(?:(?P<g3>(?P<g2>.*-{0,2}))[^ac]{0}:*|:?(?P<g0>-?[^.-]+)?(?P<g1>(?=b{0,2}[:]*)))",
            b"b:c:b:",
            mode,
            &names,
        );
        assert_eq!(a, want.to_vec(), "{mode:?}: every arm that matches contributes its captures");
        assert_eq!(a, b, "{mode:?}: captures must not depend on textual arm order");
    }
}

#[test]
fn union_arm_tiebreak_is_invariant_under_renesting_an_arm_prefix() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let named = |pat: &str, mode: UnicodeMode| {
        let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abcd").unwrap();
        assert_eq!(caps.len(), 1, "{mode:?} {pat}");
        ["g0", "g1"].iter().map(|n| caps[0].name(n).map(|m| (m.start, m.end))).collect::<Vec<_>>()
    };
    for mode in modes {
        let flat = named(r"(?:(?P<g0>a)(?:bc)(?:d)|(?:ab)(?P<g1>cd))", mode);
        let nested = named(r"(?:(?:(?P<g0>a)(?:bc))(?:d)|(?:ab)(?P<g1>cd))", mode);
        let swapped = named(r"(?:(?:ab)(?P<g1>cd)|(?P<g0>a)(?:bc)(?:d))", mode);
        assert_eq!(flat, vec![Some((0, 1)), Some((2, 4))], "{mode:?}: both arms match \"abcd\", so both groups participate");
        assert_eq!(flat, nested, "{mode:?}: a redundant group around an arm prefix must not change captures");
        assert_eq!(flat, swapped, "{mode:?}: arm order must not change captures");
    }
}

#[test]
fn union_never_picks_an_arm_every_tied_arm_contributes_its_captures() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    for mode in [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript] {
        for (pat, input, want) in [
            (r"(?<g1>a)|(?<g2>a)", &b"a"[..], [Some((0, 1)), Some((0, 1))]),
            (r"(?<g2>a)|(?<g1>a)", &b"a"[..], [Some((0, 1)), Some((0, 1))]),
            (r"(?<g1>[bc])|(?<g2>(?:c)+)", &b"c"[..], [Some((0, 1)), Some((0, 1))]),
            (r"(?<g2>(?:c)+)|(?<g1>[bc])", &b"c"[..], [Some((0, 1)), Some((0, 1))]),
            (r"(?<g1>(?=a))|(?<g2>(?=.))", &b"a"[..], [Some((0, 0)), Some((0, 0))]),
            (r"(?<g2>(?=.))|(?<g1>(?=a))", &b"a"[..], [Some((0, 0)), Some((0, 0))]),
            (r"(?<g1>a)|(?<g2>a)|(?<g1x>a)", &b"a"[..], [Some((0, 1)), Some((0, 1))]),
            (r"(?<g1>x)|(?<g2>xy)", &b"xy"[..], [Some((0, 1)), Some((0, 2))]),
            (r"(?<g2>xy)|(?<g1>x)", &b"xy"[..], [Some((0, 1)), Some((0, 2))]),
        ] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let caps = re.captures_all(input).unwrap();
            assert_eq!(
                [caps[0].name("g1").map(|m| (m.start, m.end)), caps[0].name("g2").map(|m| (m.start, m.end))],
                want,
                "mode={mode:?} pattern={pat}: `|` is UNION, so there is no arm to pick. Every arm \
                 that ties contributes its captures and every group that can participate does. \
                 Arm-order invariance follows because merging is commutative - it is not a \
                 property that needs a tie-break key, and the identical-arms case \
                 `(?<g1>a)|(?<g2>a)` is not an irreducible symmetric tie: BOTH groups report \
                 (0,1). Any future 'winner' logic here is a bug."
            );
        }
        let re = Regex::with_options(r"(?:(?<a>x)|(?<b>xy))z", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"xyz").unwrap();
        assert_eq!(
            (caps[0].name("a").map(|m| (m.start, m.end)), caps[0].name("b").map(|m| (m.start, m.end))),
            (None, Some((0, 2))),
            "mode={mode:?}: participation still requires REALLY matching. Arm `a` ends at 1, after \
             which `z` cannot match \"yz\", so no accepting run of the whole pattern uses it and `a` \
             declines. Contrast the bare `(?<g1>x)|(?<g2>xy)` case above, where the short arm IS a \
             complete match and does participate."
        );
        let re = Regex::with_options(r"(?<a>ab)c|abd", RegexOptions::default().unicode(mode)).unwrap();
        let caps = re.captures_all(b"abd").unwrap();
        assert_eq!(
            caps[0].name("a").map(|m| (m.start, m.end)),
            None,
            "mode={mode:?}: an arm that never reaches an accepting state contributes nothing, even \
             though its prefix `ab` matched byte-for-byte. Dead-end parses are not runs."
        );
    }
}

// Pins the FORWARD defect described above (see
// `is_correct_where_supported_and_its_minimal_family_works_in_every_mode`): a
// lookahead's pending `rel` must survive being fused with a zero-width tail.
// Oracle is the tag-free spelling of the same pattern -- wrapping a group in
// captures is a pure annotation and cannot change which spans match. Before the
// fix the wrapped forms reported non-zero-width spans for zero-width patterns
// (e.g. (1,2) on "ab"). The multi-byte bodies are the ones that need the rel
// RANGE preserved rather than a single collapsed value.
#[test]
fn zero_width_assertion_in_nested_capture_agrees_with_its_tag_free_equivalent() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pats = [
        r"(?P<g2>(?P<g1>(?=a))?)",
        r"(?P<g2>(?P<g1>(?!a))?)",
        r"(?P<g2>(?P<g1>(?!ab))?)",
        r"(?P<g2>(?P<g1>(?=ab))?)",
        r"(?P<g2>(?P<g1>(?!abc))?)",
        r"a(?P<g2>(?P<g1>(?=b))?)",
        r"\A(?P<g2>(?P<g1>(?=a))?)",
        r"x(?P<g2>(?P<g1>(?!ab))?)y",
        r"(?P<g2>(?P<g1>(?=a))?)\z",
        r"(?P<g2>(?P<g1>(?!a))?)\z",
        r"(?<g0>(?!a)|(?:..|b))",
        r"(?<g0>(?:..|b)|(?!a))",
        r"(?<g0>(?:(?!a))+|(?:..|[a-z]))",
        r"(?<g0>(?:..|[a-z])|(?:(?!a))+)",
        r"(?P<g2>(?P<g1>(?<=a))?)",
        r"(?P<g2>(?P<g1>(?<!a))?)",
        r"(?P<g2>(?P<g1>(?<=ab))?)",
        r"(?P<g1>(?<=a))",
    ];
    let inputs: [&[u8]; 12] = [b"", b"a", b"b", b"ab", b"ba", b"aab", b"xay", b"abab", b"xaby", b":", b"xb:", b"-b:"];
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let mut checked = 0;
    for p in pats {
        let oracle = p.replace("(?P<g1>", "(?:").replace("(?P<g2>", "(?:");
        for mode in modes {
            let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("{mode:?}: {p} must compile: {e:?}"));
            let re_oracle = Regex::with_options(&oracle, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("{mode:?}: oracle {oracle} must compile: {e:?}"));
            for inp in inputs {
                let spans = |r: &Regex| -> Vec<(usize, usize)> {
                    r.find_all(inp).unwrap().iter().map(|m| (m.start, m.end)).collect()
                };
                assert_eq!(
                    spans(&re),
                    spans(&re_oracle),
                    "{mode:?}: {p} disagrees with its tag-free oracle {oracle} on {:?}",
                    String::from_utf8_lossy(inp)
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, pats.len() * modes.len() * inputs.len());

    for p in [r"(?P<g2>(?P<g1>(?=a))?)$"] {
        for mode in modes {
            let e = Regex::with_options(p, RegexOptions::default().unicode(mode))
                .err()
                .unwrap_or_else(|| panic!("{mode:?}: {p} now compiles; check it against its tag-free oracle and move it into the supported list above"));
            assert!(
                format!("{e:?}").contains("UnsupportedPattern"),
                "{mode:?}: {p} rejected with the wrong error: {e:?}"
            );
        }
    }
}

// A union arm's position in the union TREE must not decide whether a following
// tail gets distributed into the arms. mk_concat distributes `(A|B).T` into
// `A.T|B.T` when an arm is a "fresh" lookahead, because the lookahead's pending
// `rel` (its retroactive end pin) is only correct once the tail is fused into
// it. That test used to require the lookahead to be an IMMEDIATE child of the
// union node, so it fired for `LA|(..|b)` (LA at depth 1) but not for the
// arm-swapped `(..|b)|LA`, which flattens to union(.., union(b, LA)) with LA at
// depth 2. The tail then stayed outside the union and the match end was wrong:
// `(?<g0>(?:..|b)|(?!a))` on ":" reported (0,1), consuming a byte that neither
// arm can match (`..` needs two bytes, `b` is not ':'). Arm position in the
// union tree is exactly the kind of incidental node shape that must never
// decide match behavior, so the predicate now walks the whole union spine.
#[test]
fn tail_distributes_into_a_lookahead_arm_at_any_union_depth() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let pairs = [
        (r"(?<g0>(?!a)|(?:..|b))", r"(?<g0>(?:..|b)|(?!a))"),
        (r"(?<g0>(?:(?!a))+|(?:..|[a-z]))", r"(?<g0>(?:..|[a-z])|(?:(?!a))+)"),
        (r"(?<g0>(?:(?!\-))+|(?:..|[a-z]))", r"(?<g0>(?:..|[a-z])|(?:(?!\-))+)"),
        (r"(?<g0>(?:(?!a))+|(?:xy|b))", r"(?<g0>(?:xy|b)|(?:(?!a))+)"),
        (r"x(?<g0>(?!a)|(?:..|b))y", r"x(?<g0>(?:..|b)|(?!a))y"),
    ];
    let inputs: [&[u8]; 7] = [b":", b"-b:", b"xb:", b"b:", b"ab", b"xy", b"xbyb"];
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for (a, b) in pairs {
        for mode in modes {
            let spans = |p: &str| -> Vec<Vec<(usize, usize)>> {
                let re = Regex::with_options(p, RegexOptions::default().unicode(mode))
                    .unwrap_or_else(|e| panic!("{mode:?}: {p} must compile: {e:?}"));
                inputs
                    .iter()
                    .map(|i| re.find_all(i).unwrap().iter().map(|m| (m.start, m.end)).collect())
                    .collect()
            };
            assert_eq!(spans(a), spans(b), "{mode:?}: arm order changed the match set for {a} vs {b}");
        }
    }

    // The concrete wrong answer this fixed, pinned as an absolute value rather
    // than only as an arm-order invariant: at 0 the only possible match of
    // `(?:..|b)|(?!a)` over ":" is the zero-width lookahead arm.
    for p in [r"(?<g0>(?!a)|(?:..|b))", r"(?<g0>(?:..|b)|(?!a))"] {
        let re = Regex::new(p).unwrap();
        let got: Vec<_> = re.find_all(b":").unwrap().iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(got, vec![(0, 0), (1, 1)], "{p} must not consume ':'");
    }
}

// The distribution above is what makes `(union containing a lookaround) . R`
// supportable when the left factor is zero-width: a CONSUMING union arm and a
// trailing anchor are both fine now. The left factor must still be
// LOOKAROUND-FREE, and this is the counterexample that forces it: `(\b)(\B)` is
// a contradiction (a position cannot be both a word boundary and not one), so
// it must match nothing. Allowing a lookaround-bearing left factor to be
// duplicated into both arms makes it match, because each copy of the fused
// lookbehind `prev` is resolved independently.
#[test]
fn zero_width_left_factor_may_not_contain_a_lookaround() {
    match Regex::new(r"(\b)(\B)") {
        Ok(re) => assert!(
            re.find_all(b"ab").unwrap().is_empty(),
            "(\\b)(\\B) is a contradiction and must match nothing"
        ),
        Err(_) => {}
    }
}

// A capture group whose body is a LOOKBEHIND used to be rejected outright
// (`Algebra(UnsupportedPattern)`), which docs/capture-posix-parse.md attributed
// to the capture subset ("lookbehind in the capture root"). It was actually
// `normalize_rev`, which refused any lookahead carrying a fused tail: reversing
// `(?<=a)` produces a lookahead, and the group's closing tag becomes its tail.
// Normalizing the tail (leaving the body alone, exactly as the untailed
// lookahead arm already does) is enough. The values below are the whole point:
// `(?<=a)` can only participate at the one position preceded by "a", and
// `(?<!a)` is its exact complement.
#[test]
fn capture_group_around_a_lookbehind_body() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let caps = |p: &str| -> Vec<Vec<Option<(usize, usize)>>> {
            Regex::with_options(p, RegexOptions::default().unicode(mode))
                .unwrap_or_else(|e| panic!("{mode:?}: {p} must compile: {e:?}"))
                .captures_all(b"ab")
                .unwrap()
                .iter()
                .map(|c| c.spans().to_vec())
                .collect()
        };
        assert_eq!(
            caps(r"(?P<g2>(?P<g1>(?<=a))?)"),
            vec![
                vec![Some((0, 0)), Some((0, 0)), None],
                vec![Some((1, 1)), Some((1, 1)), Some((1, 1))],
                vec![Some((2, 2)), Some((2, 2)), None],
            ],
            "{mode:?}: (?<=a) may only participate where 'a' precedes"
        );
        assert_eq!(
            caps(r"(?P<g2>(?P<g1>(?<!a))?)"),
            vec![
                vec![Some((0, 0)), Some((0, 0)), Some((0, 0))],
                vec![Some((1, 1)), Some((1, 1)), None],
                vec![Some((2, 2)), Some((2, 2)), Some((2, 2))],
            ],
            "{mode:?}: (?<!a) must be the exact complement of (?<=a)"
        );
        assert_eq!(
            caps(r"(?P<g1>(?<=a))"),
            vec![vec![Some((1, 1)), Some((1, 1))]],
            "{mode:?}: bare lookbehind capture matches only after 'a'"
        );
    }
}

#[test]
fn end_anchored_capture_group_with_trailing_bytes_after_line() {
    use resharp::Regex;
    let re = Regex::new(r"^(?<user>[a-z]+)@(?<host>[a-z.]+)$").unwrap();
    let hay = b"joe@example.com\n";
    let all = re.find_all(hay).unwrap();
    assert_eq!(all.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(), vec![(0, 15)]);
    let caps = re.captures_all(hay).unwrap();
    assert_eq!(caps[0].spans(), &[Some((0, 15)), Some((0, 3)), Some((4, 15))]);
}

#[test]
fn capture_group_adjacent_to_top_wildcard() {
    use resharp::Regex;
    let re = Regex::new(r"_*(?<x>a)").unwrap();
    let caps = re.captures_all(b"za").unwrap();
    assert_eq!(caps[0].spans(), &[Some((0, 2)), Some((1, 2))]);

    let re = Regex::new(r"(?<x>a)_*").unwrap();
    let caps = re.captures_all(b"az").unwrap();
    assert_eq!(caps[0].spans(), &[Some((0, 2)), Some((0, 1))]);
}

#[test]
fn stale_nested_capture_after_abandoned_quantifier_backoff() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        let re = Regex::with_options(
            r".*(?P<g2>(?P<g1>(?=a))?)",
            RegexOptions::default().unicode(mode),
        )
        .unwrap();
        let caps = re.captures_all(b"ba").unwrap();
        assert_eq!(caps[0].spans(), &[Some((0, 2)), Some((2, 2)), None], "({mode:?})");
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn optional_word_boundary_before_bounded_repeated_z_anchor() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    for mode in modes {
        for pat in [r"\B?\z{2}", r"\b?\z{2}", r"\A?\z{2}", r"(?:(?<=a)(?=b)|(?<=b))?\z{2}"] {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let ms = re.find_all(b"ab").unwrap();
            assert_eq!(
                ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                vec![(2, 2)],
                "pattern={pat:?} mode={mode:?}"
            );
        }
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn optional_anchor_before_lookbehind_forces_match_at_end() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
        (r"\A?(?<!b)", b"ab", &[(0, 0), (1, 1)]),
        (r"\A?(?<!.)", b"ab", &[(0, 0)]),
        (r"\A?(?<!.)", b"a", &[(0, 0)]),
        (r"\A?(?<!.)", b"", &[(0, 0)]),
        (r"\A?(?<![ab])", b"ab", &[(0, 0)]),
        (r"\A?(?<!x|y)", b"ab", &[(0, 0), (1, 1), (2, 2)]),
        (r"\A?(?<!a)", b"ab", &[(0, 0), (2, 2)]),
        (r"\B(?<=b)", b"ab", &[]),
        (r"\B(?<=b)", b"abb", &[(2, 2)]),
        (r"\B(?<=b+)", b"abbb", &[(2, 2), (3, 3)]),
        (r"\B(?<=[b]+)", b"abbb", &[(2, 2), (3, 3)]),
        (r"\b(?<!(?!a*))", b"ab", &[(0, 0), (2, 2)]),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let ms = re.find_all(hay).unwrap();
            assert_eq!(
                ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                expected.to_vec(),
                "pattern={pat:?} hay={:?} mode={mode:?}",
                String::from_utf8_lossy(hay)
            );
        }
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn mandatory_boundary_before_anchor_forces_failing_lookbehind() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
        (r"\b\A(?<!^)", b"a", &[]),
        (r"\A(?<!^)", b"a", &[]),
        (r"\b(?<!^)", b"a", &[(1, 1)]),
        (r"\B\A(?<!^)", b"a", &[]),
        (r"\z{0}\A(?<!^)", b"a", &[]),
        (r"(?=a)\A(?<!^)", b"a", &[]),
        (r"\b\A(?<!^)", b"ab", &[]),
        (r"\b\A+((?<!^[b-]*))", b"bab\ncaca:c.-", &[]),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let ms = re.find_all(hay).unwrap();
            assert_eq!(
                ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                expected.to_vec(),
                "pattern={pat:?} hay={:?} mode={mode:?}",
                String::from_utf8_lossy(hay)
            );
        }
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn bounded_end_anchor_then_mandatory_boundary_drops_match() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
        (r"(?-m)${2}\b^?", b"ab", &[(2, 2)]),
        (r"(?-m)\z{3}\B^?", b"ab\n", &[(3, 3)]),
        (r"(?-m)\z{3}\B^?", b"-cba:bc:\n", &[(9, 9)]),
        (r"(?-m)${2,4}\b${2,2}", b".b.caab", &[(7, 7)]),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let ms = re.find_all(hay).unwrap();
            assert_eq!(
                ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                expected.to_vec(),
                "pattern={pat:?} hay={:?} mode={mode:?}",
                String::from_utf8_lossy(hay)
            );
        }
    }
}

#[test]
#[ignore = "slow in debug (unicode word-class build); run with --ignored or in release"]
fn order_dependent_anchor_concat_b_a_caret() {
    use resharp::{Regex, RegexOptions, UnicodeMode};
    let modes = [UnicodeMode::Ascii, UnicodeMode::Default, UnicodeMode::Full, UnicodeMode::Javascript];
    let cases: &[(&str, &[u8], &[(usize, usize)])] = &[
        (r"\B\A^", b".b", &[(0, 0)]),
        (r"\B^\A", b".b", &[(0, 0)]),
        (r"\A\B^", b".b", &[(0, 0)]),
        (r"^\B\A", b".b", &[(0, 0)]),
        (r"\B\A", b".b", &[(0, 0)]),
        (r"\A^", b".b", &[(0, 0)]),
        (r"\B^", b".b", &[(0, 0)]),
        (r"(\B)\A{1}^{1}", b"..ba-aa-.b", &[(0, 0)]),
    ];
    for mode in modes {
        for &(pat, hay, expected) in cases {
            let re = Regex::with_options(pat, RegexOptions::default().unicode(mode)).unwrap();
            let ms = re.find_all(hay).unwrap();
            assert_eq!(
                ms.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
                expected.to_vec(),
                "pattern={pat:?} hay={:?} mode={mode:?}",
                String::from_utf8_lossy(hay)
            );
        }
    }
}

#[test]
#[cfg(feature = "convergence_prefix")]
fn convergence_prefix_is_match_no_quadratic() {
    use resharp::{Regex, RegexOptions};
    let re = Regex::with_options("a*:[^b]+", RegexOptions::default()).unwrap();
    assert!(re.uses_convergence_prefix());

    let build = |n: usize| -> Vec<u8> {
        let mut v = b"a:a:".to_vec();
        v.resize(n, b'b');
        v
    };

    let time_it = |input: &[u8]| -> f64 {
        let mut best = f64::INFINITY;
        for _ in 0..3 {
            let t = std::time::Instant::now();
            assert!(re.is_match(input).unwrap());
            best = best.min(t.elapsed().as_secs_f64());
        }
        best.max(1e-9)
    };

    let small_elapsed = time_it(&build(10_000));
    let large_elapsed = time_it(&build(160_000));

    let ratio = large_elapsed / small_elapsed;
    assert!(
        ratio < 40.0,
        "expected roughly linear scaling, got {ratio}x for 16x input (small={small_elapsed}s large={large_elapsed}s)"
    );
}


