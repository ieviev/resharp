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
    ($group:expr, $pattern:expr, $input:expr, $opts:expr) => {{
        let re = resharp::Regex::with_options($pattern, $opts).unwrap();
        re.find_all($input).ok();
        $group.bench_function("resharp", |b| {
            b.iter(|| black_box(re.find_all(black_box($input)).unwrap().len()));
        });
    }};
}

macro_rules! bench_default_vs_hardened {
    ($group:expr, $pattern:expr, $input:expr) => {{
        let re_default = resharp::Regex::new($pattern).unwrap();
        re_default.find_all($input).ok();
        $group.bench_function("default", |b| {
            b.iter(|| black_box(re_default.find_all(black_box($input)).unwrap().len()));
        });
        let re_hardened = resharp::Regex::with_options(
            $pattern,
            resharp::EngineOptions::default().hardened(true),
        ).unwrap();
        re_hardened.find_all($input).ok();
        $group.bench_function("hardened", |b| {
            b.iter(|| black_box(re_hardened.find_all(black_box($input)).unwrap().len()));
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
        let haystack = load_haystack_lines("en-sampled.txt", 2500);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/bounded-repeat");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, r"\b[0-9A-Za-z_]{12,}\b", input);
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
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/lookaround");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, r"(?<=\s)[A-Z][a-z]+(?=\s)", input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("en-sampled.txt", 10_000);
        let pattern = load_regex("date.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("resharp-only/date-monster");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_resharp!(g, &pattern, input);
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
}

fn bench_hardened_regression(c: &mut Criterion) {
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/literal");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, "Sherlock Holmes", input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/literal-alternation");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, "Sherlock|Holmes|Watson|Irene|Adler", input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("en-sampled.txt", 2500);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/bounded-repeat");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, r"\b[0-9A-Za-z_]{12,}\b", input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-medium.txt");
        let pattern = load_dictionary_pattern(2663);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/dictionary-full");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/lookaround");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, r"(?<=\s)[A-Z][a-z]+(?=\s)", input);
        g.finish();
    }
    {
        let haystack = load_haystack_lines("en-sampled.txt", 10_000);
        let pattern = load_regex("date.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/date-monster");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, &pattern, input);
        g.finish();
    }
    {
        let haystack = load_haystack("en-sampled.txt");
        let words = load_dictionary_pattern(2663);
        let pattern = format!("(?i)({})", words);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("hardened/dictionary-nocase");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_default_vs_hardened!(g, &pattern, input);
        g.finish();
    }
}

criterion_group! {
    name = regression;
    config = Criterion::default().without_plots();
    targets = bench_resharp_regression, bench_hardened_regression
}
criterion_main!(regression);
