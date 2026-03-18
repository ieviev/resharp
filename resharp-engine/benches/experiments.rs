use aho_corasick::AhoCorasick;
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

    let ac = AhoCorasick::new(&words).unwrap();
    group.bench_function("aho-corasick", |b| {
        b.iter(|| black_box(ac.find_iter(black_box(input)).count()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().without_plots();
    targets = bench_dictionary_vs_aho
}
criterion_main!(benches);
