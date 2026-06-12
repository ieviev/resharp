#![cfg(feature = "stream")]
mod common;
use common::schemas::EngineFile;
use resharp::Regex;
use std::path::Path;

#[test]
fn stream_matches_find_all_for_zero_rep_group_intersection() {
    for (pat, hay) in [
        (r"(?<=b)&(a){0}", &b"b"[..]),
        (r"(?<=b)&^{0}", &b"b"[..]),
        (r"((?<=b+){2}&(\n{2,}\w{1,3}){0}^{0})", &b"b"[..]),
    ] {
        let re = Regex::new(pat).unwrap();
        let fa: Vec<[usize; 2]> = re.find_all(hay).unwrap().iter().map(|m| [m.start, m.end]).collect();
        let st: Vec<[usize; 2]> = re.stream(hay).unwrap().iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(st, fa, "stream vs find_all diverge for {pat:?} on {hay:?}");
    }
}

#[test]
fn bug15_direct_no_catch() {
    let re = resharp::Regex::new("a&b").unwrap();
    let _ = re.stream(b"aaa");
}

#[test]
fn bug15_stream_no_panic_on_extended_operators() {
    let cases: &[(&str, &[u8])] = &[
        ("a&b",             b"aaa"),
        ("(a*&b)",          b"aaa"),
        ("( &c)",           b"aaa"),
        ("((?<! )\\D)",     b"abc"),
        ("((?![\\w])1)",    b"111"),
        ("((?!a) )+",       b"   "),
        ("\\z\\A.*",        b"abc"),
    ];
    for &(pat, hay) in cases {
        let re = Regex::new(pat).unwrap();
        let result = std::panic::catch_unwind(|| re.stream(hay));
        assert!(result.is_ok(), "pat={pat:?} hay={hay:?}: stream() panicked");
    }
}

#[test]
fn bug9_stream_nonempty_when_is_match_true() {
    let cases: &[(&str, &[u8])] = &[
        (r"\A\z?",  b"a"),
        (r"(?<!b)", b"b"),
        (r"\Bb",    b"ab"),
        (r"^\D*",   b"abc"),
    ];
    for &(pat, hay) in cases {
        let re = Regex::new(pat).unwrap();
        let im = re.is_match(hay).unwrap();
        let sv = re.stream(hay).unwrap();
        assert!(
            !im || !sv.is_empty(),
            "pat={pat:?} hay={hay:?}: is_match={im} but stream={sv:?}"
        );
    }
}

#[test]
fn repro_bug03_stream_phantom_zerowidth() {
    for (p, inp) in [
        (r"(?=c)", "c"),
        (r"\b", "ab"),
        (r"(?!\A)", "ab"),
        (r"^{0}", "b"),
        (r"(?<=b)", "b"),
        (r"(?<=b+){2}", "b"),
    ] {
        let re = Regex::new(p).unwrap();
        let fa: Vec<[usize;2]> = re.find_all(inp.as_bytes()).unwrap().iter().map(|m|[m.start,m.end]).collect();
        let st: Vec<[usize;2]> = re.stream(inp.as_bytes()).unwrap().iter().map(|m|[m.start,m.end]).collect();
        assert_eq!(st, fa, "stream must match find_all for zero-width {p} on {inp}");
    }
}

#[test]
fn stream_toml() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("stream.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let file: EngineFile = toml::from_str(&content).unwrap();
    for tc in file.test {
        let input = tc.input.as_bytes();
        let re = Regex::new(&tc.pattern).unwrap_or_else(|e| panic!("{}: compile: {e}", tc.name));
        let s = re.stream(input).unwrap();
        let got: Vec<[usize; 2]> = s.iter().map(|m| [m.start, m.end]).collect();
        assert_eq!(
            got, tc.matches,
            "name={} pattern={:?} input={:?}",
            tc.name, tc.pattern, tc.input
        );
        if tc.vs_find_all {
            let f = re.find_all(input).unwrap();
            assert_eq!(s, f, "name={} stream != find_all", tc.name);
        }
    }
}

#[test]
fn test_stream_prefix_skip_helps() {
    let mut data = Vec::with_capacity(2_000_000);
    for _ in 0..50_000 {
        data.extend_from_slice(b"............................................");
        data.extend_from_slice(b"Id=\"42\" .");
    }
    let re = Regex::new(r#"Id="\d+""#).unwrap();
    let m = re.stream(&data).unwrap();
    assert_eq!(m.len(), 50_000);
}

#[test]
fn test_stream_with_callback() {
    let r = Regex::new(r"\d+").unwrap();
    let input = b"a12 b34 c5 d6789";
    let want = r.stream(input).unwrap();
    let mut got = Vec::new();
    r.stream_with(input, |m| got.push(m)).unwrap();
    assert_eq!(got, want);

    let mut count = 0usize;
    r.stream_with(input, |_| count += 1).unwrap();
    assert_eq!(count, want.len());

    let mut fired = false;
    r.stream_with(b"", |_| fired = true).unwrap();
    assert!(!fired);
}

#[test]
fn test_cross_chunk_boundary() {
    let r = resharp::Regex::new("abcdef").unwrap();
    let mut got = Vec::new();
    let mut state = resharp::StreamState::new();
    for chunk in [b"abc".as_slice(), b"def"] {
        state = r.stream_chunk(chunk, state, |e| got.push(e)).unwrap();
    }
    let want = r.stream_ends(b"abcdef").unwrap();
    assert_eq!(got, want);
}

#[test]
fn test_stream_chunk() {
    let r = Regex::new(r"\d+").unwrap();
    let input = b"a12 b34 c5 d6789";

    let want = r.stream_ends(input).unwrap();

    for chunk_size in [1, 2, 3, 4, 7, 16, input.len()] {
        let mut got = Vec::new();
        let mut state = resharp::StreamState::new();
        for chunk in input.chunks(chunk_size) {
            state = r.stream_chunk(chunk, state, |e| got.push(e)).unwrap();
        }
        assert_eq!(got, want, "chunk_size={chunk_size}");
    }
}

#[test]
fn seek_fwd_rev_cursor() {
    let re = Regex::new("a[bc]+d").unwrap();
    let input = b"xx abcd yy abbcd zz acd ww abd";
    let stream_matches: Vec<(usize, usize)> = re
        .stream(input)
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end))
        .collect();

    let mut fwd: Vec<usize> = Vec::new();
    let (mut s, mut p) = (Regex::SEEK_INITIAL, 0usize);
    while let Some((ns, end)) = re.seek_fwd(input, s, p).unwrap() {
        fwd.push(end);
        s = ns;
        p = end;
    }
    let want_ends: Vec<usize> = stream_matches.iter().map(|m| m.1).collect();
    assert_eq!(fwd, want_ends, "seek_fwd ends");

    let mut rev: Vec<usize> = Vec::new();
    let (mut s, mut p) = (Regex::SEEK_INITIAL, input.len());
    while let Some((ns, start)) = re.seek_rev(input, s, p).unwrap() {
        rev.push(start);
        s = ns;
        p = start;
    }
    let mut want_starts: Vec<usize> = stream_matches.iter().map(|m| m.0).collect();
    want_starts.reverse();
    assert_eq!(rev, want_starts, "seek_rev starts");
}

#[test]
fn seek_fwd_from_middle() {
    let re = Regex::new("lookaround").unwrap();
    let input = b"foo lookaround bar baz lookaround qux end";
    let mid = 20;
    let (_, end) = re
        .seek_fwd(input, Regex::SEEK_INITIAL, mid)
        .unwrap()
        .unwrap();
    assert_eq!(end, 33);
    assert_eq!(&input[end - 10..end], b"lookaround");
}

#[test]
fn seek_rev_from_middle() {
    let re = Regex::new("lookaround").unwrap();
    let input = b"foo lookaround bar baz lookaround qux end";
    let mid = 20;
    let (_, start) = re
        .seek_rev(input, Regex::SEEK_INITIAL, mid)
        .unwrap()
        .unwrap();
    assert_eq!(start, 4);
    assert_eq!(&input[start..start + 10], b"lookaround");
}

#[test]
fn seek_no_match() {
    let re = Regex::new("zzz").unwrap();
    let input = b"the quick brown fox jumps over the lazy dog";
    assert!(re
        .seek_fwd(input, Regex::SEEK_INITIAL, 10)
        .unwrap()
        .is_none());
    assert!(re
        .seek_rev(input, Regex::SEEK_INITIAL, 30)
        .unwrap()
        .is_none());
}

#[test]
fn seek_fwd_skips_match_before_pos() {
    let re = Regex::new("abcdef").unwrap();
    let input = b"xx abcdef yy abcdef zz";
    let (_, end) = re.seek_fwd(input, Regex::SEEK_INITIAL, 0).unwrap().unwrap();
    assert_eq!(end, 9);
    let (_, end) = re.seek_fwd(input, Regex::SEEK_INITIAL, 5).unwrap().unwrap();
    assert_eq!(end, 19);
    assert!(re
        .seek_fwd(input, Regex::SEEK_INITIAL, 20)
        .unwrap()
        .is_none());
}

#[test]
fn seek_fwd_with_class_pattern() {
    let re = Regex::new(r"\d+").unwrap();
    let input = b"abc 123 def 4567 ghi 89 jkl";
    let mut ends = Vec::new();
    let (mut s, mut p) = (Regex::SEEK_INITIAL, 8usize);
    while let Some((ns, e)) = re.seek_fwd(input, s, p).unwrap() {
        ends.push(e);
        s = ns;
        p = e;
    }
    assert_eq!(ends, vec![13, 14, 15, 16, 22, 23]);
}
