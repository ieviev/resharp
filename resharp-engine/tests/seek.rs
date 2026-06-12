#![cfg(feature = "stream")]
use resharp::Regex;

fn build_input() -> (Vec<u8>, Vec<usize>) {
    let mut buf = Vec::new();
    let mut ends = Vec::new();
    for i in 0..5 {
        buf.extend_from_slice(format!("line {i} no match here\n").as_bytes());
        let s = format!("line {i} ERROR something\n");
        let start = buf.len();
        buf.extend_from_slice(s.as_bytes());
        let off = s.find("ERROR").unwrap();
        ends.push(start + off + "ERROR".len());
    }
    (buf, ends)
}

#[test]
fn seek_fwd_walks_all_matches() {
    let re = Regex::new(r"\bERROR\b").unwrap();
    let (input, expected_ends) = build_input();

    let mut state = Regex::SEEK_INITIAL;
    let mut pos = 0usize;
    let mut got = Vec::new();
    while let Some((s, end)) = re.seek_fwd(&input, state, pos).unwrap() {
        got.push(end);
        state = s;
        pos = end;
    }
    assert_eq!(got, expected_ends);
}

#[test]
fn seek_rev_walks_all_matches_rightmost_first() {
    let re = Regex::new(r"\bERROR\b").unwrap();
    let (input, expected_ends) = build_input();
    let expected_starts: Vec<usize> = expected_ends
        .iter()
        .rev()
        .map(|e| e - "ERROR".len())
        .collect();

    let mut state = Regex::SEEK_INITIAL;
    let mut pos = input.len();
    let mut got = Vec::new();
    while let Some((s, start)) = re.seek_rev(&input, state, pos).unwrap() {
        got.push(start);
        state = s;
        pos = start;
    }
    assert_eq!(got, expected_starts);
}

#[test]
fn seek_fwd_respects_word_boundary() {
    let re = Regex::new(r"\bERROR\b").unwrap();
    let input = b"xERRORx ERROR yERRORy ERROR.";
    let mut got = Vec::new();
    let mut state = Regex::SEEK_INITIAL;
    let mut pos = 0;
    while let Some((s, end)) = re.seek_fwd(input, state, pos).unwrap() {
        got.push(end);
        state = s;
        pos = end;
    }
    assert_eq!(got, vec![13, 27]);
}

#[test]
fn seek_fwd_from_offset_skips_earlier_matches() {
    let re = Regex::new(r"\bERROR\b").unwrap();
    let input = b"ERROR aaa ERROR bbb ERROR";
    assert!(matches!(
        re.seek_fwd(input, Regex::SEEK_INITIAL, 6).unwrap(),
        Some((_, 15))
    ));
    assert!(matches!(
        re.seek_fwd(input, Regex::SEEK_INITIAL, 16).unwrap(),
        Some((_, 25))
    ));
    assert_eq!(re.seek_fwd(input, Regex::SEEK_INITIAL, 25).unwrap(), None);
}

#[test]
fn seek_rev_from_offset_skips_later_matches() {
    let re = Regex::new(r"\bERROR\b").unwrap();
    let input = b"ERROR aaa ERROR bbb ERROR";
    assert!(matches!(
        re.seek_rev(input, Regex::SEEK_INITIAL, 10).unwrap(),
        Some((_, 0))
    ));
    // assert!(matches!(re.seek_rev(input, Regex::SEEK_INITIAL, 25).unwrap(), Some((_, 20))));
    // assert!(matches!(re.seek_rev(input, Regex::SEEK_INITIAL, 20).unwrap(), Some((_, 10))));
    assert_eq!(re.seek_rev(input, Regex::SEEK_INITIAL, 0).unwrap(), None);
}
