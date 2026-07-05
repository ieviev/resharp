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
Full benchmark source: [resharp-engine/examples/popular-crates.rs](resharp-engine/examples/popular-crates.rs). Ratios are vs the fastest per row.

CPU: AMD Ryzen 7 5800X 8-Core Processor

### Scan (find_all over a 1 MiB haystack), throughput

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `\s+` | **376.54 MiB/s (1.00x)** | 154.2 MiB/s (2.44x) | 150.46 MiB/s (2.50x) | 158.71 MiB/s (2.37x) |
| `\d+` | **1.86 GiB/s (1.00x)** | 417.7 MiB/s (4.55x) | 399.93 MiB/s (4.75x) | 358.99 MiB/s (5.29x) |
| `.*` | **2.37 GiB/s (1.00x)** | 179.62 MiB/s (13.52x) | 178.81 MiB/s (13.58x) | 772.91 MiB/s (3.14x) |
| `<[^>]+>` | 2.11 GiB/s (1.52x) | 548.65 MiB/s (5.98x) | 540.81 MiB/s (6.06x) | **3.2 GiB/s (1.00x)** |
| `\n{3,}` | **20.96 GiB/s (1.00x)** | 14 GiB/s (1.50x) | 13.7 GiB/s (1.53x) | 6.66 GiB/s (3.15x) |
| `\x1b\[[0-9;]*m` | **11.88 GiB/s (1.00x)** | 8.09 GiB/s (1.47x) | 7.62 GiB/s (1.56x) | 11.03 GiB/s (1.08x) |
| `\$\{([^}]+)\}` | 5.58 GiB/s (1.86x) | 892.82 MiB/s (11.90x) | 885.68 MiB/s (12.00x) | **10.38 GiB/s (1.00x)** |
| `<[^>]*>` | 2.85 GiB/s (1.05x) | 538.69 MiB/s (5.68x) | 528.73 MiB/s (5.79x) | **2.99 GiB/s (1.00x)** |
| `\w+` | **278.02 MiB/s (1.00x)** | 121.11 MiB/s (2.30x) | 111.78 MiB/s (2.49x) | 197.13 MiB/s (1.41x) |
| `-+` | **6.94 GiB/s (1.00x)** | 2.4 GiB/s (2.89x) | 2.26 GiB/s (3.07x) | 4.49 GiB/s (1.55x) |
| `test` | 12.34 GiB/s (1.48x) | **18.3 GiB/s (1.00x)** | 15.33 GiB/s (1.19x) | 6.09 GiB/s (3.00x) |
| `\d+\.\d+\.\d+` | **4.91 GiB/s (1.00x)** | 2.87 GiB/s (1.71x) | 2.81 GiB/s (1.75x) | 393.74 MiB/s (12.77x) |
| `[0-9]+` | **1.98 GiB/s (1.00x)** | 549.92 MiB/s (3.69x) | 511.46 MiB/s (3.97x) | 1.01 GiB/s (1.96x) |
| ``([^`]+)`` | 12.84 GiB/s (1.11x) | 8.67 GiB/s (1.64x) | 8.5 GiB/s (1.67x) | **14.22 GiB/s (1.00x)** |
| `"([^"]+)"` | **5.55 GiB/s (1.00x)** | 3 GiB/s (1.85x) | 2.84 GiB/s (1.95x) | 5.39 GiB/s (1.03x) |
| `\[([^\]]+)\]\(([^)]+)\)` | 4.94 GiB/s (1.17x) | 573.78 MiB/s (10.30x) | 573.63 MiB/s (10.30x) | **5.77 GiB/s (1.00x)** |
| `\s{2,}` | 613.54 MiB/s (1.03x) | 631.14 MiB/s (1.00x) | **631.18 MiB/s (1.00x)** | 327.38 MiB/s (1.93x) |
| `[A-Z]` | **2.23 GiB/s (1.00x)** | 410.38 MiB/s (5.57x) | 405.08 MiB/s (5.64x) | 1.12 GiB/s (1.99x) |
| `(?is)<script[^>]*>.*?</script>` | 785.35 MiB/s (9.25x) | 631.93 MiB/s (11.50x) | 647.29 MiB/s (11.22x) | **7.1 GiB/s (1.00x)** |

### Validate (is_match on a single value), latency

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9....` | 42.58 ns (1.17x) | 42.55 ns (1.17x) | 38.89 ns (1.07x) | **36.29 ns (1.00x)** |
| `^\d{4}-\d{2}-\d{2}$` | 22.83 ns (1.04x) | 23.52 ns (1.08x) | **21.86 ns (1.00x)** | 27.54 ns (1.26x) |
| `^$` | **2.77 ns (1.00x)** | 17.27 ns (6.23x) | 14.29 ns (5.15x) | 18.23 ns (6.58x) |
| `^\d+$` | 23.52 ns (1.09x) | 23.5 ns (1.09x) | **21.52 ns (1.00x)** | 28.96 ns (1.35x) |
| `^[a-zA-Z_][a-zA-Z0-9_]*$` | 34.59 ns (1.24x) | 33.76 ns (1.21x) | 30.49 ns (1.09x) | **27.97 ns (1.00x)** |
| `^[a-zA-Z0-9_-]+$` | 34.42 ns (1.17x) | 33.74 ns (1.15x) | 30.31 ns (1.03x) | **29.36 ns (1.00x)** |

### Lookaround (rare in the corpus)

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `(?<!_)deleted_at(?!_)` | **8.37 GiB/s (1.00x)** | 37.55 MiB/s (228.26x) | 6.33 GiB/s (1.32x) |
| `(?<=\d)\.(?=\S)` | 2.62 GiB/s (1.23x) | 31.27 MiB/s (105.56x) | **3.22 GiB/s (1.00x)** |
| `(?<="\|')\s+(?=[^<>\s]+=)` | **2.53 GiB/s (1.00x)** | 39.51 MiB/s (65.48x) | 417.48 MiB/s (6.20x) |

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `^(?=.*[a-z])(?=.*[A-Z])(?=.*\d...` | **21.28 ns (1.00x)** | 425.63 ns (20.00x) | 63.45 ns (2.98x) |
| `^(?![-_]*$)[A-Za-z0-9][A-Za-z0...` | **19.95 ns (1.00x)** | 351.86 ns (17.64x) | 28.75 ns (1.44x) |

<!-- POPULAR-BENCH:END -->
