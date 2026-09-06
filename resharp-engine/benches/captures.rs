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

#[cfg(feature = "pcre2-bench")]
fn pcre2_regex(pat: &str) -> pcre2::bytes::Regex {
    pcre2::bytes::RegexBuilder::new()
        .jit_if_available(true)
        .build(pat)
        .unwrap()
}

fn sum_resharp(re: &resharp::Regex, input: &[u8]) -> u64 {
    let mut acc = 0u64;
    for caps in re.captures_all(input).unwrap() {
        for (s, e) in caps.spans()[1..].iter().flatten() {
            acc = acc.wrapping_add(*s as u64).wrapping_add(*e as u64);
        }
    }
    acc
}

fn sum_regex(re: &regex::bytes::Regex, input: &[u8]) -> u64 {
    let mut acc = 0u64;
    for caps in re.captures_iter(input) {
        for m in caps.iter().skip(1).flatten() {
            acc = acc.wrapping_add(m.start() as u64).wrapping_add(m.end() as u64);
        }
    }
    acc
}

fn sum_fancy(re: &fancy_regex::Regex, input: &str) -> u64 {
    let mut acc = 0u64;
    for caps in re.captures_iter(input) {
        let caps = caps.unwrap();
        for i in 1..caps.len() {
            if let Some(m) = caps.get(i) {
                acc = acc.wrapping_add(m.start() as u64).wrapping_add(m.end() as u64);
            }
        }
    }
    acc
}

#[cfg(feature = "pcre2-bench")]
fn sum_pcre2(re: &pcre2::bytes::Regex, input: &[u8]) -> u64 {
    let mut acc = 0u64;
    for caps in re.captures_iter(input) {
        let caps = caps.unwrap();
        for i in 1..caps.len() {
            if let Some(m) = caps.get(i) {
                acc = acc.wrapping_add(m.start() as u64).wrapping_add(m.end() as u64);
            }
        }
    }
    acc
}

macro_rules! bench_captures {
    ($group:expr, $pattern:expr, $input:expr) => {{
        let pattern: &str = $pattern;
        let input: &[u8] = $input;
        let text: &str = std::str::from_utf8(input).unwrap();

        let re_resharp = resharp::Regex::new(pattern).unwrap();
        sum_resharp(&re_resharp, input);
        $group.bench_function("resharp", |b| {
            b.iter(|| black_box(sum_resharp(&re_resharp, black_box(input))));
        });
        $group.bench_function("resharp-find-only", |b| {
            b.iter(|| black_box(re_resharp.find_all(black_box(input)).unwrap().len()));
        });

        let re_regex = regex::bytes::Regex::new(pattern).unwrap();
        $group.bench_function("regex", |b| {
            b.iter(|| black_box(sum_regex(&re_regex, black_box(input))));
        });

        let re_fancy = fancy_regex::Regex::new(pattern).unwrap();
        $group.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(sum_fancy(&re_fancy, black_box(text))));
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let re_pcre2 = pcre2_regex(pattern);
            $group.bench_function("pcre2", |b| {
                b.iter(|| black_box(sum_pcre2(&re_pcre2, black_box(input))));
            });
        }
    }};
}

fn bench_captures_ipv4(c: &mut Criterion) {
    let haystack = load_haystack("apache.input");
    let input = haystack.as_bytes();
    let pattern = r"(?P<a>\d{1,3})\.(?P<b>\d{1,3})\.(?P<c>\d{1,3})\.(?P<d>\d{1,3})";

    let mut group = c.benchmark_group("captures/ipv4-4-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_email(c: &mut Criterion) {
    let haystack = load_haystack("emails.input");
    let input = haystack.as_bytes();
    let pattern = r"(?P<user>[a-zA-Z0-9._%+-]+)@(?P<host>[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})";

    let mut group = c.benchmark_group("captures/email-2-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_credit_card_groups(c: &mut Criterion) {
    let haystack = load_haystack("credit-card.input");
    let input = haystack.as_bytes();
    let pattern = r"(?P<g1>\d{4})(?P<g2>\d{4})(?P<g3>\d{4})(?P<g4>\d{4})";

    let mut group = c.benchmark_group("captures/credit-card-4-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_credit_card_16_groups(c: &mut Criterion) {
    let haystack = load_haystack("credit-card.input");
    let input = haystack.as_bytes();
    let pattern = concat!(
        r"(?P<g1>\d)(?P<g2>\d)(?P<g3>\d)(?P<g4>\d)",
        r"(?P<g5>\d)(?P<g6>\d)(?P<g7>\d)(?P<g8>\d)",
        r"(?P<g9>\d)(?P<g10>\d)(?P<g11>\d)(?P<g12>\d)",
        r"(?P<g13>\d)(?P<g14>\d)(?P<g15>\d)(?P<g16>\d)",
    );

    let mut group = c.benchmark_group("captures/credit-card-16-single-digit-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_apache_datetime(c: &mut Criterion) {
    let haystack = load_haystack("apache.input");
    let input = haystack.as_bytes();
    let pattern = r"\[(?P<day>\d{2})/(?P<mon>[A-Za-z]{3})/(?P<year>\d{4}):(?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2}) (?P<tz>[+-]\d{4})\]";

    let mut group = c.benchmark_group("captures/apache-datetime-7-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_apache_full_log_line(c: &mut Criterion) {
    let haystack = load_haystack("apache.input");
    let input = haystack.as_bytes();
    let pattern = concat!(
        r"(?P<ip>\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}) (?P<ident>[^ ]+) (?P<auth>[^ ]+) ",
        r"\[(?P<day>\d{2})/(?P<mon>[A-Za-z]{3})/(?P<year>\d{4}):(?P<h>\d{2}):(?P<mi>\d{2}):(?P<s>\d{2}) (?P<tz>[+-]\d{4})\] ",
        r#""(?P<method>[A-Z]+) (?P<path>[^ ]+) HTTP/(?P<ver>[^ "]+)" (?P<status>\d{3}) (?P<size>[^ ]+)"#,
    );

    let mut group = c.benchmark_group("captures/apache-full-log-line-15-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

fn bench_captures_bounded_repetition_lines(c: &mut Criterion) {
    let haystack = load_haystack_lines("en-sampled.txt", 5000);
    let input = haystack.as_bytes();
    let pattern = r"(?P<w1>[A-Za-z]{8,13})\s+(?P<w2>[A-Za-z]{8,13})";

    let mut group = c.benchmark_group("captures/bounded-repeat-2-groups");
    group.throughput(Throughput::Bytes(input.len() as u64));
    bench_captures!(group, pattern, input);
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().without_plots();
    targets =
        bench_captures_ipv4,
        bench_captures_email,
        bench_captures_credit_card_groups,
        bench_captures_credit_card_16_groups,
        bench_captures_apache_datetime,
        bench_captures_apache_full_log_line,
        bench_captures_bounded_repetition_lines,
}
criterion_main!(benches);
