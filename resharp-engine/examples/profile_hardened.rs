use std::time::Instant;

fn profile(label: &str, pattern: &str, input: &[u8], iters: u32) {
    let opts = resharp::EngineOptions::default().hardened(true);
    let re_h = match resharp::Regex::with_options(pattern, opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  skip {}: {}", label, e);
            return;
        }
    };
    let re_n = resharp::Regex::new(pattern).unwrap();

    // warm up both
    let _ = re_h.find_all(input);
    let _ = re_n.find_all(input);
    let _ = re_h.find_all(input);
    let _ = re_n.find_all(input);

    let nulls = re_h.collect_rev_nulls_debug(input);
    let null_count = nulls.len();

    // time hardened
    let t = Instant::now();
    let mut n = 0;
    for _ in 0..iters {
        n = re_h.find_all(input).unwrap().len();
    }
    let th = t.elapsed() / iters;

    // time normal
    let t = Instant::now();
    for _ in 0..iters {
        re_n.find_all(input).unwrap();
    }
    let tn = t.elapsed() / iters;

    let len = input.len() as f64;
    let tp_h = len / th.as_secs_f64() / 1e6;
    let tp_n = len / tn.as_secs_f64() / 1e6;
    let slowdown = th.as_secs_f64() / tn.as_secs_f64();
    println!(
        "{:35} nulls={:5} m={:5}  hardened={:>10?} ({:6.1} MB/s)  normal={:>10?} ({:6.1} MB/s)  {:.1}x slower",
        label, null_count, n, th, tp_h, tn, tp_n, slowdown,
    );
}

fn main() {
    let en = std::fs::read_to_string(format!(
        "{}/../data/haystacks/en-sampled.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let input = en.as_bytes();
    let len = input.len();

    println!("--- normal patterns on en-sampled.txt ({} bytes) ---", len);
    let cases: Vec<(&str, &str)> = vec![
        ("digits", r"\d+"),
        ("capitalized-words", r"[A-Z][a-z]+"),
        ("words-3-8", r"\w{3,8}"),
        ("vowels", r"[aeiou]+"),
        ("alternation", r"the|and|for|that|with"),
        ("ip-like", r"[0-9]{1,3}\.[0-9]{1,3}"),
        ("allcaps", r"[A-Z]{2,}"),
        ("long-words", r"[A-Za-z]{8,13}"),
        ("date-iso", r"\d{4}-\d{2}-\d{2}"),
        ("sherlock-suffix", r"(Sherlock|Holmes|Watson)[a-z]{0,5}"),
        ("lookahead", r"(?<=\s)[A-Z][a-z]+(?=\s)"),
        ("dotstar-literal", r"Sherlock.*Watson"),
    ];
    for (label, pattern) in &cases {
        profile(label, pattern, input, 50);
    }

    let date_monster = std::fs::read_to_string(format!(
        "{}/tests/date_pattern.toml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let date_pat: String = date_monster
        .lines()
        .find(|l| l.starts_with("pattern = '((19"))
        .map(|l| {
            l.strip_prefix("pattern = '")
                .unwrap()
                .strip_suffix("'")
                .unwrap()
                .to_string()
        })
        .unwrap();
    println!("\n--- date-monster on en-sampled.txt ---");
    profile("date-monster", &date_pat, input, 10);

    println!("\n--- pathological pattern on synthetic input ---");
    let aaaa = b"A".repeat(10_000);
    profile("pathological-10k", r".*[^A-Z]|[A-Z]", &aaaa, 10);
}
