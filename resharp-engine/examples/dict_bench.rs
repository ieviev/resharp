use std::time::Instant;

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load(name: &str) -> String {
    std::fs::read_to_string(format!("{}/haystacks/{}", data_dir(), name)).unwrap()
}

fn load_dictionary_pattern(n: usize) -> String {
    let path = format!("{}/regexes/length-15.txt", data_dir());
    let contents = std::fs::read_to_string(&path).unwrap();
    contents.lines().take(n).collect::<Vec<_>>().join("|")
}

fn main() {
    let haystack = load("en-sampled.txt");
    let pattern = load_dictionary_pattern(2663);
    let input = haystack.as_bytes();

    let mut b = resharp::RegexBuilder::new();
    let node = resharp_parser::parse_ast(&mut b, &pattern).unwrap();

    let fwd_prefix = resharp::calc_potential_start(&mut b, node, 16, 64).unwrap();
    eprintln!("fwd prefix sets: {}", fwd_prefix.len());
    for (i, &set) in fwd_prefix.iter().enumerate() {
        let bytes = b.solver().collect_bytes(set);
        eprintln!("  fwd set[{}]: {} bytes", i, bytes.len());
    }

    let rev_start = b.reverse(node).unwrap();
    let rev_prefix = resharp::calc_prefix_sets(&mut b, rev_start).unwrap();
    eprintln!("rev prefix sets: {}", rev_prefix.len());
    for (i, &set) in rev_prefix.iter().enumerate() {
        let bytes = b.solver().collect_bytes(set);
        eprintln!("  rev set[{}]: {} bytes", i, bytes.len());
    }
    if rev_prefix.is_empty() {
        let rev_potential = resharp::calc_potential_start(&mut b, rev_start, 16, 64).unwrap();
        eprintln!("rev potential_start sets: {}", rev_potential.len());
        for (i, &set) in rev_potential.iter().enumerate() {
            let bytes = b.solver().collect_bytes(set);
            eprintln!("  rev pot[{}]: {} bytes", i, bytes.len());
        }
    }

    let re = resharp::Regex::new(&pattern).unwrap();
    // warmup
    let _ = re.find_all(input);

    let iters = 5;
    let t = Instant::now();
    let mut n = 0;
    for _ in 0..iters {
        n = re.find_all(input).unwrap().len();
    }
    let elapsed = t.elapsed() / iters;
    let mibs = input.len() as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);
    println!(
        "  resharp: {} matches  {:?}  ({:.0} MiB/s)",
        n, elapsed, mibs
    );
}
