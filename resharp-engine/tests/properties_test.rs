use resharp_algebra::RegexBuilder;

fn fixed_length(pattern: &str) -> Option<u32> {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.get_fixed_length(node)
}

fn min_max(pattern: &str) -> (u32, u32) {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.get_min_max_length(node)
}

fn is_infinite(pattern: &str) -> bool {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.is_infinite(node)
}

fn has_look(pattern: &str) -> bool {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.contains_look(node)
}

fn has_anchors(pattern: &str) -> bool {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    b.contains_anchors(node)
}

#[test]
fn fixed_literal() {
    assert_eq!(fixed_length("abc"), Some(3));
}

#[test]
fn fixed_pred() {
    assert_eq!(fixed_length("[A-Z]"), Some(1));
}

#[test]
fn fixed_union_same() {
    assert_eq!(fixed_length("abc|def"), Some(3));
}

#[test]
fn fixed_union_different() {
    assert_eq!(fixed_length("ab|cde"), None);
}

#[test]
fn fixed_repeat_exact() {
    assert_eq!(fixed_length("[A-Z]{5}"), Some(5));
}

#[test]
fn fixed_repeat_range() {
    assert_eq!(fixed_length("[A-Z]{3,5}"), None);
}

#[test]
fn minmax_literal() {
    assert_eq!(min_max("abc"), (3, 3));
}

#[test]
fn minmax_bounded_repeat() {
    assert_eq!(min_max("[A-Za-z]{8,13}"), (8, 13));
}

#[test]
fn minmax_star() {
    assert_eq!(min_max("a*"), (0, u32::MAX));
}

#[test]
fn minmax_plus() {
    assert_eq!(min_max("a+"), (1, u32::MAX));
}

#[test]
fn minmax_optional() {
    assert_eq!(min_max("a?"), (0, 1));
}

#[test]
fn minmax_union() {
    assert_eq!(min_max("ab|cde"), (2, 3));
}

#[test]
fn minmax_concat_bounded() {
    assert_eq!(min_max("a{2,3}b{1,2}"), (3, 5));
}

#[test]
fn minmax_dotstar_literal() {
    assert_eq!(min_max(".*abc"), (3, u32::MAX));
}

#[test]
fn minmax_aws_key() {
    assert_eq!(min_max(r"(?:ASIA|AKIA|AROA|AIDA)[A-Z0-7]{16}"), (20, 20));
}

#[test]
fn minmax_alt_suffix() {
    // "Sherlock" = 8, "Holmes" = 6, suffix = 0..5
    assert_eq!(min_max("(Sherlock|Holmes)[a-z]{0,5}"), (6, 13));
}

#[test]
fn inf_star() {
    assert!(is_infinite("a*"));
}

#[test]
fn inf_plus() {
    assert!(is_infinite("a+"));
}

#[test]
fn inf_bounded() {
    assert!(!is_infinite("[A-Za-z]{8,13}"));
}

#[test]
fn inf_literal() {
    assert!(!is_infinite("abc"));
}

#[test]
fn inf_dotstar_prefix() {
    assert!(is_infinite(".*abc"));
}

#[test]
fn inf_optional() {
    assert!(!is_infinite("a?"));
}

#[test]
fn look_lookahead() {
    assert!(has_look(r"a(?=b)"));
}

#[test]
fn look_lookbehind() {
    assert!(has_look(r"(?<=a)b"));
}

#[test]
fn look_word_boundary() {
    // \b in concat context is rewritten to lookaround
    assert!(has_look(r"\bfoo\b"));
}

#[test]
fn look_none() {
    assert!(!has_look("abc"));
}

fn bdfa_eligible(pattern: &str) -> bool {
    let fl = fixed_length(pattern);
    let (_, max) = min_max(pattern);
    let max_length = if max != u32::MAX { Some(max) } else { None };
    max_length.is_some() && fl.is_none() && !has_look(pattern) && !has_anchors(pattern)
}

#[test]
fn bdfa_bounded_repeat() {
    assert!(bdfa_eligible("[A-Za-z]{8,13}"));
}

#[test]
fn bdfa_alt_suffix() {
    assert!(bdfa_eligible("(Sherlock|Holmes)[a-z]{0,5}"));
}

#[test]
fn bdfa_not_fixed() {
    // fixed length uses faster path
    assert!(!bdfa_eligible("abc"));
}

#[test]
fn bdfa_not_unbounded() {
    assert!(!bdfa_eligible("a+"));
}

#[test]
fn bdfa_not_look() {
    assert!(!bdfa_eligible(r"(?<=\s)[A-Z]{3,5}"));
}

#[test]
fn bdfa_union_variable() {
    assert!(bdfa_eligible("ab|cde"));
}

#[test]
fn bdfa_aws_key() {
    assert!(!bdfa_eligible(r"(?:ASIA|AKIA|AROA|AIDA)[A-Z0-7]{16}"));
}

#[test]
fn bdfa_phone_bounded() {
    assert!(!bdfa_eligible(r"[0-9_ \-()]{7,}"));
}

fn dispatch_info(pattern: &str) -> (bool, bool, bool) {
    let re = resharp::Regex::new(pattern).unwrap();
    let (fwd_accel, rev_accel) = re.has_accel();
    let has_bdfa = re.bdfa_stats().is_some();
    (has_bdfa, fwd_accel, rev_accel)
}

#[test]
fn dispatch_alt_suffix() {
    let (bdfa, fwd, _rev) = dispatch_info("(Sherlock|Holmes)[a-z]{0,5}");
    assert!(!bdfa);
    assert!(fwd);
}

#[test]
fn dispatch_literal() {
    let (bdfa, _fwd, _rev) = dispatch_info("Sherlock Holmes");
    assert!(!bdfa);
}

#[test]
fn dispatch_word_boundary_the() {
    let (_bdfa, _fwd, rev) = dispatch_info(r"\bthe\b");
    assert!(rev, r"\bthe\b should have rev accel via strip_lb fallback");
}
