use std::time::Instant;

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load_haystack(name: &str) -> Vec<u8> {
    let path = format!("{}/haystacks/{}", data_dir(), name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to load {}: {}", path, e))
}

fn bench_compare(label: &str, pattern: &str, input: &[u8], iters: u32) {
    eprintln!("\n=== {} ===", label);
    eprintln!("pattern: {}", &pattern[..pattern.len().min(120)]);
    eprintln!("input: {} bytes", input.len());

    // resharp
    let re = resharp::Regex::new(pattern).unwrap();
    let (fwd_accel, rev_accel) = re.has_accel();
    eprintln!("resharp nodes={} accel: fwd={} rev={}", re.node_count(), fwd_accel, rev_accel);
    if let Some((states, minterms, prefix_len)) = re.bdfa_stats() {
        eprintln!("resharp bdfa: states={} minterms={} prefix_len={}", states, minterms, prefix_len);
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
    eprintln!("resharp:  {} matches, {:.2} MiB/s ({:.2?} / {} iters)", count, mib_s, elapsed, iters);

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
    eprintln!("regex:    {} matches, {:.2} MiB/s ({:.2?} / {} iters)", count2, mib_s, elapsed, iters);

    if count != count2 {
        eprintln!("WARNING: match count differs! resharp={} regex={}", count, count2);
    }
}

fn byte_hit_rate(input: &[u8], bytes: &[u8]) -> f64 {
    let hits = input.iter().filter(|b| bytes.contains(b)).count();
    hits as f64 / input.len() as f64 * 100.0
}

fn main() {
    let en = load_haystack("en-sampled.txt");
    let ru = load_haystack("ru-sampled.txt");
    let zh = load_haystack("zh-sampled.txt");

    // measure actual hit rates for different anchor positions
    eprintln!("=== actual hit rates in input ===");
    eprintln!("russian pos0 [D0]:           {:.1}%", byte_hit_rate(&ru, &[0xD0]));
    eprintln!("russian pos1 [94,98,A8,B8,BF]: {:.1}%", byte_hit_rate(&ru, &[0x94, 0x98, 0xA8, 0xB8, 0xBF]));
    eprintln!("russian pos2 [D0,D1]:        {:.1}%", byte_hit_rate(&ru, &[0xD0, 0xD1]));
    eprintln!("russian rarest [94]:         {:.1}%", byte_hit_rate(&ru, &[0x94]));
    eprintln!("russian rarest [A8]:         {:.1}%", byte_hit_rate(&ru, &[0xA8]));
    // pos7 bytes: BB (л), BD (н), BF (п), 84 (ф continuation)
    eprintln!("russian pos7 [BB,BD,BF,84]:  {:.1}%", byte_hit_rate(&ru, &[0xBB, 0xBD, 0xBF, 0x84]));
    eprintln!();
    eprintln!("chinese pos0 [E5,E7,E8,E9]: {:.1}%", byte_hit_rate(&zh, &[0xE5, 0xE7, 0xE8, 0xE9]));
    // chinese pos1: second bytes of 夏(A4),约(BA),阿(98),雷(9B),莫(8E)
    eprintln!("chinese pos1 [A4,BA,98,9B,8E]: {:.1}%", byte_hit_rate(&zh, &[0xA4, 0xBA, 0x98, 0x9B, 0x8E]));
    eprintln!("chinese rarest [9B]:         {:.1}%", byte_hit_rate(&zh, &[0x9B]));
    eprintln!();

    // benchmarks
    bench_compare(
        "english-alternation (control)",
        "Sherlock|Holmes|Watson|Irene|Adler",
        &en,
        20,
    );

    bench_compare(
        "russian-alternation",
        "Шерлок Холмс|Джон Уотсон|Ирен Адлер|инспектор Лестрейд|профессор Мориарти",
        &ru,
        20,
    );

    bench_compare(
        "chinese-alternation",
        "夏洛克·福尔摩斯|约翰华生|阿德勒|雷斯垂德|莫里亚蒂教授",
        &zh,
        50,
    );
}
