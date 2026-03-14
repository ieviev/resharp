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

fn load_regex(name: &str) -> String {
    let path = format!("{}/regexes/{}", data_dir(), name);
    std::fs::read_to_string(&path).unwrap().trim().to_string()
}

fn load_dictionary_pattern(n: usize) -> String {
    let path = format!("{}/regexes/length-15.txt", data_dir());
    let contents = std::fs::read_to_string(&path).unwrap();
    contents.lines().take(n).collect::<Vec<_>>().join("|")
}

macro_rules! bench_resharp {
    ($group:expr, $pattern:expr, $input:expr) => {{
        let re = resharp::Regex::new($pattern).unwrap();
        re.find_all($input).ok();
        $group.bench_function("resharp", |b| {
            b.iter(|| black_box(re.find_all(black_box($input)).unwrap().len()));
        });
    }};
}

fn bench_resharp_regression(c: &mut Criterion) {
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/literal");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, "Sherlock Holmes", input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/literal-alternation");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, "Sherlock|Holmes|Watson|Irene|Adler", input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("en-sampled.txt", 5000);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/bounded-repeat");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, "[A-Za-z]{8,13}", input);
        g.finish();
    }
    {
        let haystack = load_haystack("rust-src-tools-3b0d4813.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/url");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, r"https?://[a-zA-Z0-9./_?&=#%-]+", input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("rust-src-tools-3b0d4813.txt", 500);
        let input = haystack.as_bytes();
        let pattern = r"[A-Za-z]{10}\s+[\s\S]{0,100}Result[\s\S]{0,100}\s+[A-Za-z]{10}";
        let mut g = c.benchmark_group("resharp-only/bounded-repeat-context");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("rust-src-tools-3b0d4813.txt", 10_000);
        let pattern = load_regex("date.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/date-monster");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-medium.txt");
        let pattern = load_dictionary_pattern(500);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/dictionary-500");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-medium.txt");
        let pattern = load_dictionary_pattern(2663);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/dictionary-full");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("en-sampled.txt", 5000);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/phone");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, r"(\(?\+?[0-9]*\)?)?[0-9_\- ()]{7,}", input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/lookaround");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, r"(?<=\s)[A-Z][a-z]+(?=\s)", input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let words = load_dictionary_pattern(2663);
        let pattern = format!("(?i)({})", words);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/dictionary-nocase");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack("cloud-flare-redos.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/dotstar-eq-redos");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, ".*=.*", input);
        g.finish();
    }
}

criterion_group! {
    name = regression;
    config = Criterion::default().without_plots();
    targets = bench_resharp_regression
}
criterion_main!(regression);
