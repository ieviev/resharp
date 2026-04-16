use resharp::{NodeId, RegexBuilder};
use resharp_algebra::nulls::Nullability;
use std::path::Path;

struct DerivTestCase {
    name: String,
    pattern: String,
    ignore: bool,
    rev_steps: Vec<(u8, Option<String>, Option<String>)>,
    fwd_steps: Vec<(u8, Option<String>, Option<String>)>,
}

fn parse_steps(t: &toml::Value, key: &str) -> Vec<(u8, Option<String>, Option<String>)> {
    t.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|entry| {
                    let pair = entry.as_array().unwrap();
                    let byte_str = pair[0].as_str().unwrap();
                    let byte = if byte_str.len() == 1 {
                        byte_str.as_bytes()[0]
                    } else {
                        byte_str
                            .parse::<u8>()
                            .expect("byte must be single char or decimal")
                    };
                    let field = |i: usize| -> Option<String> {
                        pair.get(i).and_then(|v| v.as_str()).and_then(|s| {
                            if s == "?" {
                                None
                            } else {
                                Some(s.to_string())
                            }
                        })
                    };
                    (byte, field(1), field(2))
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
            rev_steps: parse_steps(t, "rev_steps"),
            fwd_steps: parse_steps(t, "fwd_steps"),
        })
        .collect()
}

fn walk_steps(
    b: &mut RegexBuilder,
    mut node: NodeId,
    steps: &[(u8, Option<String>, Option<String>)],
    dir: &str,
    name: &str,
    first_mask: Nullability,
    last_mask: Nullability,
) {
    for (i, (byte, exp_pp, exp_nulls)) in steps.iter().enumerate() {
        let mask = if i == 0 {
            first_mask
        } else if i == steps.len() - 1 {
            last_mask
        } else {
            Nullability::CENTER
        };
        let tset = b.solver().u8_to_set_id(*byte);
        let tregex = b.der(node, mask).unwrap();
        let next = b.transition_term(tregex, tset);
        let pp = b.pp(next);
        let nulls_pp = b.pp_nulls(next);
        eprintln!(
            "  [{}] step={} byte='{}' (0x{:02x}) node={:?} nulls={} => {}",
            dir,
            i,
            *byte as char,
            byte,
            next,
            nulls_pp,
            // truncate long pp for readability in --nocapture output
            if pp.len() > 80 {
                format!("{}...", &pp[..80])
            } else {
                pp.clone()
            }
        );
        if let Some(exp) = exp_pp {
            assert_eq!(
                pp, *exp,
                "deriv pp mismatch: name={} dir={} step={} byte='{}'",
                name, dir, i, *byte as char
            );
        }
        if let Some(exp) = exp_nulls {
            assert_eq!(
                nulls_pp, *exp,
                "deriv nulls mismatch: name={} dir={} step={} byte='{}'",
                name, dir, i, *byte as char
            );
        }
        node = next;
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

        if !tc.rev_steps.is_empty() {
            let rev = b.reverse(node).unwrap();
            eprintln!("\n[{}] rev initial: node={:?}", tc.name, rev);
            walk_steps(
                &mut b,
                rev,
                &tc.rev_steps,
                "rev",
                &tc.name,
                Nullability::BEGIN,
                Nullability::END,
            );
        }

        if !tc.fwd_steps.is_empty() {
            eprintln!("\n[{}] fwd initial: node={:?}", tc.name, node);
            walk_steps(
                &mut b,
                node,
                &tc.fwd_steps,
                "fwd",
                &tc.name,
                Nullability::BEGIN,
                Nullability::END,
            );
        }
    }
}
