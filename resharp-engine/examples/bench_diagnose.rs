#![allow(dead_code)]
use std::time::Instant;

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load_haystack(name: &str) -> Vec<u8> {
    let path = format!("{}/haystacks/{}", data_dir(), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to load {}: {}", path, e))
}

fn bench_compare(label: &str, pattern: &str, input: &[u8], iters: u32) {
    eprintln!("\n# {}", label);
    eprintln!("pattern: {}", &pattern[..pattern.len().min(120)]);
    eprintln!("input: {} bytes", input.len());

    // resharp
    let re = resharp::Regex::new(pattern).unwrap();
    let (fwd_accel, rev_accel) = re.has_accel();
    eprintln!("resharp accel: fwd={} rev={}", fwd_accel, rev_accel);
    if let Some((states, minterms, prefix_len)) = re.bdfa_stats() {
        eprintln!(
            "resharp bdfa: states={} minterms={} prefix_len={}",
            states, minterms, prefix_len
        );
    } else {
        let (fwd, rev) = re.dfa_stats();
        eprintln!("resharp lazy-dfa: fwd_states={} rev_states={}", fwd, rev);
    }
    re.find_all(input).ok(); // warmup
    let start = Instant::now();
    let mut count = 0;
    for _ in 0..iters {
        count = re.find_all(input).unwrap().len();
    }
    let elapsed = start.elapsed();
    let mib_s = (input.len() as f64 * iters as f64) / elapsed.as_secs_f64() / (1024.0 * 1024.0);
    eprintln!(
        "resharp:  {} matches, {:.2} MiB/s ({:.2?} / {} iters)",
        count, mib_s, elapsed, iters
    );

    // regex crate
    let re2 = regex::bytes::Regex::new(pattern).unwrap();
    let _ = re2.find_iter(input).count(); // warmup
    let start = Instant::now();
    let mut count2 = 0;
    for _ in 0..iters {
        count2 = re2.find_iter(input).count();
    }
    let elapsed = start.elapsed();
    let mib_s = (input.len() as f64 * iters as f64) / elapsed.as_secs_f64() / (1024.0 * 1024.0);
    eprintln!(
        "regex:    {} matches, {:.2} MiB/s ({:.2?} / {} iters)",
        count2, mib_s, elapsed, iters
    );

    if count != count2 {
        eprintln!(
            "WARNING: match count differs! resharp={} regex={}",
            count, count2
        );
    }
}

fn main() {
    // let en = load_haystack("en-sampled.txt");

    // bench_compare(
    //     "english-alternation (control)",
    //     "Sherlock|Holmes|Watson|Irene|Adler",
    //     &en,
    //     20,
    // );

    // bench_compare("unicode-letter-bounded-en", r"\p{L}{8,13}", &en, 5);

    // bench_compare("ascii-letter-bounded-en", "[A-Za-z]{8,13}", &en, 5);

    // bench_compare(
    //     "credit-card (en)",
    //     r"\b(?:4\d{3}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}|5[1-5]\d{2}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}|3[47]\d{2}[\s\-]?\d{6}[\s\-]?\d{5}|6011[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4})\b",
    //     &en,
    //     10,
    // );

    // {
    //     let cc_input =
    //         std::fs::read(format!("{}/haystacks/credit-card-input.bin", data_dir())).unwrap();
    //     bench_compare(
    //         "credit-card (credit-card-input.bin)",
    //         r"\b(?:4\d{3}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}|5[1-5]\d{2}[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4}|3[47]\d{2}[\s\-]?\d{6}[\s\-]?\d{5}|6011[\s\-]?\d{4}[\s\-]?\d{4}[\s\-]?\d{4})\b",
    //         &cc_input,
    //         10,
    //     );
    // }

    // {
    //     let haystack =
    //         std::fs::read_to_string(format!("{}/haystacks/en-medium.txt", data_dir())).unwrap();
    //     let dict_path = format!("{}/regexes/length-15.txt", data_dir());
    //     let pattern: String = std::fs::read_to_string(&dict_path)
    //         .unwrap()
    //         .lines()
    //         .take(2663)
    //         .collect::<Vec<_>>()
    //         .join("|");
    //     let input = haystack.as_bytes();
    //     bench_compare("dictionary-full", &pattern, input, 20);
    // }

    {
        let apache = load_haystack("apache.input");
        bench_compare(
            "apache-log",
            r#"(?m)^(?:\S+) \S+ \S+ \[(?:[^\]]+)\] "(?:\S+) (?:\S+) [^"]*" (?:\d{3}) (?:\d+|-)"#,
            &apache,
            50,
        );
    }

    // {
    //     let emails = load_haystack("emails.input");
    //     bench_compare(
    //         "email",
    //         r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}",
    //         &emails,
    //         50,
    //     );
    // }
}
