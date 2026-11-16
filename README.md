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
use resharp::{HardeningMode, RegexOptions, UnicodeMode};

let defaults = RegexOptions {
    max_dfa_capacity: u16::MAX as usize, // max cached DFA states
    max_node_capacity: 1_000_000,        // max AST nodes
    lookahead_context_max: 800,          // max lookahead context distance
    unicode: UnicodeMode::Default,       // Ascii | Default | Full | Javascript
    case_insensitive: false,
    dot_matches_new_line: false,         // `.` matches `\n`
    multiline: true,                     // `^`/`$` match start/end of line
    ignore_whitespace: false,            // verbose mode
    hardening: HardeningMode::Auto,      // Off | Auto | Always (Always: forces O(n), slower on average)
    unbounded_size: false,               // lift size caps
    ..Default::default()
};
```

Override with the builder methods:

```rust
let opts = RegexOptions::default().unicode(UnicodeMode::Ascii).hardening(HardeningMode::Always);
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
| `\s+` | **374.28 MiB/s (1.00x)** | 148.93 MiB/s (2.51x) | 133.58 MiB/s (2.80x) | 160.68 MiB/s (2.33x) |
| `\d+` | **1.68 GiB/s (1.00x)** | 427.55 MiB/s (4.03x) | 414.8 MiB/s (4.16x) | 366.25 MiB/s (4.71x) |
| `.*` | **2.3 GiB/s (1.00x)** | 180.81 MiB/s (13.02x) | 178.09 MiB/s (13.22x) | 798.55 MiB/s (2.95x) |
| `<[^>]+>` | 2.15 GiB/s (1.55x) | 548.5 MiB/s (6.21x) | 534.96 MiB/s (6.37x) | **3.33 GiB/s (1.00x)** |
| `\n{3,}` | **21.25 GiB/s (1.00x)** | 14.47 GiB/s (1.47x) | 13.12 GiB/s (1.62x) | 6.73 GiB/s (3.16x) |
| `\x1b\[[0-9;]*m` | **15.12 GiB/s (1.00x)** | 7.97 GiB/s (1.90x) | 7.03 GiB/s (2.15x) | 11.13 GiB/s (1.36x) |
| `\$\{([^}]+)\}` | 5.54 GiB/s (1.96x) | 883.97 MiB/s (12.58x) | 874.29 MiB/s (12.72x) | **10.86 GiB/s (1.00x)** |
| `<[^>]*>` | 2.87 GiB/s (1.05x) | 537.94 MiB/s (5.71x) | 520.1 MiB/s (5.91x) | **3 GiB/s (1.00x)** |
| `\w+` | **277.68 MiB/s (1.00x)** | 127.74 MiB/s (2.17x) | 120.16 MiB/s (2.31x) | 204.49 MiB/s (1.36x) |
| `-+` | **7.16 GiB/s (1.00x)** | 2.61 GiB/s (2.74x) | 2.32 GiB/s (3.08x) | 4.68 GiB/s (1.53x) |
| `test` | 12.92 GiB/s (1.40x) | **18.04 GiB/s (1.00x)** | 14.36 GiB/s (1.26x) | 6.14 GiB/s (2.94x) |
| `\d+\.\d+\.\d+` | 2.21 GiB/s (1.40x) | **3.09 GiB/s (1.00x)** | 2.96 GiB/s (1.04x) | 399 MiB/s (7.94x) |
| `[0-9]+` | **1.77 GiB/s (1.00x)** | 550.37 MiB/s (3.29x) | 473.5 MiB/s (3.83x) | 1.03 GiB/s (1.72x) |
| ``([^`]+)`` | 13.04 GiB/s (1.08x) | 8.42 GiB/s (1.67x) | 7.93 GiB/s (1.78x) | **14.11 GiB/s (1.00x)** |
| `"([^"]+)"` | **5.51 GiB/s (1.00x)** | 3.02 GiB/s (1.82x) | 2.77 GiB/s (1.99x) | 5.47 GiB/s (1.01x) |
| `\[([^\]]+)\]\(([^)]+)\)` | 5.04 GiB/s (1.16x) | 592.13 MiB/s (10.10x) | 589.01 MiB/s (10.15x) | **5.84 GiB/s (1.00x)** |
| `\s{2,}` | 613.6 MiB/s (1.03x) | **634.43 MiB/s (1.00x)** | 620.66 MiB/s (1.02x) | 327.83 MiB/s (1.94x) |
| `[A-Z]` | **2.03 GiB/s (1.00x)** | 412.46 MiB/s (5.05x) | 383.75 MiB/s (5.42x) | 1.15 GiB/s (1.77x) |
| `(?is)<script[^>]*>.*?</script>` \* | 815.25 MiB/s (9.10x) | 664.07 MiB/s (11.17x) | 664.97 MiB/s (11.16x) | **7.24 GiB/s (1.00x)** |

### Validate (is_match on a single value), latency

| Pattern | resharp | regex | fancy-regex | pcre2 |
|---|---|---|---|---|
| `^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9....` | 41.83 ns (1.16x) | 42.53 ns (1.18x) | 39.05 ns (1.08x) | **36.13 ns (1.00x)** |
| `^\d{4}-\d{2}-\d{2}$` | 21.52 ns (1.05x) | 24.18 ns (1.18x) | **20.52 ns (1.00x)** | 27.63 ns (1.35x) |
| `^$` | **2.8 ns (1.00x)** | 20.31 ns (7.24x) | 14.59 ns (5.20x) | 19.05 ns (6.80x) |
| `^\d+$` | 21.88 ns (1.09x) | 23.06 ns (1.15x) | **20.13 ns (1.00x)** | 32.23 ns (1.60x) |
| `^[a-zA-Z_][a-zA-Z0-9_]*$` | 33.71 ns (1.21x) | 36.48 ns (1.31x) | 29.25 ns (1.05x) | **27.79 ns (1.00x)** |
| `^[a-zA-Z0-9_-]+$` | 33.54 ns (1.14x) | 32.54 ns (1.10x) | 29.57 ns (1.00x) | **29.49 ns (1.00x)** |

### Lookaround scan

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `(?<!_)deleted_at(?!_)` \* | **7.72 GiB/s (1.00x)** | 38.91 MiB/s (203.23x) | 6.34 GiB/s (1.22x) |
| `(?<=\d)\.(?=\S)` | 615.74 MiB/s (5.44x) | 32.43 MiB/s (103.38x) | **3.27 GiB/s (1.00x)** |
| `(?<="\|')\s+(?=[^<>\s]+=)` | **2.65 GiB/s (1.00x)** | 40.21 MiB/s (67.51x) | 417.11 MiB/s (6.51x) |

### Lookaround validate

| Pattern | resharp | fancy-regex | pcre2 |
|---|---|---|---|
| `^(?=.*[a-z])(?=.*[A-Z])(?=.*\d...` | **20.41 ns (1.00x)** | 426.53 ns (20.90x) | 65.17 ns (3.19x) |
| `^(?![-_]*$)[A-Za-z0-9][A-Za-z0...` \* | **19.46 ns (1.00x)** | 337.78 ns (17.36x) | 28.71 ns (1.48x) |

<!-- POPULAR-BENCH:END -->
