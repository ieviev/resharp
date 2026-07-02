use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use resharp::{Regex, RegexOptions, UnicodeMode};
use std::time::Duration;

fn resharp_regex(pat: &str) -> Regex {
    let opts = RegexOptions::default()
        .unicode(UnicodeMode::Full)
        .multiline(false);
    Regex::with_options(pat, opts).unwrap()
}

const TARGET_LEN: usize = 1 << 20;

const SCAN_PATTERNS: &[(&str, &str, &str)] = &[
    ("whitespace", r"\s+", r"\s+"),
    ("digits", r"\d+", r"\d+"),
    ("dot-star", r".*", r".*"),
    ("sha256-hex", r"[0-9a-f]{64}", r"[0-9a-f]{64}"),
    ("url", r"https?://\S+", r"https?://\S+"),
    ("version-capture", r"Version/([.0-9]+)", r"(?<=Version/)[.0-9]+"),
    ("blank-lines", r"\n{3,}", r"\n{3,}"),
    ("punct-run", r"[-_.]+", r"[-_.]+"),
];

const VALIDATE_PATTERNS: &[(&str, &str, &str)] = &[
    ("iso-date", r"^\d{4}-\d{2}-\d{2}$", "2021-08-14"),
    ("ident", r"^([a-zA-Z][a-zA-Z0-9_-]+)$", "some_ident-name123"),
    ("num-line", r"^[0-9]+$", "1234567890"),
];

fn build_haystack() -> String {
    let lines = [
        "2021-08-14 request Version/12.4 from https://example.com/path?q=1",
        "user_name-01 committed abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        "apple and carrot were 42 items --- separated___by...punctuation",
        "the quick brown fox jumps over 1024 lazy dogs near https://rust-lang.org",
        "",
        "",
        "",
        "42",
        "config value = 3.14159 tag=v2 build#77 status: OK",
    ];
    let mut s = String::with_capacity(TARGET_LEN + 256);
    let mut i = 0usize;
    while s.len() < TARGET_LEN {
        s.push_str(lines[i % lines.len()]);
        s.push('\n');
        i += 1;
    }
    s
}

fn bench_scan(c: &mut Criterion) {
    let haystack = build_haystack();
    let input = haystack.as_bytes();

    for (name, pat, rs_pat) in SCAN_PATTERNS {
        let mut g = c.benchmark_group(format!("scan/{}", name));
        g.throughput(Throughput::Bytes(input.len() as u64));

        let rs = resharp_regex(rs_pat);
        rs.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.find_all(black_box(input)).unwrap().len()));
        });

        let rx = regex::Regex::new(pat).unwrap();
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.find_iter(black_box(&haystack)).count()));
        });

        let fx = fancy_regex::Regex::new(pat).unwrap();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(fx.find_iter(black_box(&haystack)).count()));
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let pc = pcre2::bytes::Regex::new(pat).unwrap();
            g.bench_function("pcre2", |b| {
                b.iter(|| black_box(pc.find_iter(black_box(input)).count()));
            });
        }

        g.finish();
    }
}

fn bench_validate(c: &mut Criterion) {
    for (name, pat, sample) in VALIDATE_PATTERNS {
        let bytes = sample.as_bytes();
        let mut g = c.benchmark_group(format!("validate/{}", name));
        g.throughput(Throughput::Bytes(bytes.len() as u64));

        let rs = resharp_regex(pat);
        assert!(rs.is_match(bytes).unwrap());
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(bytes)).unwrap()));
        });

        let rx = regex::Regex::new(pat).unwrap();
        assert!(rx.is_match(sample));
        g.bench_function("regex", |b| {
            b.iter(|| black_box(rx.is_match(black_box(sample))));
        });

        let fx = fancy_regex::Regex::new(pat).unwrap();
        assert!(fx.is_match(sample).unwrap());
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(fx.is_match(black_box(sample)).unwrap()));
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let pc = pcre2::bytes::Regex::new(pat).unwrap();
            assert!(pc.is_match(bytes).unwrap());
            g.bench_function("pcre2", |b| {
                b.iter(|| black_box(pc.is_match(black_box(bytes)).unwrap()));
            });
        }

        g.finish();
    }
}

criterion_group! {
    name = popular;
    config = Criterion::default()
        .without_plots()
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_millis(800))
        .sample_size(20);
    targets = bench_scan, bench_validate
}
criterion_main!(popular);
