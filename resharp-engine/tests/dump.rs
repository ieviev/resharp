#![cfg(feature = "serialize")]

use resharp::Regex;

fn roundtrip(pattern: &str, inputs: &[&str]) {
    let re = Regex::new(pattern).unwrap();
    let bytes = re.dump().unwrap_or_else(|e| panic!("dump {pattern}: {e}"));
    let re2 = Regex::load(&bytes).unwrap_or_else(|e| panic!("load {pattern}: {e}"));
    for s in inputs {
        let a = re.find_all(s.as_bytes()).unwrap();
        let b = re2.find_all(s.as_bytes()).unwrap();
        assert_eq!(a, b, "pattern {pattern:?} input {s:?}");
    }
}

#[test]
fn digits() {
    roundtrip(r"\d+", &["abc 123 def 456", "", "no digits", "9"]);
}

#[test]
fn word() {
    roundtrip(r"\w+", &["hello world", "  ", "x"]);
}

#[test]
fn alt() {
    roundtrip(r"cat|dog|bird", &["cat dog bird fish", "catdog", ""]);
}

#[test]
fn lookbehind_inter() {
    roundtrip(r"\d{3}-\d{4}", &["call 555-1234 or 555-5678", "no"]);
}

#[test]
fn begin_anchored() {
    roundtrip(r"\Aabc\d+", &["abc123 xyz", "xyz abc123", "abc", ""]);
}

#[test]
fn anchored_fwd_prefix() {
    roundtrip(
        r"hello+ world",
        &["say hello world", "helloo world", "none"],
    );
}

#[test]
fn ranges_prefix() {
    roundtrip(r"[A-Z]{3}\d+", &["ABC123 XYZ7 hi", "no caps"]);
}

#[test]
fn bdfa_short_alt() {
    // small fixed-max-length pattern: triggers BDFA path in build
    roundtrip(
        r"foo|barz|qux",
        &["foo bar barz qux quxx no", "foobarzqux", "none"],
    );
}

#[test]
fn star_loop_always_nullable() {
    roundtrip(r".*", &["abc", "", "a\nb"]);
    roundtrip(r"(a|b)*", &["aabba c", "", "xyz"]);
}

#[test]
fn case_insensitive() {
    let opts = resharp::RegexOptions::default().case_insensitive(true);
    let re = resharp::Regex::with_options(r"hello", opts).unwrap();
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    let i = b"say HELLO and Hello";
    assert_eq!(re.find_all(i).unwrap(), re2.find_all(i).unwrap());
}
