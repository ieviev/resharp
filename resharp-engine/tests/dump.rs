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
fn captures_fixed_offsets_roundtrip() {
    let re = Regex::new(r"(?P<y>\d{4})-(?P<m>\d{2})-(?P<d>\d{2})").unwrap();
    assert_eq!(re.captures_kind_name(), "FixedOffsets");
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    assert_eq!(re2.captures_kind_name(), "FixedOffsets");
    assert_eq!(re2.capture_names(), re.capture_names());
    let input = b"logged 2024-01-15 done";
    assert_eq!(
        re2.captures_all(input).unwrap().remove(0).spans(),
        &[Some((7, 17)), Some((7, 11)), Some((12, 14)), Some((15, 17))]
    );
}

#[test]
fn captures_empty_roundtrip() {
    let re = Regex::new(r"\d+").unwrap();
    assert_eq!(re.captures_kind_name(), "Empty");
    let bytes = re.dump().unwrap();
    let re2 = Regex::load(&bytes).unwrap();
    assert_eq!(re2.captures_kind_name(), "Empty");
    assert_eq!(re2.captures_all(b"abc 123").unwrap().remove(0).spans(), &[Some((4, 7))]);
}

#[test]
fn captures_dfa_dispatch_is_rejected_at_dump_time() {
    let re = Regex::new(r"(?P<a>\d{1,3})\.(?P<b>\d{1,3})").unwrap();
    assert_eq!(re.captures_kind_name(), "Dfa");
    assert!(re.dump().is_err());
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

#[test]
fn disable_prefixes_dump_matches_prefix_accelerated() {
    let pat = r"age-secret-key-1[0-9a-z]{58}";
    let fast = Regex::new(pat).unwrap();
    assert_eq!(fast.find_all_kind_name(), "FwdPrefix");
    let opts = resharp::RegexOptions { disable_prefixes: true, ..Default::default() };
    let plain = Regex::with_options(pat, opts).unwrap();
    assert!(!plain.has_prefix());
    let back = Regex::load(&plain.dump().unwrap()).unwrap();
    let hay = format!("x age-secret-key-1{} y", "q".repeat(58));
    assert_eq!(
        back.find_all(hay.as_bytes()).unwrap(),
        fast.find_all(hay.as_bytes()).unwrap()
    );
}
