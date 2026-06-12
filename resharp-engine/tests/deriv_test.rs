mod common;
use common::schemas::{DerivCase, DerivFile};
use resharp::{NodeId, RegexBuilder};
use resharp_algebra::nulls::Nullability;
use std::path::Path;

fn load_tests() -> Vec<DerivCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("deriv.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let file: DerivFile = toml::from_str(&content).unwrap();
    file.test
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
    expected: &[String],
    expected_nulls: Option<&[usize]>,
    expected_effects: &[String],
    dir: &str,
    name: &str,
) {
    if !expected.is_empty() {
        assert_eq!(
            bytes.len(),
            expected.len(),
            "input length must match {dir} expected length for {name}"
        );
    }
    let n = bytes.len();
    let fmt_effects = |b: &mut RegexBuilder, node: NodeId| -> String {
        let nulls_id = b.get_nulls_id(node);
        let entries = b.nulls_entry_vec(nulls_id.0);
        format!("{:?}", entries)
    };
    let mut got_nulls: Vec<usize> = Vec::new();
    let init_eff = fmt_effects(b, node);
    eprintln!("  [{}] initial pos=0 effects={}", dir, init_eff);
    {
        let nulls_id = b.get_nulls_id(node);
        for e in b.nulls_entry_vec(nulls_id.0) {
            got_nulls.push(e.rel as usize);
        }
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
        if let Some(exp) = expected.get(i) {
            if exp != "?" {
                assert_eq!(
                    pp, *exp,
                    "deriv pp mismatch: name={} dir={} step={} byte='{}'",
                    name, dir, i, *byte as char
                );
            }
        }
        node = next;
        let eff_str = fmt_effects(b, node);
        eprintln!("  [{}] after pos={} effects={}", dir, i + 1, eff_str);
        if let Some(exp) = expected_effects.get(i) {
            if exp != "?" {
                assert_eq!(
                    eff_str, *exp,
                    "effects mismatch: name={} dir={} step={} byte='{}'",
                    name, dir, i, *byte as char
                );
            }
        }
        {
            let nulls_id = b.get_nulls_id(node);
            for e in b.nulls_entry_vec(nulls_id.0) {
                got_nulls.push((i + 1) + e.rel as usize);
            }
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
        let node = if tc.ascii {
            let flags = resharp_parser::PatternFlags {
                unicode: false,
                full_unicode: false,
                ascii_perl_classes: true,
                ..Default::default()
            };
            resharp_parser::parse_ast_with(&mut b, &tc.pattern, &flags).unwrap()
        } else {
            resharp_parser::parse_ast(&mut b, &tc.pattern).unwrap()
        };

        if !tc.rev.is_empty() || tc.rev_nulls.is_some() || !tc.rev_effects.is_empty() {
            let rev = b.reverse(node).unwrap();
            let rev = b.normalize_rev(rev, 0).unwrap();
            let rev = b.mk_concat(NodeId::TS, rev);

            eprintln!(
                "\n[{}] rev initial: node={:?} pp={}",
                tc.name,
                rev,
                b.pp(rev)
            );
            let bytes: Vec<u8> = tc.input.as_bytes().iter().rev().copied().collect();
            walk_bytes(
                &mut b,
                rev,
                &bytes,
                &tc.rev,
                tc.rev_nulls.as_deref(),
                &tc.rev_effects,
                "rev",
                &tc.name,
            );
        }

        if !tc.fwd.is_empty() || tc.fwd_nulls.is_some() || !tc.fwd_effects.is_empty() {
            eprintln!(
                "\n[{}] fwd initial: node={:?} kind={:?} pp={}",
                tc.name,
                node,
                b.get_kind(node),
                b.pp(node)
            );
            let bytes: Vec<u8> = tc.input.as_bytes().to_vec();
            walk_bytes(
                &mut b,
                node,
                &bytes,
                &tc.fwd,
                tc.fwd_nulls.as_deref(),
                &tc.fwd_effects,
                "fwd",
                &tc.name,
            );
        }
    }
}

fn end_nullable_after(pattern: &str, input: &[u8]) -> bool {
    use resharp_algebra::nulls::Nullability;
    let mut b = RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, pattern).unwrap();
    let n = input.len();
    let mut cur = node;
    for (i, &byte) in input.iter().enumerate() {
        let der_mask = pos_mask(i, n);
        let tset = b.solver().u8_to_set_id(byte);
        let tregex = b.der(cur, der_mask).unwrap();
        cur = b.transition_term(tregex, tset);
    }
    b.nullability(cur).has(Nullability::END)
}

#[test]
fn deriv_complement_counted_z_not_end_nullable() {
    assert!(
        !end_nullable_after(r"~(.{1,3}\z){2,4}", b"ab"),
        "(.{{1,3}}\\z){{2,4}} cannot complete 2 reps before \\z, so ~(...) matches 'ab' \
         as a substring but NOT ending at the absolute end: residual must not be END-nullable"
    );
    assert!(!end_nullable_after(r"~(.{1,3}\z){2,4}", b"a"));
    assert!(!end_nullable_after(r"~(.{1,3}\z)", b"ab"));
}
