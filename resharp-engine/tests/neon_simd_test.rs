use resharp::{EngineOptions, Regex};

fn lazy_opts() -> EngineOptions {
    EngineOptions {
        dfa_threshold: 0,
        max_dfa_capacity: 10000,
        ..Default::default()
    }
}

fn find_lazy(pattern: &str, input: &[u8]) -> Vec<(usize, usize)> {
    let re = Regex::with_options(pattern, lazy_opts()).unwrap();
    re.find_all(input)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect()
}

fn find_default(pattern: &str, input: &[u8]) -> Vec<(usize, usize)> {
    let re = Regex::new(pattern).unwrap();
    re.find_all(input)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect()
}

fn assert_simd_eq(pattern: &str, input: &[u8]) {
    let lazy = find_lazy(pattern, input);
    let default = find_default(pattern, input);
    assert_eq!(
        lazy,
        default,
        "SIMD mismatch: pattern={:?}, input_len={}, lazy={:?}, default={:?}",
        pattern,
        input.len(),
        lazy,
        default
    );
}

fn assert_simd(pattern: &str, input: &[u8], expected: &[(usize, usize)]) {
    let lazy = find_lazy(pattern, input);
    assert_eq!(
        lazy,
        expected,
        "SIMD wrong: pattern={:?}, input_len={}",
        pattern,
        input.len()
    );
}

#[test]
fn rev_skip_single_byte_every_position() {
    for pos in 0..64 {
        let mut hay = vec![b'.'; 64];
        hay[pos] = b'Z';
        let r = find_lazy("Z", &hay);
        assert_eq!(r, vec![(pos, pos + 1)], "pos={}", pos);
    }
}

#[test]
fn rev_skip_two_bytes_sweep() {
    for size in 1..=80 {
        let mut hay = vec![b'.'; size];
        hay[0] = b'X';
        assert_simd_eq("[XY]", &hay);
        hay[0] = b'.';
        hay[size - 1] = b'Y';
        assert_simd_eq("[XY]", &hay);
    }
}

#[test]
fn rev_skip_no_match_long() {
    let hay = vec![b'.'; 1024];
    assert_simd("Z", &hay, &[]);
}

#[test]
fn rev_skip_all_match() {
    let hay = vec![b'Z'; 100];
    let expected: Vec<(usize, usize)> = (0..100).map(|i| (i, i + 1)).collect();
    assert_simd("Z", &hay, &expected);
}

#[test]
fn fwd_literal_at_every_offset() {
    for pos in 0..98 {
        let mut hay = vec![b'.'; 100];
        hay[pos] = b'a';
        hay[pos + 1] = b'b';
        hay[pos + 2] = b'c';
        let r = find_lazy("abc", &hay);
        assert_eq!(r, vec![(pos, pos + 3)], "pos={}", pos);
    }
}

#[test]
fn fwd_literal_adjacent_non_overlapping() {
    assert_simd("abab", b"abababababab", &[(0, 4), (4, 8), (8, 12)]);
}

#[test]
fn fwd_literal_long_needle() {
    let needle16 = "ABCDEFGHIJKLMNOP";
    let mut hay = vec![b'.'; 200];
    hay[100..116].copy_from_slice(needle16.as_bytes());
    assert_simd(needle16, &hay, &[(100, 116)]);

    let needle20 = "ABCDEFGHIJKLMNOPQRST";
    hay[100..120].copy_from_slice(needle20.as_bytes());
    assert_simd_eq(needle20, &hay);
}

#[test]
fn fwd_literal_haystack_equals_needle() {
    assert_simd("exact", b"exact", &[(0, 5)]);
}

#[test]
fn fwd_literal_single_byte_haystack() {
    assert_simd("a", b"a", &[(0, 1)]);
    assert_simd("a", b"b", &[]);
}

#[test]
fn fwd_literal_near_end_boundary() {
    for size in [15, 16, 17, 31, 32, 33, 47, 48, 49, 63, 64, 65] {
        let mut hay = vec![b'.'; size];
        if size >= 3 {
            hay[size - 3] = b'x';
            hay[size - 2] = b'y';
            hay[size - 1] = b'z';
            let r = find_lazy("xyz", &hay);
            assert_eq!(r, vec![(size - 3, size)], "size={}", size);
        }
    }
}

#[test]
fn fwd_literal_bulk_find_all() {
    let re = Regex::new("the").unwrap();
    let input = b"the quick brown fox jumps over the lazy dog and the cat";
    let m = re.find_all(input).unwrap();
    let r: Vec<(usize, usize)> = m.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(r, vec![(0, 3), (31, 34), (48, 51)]);
}

#[test]
fn teddy_digit_class_sweep() {
    for size in [10, 15, 16, 17, 31, 32, 33, 48, 64, 100] {
        let mut hay = vec![b'.'; size];
        let mid = size / 2;
        hay[mid] = b'5';
        hay[mid + 1] = b'7';
        assert_simd_eq("[0-9]+", &hay);
    }
}

#[test]
fn teddy_upper_lower_class() {
    assert_simd_eq("[A-Z][a-z]+", b"Hello World Foo Bar");
    let mut hay = "....".repeat(50);
    hay.push_str("Hello");
    hay.push_str(&"....".repeat(50));
    assert_simd_eq("[A-Z][a-z]+", hay.as_bytes());
}

#[test]
fn teddy_alternation_three_way() {
    assert_simd_eq("cat|dog|fox", b"the cat sat on the dog and the fox ran");
}

#[test]
fn teddy_pattern_no_match() {
    let hay = vec![b'.'; 200];
    assert_simd("[0-9]+", &hay, &[]);
    assert_simd("[A-Z][a-z]+", &hay, &[]);
}

#[test]
fn teddy_at_position_zero() {
    assert_simd("[0-9]+", b"123abc", &[(0, 3)]);
}

#[test]
fn teddy_at_end() {
    assert_simd("[0-9]+", b"abc123", &[(3, 6)]);
}

#[test]
fn teddy_dense_matches() {
    let hay: Vec<u8> = (0..100).map(|i| b'0' + (i % 10)).collect();
    assert_simd_eq("[0-9]+", &hay);
}

#[test]
fn bounded_rep_size_sweep() {
    let pattern = "ab{2,4}c";
    for size in 4..=100 {
        let mut hay = vec![b'.'; size];
        if size >= 6 {
            hay[1] = b'a';
            hay[2] = b'b';
            hay[3] = b'b';
            hay[4] = b'c';
            assert_simd_eq(pattern, &hay);
        }
    }
}

#[test]
fn bounded_rep_multiple_at_boundaries() {
    let pattern = "ab{2,4}c";
    let mut hay = vec![b'.'; 128];
    hay[0] = b'a';
    hay[1..4].copy_from_slice(b"bbb");
    hay[4] = b'c';
    hay[15] = b'a';
    hay[16..18].copy_from_slice(b"bb");
    hay[18] = b'c';
    hay[32] = b'a';
    hay[33..37].copy_from_slice(b"bbbb");
    hay[37] = b'c';
    hay[64] = b'a';
    hay[65..67].copy_from_slice(b"bb");
    hay[67] = b'c';
    assert_simd_eq(pattern, &hay);
}

#[test]
fn all_accel_skip_patterns_simd_vs_default() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("accel_skip.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    for (i, t) in tests.iter().enumerate() {
        let pattern = t["pattern"].as_str().unwrap();
        let input = t["input"].as_str().unwrap();
        let expected: Vec<(usize, usize)> = t["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                let a = m.as_array().unwrap();
                (
                    a[0].as_integer().unwrap() as usize,
                    a[1].as_integer().unwrap() as usize,
                )
            })
            .collect();
        let lazy = find_lazy(pattern, input.as_bytes());
        assert_eq!(
            lazy, expected,
            "accel_skip test #{}: pattern={:?}, input={:?}",
            i, pattern, input
        );
    }
}

fn run_toml_lazy(filename: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(filename);
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    for (i, t) in tests.iter().enumerate() {
        let pattern = t["pattern"].as_str().unwrap();
        let input = t.get("input").and_then(|v| v.as_str()).unwrap_or("");
        let ignore = t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false);
        let expect_error = t
            .get("expect_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let anchored = t.get("anchored").and_then(|v| v.as_bool()).unwrap_or(false);
        if ignore || expect_error || anchored {
            continue;
        }
        let expected: Vec<(usize, usize)> = t
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
            .unwrap_or_default();
        let re = match Regex::with_options(pattern, lazy_opts()) {
            Ok(re) => re,
            Err(_) => continue,
        };
        let result = match re.find_all(input.as_bytes()) {
            Ok(m) => m.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>(),
            Err(_) => continue,
        };
        assert_eq!(
            result, expected,
            "file={}, test #{}: pattern={:?}, input={:?}",
            filename, i, pattern, input
        );
    }
}

#[test]
fn lazy_basic_toml() {
    run_toml_lazy("basic.toml");
}

#[test]
fn lazy_anchors_toml() {
    run_toml_lazy("anchors.toml");
}

#[test]
fn lazy_semantics_toml() {
    run_toml_lazy("semantics.toml");
}

#[test]
fn lazy_edge_cases_toml() {
    run_toml_lazy("edge_cases.toml");
}

#[test]
fn lazy_boolean_toml() {
    run_toml_lazy("boolean.toml");
}

#[test]
fn lazy_lookaround_toml() {
    run_toml_lazy("lookaround.toml");
}

#[test]
fn lazy_paragraph_toml() {
    run_toml_lazy("paragraph.toml");
}

#[test]
fn literal_in_1kb_haystack() {
    let mut hay = vec![b'.'; 1024];
    hay[500..506].copy_from_slice(b"needle");
    assert_simd("needle", &hay, &[(500, 506)]);
    assert_simd_eq("needle", &hay);
}

#[test]
fn literal_in_64kb_haystack() {
    let mut hay = vec![b'.'; 65536];
    hay[32000..32006].copy_from_slice(b"target");
    hay[64000..64006].copy_from_slice(b"target");
    assert_simd("target", &hay, &[(32000, 32006), (64000, 64006)]);
}

#[test]
fn class_pattern_in_1kb_haystack() {
    let mut hay = vec![b'.'; 1024];
    hay[100..105].copy_from_slice(b"12345");
    hay[900..903].copy_from_slice(b"678");
    assert_simd_eq("[0-9]+", &hay);
}

#[test]
fn dot_pattern_long_haystack() {
    let mut hay = vec![b'.'; 500];
    hay[100] = b'x';
    hay[300] = b'x';
    assert_simd_eq("..x", &hay);
}

#[test]
fn teddy1_vowel_class() {
    assert_simd_eq("[aeiou]", b"bcdfghjklmnpqrstvwxyz");
    assert_simd_eq("[aeiou]", b"hello world");
}

#[test]
fn teddy2_two_char_classes() {
    assert_simd_eq("[A-Z][0-9]", b"___A5___B7___");
    let mut hay = vec![b'.'; 100];
    hay[50] = b'Q';
    hay[51] = b'3';
    assert_simd_eq("[A-Z][0-9]", &hay);
}

#[test]
fn teddy3_three_char_classes() {
    assert_simd_eq("[a-z][0-9][A-Z]", b"___a5B___c7D___");
    let mut hay = vec![b'.'; 200];
    hay[100] = b'x';
    hay[101] = b'9';
    hay[102] = b'Z';
    assert_simd_eq("[a-z][0-9][A-Z]", &hay);
}

#[test]
fn empty_input() {
    assert_simd("abc", b"", &[]);
    assert_simd("[0-9]+", b"", &[]);
}

#[test]
fn input_shorter_than_pattern() {
    assert_simd("abcdef", b"abc", &[]);
    assert_simd("[A-Z][a-z]+", b"H", &[]);
}

#[test]
fn one_byte_input() {
    assert_simd("a", b"a", &[(0, 1)]);
    assert_simd("a", b"b", &[]);
    assert_simd("[0-9]", b"5", &[(0, 1)]);
}

#[test]
fn greedy_dot_star() {
    assert_simd_eq("a.*b", b"a---b---b");
    let mut hay = vec![b'-'; 200];
    hay[0] = b'a';
    hay[199] = b'b';
    assert_simd_eq("a.*b", &hay);
}

#[test]
fn non_overlapping_adjacent() {
    assert_simd("ab", b"ababab", &[(0, 2), (2, 4), (4, 6)]);
}

#[test]
fn ip_address_long_input() {
    let pattern = r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}";
    let mut hay = "padding ".repeat(100);
    hay.push_str("connect from 192.168.1.100 to 10.0.0.1");
    hay.push_str(&" padding".repeat(100));
    assert_simd_eq(pattern, hay.as_bytes());
}

#[test]
fn email_pattern() {
    let pattern = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}";
    assert_simd_eq(pattern, b"contact user@example.com or admin@test.org");
    let mut hay = "xxxx ".repeat(200);
    hay.push_str("test@foo.bar");
    hay.push_str(&" yyyy".repeat(200));
    assert_simd_eq(pattern, hay.as_bytes());
}

#[test]
fn html_tag_pattern() {
    assert_simd_eq(r"<h[1-6]>.*</h[1-6]>", b"<h1>Title</h1> and <h2>Sub</h2>");
}

#[test]
fn quoted_string() {
    let pattern = r#""[^"]*""#;
    assert_simd_eq(pattern, br#"say "hello" and "bye""#);
    let mut hay = vec![b'.'; 500];
    hay[200] = b'"';
    hay[201..206].copy_from_slice(b"inner");
    hay[206] = b'"';
    assert_simd_eq(pattern, &hay);
}

#[test]
fn anchored_start() {
    assert_simd_eq("^hello", b"hello world");
    assert_simd_eq("^hello", b"world hello");
}

#[test]
fn anchored_end() {
    assert_simd_eq("world$", b"hello world");
}

#[test]
fn anchored_both() {
    assert_simd_eq("^exact$", b"exact");
    assert_simd_eq("^exact$", b"not exact");
}

#[test]
fn lookahead_with_simd() {
    assert_simd_eq(r"a(?=b)", b"_ab_ab_");
    assert_simd_eq(r"\d+(?=[aA]\.?[mM]\.?)", b"10am");
}

#[test]
fn lookbehind_with_simd() {
    assert_simd_eq(r"(?<=b)a", b"bbbba");
    assert_simd_eq(r"(?<=author).*", b"author: abc and def");
}

#[test]
fn neg_lookbehind_with_simd() {
    assert_simd_eq(r"(?<!\d)a", b"1a__a__a");
}

#[test]
fn complement_simd() {
    assert_simd_eq(r"~(_*\d\d_*)", b"Aa11aBaAA");
}

#[test]
fn intersection_simd() {
    assert_simd_eq(r"c...&...s", b"raining cats and dogs");
}

#[test]
fn complement_intersection_simd() {
    assert_simd_eq(r"~(.*\d\d.*)&[a-zA-Z\d]{8,}", b"tej55zhA25wXu8bvQxFxt");
}

#[test]
fn multiline_simd() {
    assert_simd_eq(r"(?:.+\n)+\n", b"\naaa\n\nbbb\n\nccc\n\n");
}

#[test]
fn deep_alternation() {
    assert_simd_eq(
        "accommodating|acknowledging|comprehensive|corresponding|disappointing",
        b"a]comprehensive/disappointing;acknowledging",
    );
}

#[test]
fn alternation_factored_prefix() {
    assert_simd_eq("bar|baz", b"bar baz bar");
}

#[test]
fn alternation_with_suffix() {
    assert_simd_eq(r"(cat|dog)\d+", b"cat123 dog45 cat bird99");
}

#[test]
fn size_sweep_literal() {
    for size in (1..=200).step_by(7) {
        let mut hay = vec![b'.'; size];
        if size >= 5 {
            let pos = size / 2;
            hay[pos..pos + 3].copy_from_slice(b"abc");
        }
        assert_simd_eq("abc", &hay);
    }
}

#[test]
fn size_sweep_class() {
    for size in (1..=200).step_by(7) {
        let mut hay = vec![b'.'; size];
        if size >= 3 {
            hay[size / 2] = b'7';
        }
        assert_simd_eq("[0-9]+", &hay);
    }
}

#[test]
fn size_sweep_bounded_rep() {
    for size in (1..=200).step_by(3) {
        let mut hay = vec![b'.'; size];
        if size >= 6 {
            let pos = size / 2;
            hay[pos] = b'a';
            hay[pos + 1] = b'b';
            hay[pos + 2] = b'b';
            hay[pos + 3] = b'c';
        }
        assert_simd_eq("ab{2,4}c", &hay);
    }
}

#[test]
fn match_spans_chunk_boundary() {
    let mut hay = vec![b'.'; 64];
    hay[14..18].copy_from_slice(b"abcd");
    assert_simd("abcd", &hay, &[(14, 18)]);
    hay[14..18].fill(b'.');
    hay[15..19].copy_from_slice(b"abcd");
    assert_simd("abcd", &hay, &[(15, 19)]);
    hay[15..19].fill(b'.');
    hay[30..34].copy_from_slice(b"abcd");
    assert_simd("abcd", &hay, &[(30, 34)]);
}

#[test]
fn many_single_char_matches() {
    let hay = vec![b'a'; 500];
    let expected: Vec<(usize, usize)> = (0..500).map(|i| (i, i + 1)).collect();
    assert_simd("a", &hay, &expected);
}

#[test]
fn many_two_char_matches() {
    let hay = b"ababababababababababababababababababababababababababababababababab";
    let expected: Vec<(usize, usize)> = (0..hay.len()).step_by(2).map(|i| (i, i + 2)).collect();
    assert_simd("ab", hay, &expected);
}

#[test]
fn is_match_lazy_long_input() {
    let re = Regex::with_options("needle", lazy_opts()).unwrap();
    let mut hay = vec![b'.'; 10000];
    assert!(!re.is_match(&hay).unwrap());
    hay[9990..9996].copy_from_slice(b"needle");
    assert!(re.is_match(&hay).unwrap());
}

#[test]
fn is_match_class_lazy() {
    let re = Regex::with_options("[0-9]+", lazy_opts()).unwrap();
    let hay = vec![b'.'; 1000];
    assert!(!re.is_match(&hay).unwrap());
    let mut hay2 = vec![b'.'; 1000];
    hay2[500] = b'5';
    assert!(re.is_match(&hay2).unwrap());
}

#[test]
fn rev_range_skip_digit_sweep() {
    for pos in 0..64 {
        let mut hay = vec![b'.'; 64];
        hay[pos] = b'0' + (pos as u8 % 10);
        assert_simd_eq("[0-9]+", &hay);
    }
}

#[test]
fn rev_range_skip_uppercase_sweep() {
    for pos in 0..64 {
        let mut hay = vec![b'.'; 64];
        hay[pos] = b'A' + (pos as u8 % 26);
        assert_simd_eq("[A-Z]+", &hay);
    }
}

#[test]
fn rev_range_skip_two_ranges() {
    // hex digits: [0-9A-F]
    for pos in 0..64 {
        let mut hay = vec![b'.'; 64];
        hay[pos] = if pos % 2 == 0 {
            b'0' + (pos as u8 % 10)
        } else {
            b'A' + (pos as u8 % 6)
        };
        assert_simd_eq("[0-9A-F]+", &hay);
    }
}

#[test]
fn rev_range_skip_no_match_long() {
    let hay = vec![b'.'; 1024];
    assert_simd("[0-9]+", &hay, &[]);
    assert_simd("[A-Z]+", &hay, &[]);
}

#[test]
fn rev_range_skip_all_match() {
    let hay: Vec<u8> = (0..100).map(|i| b'0' + (i % 10)).collect();
    assert_simd_eq("[0-9]+", &hay);
}

#[test]
fn rev_range_skip_size_sweep() {
    for size in (1..=200).step_by(7) {
        let mut hay = vec![b'.'; size];
        if size >= 3 {
            hay[size / 2] = b'3';
        }
        assert_simd_eq("[0-9]+", &hay);
        if size >= 3 {
            hay[size / 2] = b'M';
        }
        assert_simd_eq("[A-Z]+", &hay);
    }
}

#[test]
#[ignore = "reimplement prefix selection first"]
fn range_skip_digit_plus_has_accel() {
    let re = Regex::with_options("[0-9]+", lazy_opts()).unwrap();
    let (_fwd, rev) = re.has_accel();
    assert!(rev, "[0-9]+ should have rev accel");
}

#[test]
#[ignore = "reimplement prefix selection first"]
fn range_skip_uppercase_plus_has_accel() {
    let re = Regex::with_options("[A-Z]+", lazy_opts()).unwrap();
    let (_fwd, rev) = re.has_accel();
    assert!(rev, "[A-Z]+ should have rev accel");
}

#[test]
fn range_skip_ip_address() {
    let pattern = r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}";
    let mut hay = vec![b' '; 500];
    hay[200..213].copy_from_slice(b"192.168.1.100");
    assert_simd_eq(pattern, &hay);
}

#[test]
fn range_skip_uppercase_in_long_input() {
    let pattern = "[A-Z]+";
    let mut hay = vec![b'.'; 2000];
    hay[500..503].copy_from_slice(b"ABC");
    hay[1500..1504].copy_from_slice(b"WXYZ");
    assert_simd_eq(pattern, &hay);
}
