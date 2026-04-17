use resharp::{NodeId, RegexBuilder};
use resharp_algebra::nulls::Nullability;
use std::path::Path;

struct TestCase {
    name: String,
    pattern: String,
    ignore: bool,
    input: String,
    rev_nulls: Vec<Option<String>>,
}

fn parse_expected(t: &toml::Value, key: &str) -> Vec<Option<String>> {
    t.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|e| {
                    let s = e.as_str().unwrap();
                    if s == "?" {
                        None
                    } else {
                        Some(s.to_string())
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn load_tests() -> Vec<TestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("rev_nulls.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| TestCase {
            name: t["name"].as_str().unwrap().to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
            input: t
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            rev_nulls: parse_expected(t, "rev_nulls"),
        })
        .collect()
}

fn walk_rev(
    b: &mut RegexBuilder,
    mut node: NodeId,
    bytes: &[u8],
    expected: &[Option<String>],
    name: &str,
) {
    assert_eq!(
        bytes.len(),
        expected.len(),
        "input length must match rev_nulls length for {name}"
    );
    let n = bytes.len();
    for (i, byte) in bytes.iter().enumerate() {
        let mask = if i == 0 {
            Nullability::BEGIN
        } else if i == n - 1 {
            Nullability::END
        } else {
            Nullability::CENTER
        };
        let tset = b.solver().u8_to_set_id(*byte);
        let tregex = b.der(node, mask).unwrap();
        let next = b.transition_term(tregex, tset);
        let pp = b.pp(next);
        let nulls_pp = b.pp_nulls(next);
        eprintln!(
            "  [rev] step={} byte='{}' (0x{:02x}) node={:?} nulls={} => {}",
            i,
            *byte as char,
            byte,
            next,
            nulls_pp,
            if pp.len() > 40 {
                format!("{}...", &pp[..40])
            } else {
                pp.clone()
            }
        );
        if let Some(exp) = &expected[i] {
            assert_eq!(
                nulls_pp, *exp,
                "nulls mismatch: name={} step={} byte='{}'",
                name, i, *byte as char
            );
        }
        node = next;
    }
}

#[test]
fn test_rev_nulls_toml() {
    for tc in load_tests() {
        if tc.ignore {
            continue;
        }
        let mut b = RegexBuilder::new();
        let node = resharp_parser::parse_ast(&mut b, &tc.pattern).unwrap();
        let rev = b.reverse(node).unwrap();

        // let rev_ts = b.mk_concat(NodeId::TS, rev);
        let bytes: Vec<u8> = tc.input.as_bytes().iter().rev().copied().collect();
        walk_rev(&mut b, rev, &bytes, &tc.rev_nulls, &tc.name);
    }
}
