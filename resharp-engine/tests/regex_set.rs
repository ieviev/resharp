use resharp::{Error, RegexOptions, RegexSet};

fn cat(pats: &[&str], input: &str) -> Vec<(usize, usize, usize)> {
    cat_with(pats, input, RegexOptions::default())
}

fn cat_disjoint(pats: &[&str], input: &str) -> Vec<(usize, usize, usize)> {
    cat_with(pats, input, RegexOptions::default().regexset_enforce_disjoint(true))
}

fn cat_with(pats: &[&str], input: &str, opts: RegexOptions) -> Vec<(usize, usize, usize)> {
    let set = RegexSet::with_options(pats, opts).unwrap();
    set.classify_all(input.as_bytes())
        .unwrap()
        .iter()
        .map(|m| (m.start, m.end, m.tag))
        .collect()
}

#[test]
fn set_basic_classify() {
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
    let cats: Vec<_> = set.classify_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
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
fn set_trailing_lookahead_shared_prefix_reports_lowest_index() {
    assert_eq!(cat(&["q(?=rs)", "q(?=r)"], "qrs"), vec![(0, 1, 0)]);
    assert_eq!(cat(&["q(?=rs)", "q(?=r)"], "qrx"), vec![(0, 1, 1)]);
    assert_eq!(cat(&["a(?=bcd)", "a(?=b)"], "abcd abc"), vec![(0, 1, 0), (5, 6, 1)]);
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
    let m = set.classify_all(b"k7 k199 k42x k0").unwrap();
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
fn set_lookbehind_member_must_be_disjoint() {
    let opts = || RegexOptions::default().regexset_enforce_disjoint(true);
    assert!(matches!(RegexSet::with_options(["(?<=a)c", "c"], opts()), Err(Error::SetMember(1, _))));
    assert!(matches!(RegexSet::with_options(["\\bc", "c"], opts()), Err(Error::SetMember(1, _))));
    assert!(matches!(RegexSet::with_options(["(?<g>c)"], opts()), Err(Error::SetMember(0, _))));
    assert!(RegexSet::with_options(["(?<=a)c", "d"], opts()).is_ok());
}

#[test]
fn set_leading_boundary_member_attributes_only_when_context_holds() {
    assert_eq!(cat_disjoint(&["\\bc", "d"], "c d xc dc"), vec![(0, 1, 0), (2, 3, 1), (7, 8, 1)]);
    assert_eq!(cat_disjoint(&["\\bc", "d"], "ac bc"), vec![]);
    assert_eq!(cat_disjoint(&["^c", "d"], "c cd dc"), vec![(0, 1, 0), (3, 4, 1), (5, 6, 1)]);
    assert_eq!(cat_disjoint(&["\\bc\\b", "d"], "c cc d"), vec![(0, 1, 0), (5, 6, 1)]);
}

#[test]
fn set_lookbehind_member_attributes_only_when_context_holds() {
    assert_eq!(cat_disjoint(&["(?<=a)c", "d"], "bc d ac"), vec![(3, 4, 1), (6, 7, 0)]);
    assert_eq!(cat_disjoint(&["(?<=a)bc", "cd"], "bc abc cd"), vec![(4, 6, 0), (7, 9, 1)]);
    assert_eq!(cat_disjoint(&["(?<=a)c", "dd"], "bc ac dd"), vec![(4, 5, 0), (6, 8, 1)]);
}

#[test]
fn set_empty_set() {
    let set = RegexSet::new(Vec::<&str>::new()).unwrap();
    assert!(set.is_empty());
    assert_eq!(set.classify_all(b"abc").unwrap(), vec![]);
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
#[ignore = "slow randomized crosscheck; run explicitly with --ignored"]
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
            let got = set.classify_all(input.as_bytes()).unwrap();
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

#[test]
fn set_classify_tag_decided_by_lookahead_at_end() {
    assert_eq!(cat(&["ab(?=c)", "ab"], "abc ab"), vec![(0, 2, 0), (4, 6, 1)]);
    assert_eq!(cat(&["ab(?=cd)", "a[b-z]+"], "abcd"), vec![(0, 4, 1)]);
}

#[test]
fn set_classify_spans_match_members_on_lexer_input() {
    let pats = [
        r"(?:\r\n|\r|\n)",
        r"[\t\v\f ]+",
        r"//.*",
        r"[0-9]+(?:\_[0-9]+)*",
        r"[a-zA-Z\_][0-9a-zA-Z\_]*",
        r"assign",
        r"always\_ff",
        r"module",
        r"posedge",
        r"\+",
        r"=",
        r"<=",
        r";",
        r"@",
        r"\(",
        r"\)",
        r"\.",
        r",",
        r"\*",
    ];
    let set = RegexSet::new(&pats).unwrap();
    let input = b"module m; assign a = b + 12; always_ff @(posedge clk) q <= d; // x\n";
    let spans: Vec<_> = set.find_all(input).unwrap().iter().map(|m| (m.start, m.end)).collect();
    let classified = set.classify_all(input).unwrap();
    let cats: Vec<_> = classified.iter().map(|m| (m.start, m.end)).collect();
    assert_eq!(spans, cats);
    let mut cache = std::collections::HashMap::new();
    for m in classified {
        assert!(
            member_matches_span(&mut cache, pats[m.tag], input, m.start, m.end),
            "tag {} does not match span {:?}",
            m.tag,
            (m.start, m.end)
        );
    }
}

fn labels(pats: &[&str], input: &str) -> Vec<(usize, usize, Vec<usize>)> {
    let set = RegexSet::new(pats).unwrap();
    set.label_all(input.as_bytes())
        .unwrap()
        .iter()
        .map(|(m, tags)| (m.start, m.end, tags.to_vec()))
        .collect()
}

#[test]
fn set_label_all_reports_every_member_on_span() {
    assert_eq!(
        labels(&["ab", "[a-z]+", "c"], "ab c"),
        vec![(0, 2, vec![0, 1]), (3, 4, vec![1, 2])]
    );
    assert_eq!(labels(&["x", "y"], "ab"), vec![]);
    assert_eq!(labels(&[], "ab"), vec![]);
}

#[test]
fn set_label_all_lowest_matches_classify_all() {
    let pats = ["ab+", "a[a-z]*", "c", "[0-9]+", "[0-9]{2}", "abb"];
    let input = "xx abbb c 12 zz 123 abz";
    let set = RegexSet::new(pats).unwrap();
    let classified = set.classify_all(input.as_bytes()).unwrap();
    let labelled = set.label_all(input.as_bytes()).unwrap();
    assert_eq!(labelled.len(), classified.len());
    assert_eq!(labelled.is_empty(), classified.is_empty());
    for (c, (m, tags)) in classified.iter().zip(labelled.iter()) {
        assert_eq!((c.start, c.end), (m.start, m.end));
        assert_eq!(tags.first(), Some(&c.tag));
        assert!(tags.windows(2).all(|w| w[0] < w[1]));
    }
}

#[test]
fn set_label_all_empty_input_and_no_members_match() {
    assert_eq!(labels(&["a"], ""), vec![]);
    assert_eq!(labels(&["a", "b"], "cc"), vec![]);
}

#[test]
fn set_label_all_identical_spans_share_member_set() {
    assert_eq!(
        labels(&["ab", "a."], "ab ab"),
        vec![(0, 2, vec![0, 1]), (3, 5, vec![0, 1])]
    );
}

#[test]
fn set_label_all_anchored_member_only_on_its_span() {
    assert_eq!(
        labels(&["\\Aab", "ab"], "ab ab"),
        vec![(0, 2, vec![0, 1]), (3, 5, vec![1])]
    );
}

#[test]
fn set_label_all_lookahead_member_requires_context() {
    assert_eq!(
        labels(&["a(?=b)", "a"], "ab a"),
        vec![(0, 1, vec![0, 1]), (3, 4, vec![1])]
    );
}

#[test]
fn set_label_all_many_members_subset_per_span() {
    assert_eq!(
        labels(&["[a-z]+", "[0-9]+", "abc", "c"], "abc 42"),
        vec![(0, 3, vec![0, 2]), (4, 6, vec![1])]
    );
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
