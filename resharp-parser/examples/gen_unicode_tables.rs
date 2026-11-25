use regex_syntax::hir::{Class, ClassUnicode, ClassUnicodeRange, HirKind};

fn fold_extras(c: char) -> Vec<char> {
    let mut cls = ClassUnicode::new([ClassUnicodeRange::new(c, c)]);
    cls.try_case_fold_simple().expect("regex-syntax built with unicode-case");
    cls.iter().flat_map(|r| r.start()..=r.end()).filter(|&d| d != c).collect()
}

fn perl_ranges(pattern: &str) -> Vec<(char, char)> {
    let hir = regex_syntax::parse(pattern).expect("perl class parses");
    match hir.kind() {
        HirKind::Class(Class::Unicode(cls)) => cls.iter().map(|r| (r.start(), r.end())).collect(),
        other => panic!("{pattern}: expected unicode class, got {other:?}"),
    }
}

fn emit_pairs(name: &str, cfg: Option<&str>, pairs: &[(char, char)]) {
    if let Some(cfg) = cfg {
        println!("#[cfg({cfg})]");
    }
    println!("#[rustfmt::skip]");
    println!("pub(crate) const {name}: &[(char, char)] = &[");
    for chunk in pairs.chunks(4) {
        let line: Vec<String> = chunk
            .iter()
            .map(|(a, b)| format!("('\\u{{{:x}}}', '\\u{{{:x}}}')", *a as u32, *b as u32))
            .collect();
        println!("    {},", line.join(", "));
    }
    println!("];");
}

fn main() {
    let mut extras = Vec::new();
    for c in '\0'..=char::MAX {
        for d in fold_extras(c) {
            extras.push((c, d));
        }
    }
    emit_pairs(
        "CASE_EXTRA",
        Some(r#"not(feature = "unicode-case")"#),
        &extras,
    );
    println!();
    for (name, pat) in [("PERL_WORD", r"\w"), ("PERL_DIGIT", r"\d"), ("PERL_SPACE", r"\s")] {
        let ranges = perl_ranges(pat);
        emit_pairs(name, None, &ranges);
        println!();
    }
}
