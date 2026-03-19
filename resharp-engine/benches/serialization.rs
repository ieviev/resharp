#![allow(dead_code)]
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load_haystack(name: &str) -> String {
    let path = format!("{}/haystacks/{}", data_dir(), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to load {}: {}", path, e))
}

fn load_regex(name: &str) -> String {
    let path = format!("{}/regexes/{}", data_dir(), name);
    std::fs::read_to_string(&path).unwrap().trim().to_string()
}

fn load_dictionary_pattern(n: usize) -> String {
    let path = format!("{}/regexes/length-15.txt", data_dir());
    let contents = std::fs::read_to_string(&path).unwrap();
    contents.lines().take(n).collect::<Vec<_>>().join("|")
}

fn bench_serialization(c: &mut Criterion) {
    let haystack = load_haystack("en-sampled.txt");
    let input = haystack.as_bytes();

    let patterns: Vec<(&str, String)> = vec![
        ("dict-500", load_dictionary_pattern(500)),
        ("dict-full", load_dictionary_pattern(usize::MAX)),
        ("date-monster", load_regex("date.txt")),
        ("email-like", r"\b\w+@\w+\.\w+\b".to_string()),
    ];

    for (name, pattern) in &patterns {
        let re = match resharp::Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skip {}: {}", name, e);
                continue;
            }
        };

        let bytes = match re.to_bytes() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {} serialization: {}", name, e);
                continue;
            }
        };

        let mut group = c.benchmark_group(format!("serialization/{}", name));
        group.throughput(Throughput::Bytes(input.len() as u64));

        group.bench_function("compile", |b| {
            b.iter(|| black_box(resharp::Regex::new(black_box(pattern)).unwrap()));
        });

        group.bench_function("from_bytes", |b| {
            b.iter(|| black_box(resharp::Regex::from_bytes(black_box(&bytes)).unwrap()));
        });

        // warm up both
        re.find_all(input).ok();
        let re2 = resharp::Regex::from_bytes(&bytes).unwrap();
        re2.find_all(input).ok();

        group.bench_function("match/compiled", |b| {
            b.iter(|| black_box(re.find_all(black_box(input)).unwrap().len()));
        });

        group.bench_function("match/deserialized", |b| {
            b.iter(|| black_box(re2.find_all(black_box(input)).unwrap().len()));
        });

        group.finish();

        eprintln!(
            "  {}: serialized size = {} bytes, states = fwd:{} rev:{}",
            name,
            bytes.len(),
            u16::from_le_bytes([bytes[8], bytes[9]]),
            u16::from_le_bytes([bytes[10], bytes[11]]),
        );
    }
}

criterion_group!(benches, bench_serialization);
criterion_main!(benches);
