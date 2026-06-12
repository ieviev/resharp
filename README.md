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

Throughput comparison with `regex` and `fancy-regex`, compiled with `--release`. Compile time is excluded; only matching is measured. Uses SIMD intrinsics (AVX2, NEON, WASM). Run with `cargo bench -- 'readme/' --list`.

### AMD Ryzen 7 5800X (105W TDP)

| Benchmark | resharp | regex | fancy-regex |
|---|---|---|---|
| dictionary 2663 words (900KB, ~15 matches) | **633 MiB/s** | 541 MiB/s | 531 MiB/s |
| dictionary 2663 words (944KB, ~2678 matches) | **535 MiB/s** | 58 MiB/s | 20 MiB/s |
| dictionary `(?i)` 2663 words (900KB) | **632 MiB/s** | 0.03 MiB/s | 0.03 MiB/s |
| lookaround `(?<=\s)[A-Z][a-z]+(?=\s)` (900KB) | **460 MiB/s** | -- | 25 MiB/s |
| `Sherlock\|Holmes\|Watson\|...` (900KB) | **12.0 GiB/s** | 11.2 GiB/s | 10.1 GiB/s |
| literal `"Sherlock Holmes"` (900KB) | 33.2 GiB/s | 34.0 GiB/s | 30.3 GiB/s |

<details>
<summary>Rockchip RK3588 ARM (5-10W TDP), also runs well on low-power chips</summary>


| Benchmark | resharp | regex | fancy-regex |
|---|---|---|---|
| dictionary 2663 words (900KB, ~15 matches) | 271 MiB/s | 315 MiB/s | 317 MiB/s |
| dictionary 2663 words (944KB, ~2678 matches) | **214 MiB/s** | 25 MiB/s | 9 MiB/s |
| dictionary `(?i)` 2663 words (900KB) | **271 MiB/s** | 0.01 MiB/s | 0.01 MiB/s |
| lookaround `(?<=\s)[A-Z][a-z]+(?=\s)` (900KB) | **198 MiB/s** | -- | 10 MiB/s |
| `Sherlock\|Holmes\|Watson\|...` (900KB) | 1.73 GiB/s | 2.00 GiB/s | 1.95 GiB/s |
| literal `"Sherlock Holmes"` (900KB) | 6.74 GiB/s | 7.05 GiB/s | 6.78 GiB/s |

</details>

**Notes:**

- **Sparse matches (~15 in 900KB)**: roughly tied. Everyone spends most of their time scanning past non-matching bytes in a tiny purpose-built automaton, so the DFA size barely matters here.
- **Dense matches (~2678 in 944KB)**: the other engines degrade sharply because they must construct more of the lazy state machine. RE# stays at 535 MiB/s vs 58 MiB/s for `regex` on x86.
- **`(?i)` case-insensitive**: `regex` falls back to a slower engine and drops to 0.01 MiB/s. RE# folds case into the DFA and stays at full speed.
- **Lookarounds**: RE# compiles them directly into the automaton. `regex` doesn't support them (except anchors); `fancy-regex` backtracks, which can be orders of magnitude slower.
- See also the [rebar](https://github.com/ieviev/rebar) comparison: not apples-to-apples (different match semantics, short repeated inputs), but a useful ballpark.
- Found a pattern where RE# is >5x slower than `regex` or `fancy-regex`? Please [open an issue](https://github.com/ieviev/resharp/issues).
