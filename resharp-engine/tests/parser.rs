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
        // 10 levels of {3,6}: ~6^10 expanded nodes.
        "(?:a(?:b(?:c(?:d(?:e(?:f(?:g(?:h(?:i(?:FooBar){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}){3,6}",
        // 13 levels of {2}: ~2^13 * 6 expanded.
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
