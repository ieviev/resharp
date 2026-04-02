use resharp::{calc_potential_start, calc_prefix_sets, RegexBuilder};
use std::path::Path;

fn prefix_rev(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();

    let sets = calc_prefix_sets(&mut b, rev).unwrap();
    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

fn potential_rev(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let node = b.reverse(node).unwrap();
    // println!("PRE {:?}", b.pp(node));
    let node = b.prune_begin(node);
    let node = b.strip_prefix_safe(node);
    // println!("POST {:?}", b.pp(node));

    let sets = calc_potential_start(&mut b, node, 16, 64).unwrap();
    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

fn fwd_prefix_pp(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    // println!("PRE {:?}", b.pp(node));
    let node = b.prune_begin(node);
    let node = b.strip_prefix_safe(node);
    // println!("POST {:?}", b.pp(node));
    let sets = calc_prefix_sets(&mut b, node).unwrap();

    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

fn potential_fwd(pattern: &str) -> String {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let sets = calc_potential_start(&mut b, node, 16, 64).unwrap();
    sets.iter()
        .map(|&set| b.solver_ref().pp(set))
        .collect::<Vec<_>>()
        .join(";")
}

const KINDS: &[&str] = &["prefix_rev", "prefix_fwd", "potential_rev", "potential_fwd"];

struct PrefixTestCase {
    name: String,
    pattern: String,
    ignore: bool,
    checks: Vec<(&'static str, String)>,
}

fn load_prefix_tests() -> Vec<PrefixTestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("prefix.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| PrefixTestCase {
            name: t["name"].as_str().unwrap().to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
            checks: KINDS
                .iter()
                .filter_map(|&kind| {
                    t.get(kind)
                        .and_then(|v| v.as_str())
                        .map(|expected| (kind, expected.to_string()))
                })
                .collect(),
        })
        .collect()
}

#[test]
fn test_prefix_toml() {
    for tc in load_prefix_tests() {
        if tc.ignore {
            continue;
        }
        for (kind, expected) in &tc.checks {
            let result = match *kind {
                "prefix_rev" => prefix_rev(&tc.pattern),
                "prefix_fwd" => fwd_prefix_pp(&tc.pattern),
                "potential_rev" => potential_rev(&tc.pattern),
                "potential_fwd" => potential_fwd(&tc.pattern),
                k => panic!("unknown prefix test kind: {}", k),
            };
            assert_eq!(
                result, *expected,
                "prefix test failed: name={}, kind={}",
                tc.name, kind
            );
        }
    }
}

#[test]
fn prefix_bounded_repeat() {
    let p = prefix_rev("ab{2,4}c");
    assert_eq!(p, "c;b;b");
}

#[test]
fn prefix_dotdot_g() {
    let p = prefix_rev("..g");
    assert!(!p.is_empty(), "expected at least 1 prefix position");
    assert_eq!(p, "g;.;.");
}

#[test]
fn collect_rev_intersection_abc() {
    use resharp_algebra::TRegex;

    let pattern = ".*a.*&.*b.*&.*c.*";
    let mut b = resharp::RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let ts_rev = b.mk_concat(resharp::NodeId::TS, rev);

    eprintln!("rev node: {}", b.pp(rev));
    eprintln!("ts_rev node: {}", b.pp(ts_rev));
    eprintln!("ts_rev nullability: {:?}", b.nullability(ts_rev));

    fn resolve(
        b: &resharp::RegexBuilder,
        der_id: resharp_algebra::TRegexId,
        set: resharp::TSetId,
    ) -> resharp::NodeId {
        match b.get_tregex(der_id).clone() {
            TRegex::Leaf(n) => n,
            TRegex::ITE(cond, then_br, else_br) => {
                let s = b.solver_ref();
                let sat =
                    resharp_algebra::solver::Solver::is_sat(&s.get_set(set), &s.get_set(cond));
                if sat {
                    resolve(b, then_br, set)
                } else {
                    resolve(b, else_br, set)
                }
            }
        }
    }

    // walk "abc" in reverse: positions 2,1,0 reading c,b,a
    // first step uses BEGIN mask (like the DFA begin_table), rest use CENTER
    let input = b"abc";
    let mut current = ts_rev;
    for i in (0..input.len()).rev() {
        let byte = input[i];
        let byte_set = b.solver().u8_to_set_id(byte);
        let mask = if i == input.len() - 1 {
            resharp::Nullability::BEGIN
        } else {
            resharp::Nullability::CENTER
        };

        let der = b.der(current, mask).unwrap();
        let next = resolve(&b, der, byte_set);

        eprintln!(
            "pos={} byte='{}' mask={:?}: {} -> {}  nullable={:?}",
            i,
            byte as char,
            mask,
            b.pp(current),
            b.pp(next),
            b.nullability(next)
        );
        current = next;
    }

    eprintln!(
        "final node: {} nullability={:?} nulls_id={:?}",
        b.pp(current),
        b.nullability(current),
        b.get_nulls_id(current)
    );

    let re = resharp::Regex::new(pattern).unwrap();
    let nulls = re.collect_rev_nulls_debug(input);
    eprintln!("collect_rev nulls: {:?}", nulls);
    assert!(nulls.contains(&0), "expected 0 in nulls, got {:?}", nulls);
}

#[test]
fn debug_huck_complement_prefix() {
    use resharp::{calc_potential_start, RegexBuilder};
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, ".*Huck.*&~(.*F.*)").unwrap();
    let fwd_sets = calc_potential_start(&mut b, node, 16, 64).unwrap();
    let fwd_pp: Vec<_> = fwd_sets.iter().map(|&s| b.solver_ref().pp(s)).collect();
    eprintln!("fwd potential_start: {:?}", fwd_pp);
    let rev = b.reverse(node).unwrap();
    let rev_sets = calc_potential_start(&mut b, rev, 16, 64).unwrap();
    let rev_pp: Vec<_> = rev_sets.iter().map(|&s| b.solver_ref().pp(s)).collect();
    eprintln!("rev potential_start: {:?}", rev_pp);
}

#[test]
fn debug_datetime_potential_rev() {
    use resharp::{calc_potential_start, RegexBuilder};
    let pattern = r"\d+(?=[aA]\.?[mM]\.?)";
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let rev_stripped = b.prune_begin(rev);
    eprintln!("datetime rev stripped: {}", b.pp(rev_stripped));
    let sets = calc_potential_start(&mut b, rev_stripped, 16, 64).unwrap();
    let pp: Vec<_> = sets.iter().map(|&s| b.solver_ref().pp(s)).collect();
    eprintln!("datetime potential_rev: {:?}", pp);

    let email_pat = r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}";
    let mut b2 = RegexBuilder::new();
    let enode = resharp_parser::parse_ast(&mut b2, email_pat).unwrap();
    let enode_stripped = b2.prune_begin(enode);
    eprintln!("email fwd stripped: {}", b2.pp(enode_stripped));
    let esets = calc_potential_start(&mut b2, enode_stripped, 16, 64).unwrap();
    let epp: Vec<_> = esets.iter().map(|&s| b2.solver_ref().pp(s)).collect();
    eprintln!("email potential_fwd: {:?}", epp);
}
