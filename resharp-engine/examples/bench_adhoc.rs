use std::time::Instant;

fn bench(pattern: &str, input: &[u8], iters: u32) {
    if let Ok(re) = resharp::Regex::new(pattern) {
        let _ = re.find_all(input);
        let (fwd_states, rev_states) = re.dfa_stats();
        let (fwd_accel, rev_accel) = re.has_accel();
        if let Some((bstates, bmt, bprefix)) = re.bdfa_stats() {
            println!(
                "  bdfa: states={} minterms={} prefix_len={}",
                bstates, bmt, bprefix
            );
        } else {
            println!(
                "  fwd={} rev={} accel=({},{})",
                fwd_states, rev_states, fwd_accel, rev_accel
            );
        }
        let t = Instant::now();
        let mut n = 0;
        for _ in 0..iters {
            n = re.find_all(input).unwrap().len();
        }
        println!("  resharp:  {:>6} matches  {:>10?}", n, t.elapsed() / iters);
    }

    if let Ok(re) = regex::bytes::Regex::new(pattern) {
        let _: Vec<_> = re.find_iter(input).collect();
        let t = Instant::now();
        let mut n = 0;
        for _ in 0..iters {
            let m: Vec<_> = re.find_iter(input).collect();
            n = m.len();
        }
        println!("  regex:    {:>6} matches  {:>10?}", n, t.elapsed() / iters);
    }
    println!();
}

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load(name: &str) -> String {
    std::fs::read_to_string(format!("{}/haystacks/{}", data_dir(), name)).unwrap()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filter = args.get(1).map(|s| s.as_str());

    let en = load("en-sampled.txt");
    let benchmarks: Vec<(&str, &str, &[u8], u32)> = vec![
        ("literal-single", "Sherlock Holmes", en.as_bytes(), 10),
        (
            "multi-literal",
            "Sherlock|Holmes|Watson|Irene|Adler",
            en.as_bytes(),
            10,
        ),
        (
            "literal-alt+suffix",
            "(Sherlock|Holmes|Watson|Irene|Adler)[a-z]{0,5}",
            en.as_bytes(),
            10,
        ),
        ("date", r"\d{4}-\d{2}-\d{2}", en.as_bytes(), 10),
        ("digit-class", r"[0-9]+", en.as_bytes(), 10),
        ("upper-class", r"[A-Z]+", en.as_bytes(), 10),
        ("date-iso", r"\d{4}-\d{2}-\d{2}", en.as_bytes(), 10),
        (
            "ip-addr",
            r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}",
            en.as_bytes(),
            10,
        ),
        ("dotstar-prefix", r"Holmes.*Watson", en.as_bytes(), 10),
        ("char-class-prefix", r"[A-Z][a-z]e [A-Z]", en.as_bytes(), 10),
        ("bounded-repeat", r"[A-Za-z]{8,13}", en.as_bytes(), 100),
        (
            "aws-key",
            r"(?:ASIA|AKIA|AROA|AIDA)[A-Z0-7]{16}",
            en.as_bytes(),
            100,
        ),
        (
            "phone",
            r"(\(?\+?[0-9]*\)?)?[0-9_\- ()]{7,}",
            en.as_bytes(),
            100,
        ),
        (
            "bounded-alt-suffix",
            r"(Sherlock|Holmes|Watson|Irene|Adler)[a-z]{0,5}",
            en.as_bytes(),
            100,
        ),
    ];

    for (name, pattern, input, iters) in &benchmarks {
        if let Some(f) = filter {
            if !name.contains(f) {
                continue;
            }
        }
        println!("{} ({:.0}KB):", name, input.len() as f64 / 1024.0);
        bench(pattern, input, *iters);
    }
}
