#![allow(dead_code)]
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

macro_rules! bench_three {
    ($group:expr, $pattern:expr, $input:expr) => {{
        let input = $input;
        let pattern = $pattern;

        let re_resharp = resharp::Regex::new(pattern).unwrap();
        re_resharp.find_all(input).ok();
        $group.bench_function("resharp", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });

        let re_regex = regex::bytes::Regex::new(pattern).unwrap();
        $group.bench_function("regex", |b| {
            b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
        });

        let re_fancy = fancy_regex::Regex::new(pattern).unwrap();
        let text = std::str::from_utf8(input).unwrap();
        $group.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
        });
    }};
}

fn bench_literal(c: &mut Criterion) {
    let haystack = load_haystack("en-sampled.txt");
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("literal");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_three!(group, "Sherlock Holmes", input);
    group.finish();
}

fn bench_literal_alternation(c: &mut Criterion) {
    let haystack = load_haystack("en-sampled.txt");
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("literal-alternation");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_three!(group, "Sherlock|Holmes|Watson|Irene|Adler", input);
    group.finish();
}

fn bench_bounded_repeat(c: &mut Criterion) {
    let haystack = load_haystack_lines("en-sampled.txt", 5000);
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("bounded-repeat");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_three!(group, "[A-Za-z]{8,13}", input);
    group.finish();
}

fn bench_bounded_repeat_context(c: &mut Criterion) {
    let haystack = load_haystack_lines("rust-src-tools-3b0d4813.txt", 500);
    let input = haystack.as_bytes();
    let pattern = r"[A-Za-z]{10}\s+[\s\S]{0,100}Result[\s\S]{0,100}\s+[A-Za-z]{10}";

    let mut group = c.benchmark_group("bounded-repeat-context");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_three!(group, pattern, input);
    group.finish();
}

fn bench_date_monster(c: &mut Criterion) {
    let haystack = load_haystack_lines("rust-src-tools-3b0d4813.txt", 10_000);
    let pattern = load_regex("date.txt");
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("date-monster");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = resharp::Regex::new(&pattern).unwrap();
    re_resharp.find_all(input).ok();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
    let text = std::str::from_utf8(input).unwrap();
    group.bench_function("fancy-regex", |b| {
        b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
    });

    group.finish();
}

fn bench_dictionary_500(c: &mut Criterion) {
    let haystack = load_haystack("en-medium.txt");
    let pattern = load_dictionary_pattern(500);
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("dictionary-500");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = resharp::Regex::new(&pattern).unwrap();
    re_resharp.find_all(input).ok();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
    let text = std::str::from_utf8(input).unwrap();
    group.bench_function("fancy-regex", |b| {
        b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
    });

    group.finish();
}

fn bench_dictionary_full(c: &mut Criterion) {
    let haystack = load_haystack("en-medium.txt");
    let pattern = load_dictionary_pattern(2663);
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("dictionary-full");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = resharp::Regex::new(&pattern).unwrap();
    re_resharp.find_all(input).ok();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
    let text = std::str::from_utf8(input).unwrap();
    group.bench_function("fancy-regex", |b| {
        b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
    });

    group.finish();
}

fn bench_phone(c: &mut Criterion) {
    let haystack = load_haystack_lines("en-sampled.txt", 5000);
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("phone");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_three!(group, r"(\(?\+?[0-9]*\)?)?[0-9_\- ()]{7,}", input);
    group.finish();
}

fn bench_dictionary_sorted(c: &mut Criterion) {
    let haystack = load_haystack("length-15-sorted.txt");
    let pattern = load_dictionary_pattern(2663);
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("dictionary-sorted");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = resharp::Regex::new(&pattern).unwrap();
    re_resharp.find_all(input).ok();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    group.finish();
}

fn bench_dictionary_context(c: &mut Criterion) {
    let haystack = load_haystack("length-15-sorted.txt");
    let pattern = load_regex("dictionary-fixed-context.txt");
    let input = haystack.as_bytes();

    let mut group = c.benchmark_group("dictionary-context");
    group.throughput(Throughput::Bytes(input.len() as u64));

    let re_resharp = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn({
            let pattern = pattern.clone();
            let input = input.to_vec();
            move || {
                let re = resharp::Regex::new(&pattern).unwrap();
                re.find_all(&input).ok();
                re
            }
        })
        .unwrap()
        .join()
        .unwrap();
    group.bench_function("resharp", |b| {
        b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
    });

    let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
    group.bench_function("regex", |b| {
        b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
    });

    group.finish();
}

// readme benchmarks

fn bench_readme(c: &mut Criterion) {
    // 1. dictionary search (2663 words, 900KB prose)
    {
        let haystack = load_haystack("en-sampled.txt");
        let pattern = load_dictionary_pattern(2663);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("readme/dictionary");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let re_resharp = resharp::Regex::new(&pattern).unwrap();
        re_resharp.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });
        let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
        g.bench_function("regex", |b| {
            b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
        });
        let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
        let text = std::str::from_utf8(input).unwrap();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
        });
        g.finish();
    }

    // 1b. dictionary search with seeded matches
    {
        let haystack = load_haystack("en-sampled-seeded.txt");
        let pattern = load_dictionary_pattern(2663);
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("readme/dictionary-seeded");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let re_resharp = resharp::Regex::new(&pattern).unwrap();
        re_resharp.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });
        let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
        g.bench_function("regex", |b| {
            b.iter(|| black_box(re_regex.find_iter(black_box(input)).count()));
        });
        let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
        let text = std::str::from_utf8(input).unwrap();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
        });
        g.finish();
    }

    // 2. case-insensitive dictionary (2663 words, 900KB prose)
    //    regex/fancy-regex are ~10000x slower here, timed manually to avoid 270s+ criterion wait
    {
        let haystack = load_haystack("en-sampled.txt");
        let words = load_dictionary_pattern(2663);
        let pattern = format!("(?i)({})", words);
        let input = haystack.as_bytes();
        let len = input.len() as f64;

        let mut g = c.benchmark_group("readme/dictionary-nocase");
        g.throughput(Throughput::Bytes(input.len() as u64));
        let re_resharp = resharp::Regex::new(&pattern).unwrap();
        re_resharp.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });
        g.finish();

        // single-iteration manual timing for the very slow engines
        let re_regex = regex::bytes::Regex::new(&pattern).unwrap();
        let start = std::time::Instant::now();
        let count = re_regex.find_iter(input).count();
        let elapsed = start.elapsed();
        let mib_s = len / elapsed.as_secs_f64() / (1024.0 * 1024.0);
        eprintln!(
            "readme/dictionary-nocase/regex: {count} matches, {mib_s:.2} MiB/s ({elapsed:.2?})"
        );

        let re_fancy = fancy_regex::Regex::new(&pattern).unwrap();
        let text = std::str::from_utf8(input).unwrap();
        let start = std::time::Instant::now();
        let count = re_fancy.find_iter(text).count();
        let elapsed = start.elapsed();
        let mib_s = len / elapsed.as_secs_f64() / (1024.0 * 1024.0);
        eprintln!("readme/dictionary-nocase/fancy-regex: {count} matches, {mib_s:.2} MiB/s ({elapsed:.2?})");
    }

    // 3. literal alternation
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("readme/literal-alternation");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_three!(g, "Sherlock|Holmes|Watson|Irene|Adler", input);
        g.finish();
    }

    // 4. literal
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("readme/literal");
        g.throughput(Throughput::Bytes(input.len() as u64));
        bench_three!(g, "Sherlock Holmes", input);
        g.finish();
    }

    // 5. lookaround
    {
        let haystack = load_haystack("en-sampled.txt");
        let input = haystack.as_bytes();
        let mut g = c.benchmark_group("readme/lookaround");
        g.throughput(Throughput::Bytes(input.len() as u64));

        let pattern = r"(?<=\s)[A-Z][a-z]+(?=\s)";
        let re_resharp = resharp::Regex::new(pattern).unwrap();
        re_resharp.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });

        // regex crate does not support lookaround

        let re_fancy = fancy_regex::Regex::new(pattern).unwrap();
        let text = std::str::from_utf8(input).unwrap();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(re_fancy.find_iter(black_box(text)).count()));
        });
        g.finish();
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().without_plots();
    targets = bench_readme
}
criterion_main!(benches);
