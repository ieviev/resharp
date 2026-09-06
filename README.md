# RE#

[![crates.io](https://img.shields.io/crates/v/resharp.svg)](https://crates.io/crates/resharp)
[![docs.rs](https://docs.rs/resharp/badge.svg)](https://docs.rs/resharp)

A high-performance, derivative-based regex engine with first-class support for **intersection** (`&`) and **complement** (`~`). Non-backtracking. Built for complex patterns (large alternations, lookarounds, boolean combinations) that make other engines degrade or fall back to slower paths.

[paper](https://dl.acm.org/doi/10.1145/3704837) | [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) | [syntax docs](https://github.com/ieviev/resharp/blob/main/docs/syntax.md) | [dotnet version](https://github.com/ieviev/resharp-dotnet) and [web playground](https://ieviev.github.io/resharp-webapp/)

## Quick start

```sh
cargo add resharp
```

```rust
//                             8+ alphanumeric & digit & uppercase
let re = resharp::Regex::new(r"[A-Za-z0-9]{8,}&_*[0-9]_*&_*[A-Z]_*").unwrap();

let found = re.is_match(b"Hunter2024").unwrap();
let matches = re.find_all(b"try Hunter2024 or password1").unwrap();
```

## When to use RE#

On standard patterns RE# matches [`regex`](https://crates.io/crates/regex) throughput. Key features:

- intersection (`&`), complement (`~`) and lookarounds
- large alternations with high throughput (at the cost of memory)
- linear time for **all matches**, see [hardened mode](docs/features.md#hardened-mode)
- fail-loud behavior: capacity / lookahead overflow returns `Err` instead of silently degrading

RE# supports `is_match` and `find_all`. No single-match `find`/`captures`, apart from a few special cases like `find_anchored`. See [docs/api.md](docs/api.md) and [docs/features.md](docs/features.md).
(Capture groups exist behind an experimental feature flag, not recommended for production; see [docs/api.md](docs/api.md).)

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

You combine all of these with `&` to get more complex patterns. RE# also supports lookarounds (`(?=...)`, `(?<=...)`, `(?!...)`, `(?<!...)`), compiled into the automaton with no backtracking.

## Differences from PCRE-style engines

- **Leftmost-longest, not leftmost-first.** `y|yes` on `"yes"` matches `yes`. Branch order is irrelevant.
- **Multiline on by default.** `^`/`$` match start/end of line, disable with `multiline(false)`. `\A`/`\z` match start/end of input.

Lazy quantifiers (`*?`, `+?`, ...) are parse errors: rewrite with complement when possible: `<div>.*?</div>` -> `<div>(.*&~(_*</div>_*))</div>`. [other unsupported features](docs/syntax.md#unsupported-features). Full syntax: [syntax.md](docs/syntax.md).

## Configuration

All options and their defaults:

```rust
use resharp::{RegexOptions, UnicodeMode};

let defaults = RegexOptions {
    max_dfa_capacity: u16::MAX as usize, // max cached DFA states
    lookahead_context_max: 800,          // max lookahead context distance
    unicode: UnicodeMode::Default,       // Ascii | Default | Full | Javascript
    case_insensitive: false,
    dot_matches_new_line: false,         // `.` matches `\n`
    multiline: true,                     // `^`/`$` match start/end of line
    ignore_whitespace: false,            // verbose mode
    hardened: false,                     // true: linear find_all, slower
    unbounded_size: false,               // lift size caps
    ..Default::default()
};
```

Override with the builder methods:

```rust
let opts = RegexOptions::default().unicode(UnicodeMode::Ascii).hardened(true);
let re = resharp::Regex::with_options(r"pattern", opts).unwrap();
```

## Benchmarks

RE# against `regex`, `fancy-regex`, and PCRE2 on a few popular patterns from crates.io. Regenerate with:

```sh
node scripts/bench-popular-table.mts
```

<!-- POPULAR-BENCH:BEGIN -->
Full benchmark source: [resharp-engine/examples/popular-crates.rs](resharp-engine/examples/popular-crates.rs).

CPU: AMD Ryzen 7 5800X 8-Core Processor

### Scan (find_all over a 1 MiB haystack), throughput

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `\s+` | **295.24 MiB/s (1.00x)** | 146.08 MiB/s (2.02x) | 135.37 MiB/s (2.18x) | 153.13 MiB/s (1.93x) |
| `\d+` | **1.76 GiB/s (1.00x)** | 434.06 MiB/s (4.15x) | 423.04 MiB/s (4.26x) | 358.25 MiB/s (5.03x) |
| `.*` | **2.27 GiB/s (1.00x)** | 186.8 MiB/s (12.47x) | 186.98 MiB/s (12.45x) | 768.24 MiB/s (3.03x) |
| `<[^>]+>` | 2.09 GiB/s (1.25x) | 530.64 MiB/s (5.03x) | 513.61 MiB/s (5.20x) | **2.61 GiB/s (1.00x)** |
| `\n{3,}` | **19.68 GiB/s (1.00x)** | 14.36 GiB/s (1.37x) | 13.57 GiB/s (1.45x) | 6.61 GiB/s (2.98x) |
| `\x1b\[[0-9;]*m` | **11.78 GiB/s (1.00x)** | 7.98 GiB/s (1.48x) | 7.58 GiB/s (1.55x) | 11.07 GiB/s (1.06x) |
| `\$\{([^}]+)\}` | 5.63 GiB/s (1.89x) | 885.98 MiB/s (12.32x) | 884.81 MiB/s (12.33x) | **10.66 GiB/s (1.00x)** |
| `<[^>]*>` | 2.82 GiB/s (1.07x) | 531.41 MiB/s (5.80x) | 527.65 MiB/s (5.85x) | **3.01 GiB/s (1.00x)** |
| `\w+` | **270.3 MiB/s (1.00x)** | 129.23 MiB/s (2.09x) | 114.57 MiB/s (2.36x) | 203.64 MiB/s (1.33x) |
| `-+` | **6.8 GiB/s (1.00x)** | 2.34 GiB/s (2.91x) | 2.24 GiB/s (3.03x) | 4.49 GiB/s (1.52x) |
| `test` | 12.06 GiB/s (1.48x) | **17.89 GiB/s (1.00x)** | 15.1 GiB/s (1.19x) | 6.07 GiB/s (2.95x) |
| `\d+\.\d+\.\d+` | **4.86 GiB/s (1.00x)** | 2.88 GiB/s (1.69x) | 2.82 GiB/s (1.73x) | 395.43 MiB/s (12.59x) |
| `[0-9]+` | **1.92 GiB/s (1.00x)** | 548.14 MiB/s (3.58x) | 503.93 MiB/s (3.90x) | 997.63 MiB/s (1.97x) |
| ``([^`]+)`` | 12.99 GiB/s (1.09x) | 8.28 GiB/s (1.71x) | 8.25 GiB/s (1.72x) | **14.17 GiB/s (1.00x)** |
| `"([^"]+)"` | **5.44 GiB/s (1.00x)** | 2.93 GiB/s (1.85x) | 2.78 GiB/s (1.96x) | 5.43 GiB/s (1.00x) |
| `\[([^\]]+)\]\(([^)]+)\)` | 4.99 GiB/s (1.13x) | 578.99 MiB/s (9.98x) | 576.55 MiB/s (10.03x) | **5.65 GiB/s (1.00x)** |
| `\s{2,}` | 609.32 MiB/s (1.04x) | **631.77 MiB/s (1.00x)** | 627.11 MiB/s (1.01x) | 324.2 MiB/s (1.95x) |
| `[A-Z]` | **2.18 GiB/s (1.00x)** | 405.69 MiB/s (5.51x) | 391.59 MiB/s (5.71x) | 1.12 GiB/s (1.96x) |
| `(?is)<script[^>]*>.*?</script>` \* | 807.97 MiB/s (8.97x) | 651.21 MiB/s (11.13x) | 637.2 MiB/s (11.38x) | **7.08 GiB/s (1.00x)** |

### Validate (is_match on a single value), latency

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9....` | 43.45 ns (1.20x) | 43.08 ns (1.19x) | 39.3 ns (1.09x) | **36.22 ns (1.00x)** |
| `^\d{4}-\d{2}-\d{2}$` | 22.88 ns (1.05x) | 22.57 ns (1.04x) | **21.75 ns (1.00x)** | 27.97 ns (1.29x) |
| `^$` | **2.76 ns (1.00x)** | 18.05 ns (6.55x) | 14.29 ns (5.18x) | 18.56 ns (6.73x) |
| `^\d+$` | 24.04 ns (1.07x) | 23.77 ns (1.06x) | **22.46 ns (1.00x)** | 30.05 ns (1.34x) |
| `^[a-zA-Z_][a-zA-Z0-9_]*$` | 35.05 ns (1.10x) | 34.92 ns (1.10x) | **31.81 ns (1.00x)** | 32.37 ns (1.02x) |
| `^[a-zA-Z0-9_-]+$` | 35.67 ns (1.20x) | 34.64 ns (1.17x) | 31.28 ns (1.05x) | **29.66 ns (1.00x)** |

### Lookaround scan

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `(?<!_)deleted_at(?!_)` \* | **8.32 GiB/s (1.00x)** | 37.6 MiB/s (226.64x) | 6.24 GiB/s (1.33x) |
| `(?<=\d)\.(?=\S)` | 2.61 GiB/s (1.19x) | 32.87 MiB/s (97.06x) | **3.12 GiB/s (1.00x)** |
| `(?<="\|')\s+(?=[^<>\s]+=)` | **2.55 GiB/s (1.00x)** | 39.62 MiB/s (65.84x) | 406.05 MiB/s (6.42x) |

### Lookaround validate

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `^(?=.*[a-z])(?=.*[A-Z])(?=.*\d...` | **21.28 ns (1.00x)** | 414.81 ns (19.50x) | 65.23 ns (3.07x) |
| `^(?![-_]*$)[A-Za-z0-9][A-Za-z0...` \* | **19.94 ns (1.00x)** | 352.32 ns (17.67x) | 31.32 ns (1.57x) |

<!-- POPULAR-BENCH:END -->
