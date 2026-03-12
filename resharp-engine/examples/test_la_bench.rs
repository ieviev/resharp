use resharp::Regex;

fn debug(label: &str, pattern: &str, input: &[u8]) {
    let re = Regex::new(pattern).unwrap();
    let nulls = re.collect_rev_nulls_debug(input);
    eprintln!("{} input={:?}", label, std::str::from_utf8(input).unwrap());
    eprintln!("  nulls({})={:?}", nulls.len(), nulls);
    for i in 1..nulls.len() {
        assert!(
            nulls[i] <= nulls[i - 1],
            "nulls not sorted descending at [{}]: {} > {}",
            i,
            nulls[i],
            nulls[i - 1]
        );
    }
    eprintln!("  sorted: ok");
}

fn main() {
    debug("space-var ", r" [A-Z][a-z]+ ", b" Hello World Foo ");
    debug("var-len   ", r"[A-Z][a-z]+", b" Hello World Foo ");
    debug(
        "lookaround",
        r"(?<=\s)[A-Z][a-z]+(?=\s)",
        b" Hello World Foo ",
    );
}
