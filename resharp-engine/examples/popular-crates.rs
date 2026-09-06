// Most-used regex patterns across crates.io users of the `regex` crate.
// Ranking (crates, occurrences, pattern):
//   1.  209   272  \s+
//   2.   81   111  \d+
//   3.   65   112  .*
//   4.   62    65  ^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$
//   5.   55    76  <[^>]+>
//   6.   54    63  \n{3,}
//   7.   52    91  \x1b\[[0-9;]*m
//   8.   39    44  ^\d{4}-\d{2}-\d{2}$
//   9.   37    49  ^$
//  10.   35    46  \$\{([^}]+)\}
//  11.   35    43  ^\d+$
//  12.   34    40  <[^>]*>
//  13.   33    37  \w+
//  14.   26    32  -+
//  15.   26    27  ^[a-zA-Z_][a-zA-Z0-9_]*$
//  16.   23    55  test
//  17.   23    26  \d+\.\d+\.\d+
//  18.   22    24  [0-9]+
//  19.   21    27  ^[a-zA-Z0-9_-]+$
//  20.   21    21  `([^`]+)`
//  21.   20    24  "([^"]+)"
//  22.   20    22  \[([^\]]+)\]\(([^)]+)\)
//  23.   20    22  \s{2,}
//  24.   20    21  [A-Z]
//  25.   19    24  (?is)<script[^>]*>.*?</script>

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use resharp::{Regex, RegexOptions, UnicodeMode};
use std::time::Duration;

fn resharp_regex(pat: &str) -> Regex {
    let opts = RegexOptions::default()
        .unicode(UnicodeMode::Full)
        .multiline(false);
    Regex::with_options(pat, opts).unwrap()
}

// PCRE2 defaults to interpreted (non-JIT) execution; `jit_if_available` is
// what any perf-sensitive pcre2 user would enable, and skipping it makes
// pcre2 look far slower than it is (measured 2-3x+ on this machine).
//
// `ucp`/`utf` are both off by default in PCRE2, which would make its
// `\s`/`\w`/`\d` classes ASCII-only while resharp runs `UnicodeMode::Full`
// above; enable both here so both engines match the same Unicode-aware
// language on every pattern.
#[cfg(feature = "pcre2-bench")]
fn pcre2_regex(pat: &str) -> pcre2::bytes::Regex {
    pcre2::bytes::RegexBuilder::new()
        .jit_if_available(true)
        .utf(true)
        .ucp(true)
        .build(pat)
        .unwrap()
}

const TARGET_LEN: usize = 1 << 20;

// (name, pattern for regex/fancy-regex, pattern for resharp)
const SCAN_PATTERNS: &[(&str, &str, &str)] = &[
    ("whitespace", r"\s+", r"\s+"),
    ("digits", r"\d+", r"\d+"),
    ("dot-star", r".*", r".*"),
    ("html-tag", r"<[^>]+>", r"<[^>]+>"),
    ("blank-lines", r"\n{3,}", r"\n{3,}"),
    ("ansi-escape", r"\x1b\[[0-9;]*m", r"\x1b\[[0-9;]*m"),
    ("template-var", r"\$\{([^}]+)\}", r"\$\{([^}]+)\}"),
    ("html-tag-any", r"<[^>]*>", r"<[^>]*>"),
    ("word", r"\w+", r"\w+"),
    ("dash-run", r"-+", r"-+"),
    ("literal-test", r"test", r"test"),
    ("semver", r"\d+\.\d+\.\d+", r"\d+\.\d+\.\d+"),
    ("digits-class", r"[0-9]+", r"[0-9]+"),
    ("code-span", r"`([^`]+)`", r"`([^`]+)`"),
    ("quoted-string", r#""([^"]+)""#, r#""([^"]+)""#),
    (
        "md-link",
        r"\[([^\]]+)\]\(([^)]+)\)",
        r"\[([^\]]+)\]\(([^)]+)\)",
    ),
    ("multi-space", r"\s{2,}", r"\s{2,}"),
    ("uppercase-letter", r"[A-Z]", r"[A-Z]"),
    (
        "script-tag",
        r"(?is)<script[^>]*>.*?</script>",
        r"(?i)<script[^>]*>~(_*</script>_*)</script>",
    ),
];

const VALIDATE_PATTERNS: &[(&str, &str, &str)] = &[
    (
        "email",
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$",
        "user.name+tag@example.co",
    ),
    ("iso-date", r"^\d{4}-\d{2}-\d{2}$", "2021-08-14"),
    ("empty-line", r"^$", ""),
    ("numeric-line", r"^\d+$", "1234567890"),
    (
        "identifier",
        r"^[a-zA-Z_][a-zA-Z0-9_]*$",
        "some_ident_name123",
    ),
    ("slug", r"^[a-zA-Z0-9_-]+$", "some-slug_name-123"),
];

// `regex` has no lookaround support, so no "regex" column below.
// (name, pattern for fancy-regex/pcre2, pattern for resharp, sample)
//
// RE#'s bare `_` matches any byte, so `username-no-symbols-only`'s literal
// underscores must be escaped as `\_`.
const LOOKAROUND_VALIDATE_PATTERNS: &[(&str, &str, &str, &str)] = &[
    (
        "password-strength",
        r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]{8,}$",
        r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]{8,}$",
        "P@ssw0rd!",
    ),
    (
        "username-no-symbols-only",
        r"^(?![-_]*$)[A-Za-z0-9][A-Za-z0-9-_]{3,20}[A-Za-z0-9]$",
        r"^(?![\-\_]*$)[A-Za-z0-9][A-Za-z0-9\-\_]{3,20}[A-Za-z0-9]$",
        "valid123",
    ),
];

// (name, pattern for fancy-regex/pcre2, pattern for resharp)
//
// RE#'s bare `_` matches any byte, so `deleted-at-token`'s literal
// underscores must be escaped as `\_`.
const LOOKAROUND_SCAN_PATTERNS: &[(&str, &str, &str)] = &[
    (
        "deleted-at-token",
        r"(?<!_)deleted_at(?!_)",
        r"(?<!\_)deleted\_at(?!\_)",
    ),
    ("decimal-point", r"(?<=\d)\.(?=\S)", r"(?<=\d)\.(?=\S)"),
    (
        "attribute-whitespace",
        r#"(?<="|')\s+(?=[^<>\s]+=)"#,
        r#"(?<="|')\s+(?=[^<>\s]+=)"#,
    ),
];

fn build_haystack() -> String {
    let lines = [
        "2021-08-14 request Version/12.4 from https://example.com/path?q=1",
        "user_name-01 committed abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdef01",
        "apple and carrot were 42 items --- separated___by...punctuation",
        "the quick brown fox jumps over 1024 lazy dogs near https://rust-lang.org",
        "",
        "",
        "",
        "42",
        "config value = 3.14159 tag=v2 build#77 status: OK",
        "<b>Bold</b> and <br/> and <> empty tag here",
        "\x1b[31mERROR\x1b[0m: build failed at 1.2.3, retry with 1.4.0",
        "Hello ${name}, your balance is ${amount} USD",
        "run `cargo build --release` then retest the test suite again",
        "set title to \"hello world\" and note to \"a  test\"",
        "see [docs](https://example.com/docs) for  more   details",
        "<script src=\"a.js\"></script> and <SCRIPT>if (a<b) { y = 2; }</SCRIPT>",
        "column deleted_at is nullable but mut_deleted_at and deleted_at_utc are not soft-delete columns",
        "Price is 5.99 and quantity is 3. Also see 12.5kg and end.",
        "<a href=\"x\"   target=\"_blank\" class='y'   data-x=1>",
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
            b.iter(|| {
                black_box(
                    rs.find_all(black_box(input))
                        .unwrap()
                        .iter()
                        .map(|m| m.end - m.start)
                        .sum::<usize>(),
                )
            });
        });

        let rx = regex::Regex::new(pat).unwrap();
        rx.find_iter(&haystack).count();
        g.bench_function("regex", |b| {
            b.iter(|| {
                black_box(
                    rx.find_iter(black_box(&haystack))
                        .map(|m| m.end() - m.start())
                        .sum::<usize>(),
                )
            });
        });

        let fx = fancy_regex::Regex::new(pat).unwrap();
        fx.find_iter(&haystack).count();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| {
                black_box(
                    fx.find_iter(black_box(&haystack))
                        .map(|m| {
                            let m = m.unwrap();
                            m.end() - m.start()
                        })
                        .sum::<usize>(),
                )
            });
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let pc = pcre2_regex(pat);
            pc.find_iter(input).count();
            g.bench_function("pcre2", |b| {
                b.iter(|| {
                    black_box(
                        pc.find_iter(black_box(input))
                            .map(|m| {
                                let m = m.unwrap();
                                m.end() - m.start()
                            })
                            .sum::<usize>(),
                    )
                });
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
            let pc = pcre2_regex(pat);
            assert!(pc.is_match(bytes).unwrap());
            g.bench_function("pcre2", |b| {
                b.iter(|| black_box(pc.is_match(black_box(bytes)).unwrap()));
            });
        }

        g.finish();
    }
}

fn bench_lookaround_scan(c: &mut Criterion) {
    let haystack = build_haystack();
    let input = haystack.as_bytes();

    for (name, pat, rs_pat) in LOOKAROUND_SCAN_PATTERNS {
        let mut g = c.benchmark_group(format!("scan/{}", name));
        g.throughput(Throughput::Bytes(input.len() as u64));

        let rs = resharp_regex(rs_pat);
        rs.find_all(input).ok();
        g.bench_function("resharp", |b| {
            b.iter(|| {
                black_box(
                    rs.find_all(black_box(input))
                        .unwrap()
                        .iter()
                        .map(|m| m.end - m.start)
                        .sum::<usize>(),
                )
            });
        });

        let fx = fancy_regex::Regex::new(pat).unwrap();
        fx.find_iter(&haystack).count();
        g.bench_function("fancy-regex", |b| {
            b.iter(|| {
                black_box(
                    fx.find_iter(black_box(&haystack))
                        .map(|m| {
                            let m = m.unwrap();
                            m.end() - m.start()
                        })
                        .sum::<usize>(),
                )
            });
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let pc = pcre2_regex(pat);
            pc.find_iter(input).count();
            g.bench_function("pcre2", |b| {
                b.iter(|| {
                    black_box(
                        pc.find_iter(black_box(input))
                            .map(|m| {
                                let m = m.unwrap();
                                m.end() - m.start()
                            })
                            .sum::<usize>(),
                    )
                });
            });
        }

        g.finish();
    }
}

fn bench_lookaround_validate(c: &mut Criterion) {
    for (name, pat, rs_pat, sample) in LOOKAROUND_VALIDATE_PATTERNS {
        let bytes = sample.as_bytes();
        let mut g = c.benchmark_group(format!("validate/{}", name));
        g.throughput(Throughput::Bytes(bytes.len() as u64));

        let rs = resharp_regex(rs_pat);
        assert!(rs.is_match(bytes).unwrap());
        g.bench_function("resharp", |b| {
            b.iter(|| black_box(rs.is_match(black_box(bytes)).unwrap()));
        });

        let fx = fancy_regex::Regex::new(pat).unwrap();
        assert!(fx.is_match(sample).unwrap());
        g.bench_function("fancy-regex", |b| {
            b.iter(|| black_box(fx.is_match(black_box(sample)).unwrap()));
        });

        #[cfg(feature = "pcre2-bench")]
        {
            let pc = pcre2_regex(pat);
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
    targets = bench_scan, bench_validate, bench_lookaround_scan, bench_lookaround_validate
}
criterion_main!(popular);
