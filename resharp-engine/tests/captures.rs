mod common;
use common::schemas::{CapturesCase, CapturesFile};
use resharp::{Regex, RegexOptions};
use std::path::Path;

fn case_options(tc: &CapturesCase) -> RegexOptions {
    assert!(
        (tc.ascii as u8 + tc.javascript as u8 + tc.full as u8) <= 1,
        "case {:?}: ascii, javascript, and full are mutually exclusive",
        tc.name
    );
    let mut opts = RegexOptions::default().implicit_captures(tc.implicit_captures);
    if tc.javascript {
        opts = opts.unicode(resharp::UnicodeMode::Javascript);
    } else if tc.ascii {
        opts = opts.unicode(resharp::UnicodeMode::Ascii);
    } else if tc.full {
        opts = opts.unicode(resharp::UnicodeMode::Full);
    }
    opts
}

fn expected_slot(slot: &[usize], tc: &CapturesCase) -> Option<(usize, usize)> {
    match slot {
        [] => None,
        [start, end] => Some((*start, *end)),
        other => panic!(
            "case {:?}: group slot must be [] or [start, end], got {:?}",
            tc.name, other
        ),
    }
}

#[test]
fn captures_toml() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("captures.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let file: CapturesFile = toml::from_str(&content).unwrap();
    let mut seen = std::collections::HashSet::new();
    for tc in &file.test {
        assert!(seen.insert(tc.name.as_str()), "duplicate case name {:?}", tc.name);
        if tc.ignore {
            continue;
        }
        let re = match Regex::with_options(&tc.pattern, case_options(tc)) {
            Err(_) if tc.expect_error => continue,
            Err(e) => panic!("name={:?}, pattern={:?}: compile error: {}", tc.name, tc.pattern, e),
            Ok(_) if tc.expect_error => {
                panic!("name={:?}, pattern={:?}: expected error but compiled Ok", tc.name, tc.pattern)
            }
            Ok(re) => re,
        };
        if let Some(kind) = &tc.kind {
            assert_eq!(
                re.captures_kind_name(),
                kind.as_str(),
                "name={:?}, pattern={:?}: captures_kind_name",
                tc.name,
                tc.pattern
            );
        }
        if tc.compile_only {
            continue;
        }
        let input = tc.input.as_bytes();
        let all = re.find_all(input).unwrap();
        if let Some(want_matches) = &tc.matches {
            let got: Vec<[usize; 2]> = all.iter().map(|m| [m.start, m.end]).collect();
            assert_eq!(
                &got, want_matches,
                "name={:?}, pattern={:?}, input={:?}: matches",
                tc.name, tc.pattern, tc.input
            );
        }
        let Some(groups) = &tc.groups else { continue };
        let all_caps = re
            .captures_all(input)
            .unwrap_or_else(|e| panic!("name={:?}, pattern={:?}, input={:?}: {e:?}", tc.name, tc.pattern, tc.input));
        let caps_list: Vec<resharp::Captures> = if tc.matches.is_some() {
            all_caps
        } else {
            all_caps.into_iter().take(1).collect()
        };
        assert!(
            !caps_list.is_empty(),
            "name={:?}, pattern={:?}: no match on {:?}",
            tc.name,
            tc.pattern,
            tc.input
        );
        assert_eq!(
            groups.len(),
            caps_list.len(),
            "name={:?}, pattern={:?}: groups must have one entry per checked match",
            tc.name,
            tc.pattern
        );
        for (caps, want) in caps_list.iter().zip(groups) {
            let expected: Vec<Option<(usize, usize)>> =
                want.iter().map(|slot| expected_slot(slot, tc)).collect();
            let whole = caps.get(0).unwrap();
            assert_eq!(
                &caps.spans()[1..],
                expected.as_slice(),
                "name={:?}, pattern={:?}, input={:?}, match={:?}: captures",
                tc.name,
                tc.pattern,
                tc.input,
                (whole.start, whole.end)
            );
        }
    }
}

#[test]
fn bare_group_never_captures_and_never_reserves_an_index_slot() {
    let re = Regex::new(r"(a)(?P<b>x)").unwrap();
    assert_eq!(re.capture_names(), &[None, Some("b".to_string())]);
    let caps = re.captures_all(b"ax").unwrap().remove(0);
    assert_eq!(caps.spans(), &[Some((0, 2)), Some((1, 2))]);
}

#[test]
fn capture_names_reports_names_in_group_order() {
    let re = Regex::new(r"(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})").unwrap();
    assert_eq!(
        re.capture_names(),
        &[
            None,
            Some("year".to_string()),
            Some("month".to_string()),
            Some("day".to_string()),
        ]
    );
}

#[test]
fn capture_index_for_name_resolves_and_rejects() {
    let re = Regex::new(r"(?P<a>x)(?P<b>y)").unwrap();
    assert_eq!(re.capture_index_for_name("a"), Some(1));
    assert_eq!(re.capture_index_for_name("b"), Some(2));
    assert_eq!(re.capture_index_for_name("nope"), None);
}

#[test]
fn capture_names_can_index_into_captures_result() {
    let re = Regex::new(r"(?P<user>[a-z]+)@(?P<host>[a-z.]+)").unwrap();
    let caps = re.captures_all(b"alice@example.com").unwrap().remove(0);
    let host_idx = re.capture_index_for_name("host").unwrap();
    assert_eq!(caps.get(host_idx), caps.name("host"));
    assert_eq!(caps.name("host"), Some(resharp::Match { start: 6, end: 17 }));
}

#[test]
fn tag_under_complement_is_rejected() {
    use resharp_algebra::RegexBuilder;

    let mut b = RegexBuilder::new();
    let open = b.mk_tag(2);
    let a = b.mk_u8(b'a');
    let tagged_a = b.mk_concat(open, a);
    let node = b.mk_compl(tagged_a);
    assert!(resharp::Regex::from_node(b, node, Default::default()).is_err());
}

#[test]
fn named_capture_split_across_union_operands_via_bare_ast_is_rejected() {
    use resharp_algebra::RegexBuilder;

    let mut b = RegexBuilder::new();
    let open = b.mk_tag(2);
    let close = b.mk_tag(3);
    let a = b.mk_u8(b'a');
    let aa = b.mk_concat(a, a);
    let left = b.mk_concat(open, a);
    let right = b.mk_concat(aa, close);
    let node = b.mk_union(left, right);
    assert!(resharp::Regex::from_node(b, node, Default::default()).is_err());
}

#[test]
fn outer_capture_wrapping_lookahead_plus_star_keeps_span_alongside_sibling() {
    let re = resharp::Regex::new(r"(?P<g1>(?P<g0>(?!c))a*)(?P<g3>.(?P<g2>.*))").unwrap();
    let hay = b"aa.b.";
    let all = re.captures_all(hay).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(
        all[0].spans(),
        &[Some((0, 5)), Some((0, 2)), Some((0, 0)), Some((2, 5)), Some((3, 5))]
    );
}

#[test]
fn anonymous_capture_group_captures_its_span_but_reports_no_name() {
    let re = Regex::new(r"(??[a-z]+)@(?P<host>[a-z.]+)").unwrap();
    assert_eq!(re.capture_names(), &[None, None, Some("host".to_string())]);
    assert_eq!(
        re.captures_all(b"foo@bar.com").unwrap().remove(0).spans(),
        &[Some((0, 11)), Some((0, 3)), Some((4, 11))]
    );
}

#[test]
fn bare_group_still_does_not_capture_by_default() {
    let re = Regex::new(r"(a)(b)(c)").unwrap();
    assert_eq!(re.capture_names(), &[None]);
    assert_eq!(re.captures_all(b"abc").unwrap().remove(0).spans(), &[Some((0, 3))]);
}

#[test]
fn bare_non_capturing_group_does_not_consume_an_index_slot() {
    let re = Regex::new(r"(a)(?P<x>b)(c)(?P<y>d)").unwrap();
    assert_eq!(re.capture_names(), &[None, Some("x".to_string()), Some("y".to_string())]);
    assert_eq!(
        re.captures_all(b"abcd").unwrap().remove(0).spans(),
        &[Some((0, 4)), Some((1, 2)), Some((3, 4))]
    );
    assert_eq!(re.capture_index_for_name("x"), Some(1));
    assert_eq!(re.capture_index_for_name("y"), Some(2));
}

#[test]
fn bare_group_matches_noncapturing_group_for_index_purposes() {
    let bare = Regex::new(r"(a)(?P<x>b)").unwrap();
    let noncap = Regex::new(r"(?:a)(?P<x>b)").unwrap();
    assert_eq!(bare.capture_names(), noncap.capture_names());
    assert_eq!(
        bare.capture_index_for_name("x"),
        noncap.capture_index_for_name("x")
    );
}

#[test]
fn implicit_captures_option_makes_every_bare_group_capture() {
    let re = Regex::with_options(r"(a)(b)(c)", RegexOptions::default().implicit_captures(true)).unwrap();
    assert_eq!(re.capture_names(), &[None, None, None, None]);
    assert_eq!(
        re.captures_all(b"abc").unwrap().remove(0).spans(),
        &[Some((0, 3)), Some((0, 1)), Some((1, 2)), Some((2, 3))]
    );
}

#[test]
fn implicit_captures_option_does_not_affect_named_groups() {
    let re = Regex::with_options(
        r"(a)(?P<mid>b)(c)",
        RegexOptions::default().implicit_captures(true),
    )
    .unwrap();
    assert_eq!(re.capture_names(), &[None, None, Some("mid".to_string()), None]);
    assert_eq!(
        re.captures_all(b"abc").unwrap().remove(0).spans(),
        &[Some((0, 3)), Some((0, 1)), Some((1, 2)), Some((2, 3))]
    );
}

#[test]
fn more_than_63_unnamed_capture_groups_is_rejected() {
    for n in [62usize, 63, 64, 70] {
        let pat: String = (0..n).map(|_| "(??a)").collect::<Vec<_>>().join("");
        let res = Regex::new(&pat);
        if n <= 63 {
            let re = res.unwrap();
            let inp = "a".repeat(n);
            let caps = re.captures_all(inp.as_bytes()).unwrap().remove(0);
            assert_eq!(caps.get(n), Some(resharp::Match { start: n - 1, end: n }));
        } else {
            assert!(res.is_err(), "{n} groups must be rejected, not silently miscomputed");
        }
    }
}
