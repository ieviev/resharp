use resharp::{Regex, RegexOptions, UnicodeMode};

const PATTERNS: &[&str] = &[
    r"^$",
    r"^",
    r"$",
    r"^a$",
    r"^a*$",
    r"\b",
    r"\B",
    r"\ba\b",
    r"a\b",
    r"\bא",
    r"(?<=a)b",
    r"(?<=a)",
    r"a(?=b)",
    r"(?=a)",
    r"(?!a)",
    r"(?<!a)b",
    r"\n",
    r"$\n",
    r"^\n",
    r".",
    r".*",
    r"a*",
    r"(a|b)*",
    r"a+",
    r"\d+",
    r"\w+",
    r"[^\n]",
    r"^.*$",
    r"(?m)^",
    r"(?m)$",
    r"(?m)^a$",
    r"\bx",
    r"x\b",
    r"\Bx\B",
    r"a{0}",
    r"\ba{0}\b",
    r"(?<=\.)y",
    r"x|(?<=\.)y",
    r".|(?<=ab)y",
];

const INPUTS: &[&[u8]] = &[
    b"",
    b"\n",
    b"\n\n",
    b"\n\n\n",
    b"a",
    b"ab",
    b"abc",
    b"a\nb",
    b"a\n\nb",
    b".y",
    b".axy",
    b"  a  ",
    b"aaa",
    b"a.b.c",
    b"\xd7\x90",
    b"x.y",
    b"abxaby",
    b"\na\n",
];

fn mk(p: &str, mode: UnicodeMode, hardened: bool) -> Option<Regex> {
    let opts = RegexOptions::default().unicode(mode).hardened(hardened);
    Regex::with_options(p, opts).ok()
}

#[test]
fn cross_api_consistency() {
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
    ];
    let mut fails: Vec<String> = Vec::new();
    for &p in PATTERNS {
        for &mode in &modes {
            let Some(re) = mk(p, mode, false) else { continue };
            for &inp in INPUTS {
                let find_all = re.find_all(inp).unwrap();
                let is_match = re.is_match(inp).unwrap();

                if is_match != !find_all.is_empty() {
                    fails.push(format!(
                        "is_match vs find_all: {p:?} {mode:?} {inp:?} is_match={is_match} find_all={find_all:?}"
                    ));
                }

                match re.find_anchored(inp) {
                    Ok(anchored) => {
                        let expected = find_all.first().copied().filter(|m| m.start == 0);
                        if anchored != expected {
                            fails.push(format!(
                                "find_anchored vs find_all leftmost-at-0: {p:?} {mode:?} {inp:?} anchored={anchored:?} expected={expected:?} find_all={find_all:?}"
                            ));
                        }
                    }
                    Err(resharp::Error::Algebra(
                        resharp_algebra::ResharpError::UnsupportedPattern,
                    )) => {}
                    Err(e) => fails.push(format!(
                        "find_anchored error: {p:?} {mode:?} {inp:?} err={e:?}"
                    )),
                }
            }
        }
    }
    assert!(
        fails.is_empty(),
        "{} divergences (showing first 10):\n{}",
        fails.len(),
        fails.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn simd_prefilter_differential() {
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
    ];
    for &p in PATTERNS {
        for &mode in &modes {
            let (Some(fast), Some(slow)) = (mk(p, mode, false), mk(p, mode, true)) else {
                continue;
            };
            for &inp in INPUTS {
                let f = fast.find_all(inp).unwrap();
                let s = slow.find_all(inp).unwrap();
                assert_eq!(
                    f, s,
                    "prefilter on/off diverge: {p:?} {mode:?} {inp:?}"
                );
            }
        }
    }
}

#[test]
fn empty_anchor_newline_seed() {
    let re = Regex::new(r"^$").unwrap();
    assert_eq!(
        re.find_all(b"\n\n").unwrap(),
        vec![
            resharp::Match { start: 0, end: 0 },
            resharp::Match { start: 1, end: 1 },
            resharp::Match { start: 2, end: 2 },
        ]
    );
}

#[test]
fn bounded_repeat_big_class_compile_budget() {
    use std::time::{Duration, Instant};
    let classes = [r"\w", r"\W", r"\D", r"\S"];
    let modes = [
        UnicodeMode::Ascii,
        UnicodeMode::Default,
    ];
    for &c in &classes {
        for &mode in &modes {
            let pat = format!("{c}{{0,64}}");
            let t = Instant::now();
            let opts = RegexOptions::default().unicode(mode);
            let re = Regex::with_options(&pat, opts);
            let elapsed = t.elapsed();
            assert!(re.is_ok(), "compile failed: {pat:?} {mode:?}");
            assert!(
                elapsed < Duration::from_secs(1),
                "bounded repeat compile budget exceeded: {pat:?} {mode:?} took {elapsed:?}"
            );
        }
    }
}
