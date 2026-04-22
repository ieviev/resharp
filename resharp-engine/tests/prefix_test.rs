use resharp::{PrefixSets, RegexBuilder};
use resharp_algebra::solver::TSetId;
use std::path::Path;

fn make_prefix_sets(pattern: &str) -> (RegexBuilder, PrefixSets) {
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let rev = b.reverse(node).unwrap();
    let sets = PrefixSets::compute(&mut b, node, rev).unwrap();
    (b, sets)
}

fn pp_sets(b: &RegexBuilder, sets: &[TSetId]) -> String {
    sets.iter()
        .map(|&s| b.solver_ref().pp(s))
        .collect::<Vec<_>>()
        .join(";")
}

const KINDS: &[&str] = &[
    "prefix_rev",
    "prefix_fwd",
    "potential_rev",
    "potential_fwd",
    "kind",
];

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
        let needs_sets = tc.checks.iter().any(|(k, _)| *k != "kind");
        let sets_pair = needs_sets.then(|| make_prefix_sets(&tc.pattern));
        let kind_result = tc.checks.iter().find(|(k, _)| *k == "kind").map(|_| {
            resharp::Regex::new(&tc.pattern)
                .unwrap()
                .prefix_kind_name()
                .unwrap_or("None")
                .to_string()
        });

        for (kind, expected) in &tc.checks {
            let result = if *kind == "kind" {
                kind_result.clone().unwrap()
            } else {
                let (b, sets) = sets_pair.as_ref().unwrap();
                match *kind {
                    "prefix_rev" => pp_sets(b, &sets.rev_anchored),
                    "prefix_fwd" => pp_sets(b, &sets.fwd_anchored),
                    "potential_rev" => pp_sets(b, &sets.rev_potential),
                    "potential_fwd" => pp_sets(b, &sets.fwd_potential),
                    k => panic!("unknown prefix test kind: {}", k),
                }
            };
            assert_eq!(
                result, *expected,
                "prefix test failed: name={}, kind={}",
                tc.name, kind
            );
        }
    }
}
