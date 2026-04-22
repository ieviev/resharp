use resharp::{NodeId, RegexBuilder};
use resharp_algebra::nulls::Nullability;
use std::path::Path;

struct DerivTestCase {
    name: String,
    pattern: String,
    ignore: bool,
    input: String,
    rev: Vec<Option<String>>,
    fwd: Vec<Option<String>>,
    rev_nulls: Option<Vec<usize>>,
    fwd_nulls: Option<Vec<usize>>,
}

fn parse_null_positions(t: &toml::Value, key: &str) -> Option<Vec<usize>> {
    t.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .map(|e| e.as_integer().expect("null pos must be integer") as usize)
            .collect()
    })
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

fn load_tests() -> Vec<DerivTestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("deriv.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| DerivTestCase {
            name: t["name"].as_str().unwrap().to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
            input: t
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            rev: parse_expected(t, "rev"),
            fwd: parse_expected(t, "fwd"),
            rev_nulls: parse_null_positions(t, "rev_nulls"),
            fwd_nulls: parse_null_positions(t, "fwd_nulls"),
        })
        .collect()
}

fn pos_mask(pos: usize, n: usize) -> Nullability {
    if n == 0 {
        Nullability::BEGIN.or(Nullability::END)
    } else if pos == 0 {
        Nullability::BEGIN
    } else if pos == n {
        Nullability::END
    } else {
        Nullability::CENTER
    }
}

fn walk_bytes(
    b: &mut RegexBuilder,
    mut node: NodeId,
    bytes: &[u8],
    expected: &[Option<String>],
    expected_nulls: Option<&[usize]>,
    dir: &str,
    name: &str,
) {
    assert_eq!(
        bytes.len(),
        expected.len(),
        "input length must match {dir} expected length for {name}"
    );
    let n = bytes.len();
    let report_null = |b: &mut RegexBuilder, node: NodeId, pos: usize, label: &str| -> bool {
        let mask = pos_mask(pos, n);
        let null = b.nullability(node).has(mask);
        eprintln!(
            "  [{}] {} pos={} mask={:?} nullable={}",
            dir, label, pos, mask, null
        );
        null
    };
    let mut got_nulls: Vec<usize> = Vec::new();
    if report_null(b, node, 0, "initial") {
        got_nulls.push(0);
    }
    for (i, byte) in bytes.iter().enumerate() {
        let der_mask = pos_mask(i, n);
        let tset = b.solver().u8_to_set_id(*byte);
        let tregex = b.der(node, der_mask).unwrap();
        let next = b.transition_term(tregex, tset);
        let pp = b.pp(next);
        eprintln!(
            "  [{}] step={} byte='{}' (0x{:02x}) der_mask={:?} node={:?} => {}",
            dir, i, *byte as char, byte, der_mask, next, pp
        );
        if let Some(exp) = &expected[i] {
            assert_eq!(
                pp, *exp,
                "deriv pp mismatch: name={} dir={} step={} byte='{}'",
                name, dir, i, *byte as char
            );
        }
        node = next;
        if report_null(b, node, i + 1, "after") {
            got_nulls.push(i + 1);
        }
    }
    if let Some(exp) = expected_nulls {
        assert_eq!(
            got_nulls, exp,
            "nullability mismatch: name={} dir={}\n  got:      {:?}\n  expected: {:?}",
            name, dir, got_nulls, exp
        );
    }
}

#[test]
fn test_deriv_toml() {
    for tc in load_tests() {
        if tc.ignore {
            continue;
        }
        let mut b = RegexBuilder::new();
        let node = resharp_parser::parse_ast(&mut b, &tc.pattern).unwrap();

        if !tc.rev.is_empty() || tc.rev_nulls.is_some() {
            let rev = b.reverse(node).unwrap();
            let rev = b.normalize_rev(rev).unwrap();
            let rev = b.mk_concat(NodeId::TS, rev);

            eprintln!(
                "\n[{}] rev initial: node={:?} pp={}",
                tc.name,
                rev,
                b.pp(rev)
            );
            let bytes: Vec<u8> = tc.input.as_bytes().iter().rev().copied().collect();
            let empty_rev = vec![None; bytes.len()];
            let rev_pp = if tc.rev.is_empty() {
                &empty_rev
            } else {
                &tc.rev
            };
            walk_bytes(
                &mut b,
                rev,
                &bytes,
                rev_pp,
                tc.rev_nulls.as_deref(),
                "rev",
                &tc.name,
            );
        }

        if !tc.fwd.is_empty() || tc.fwd_nulls.is_some() {
            eprintln!(
                "\n[{}] fwd initial: node={:?} kind={:?} pp={}",
                tc.name,
                node,
                b.get_kind(node),
                b.pp(node)
            );
            let bytes: Vec<u8> = tc.input.as_bytes().to_vec();
            let empty_fwd = vec![None; bytes.len()];
            let fwd_pp = if tc.fwd.is_empty() {
                &empty_fwd
            } else {
                &tc.fwd
            };
            walk_bytes(
                &mut b,
                node,
                &bytes,
                fwd_pp,
                tc.fwd_nulls.as_deref(),
                "fwd",
                &tc.name,
            );
        }
    }
}
