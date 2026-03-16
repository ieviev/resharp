# RE#

[![crates.io](https://img.shields.io/crates/v/resharp.svg)](https://crates.io/crates/resharp)
[![docs.rs](https://docs.rs/resharp/badge.svg)](https://docs.rs/resharp)

A high-performance, automata-based regex engine with first-class support for **intersection** and **complement** operations. RE#'s main strength is complex patterns - large lists of alternatives, lookarounds, and boolean combinations - where traditional engines degrade or fall back to slower paths.

RE# compiles patterns into deterministic automata. All matching is non-backtracking with guaranteed linear-time execution. RE# extends standard regex syntax with intersection (`&`), complement (`~`), and a universal wildcard (`_`), enabling patterns that are impossible or impractical to express with standard regex.

[paper](https://dl.acm.org/doi/10.1145/3704837) | [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) | [syntax docs](https://github.com/ieviev/resharp/blob/main/docs/syntax.md) | [dotnet version](https://github.com/ieviev/resharp-dotnet) and [web playground](https://ieviev.github.io/resharp-webapp/)

## Install

```sh
cargo add resharp
```

## Usage

```rust
let re = resharp::Regex::new(r".*cat.*&.*dog.*&.{8,15}").unwrap();

let matches = re.find_all(b"the cat and the dog");
let found = re.is_match(b"the cat and the dog");
```

## Syntax extensions

RE# supports standard regex syntax plus three extensions: `_` (universal wildcard), `&` (intersection), and `~(...)` (complement). `_` matches any character including newlines, so `_*` means "any string".

```perl
_*              any string
a_*             any string that starts with 'a'
_*a             any string that ends with 'a'
_*a_*           any string that contains 'a'
~(_*a_*)        any string that does NOT contain 'a'
(_*a_*)&~(_*b_*)  contains 'a' AND does not contain 'b'
(?<=b)_*&_*(?=a)  preceded by 'b' AND followed by 'a'
```

You combine all of these with `&` to get more complex patterns. RE# also supports lookarounds (`(?=...)`, `(?<=...)`, `(?!...)`, `(?<!...)`), compiled directly into the automaton with no backtracking.

> RE# is not compatible with some `regex` crate features, eg. lazy quantifiers (`.*?`). See the full [syntax reference](docs/syntax.md) for details, and [features](docs/features.md) for untrusted mode and other advanced options.

### When to use RE# over [`regex`](https://crates.io/crates/regex)

This is a from-scratch rust implementation operating on `&[u8]` / UTF-8 (the [dotnet version](https://github.com/ieviev/resharp-dotnet) uses UTF-16), with `regex-syntax` as a parser base. RE# aims to match `regex` crate performance on standard patterns, with trade-offs on either side. Reasons to reach for RE#:

- intersection, complement, or lookarounds
- large alternatives with high performance (at the expense of memory)
- leftmost longest matches rather than leftmost-greedy (PCRE)
- `find_anchored` and `find_all` (no `find` or `captures`)

Matching returns `Result<Vec<Match>, Error>` - capacity or lookahead overflow will fail outright rather than silently degrade. `EngineOptions` controls precompilation threshold, capacity, and lookahead context:

```rust
let opts = resharp::EngineOptions {
    dfa_threshold: 100,           // eagerly compile up to N states
    max_dfa_capacity: 65535,       // max automata states (default: u16::MAX)
    lookahead_context_max: 800,    // max lookahead context distance (default: 800)
};
let re = resharp::Regex::with_options(r"pattern", opts).unwrap();
```

## Benchmarks

Throughput comparison with `regex` and `fancy-regex`, compiled with `--release`. Compile time is excluded; only matching is measured. Uses SIMD intrinsics (AVX2, NEON) with possibly more backends in the near future. Run with `cargo bench -- 'readme/' --list`.

### AMD Ryzen 7 5800X (105W TDP)

| Benchmark | resharp | regex | fancy-regex |
|---|---|---|---|
| dictionary 2663 words (900KB, ~15 matches) | **633 MiB/s** | 541 MiB/s | 531 MiB/s |
| dictionary 2663 words (944KB, ~2678 matches) | **535 MiB/s** | 58 MiB/s | 20 MiB/s |
| dictionary `(?i)` 2663 words (900KB) | **632 MiB/s** | 0.03 MiB/s | 0.03 MiB/s |
| lookaround `(?<=\s)[A-Z][a-z]+(?=\s)` (900KB) | **460 MiB/s** | -- | 25 MiB/s |
| literal alternatives (900KB) | **12.0 GiB/s** | 11.2 GiB/s | 10.1 GiB/s |
| literal `"Sherlock Holmes"` (900KB) | 33.2 GiB/s | 34.0 GiB/s | 30.3 GiB/s |

### Rockchip RK3588 ARM (5-10W TDP)

| Benchmark | resharp | regex | fancy-regex |
|---|---|---|---|
| dictionary 2663 words (900KB, ~15 matches) | 271 MiB/s | 315 MiB/s | 317 MiB/s |
| dictionary 2663 words (944KB, ~2678 matches) | **214 MiB/s** | 25 MiB/s | 9 MiB/s |
| dictionary `(?i)` 2663 words (900KB) | **271 MiB/s** | 0.01 MiB/s | 0.01 MiB/s |
| lookaround `(?<=\s)[A-Z][a-z]+(?=\s)` (900KB) | **198 MiB/s** | -- | 10 MiB/s |
| literal alternatives (900KB) | 1.73 GiB/s | 2.00 GiB/s | 1.95 GiB/s |
| literal `"Sherlock Holmes"` (900KB) | 6.74 GiB/s | 7.05 GiB/s | 6.78 GiB/s |

<sub>(crazy how close a board smaller than a phone gets to desktop throughput these days. what a time to be alive)</sub>

**Notes on the results:**

- The first dictionary row is roughly tied - the prose haystack only contains ~15 matches, so the lazy DFA barely explores any states. RE#'s advantage is that its full DFA is smaller, but this isn't visible when most states are never materialized.
- On longer inputs or denser matches, the other engines will degrade - take lazy-dfa benchmarks with a grain of salt, you will not be matching the exact same string over and over in the real world. The seeded dictionary row confirms this: with ~2678 matches, RE# holds at 535 MiB/s vs 58 MiB/s for `regex` on x86.
- The `(?i)` row shows what happens when the pattern forces `regex` to fall back from its DFA to an NFA: throughput drops to 0.03 MiB/s. RE# handles case folding in the DFA and maintains full speed. You can increase `regex`'s DFA threshold to avoid this fallback, but only up to a point.
- RE# compiles lookarounds directly into the automaton - no back-and-forth between forward and backward passes. `regex` doesn't support lookarounds except for anchors; `fancy-regex` handles them via backtracking, which is occasionally much slower.
- The same patterns that win on x86 also win on ARM - the full DFA approach scales down well.
- If you encounter a bug or a pattern where RE# is >5x slower than `regex` or `fancy-regex`, please [open an issue](https://github.com/ieviev/resharp/issues) - it would help improve the library. Note that `regex` returns leftmost-greedy (PCRE) matches while RE# returns leftmost-longest, so match results may differ. The performance profile also differs - RE# works right to left while `regex` works left to right.
- Also see the [rebar](https://github.com/ieviev/rebar) comparison to `regex` - despite its own bias disclaimer, one of the fairest and most thorough regex benchmarks out there. Rebar targets leftmost-first engines, so RE#'s leftmost-longest semantics do some extra work. Be wary of the throughput numbers on short inputs though - they let `regex` build a tiny purpose-built automaton for matching the exact same string repeatedly, so the reported MiB/s doesn't reflect real-world scanning speed. On longer inputs the gap shifts further in RE#'s favor.

## Crate structure

| Crate | Description |
|-------|-------------|
| `resharp` | engine and public API `(resharp-engine)` |
| `resharp-algebra` | algebraic regex tree, constraint solver, nullability analysis |
| `resharp-parser` | pattern string to AST, extends `regex-syntax` with RE# operators |

And most importantly, have fun! :)
