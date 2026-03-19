use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load_haystack(name: &str) -> String {
    let path = format!("{}/haystacks/{}", data_dir(), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to load {}: {}", path, e))
}

fn load_dictionary_words(n: usize) -> Vec<String> {
    let path = format!("{}/regexes/length-15.txt", data_dir());
    let contents = std::fs::read_to_string(&path).unwrap();
    contents.lines().take(n).map(|s| s.to_string()).collect()
}

fn bench_dictionary_vs_aho(c: &mut Criterion) {
    let haystack = load_haystack("en-sampled.txt");
    let words = load_dictionary_words(2663);
    let pattern = words.join("|");
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("dictionary-vs-aho");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = resharp::Regex::new(&pattern).unwrap();
    re_resharp.find_all(input).ok();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_hardened = resharp::Regex::with_options(&pattern, resharp::EngineOptions::default().hardened(true)).unwrap();
    re_hardened.find_all(input).ok();
    group.bench_function("resharp-hardened", |b| {
        b.iter(|| black_box(re_hardened.find_all(black_box(input)).unwrap().len()));
    });

    group.finish();
}

fn bench_hardened_pathological(c: &mut Criterion) {
    let pattern = r".*[^A-Z]|[A-Z]";
    let re_default = resharp::Regex::new(pattern).unwrap();
    let re_hardened = resharp::Regex::with_options(
        pattern,
        resharp::EngineOptions::default().hardened(true),
    ).unwrap();

    for &size in &[1_000, 10_000] {
        let input = "A".repeat(size);
        let input = input.as_bytes();
        let label = format!("hardened-pathological/{}K", size / 1000);
        let mut group = c.benchmark_group(&label);
        group.throughput(Throughput::Bytes(input.len() as u64));

        let re_d = &re_default;
        re_d.find_all(input).ok();
        group.bench_function("default", |b| {
            b.iter(|| black_box(re_d.find_all(black_box(input)).unwrap().len()));
        });

        let re_h = &re_hardened;
        re_h.find_all(input).ok();
        group.bench_function("hardened", |b| {
            b.iter(|| black_box(re_h.find_all(black_box(input)).unwrap().len()));
        });

        group.finish();
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().without_plots()
        .warm_up_time(std::time::Duration::from_secs(1))
        .measurement_time(std::time::Duration::from_secs(3))
        .sample_size(20);
    targets = bench_dictionary_vs_aho, bench_hardened_pathological
}
criterion_main!(benches);
