use resharp::{EngineOptions, Regex};
fn main() {
    let data_dir = format!("{}/../data", env!("CARGO_MANIFEST_DIR"));
    let full = std::fs::read_to_string(format!("{}/haystacks/en-sampled.txt", data_dir)).unwrap();
    let haystack: String = full.lines().take(2500).collect::<Vec<_>>().join("\n");
    let input = haystack.as_bytes();
    println!("haystack: {} bytes ({:.1} KB)", input.len(), input.len() as f64 / 1024.0);

    for pat in &[
        r"\b[0-9A-Za-z_]{12,}\b",
        r"\b[0-9A-Za-z_]{8,}\b",
        r"[0-9A-Za-z_]{12,}",
        r"\w{12,}",
        r"[A-Za-z]{8,}",
        r"[A-Za-z]{8,13}",
    ] {
        let opts = EngineOptions::default().unicode(resharp::UnicodeMode::Ascii);
        let re = Regex::with_options(pat, opts).unwrap();
        let m = re.find_all(input).unwrap();
        let re3 = regex::bytes::Regex::new(pat).unwrap();
        let c3 = re3.find_iter(input).count();
        println!("{:30} resharp={:5} regex={:5}", pat, m.len(), c3);
    }
}
