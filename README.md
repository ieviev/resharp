# RE#

[![crates.io](https://img.shields.io/crates/v/resharp.svg)](https://crates.io/crates/resharp)
[![docs.rs](https://docs.rs/resharp/badge.svg)](https://docs.rs/resharp)

A high-performance, automata-based regex engine with first-class support for **intersection** (`&`), **complement** (`~`). Non-backtracking with linear-time matching. Built for complex patterns (large alternations, lookarounds, boolean combinations) that make traditional engines degrade or fall back to slower paths.

[paper](https://dl.acm.org/doi/10.1145/3704837) | [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) | [syntax docs](https://github.com/ieviev/resharp/blob/main/docs/syntax.md) | [dotnet version](https://github.com/ieviev/resharp-dotnet) and [web playground](https://ieviev.github.io/resharp-webapp/)

## Quick start

```sh
cargo add resharp
```

```rust
// 8+ alphanumeric & contains digit & contains uppercase
let re = resharp::Regex::new(r"[A-Za-z0-9]{8,}&_*[0-9]_*&_*[A-Z]_*").unwrap();

let found = re.is_match(b"Hunter2024").unwrap();
let matches = re.find_all(b"try Hunter2024 or password1").unwrap();
```

## When to use RE# over [`regex`](https://crates.io/crates/regex)

RE# operates on `&[u8]` / UTF-8 and aims to match `regex` crate throughput on standard patterns. Use RE# when you need:

- intersection (`&`), complement (`~`), or lookarounds
- large alternations with high throughput (at the cost of memory)
- fail-loud behavior: capacity / lookahead overflow returns `Err` instead of silently degrading

RE# is designed around `is_match` and `find_all`. It doesn't provide `find` or `captures`, but for simple cases you can often substitute `find_anchored`, or emulate a capture group with lookarounds. For example, `a(b)c` becomes `(?<=a)b(?=c)`. For anything more involved, use the `regex` crate instead.

## Syntax extensions

RE# supports standard regex syntax plus three extensions: `_` (any byte), `&` (intersection), and `~(...)` (complement). `_*` means "any string".

```perl
_*                any string
a_*               any string that starts with 'a'
_*a               any string that ends with 'a'
_*a_*             any string that contains 'a'
~(_*a_*)          any string that does NOT contain 'a'
(_*a_*)&~(_*b_*)  contains 'a' AND does not contain 'b'
(?<=b)_*&_*(?=a)  preceded by 'b' AND followed by 'a'
```

You combine all of these with `&` to get more complex patterns. RE# also supports lookarounds (`(?=...)`, `(?<=...)`, `(?!...)`, `(?<!...)`), compiled directly into the automaton with no backtracking.

## Differences from PCRE / `regex`

- **Leftmost-longest, not leftmost-greedy.** `y|yes` on `"yes"` matches `yes`. Branch order is irrelevant.
- **Multiline on by default.** `^`/`$` match start/end of line; disable with `(?-m)`. `\A`/`\z` always anchor to input.
- **`\w` defaults to 2-byte UTF-8.** See [UnicodeMode](docs/syntax.md#unicode).

Lazy quantifiers (`*?`, `+?`, ...) are parse errors; rewrite with complement when possible: `<div>.*?</div>` -> `<div>~(_*</div>_*)</div>`. See [syntax.md](docs/syntax.md) for the rest.

## Configuration

```rust
let opts = resharp::RegexOptions {
    max_dfa_capacity: 65535,    // max automata states (default: u16::MAX)
    lookahead_context_max: 800, // max lookahead context distance (default: 800)
    hardened: false,            // linear find_all worst-case (slower but safer)
    unicode: resharp::UnicodeMode::Default, // Ascii | Default | Full | Javascript
    ..Default::default()
};
let re = resharp::Regex::with_options(r"pattern", opts).unwrap();
```

## Benchmarks

RE# against `regex`, `fancy-regex`, and PCRE2 on a few popular patterns from crates.io. Regenerate with:

```sh
node scripts/bench-popular-table.mts
```

<!-- POPULAR-BENCH:BEGIN -->
resharp runs with `UnicodeMode::Full` and `multiline(false)` to match the other engines. Ratios are vs the fastest per row.

### Scan (find_all over a 1 MiB haystack), throughput

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `\s+` | **414.94 MiB/s (1.00x)** | 391.82 MiB/s (1.06x) | 155.91 MiB/s (2.66x) | 184.44 MiB/s (2.25x) |
| `\d+` | **1012.4 MiB/s (1.00x)** | 503.52 MiB/s (2.01x) | 304.87 MiB/s (3.32x) | 362.47 MiB/s (2.79x) |
| `.*` | **2.42 GiB/s (1.00x)** | 326.02 MiB/s (7.60x) | 166.82 MiB/s (14.86x) | 303.4 MiB/s (8.17x) |
| `[0-9a-f]{64}` | **1.3 GiB/s (1.00x)** | 718 MiB/s (1.86x) | 597.23 MiB/s (2.23x) | 180.28 MiB/s (7.39x) |
| `https?://\S+` | **4.58 GiB/s (1.00x)** | 2.35 GiB/s (1.95x) | 1.34 GiB/s (3.41x) | 1.81 GiB/s (2.53x) |
| `Version/([.0-9]+)` | 7.09 GiB/s (1.04x) | **7.38 GiB/s (1.00x)** | 3.68 GiB/s (2.01x) | 3.96 GiB/s (1.86x) |
| `\n{3,}` | **11.66 GiB/s (1.00x)** | 11.24 GiB/s (1.04x) | 5.15 GiB/s (2.27x) | 1.79 GiB/s (6.53x) |
| `[-_.]+` | **1.74 GiB/s (1.00x)** | 1008.6 MiB/s (1.77x) | 481.64 MiB/s (3.71x) | 480.85 MiB/s (3.71x) |

### Validate (is_match on a single value), latency

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `^\d{4}-\d{2}-\d{2}$` | 23.42 ns (1.05x) | 24.32 ns (1.09x) | **22.3 ns (1.00x)** | 59.97 ns (2.69x) |
| `^([a-zA-Z][a-zA-Z0-9_-]+)$` | 34.62 ns (1.05x) | 34.84 ns (1.06x) | **32.86 ns (1.00x)** | 77.11 ns (2.35x) |
| `^[0-9]+$` | 24.53 ns (1.25x) | 22.86 ns (1.16x) | **19.64 ns (1.00x)** | 56.37 ns (2.87x) |

<!-- POPULAR-BENCH:END -->
