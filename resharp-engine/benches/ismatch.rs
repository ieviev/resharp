use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn data_dir() -> String {
    format!("{}/../data", env!("CARGO_MANIFEST_DIR"))
}

fn load_haystack(name: &str) -> String {
    let path = format!("{}/haystacks/{}", data_dir(), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to load {}: {}", path, e))
}

fn load_haystack_lines(name: &str, n: usize) -> String {
    let full = load_haystack(name);
    full.lines().take(n).collect::<Vec<_>>().join("\n")
}

fn load_dictionary_pattern(n: usize) -> String {
    let path = format!("{}/regexes/length-15.txt", data_dir());
    let contents = std::fs::read_to_string(&path).unwrap();
    contents.lines().take(n).collect::<Vec<_>>().join("|")
}

fn bench_ismatch(c: &mut Criterion) {
    let haystack = load_haystack("en-sampled.txt");
    let input = haystack.as_bytes();

    // literal - match exists early in input
    {
        let mut g = c.benchmark_group("is_match/literal-hit");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let rs = resharp::Regex::new("Sherlock Holmes").unwrap();
        let rx = regex::Regex::new("Sherlock Holmes").unwrap();
        rs.is_match(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack))));
        });
        g.finish();
    }

    // literal - no match, must scan entire input
    {
        let mut g = c.benchmark_group("is_match/literal-miss");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let rs = resharp::Regex::new("ZZZYYYXXX999").unwrap();
        let rx = regex::Regex::new("ZZZYYYXXX999").unwrap();
        rs.is_match(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack))));
        });
        g.finish();
    }

    // bounded repeat - match exists
    {
        let haystack2 = load_haystack_lines("en-sampled.txt", 2500);
        let input2 = haystack2.as_bytes();
        let mut g = c.benchmark_group("is_match/bounded-repeat-hit");
        g.throughput(Throughput::Bytes(input2.len() as u64));
        let rs = resharp::Regex::new(r"\b[0-9A-Za-z_]{12,}\b").unwrap();
        let rx = regex::Regex::new(r"\b[0-9A-Za-z_]{12,}\b").unwrap();
        rs.is_match(input2).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input2)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack2))));
        });
        g.finish();
    }

    // alternation - match exists
    {
        let mut g = c.benchmark_group("is_match/alternation-hit");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let rs = resharp::Regex::new("Sherlock|Holmes|Watson|Irene|Adler").unwrap();
        let rx = regex::Regex::new("Sherlock|Holmes|Watson|Irene|Adler").unwrap();
        rs.is_match(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack))));
        });
        g.finish();
    }

    // dictionary - match exists
    {
        let pattern = load_dictionary_pattern(2663);
        let mut g = c.benchmark_group("is_match/dictionary-hit");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let rs = resharp::Regex::new(&pattern).unwrap();
        let rx = regex::Regex::new(&pattern).unwrap();
        rs.is_match(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack))));
        });
        g.finish();
    }

    // word boundary pattern - no lookaround equivalent in regex crate
    {
        let mut g = c.benchmark_group("is_match/word-boundary-hit");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let rs = resharp::Regex::new(r"\b[A-Z][a-z]+\b").unwrap();
        let rx = regex::Regex::new(r"\b[A-Z][a-z]+\b").unwrap();
        rs.is_match(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(input)).unwrap()));
        });
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(&haystack))));
        });
        g.finish();
    }

    // compare find_all vs is_match on same pattern
    {
        let mut g = c.benchmark_group("is_match/vs-find-all");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let re = resharp::Regex::new(r"\b[A-Z][a-z]+\b").unwrap();
        re.find_all(input).ok();
        g.bench_function("find_all", |b| {
            b.iter(|| black_box(re.find_all(black_box(input)).unwrap().len()));
        });
        g.bench_function("is_match", |b| {
            b.iter(|| black_box(re.is_match(black_box(input)).unwrap()));
        });
        g.finish();
    }
}

criterion_group! {
    name = ismatch;
    config = Criterion::default().without_plots();
    targets = bench_ismatch
}
criterion_main!(ismatch);
