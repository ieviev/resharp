use resharp_algebra::{Kind, NodeId, RegexBuilder};
use regex_syntax::utf8::Utf8Sequences;
use std::collections::HashMap;

fn build_class_from_ranges(b: &mut RegexBuilder, ranges: &[(char, char)], max_seq_len: usize) -> NodeId {
    let mut s1 = NodeId::BOT;
    let mut s2 = NodeId::BOT;
    let mut s3 = NodeId::BOT;
    for &(start, end) in ranges {
        for seq in Utf8Sequences::new(start, end) {
            let sl = seq.as_slice();
            if sl.len() > max_seq_len {
                continue;
            }
            let bytes: Vec<_> = sl.iter().map(|s| (s.start, s.end)).collect();
            match bytes.len() {
                1 => {
                    let node = b.mk_range_u8(bytes[0].0, bytes[0].1);
                    s1 = b.mk_union(s1, node);
                }
                2 => {
                    let n1 = b.mk_range_u8(bytes[0].0, bytes[0].1);
                    let n2 = b.mk_range_u8(bytes[1].0, bytes[1].1);
                    let conc = b.mk_concat(n1, n2);
                    s2 = b.mk_union(s2, conc);
                }
                3 => {
                    let n1 = b.mk_range_u8(bytes[0].0, bytes[0].1);
                    let n2 = b.mk_range_u8(bytes[1].0, bytes[1].1);
                    let n3 = b.mk_range_u8(bytes[2].0, bytes[2].1);
                    let conc2 = b.mk_concat(n2, n3);
                    let conc1 = b.mk_concat(n1, conc2);
                    s3 = b.mk_union(s3, conc1);
                }
                4 if max_seq_len >= 4 => {
                    let n1 = b.mk_range_u8(bytes[0].0, bytes[0].1);
                    let n2 = b.mk_range_u8(bytes[1].0, bytes[1].1);
                    let n3 = b.mk_range_u8(bytes[2].0, bytes[2].1);
                    let n4 = b.mk_range_u8(bytes[3].0, bytes[3].1);
                    let conc3 = b.mk_concat(n3, n4);
                    let conc2 = b.mk_concat(n2, conc3);
                    let conc1 = b.mk_concat(n1, conc2);
                    s3 = b.mk_union(s3, conc1);
                }
                _ => {}
            }
        }
    }
    let merged = b.mk_union(s2, s1);
    b.mk_union(s3, merged)
}

fn class_ranges(pattern: &str) -> Vec<(char, char)> {
    use regex_syntax::hir;
    let hir = regex_syntax::parse(pattern).unwrap();
    match hir.kind() {
        hir::HirKind::Class(hir::Class::Unicode(cls)) => {
            cls.ranges().iter().map(|r| (r.start(), r.end())).collect()
        }
        _ => panic!("expected unicode class for {}", pattern),
    }
}

fn emit(
    b: &RegexBuilder,
    node: NodeId,
    visited: &mut HashMap<NodeId, String>,
    counter: &mut u32,
) -> String {
    if let Some(name) = visited.get(&node) {
        return name.clone();
    }
    let name = match node {
        NodeId::BOT => "NodeId::BOT".into(),
        NodeId::EPS => "NodeId::EPS".into(),
        _ => {
            match b.get_kind(node) {
                Kind::Pred => {
                    let ranges = b.solver_ref().byte_ranges(node.pred_tset(b));
                    let var = format!("n{}", counter);
                    *counter += 1;
                    if ranges.len() == 1 {
                        println!("let {var} = b.mk_range_u8(0x{:02X}, 0x{:02X});", ranges[0].0, ranges[0].1);
                    } else {
                        let pairs: Vec<String> = ranges.iter()
                            .map(|(lo, hi)| format!("(0x{lo:02X}, 0x{hi:02X})"))
                            .collect();
                        println!("let {var} = b.mk_ranges_u8(&[{}]);", pairs.join(", "));
                    }
                    var
                }
                Kind::Concat => {
                    let l = emit(b, node.left(b), visited, counter);
                    let r = emit(b, node.right(b), visited, counter);
                    let var = format!("n{}", counter);
                    *counter += 1;
                    println!("let {var} = b.mk_concat({l}, {r});");
                    var
                }
                Kind::Union => {
                    let l = emit(b, node.left(b), visited, counter);
                    let r = emit(b, node.right(b), visited, counter);
                    let var = format!("n{}", counter);
                    *counter += 1;
                    println!("let {var} = b.mk_union({l}, {r});");
                    var
                }
                k => panic!("unexpected {:?}", k),
            }
        }
    };
    visited.insert(node, name.clone());
    name
}

fn emit_class(b: &RegexBuilder, node: NodeId, fn_name: &str, label: &str) {
    let mut visited = HashMap::new();
    let mut counter = 0u32;
    println!("pub fn {fn_name}(b: &mut RegexBuilder) -> NodeId {{");
    let v = emit(b, node, &mut visited, &mut counter);
    println!("{v}");
    println!("}}");
    eprintln!("{label} emitted {} vars", counter);
}

fn main() {
    let classes = [
        (r"\w", "build_word_class", "\\w"),
        (r"\d", "build_digit_class", "\\d"),
        (r"\s", "build_space_class", "\\s"),
    ];

    for (pos_pat, pos_fn, pos_label) in &classes {
        let mut b = RegexBuilder::new();
        let pos_ranges = class_ranges(pos_pat);
        let pos_node = build_class_from_ranges(&mut b, &pos_ranges, 2);
        eprintln!("{pos_label}={:?} nodes={}", pos_node, b.num_nodes());

        emit_class(&b, pos_node, pos_fn, pos_label);
        println!();
    }

    // full Unicode classes (all UTF-8 sequence lengths)
    for (pat, fn_name, label) in &[
        (r"\w", "build_word_class_full", "\\w(full)"),
        (r"\d", "build_digit_class_full", "\\d(full)"),
    ] {
        let mut b = RegexBuilder::new();
        let pos_ranges = class_ranges(pat);
        let pos_node = build_class_from_ranges(&mut b, &pos_ranges, 4);
        eprintln!("{label}={:?} nodes={}", pos_node, b.num_nodes());
        emit_class(&b, pos_node, fn_name, label);
        println!();
    }
}
