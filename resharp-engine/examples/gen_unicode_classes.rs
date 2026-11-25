use regex_syntax::utf8::Utf8Sequences;
use resharp_algebra::{Kind, NodeId, RegexBuilder};
use std::collections::HashMap;

type ByteSeq = Vec<(u8, u8)>;

/// UTF-8 sequences as a shared byte automaton.
fn build_seqs(
    b: &mut RegexBuilder,
    seqs: &[ByteSeq],
    cache: &mut HashMap<Vec<ByteSeq>, NodeId>,
) -> NodeId {
    let accept = seqs.iter().any(|s| s.is_empty());
    let mut key: Vec<ByteSeq> = seqs.to_vec();
    key.sort();
    key.dedup();
    if let Some(&n) = cache.get(&key) {
        return n;
    }
    let mut buckets: Vec<Vec<ByteSeq>> = vec![Vec::new(); 256];
    for s in seqs.iter().filter(|s| !s.is_empty()) {
        for byte in s[0].0..=s[0].1 {
            buckets[byte as usize].push(s[1..].to_vec());
        }
    }
    let mut groups: HashMap<NodeId, Vec<u8>> = HashMap::new();
    for (byte, bucket) in buckets.iter().enumerate() {
        if bucket.is_empty() {
            continue;
        }
        let child = build_seqs(b, bucket, cache);
        groups.entry(child).or_default().push(byte as u8);
    }
    let mut parts: Vec<(u8, NodeId, NodeId)> = groups
        .into_iter()
        .map(|(child, bytes)| {
            let ranges = byte_runs(&bytes);
            let pred = b.mk_ranges_u8(&ranges);
            (bytes[0], pred, child)
        })
        .collect();
    parts.sort_by_key(|p| p.0);
    let mut node = if accept { NodeId::EPS } else { NodeId::BOT };
    for (_, pred, child) in parts {
        let branch = b.mk_concat(pred, child);
        node = b.mk_union(node, branch);
    }
    cache.insert(key, node);
    node
}

fn byte_runs(bytes: &[u8]) -> Vec<(u8, u8)> {
    let mut runs: Vec<(u8, u8)> = Vec::new();
    for &x in bytes {
        match runs.last_mut() {
            Some(r) if r.1 + 1 == x => r.1 = x,
            _ => runs.push((x, x)),
        }
    }
    runs
}

fn build_class_from_ranges(
    b: &mut RegexBuilder,
    ranges: &[(char, char)],
    max_seq_len: usize,
) -> NodeId {
    let mut seqs: Vec<ByteSeq> = Vec::new();
    for &(start, end) in ranges {
        for seq in Utf8Sequences::new(start, end) {
            let sl = seq.as_slice();
            if sl.len() > max_seq_len {
                continue;
            }
            seqs.push(sl.iter().map(|s| (s.start, s.end)).collect());
        }
    }
    let mut cache = HashMap::new();
    build_seqs(b, &seqs, &mut cache)
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
        _ => match b.get_kind(node) {
            Kind::Pred => {
                let ranges = b.solver_ref().byte_ranges(node.pred_tset(b));
                let var = format!("n{}", counter);
                *counter += 1;
                if ranges.len() == 1 {
                    println!(
                        "let {var} = b.mk_range_u8(0x{:02X}, 0x{:02X});",
                        ranges[0].0, ranges[0].1
                    );
                } else {
                    let pairs: Vec<String> = ranges
                        .iter()
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
        },
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
        (r"\w", "build_word_class", "\\w", 2),
        (r"\d", "build_digit_class", "\\d", 1),
        (r"\s", "build_space_class", "\\s", 1),
    ];

    for (pos_pat, pos_fn, pos_label, max_len) in &classes {
        let mut b = RegexBuilder::new();
        let pos_ranges = class_ranges(pos_pat);
        let pos_node = build_class_from_ranges(&mut b, &pos_ranges, *max_len);
        eprintln!("{pos_label}={:?} nodes={}", pos_node, b.num_nodes());

        emit_class(&b, pos_node, pos_fn, pos_label);
        println!();
    }

    for (pat, fn_name, label) in &[
        (r"\w", "build_word_class_full", "\\w(full)"),
        (r"\d", "build_digit_class_full", "\\d(full)"),
        (r"\s", "build_space_class_full", "\\s(full)"),
    ] {
        let mut b = RegexBuilder::new();
        let pos_ranges = class_ranges(pat);
        let pos_node = build_class_from_ranges(&mut b, &pos_ranges, 4);
        eprintln!("{label}={:?} nodes={}", pos_node, b.num_nodes());
        emit_class(&b, pos_node, fn_name, label);
        println!();
    }
}
