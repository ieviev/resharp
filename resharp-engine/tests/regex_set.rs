use resharp::{Error, RegexSet};

fn cat(pats: &[&str], input: &str) -> Vec<(usize, usize, usize)> {
    let set = RegexSet::new(pats).unwrap();
    set.categorize_all(input.as_bytes())
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end, m.tag))
        .collect()
}

#[test]
fn set_basic_categorize() {
    assert_eq!(
        cat(&["ab+", "a[a-z]*", "c", "[0-9]+", "[0-9]{2}"], "xx abbb c 12 zz 123 abz"),
        vec![(3, 7, 0), (8, 9, 2), (10, 12, 3), (16, 19, 3), (20, 23, 1)]
    );
}

#[test]
fn set_spans_equal_union_find_all() {
    let pats = ["ab+", "a[a-z]*", "c", "[0-9]+"];
    let set = RegexSet::new(pats).unwrap();
    let input = b"xx abbb c 12 zz 123 abz";
    let spans: Vec<_> = set.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
    let cats: Vec<_> = set.categorize_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(spans, cats);
    let union = resharp::Regex::new("ab+|a[a-z]*|c|[0-9]+").unwrap();
    let u: Vec<_> = union.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(spans, u);
}

#[test]
fn set_tie_reports_lowest_index() {
    assert_eq!(cat(&["[a-z]+", "abc"], "abc"), vec![(0, 3, 0)]);
    assert_eq!(cat(&["abc", "[a-z]+"], "abc"), vec![(0, 3, 0)]);
    assert_eq!(cat(&["abc", "[a-z]+"], "abcd"), vec![(0, 4, 1)]);
}

#[test]
fn set_trailing_word_boundary() {
    assert_eq!(
        cat(&["[a-z]+\\b", "[0-9]+"], "foo bar1 7"),
        vec![(0, 3, 0), (7, 8, 1), (9, 10, 1)]
    );
    assert_eq!(cat(&["c\\b", "c"], "cc c"), vec![(0, 1, 1), (1, 2, 0), (3, 4, 0)]);
}

#[test]
fn set_trailing_lookahead_distinguishes_members() {
    assert_eq!(
        cat(&["a(?=x)", "a(?=y)", "a"], "ax ay az a"),
        vec![(0, 1, 0), (3, 4, 1), (6, 7, 2), (9, 10, 2)]
    );
}

#[test]
fn set_anchors() {
    assert_eq!(cat(&["\\Aa", "a$", "a"], "a a a"), vec![(0, 1, 0), (2, 3, 2), (4, 5, 1)]);
    assert_eq!(cat(&["\\Aa", "a$", "a"], "a"), vec![(0, 1, 0)]);
}

#[test]
fn set_empty_input_and_empty_match() {
    assert_eq!(cat(&["x", ""], ""), vec![(0, 0, 1)]);
    assert_eq!(cat(&["x", "y*"], ""), vec![(0, 0, 1)]);
    assert_eq!(cat(&["x"], ""), vec![]);
}

#[test]
fn set_many_members() {
    let pats: Vec<String> = (0..200).map(|i| format!("k{i}\\b")).collect();
    let set = RegexSet::new(&pats).unwrap();
    let m = set.categorize_all(b"k7 k199 k42x k0").unwrap();
    let got: Vec<_> = m.iter().map(|m| (m.start, m.end, m.tag)).collect();
    assert_eq!(got, vec![(0, 2, 7), (3, 7, 199), (13, 15, 0)]);
}

#[test]
fn set_rejects_lookbehind_and_captures() {
    assert!(matches!(RegexSet::new(["\\bc", "c"]), Err(Error::SetMember(0, _))));
    assert!(matches!(RegexSet::new(["c", "(?<=a)c"]), Err(Error::SetMember(1, _))));
    assert!(matches!(RegexSet::new(["(?<g>c)"]), Err(Error::SetMember(0, _))));
    assert!(matches!(RegexSet::new(["^c"]), Err(Error::SetMember(0, _))));
    assert!(RegexSet::new(["c\\b", "c(?=d)", "\\Ac", "c$"]).is_ok());
}

#[test]
fn set_empty_set() {
    let set = RegexSet::new(Vec::<&str>::new()).unwrap();
    assert!(set.is_empty());
    assert_eq!(set.categorize_all(b"abc").unwrap(), vec![]);
    assert_eq!(set.matched(b"abc").unwrap(), vec![]);
    assert_eq!(set.matched(b"").unwrap(), vec![]);
}

fn member_matches_span(
    cache: &mut std::collections::HashMap<String, resharp::Regex>,
    pat: &str,
    input: &[u8],
    s: usize,
    e: usize,
) -> bool {
    let n = input.len() - e;
    let pat = match pat.strip_prefix("\\A") {
        Some(rest) if s == 0 => rest,
        Some(_) => return false,
        None => pat,
    };
    let anchored = format!("\\A_{{{s}}}(?:{pat})_{{{n}}}\\z");
    let re = cache
        .entry(anchored.clone())
        .or_insert_with(|| resharp::Regex::new(&anchored).unwrap_or_else(|e| panic!("{anchored}: {e}")));
    re.is_match(input).unwrap()
}

#[test]
fn set_crosscheck_against_anchored_members() {
    let pool = [
        "a", "ab", "b+", "a[ab]*", "ba", "a(?=b)", "b(?=a)", "b\\b", "a$", "\\Aab", "[ab]{2}",
        "a|bb", "b(?=b)", "(a|b)b", "a\\b", "b*a", "[ab]+\\b", "a(?!b)", "ab(?= )", " a",
    ];
    let inputs = ["ab ba bb a", "aabb ab", "b", "abab a", "ba ab ba", "bbb a b", "a", "ab", ""];
    let mut seed = 0x9E3779B9u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut cache = std::collections::HashMap::new();
    for _ in 0..150 {
        let k = 1 + (next() % 5) as usize;
        let pats: Vec<&str> = (0..k).map(|_| pool[(next() % pool.len() as u64) as usize]).collect();
        let set = match RegexSet::new(&pats) {
            Ok(s) => s,
            Err(e) => panic!("pats={pats:?}: {e}"),
        };
        for input in inputs {
            let got = set.categorize_all(input.as_bytes()).unwrap();
            for m in got {
                let expect = pats
                    .iter()
                    .position(|p| member_matches_span(&mut cache, p, input.as_bytes(), m.start, m.end))
                    .unwrap_or_else(|| panic!("no member matches {:?} on {input:?} pats={pats:?}", (m.start, m.end)));
                assert_eq!(
                    m.tag, expect,
                    "pats={pats:?} input={input:?} span={:?}",
                    (m.start, m.end)
                );
            }
        }
    }
}

fn matched(pats: &[&str], input: &str) -> Vec<usize> {
    RegexSet::new(pats).unwrap().matched(input.as_bytes()).unwrap()
}

#[test]
fn set_matched_is_independent_of_union_spans() {
    assert_eq!(matched(&["[a-z]+", "abc"], "abc"), vec![0, 1]);
    assert_eq!(matched(&["ab", "b"], "ab"), vec![0, 1]);
    assert_eq!(matched(&["ab", "b", "x"], "ab"), vec![0, 1]);
    assert_eq!(matched(&["x", "y"], "ab"), vec![]);
    assert_eq!(matched(&["a(?=x)", "a(?=y)", "a\\b"], "ax a"), vec![0, 2]);
    assert_eq!(matched(&["\\Aa", "a$", "a"], "b a"), vec![1, 2]);
    assert_eq!(matched(&["x", ""], ""), vec![1]);
    assert_eq!(matched(&["x", "\\A"], "x"), vec![0, 1]);
    assert_eq!(matched(&["x", "\\z"], "y"), vec![1]);
}

#[test]
fn set_matched_crosscheck_against_members() {
    let pool = [
        "a", "ab", "b+", "a[ab]*", "ba", "a(?=b)", "b(?=a)", "b\\b", "a$", "\\Aab", "[ab]{2}",
        "a|bb", "b(?=b)", "a\\b", "b*a", "[ab]+\\b", "a(?!b)", "ab(?= )", " a", "\\z", "",
    ];
    let inputs = ["ab ba bb a", "aabb ab", "b", "abab a", "ba ab ba", "bbb a b", "a", "ab", ""];
    let singles: Vec<resharp::Regex> = pool.iter().map(|p| resharp::Regex::new(p).unwrap()).collect();
    let mut seed = 0x51ED270Bu64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..150 {
        let k = 1 + (next() % 5) as usize;
        let idx: Vec<usize> = (0..k).map(|_| (next() % pool.len() as u64) as usize).collect();
        let pats: Vec<&str> = idx.iter().map(|&i| pool[i]).collect();
        let set = RegexSet::new(&pats).unwrap();
        for input in inputs {
            let got = set.matched(input.as_bytes()).unwrap();
            let expect: Vec<usize> = idx
                .iter()
                .enumerate()
                .filter(|(_, &i)| singles[i].is_match(input.as_bytes()).unwrap())
                .map(|(j, _)| j)
                .collect();
            assert_eq!(got, expect, "pats={pats:?} input={input:?}");
        }
    }
}
