# RE#

[![crates.io](https://img.shields.io/crates/v/resharp.svg)](https://crates.io/crates/resharp)
[![docs.rs](https://docs.rs/resharp/badge.svg)](https://docs.rs/resharp)

A high-performance, automata-based regex engine with first-class support for **intersection** and **complement** operations.

RE# compiles patterns into deterministic automata. All matching is non-backtracking with guaranteed linear-time execution. RE# extends standard regex syntax with intersection (`&`), complement (`~`), and a universal wildcard (`_`), enabling patterns that are impossible or impractical to express with standard regex.

[paper](https://dl.acm.org/doi/10.1145/3704837) | [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) | [syntax docs](https://github.com/ieviev/resharp/blob/main/docs/syntax.md) | [dotnet version](https://github.com/ieviev/resharp-dotnet) and [web playground](https://ieviev.github.io/resharp-webapp/)

## Install

```
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

```
_*              any string
a_*             any string that starts with 'a'
_*a             any string that ends with 'a'
_*a_*           any string that contains 'a'
~(_*a_*)        any string that does NOT contain 'a'
(_*a_*)&~(_*b_*)  contains 'a' AND does not contain 'b'
(?<=b)_*&_*(?=a)  preceded by 'b' AND followed by 'a'
```

You combine all of these with `&` to get more complex patterns. RE# also supports lookarounds (`(?=...)`, `(?<=...)`, `(?!...)`, `(?<!...)`), compiled directly into the automaton with no backtracking. 

NOTE: RE# is not compatible with some `regex` crate features, eg. lazy quantifiers (`.*?`). See the full [syntax reference](docs/syntax.md) for details.

### Differences from `resharp-dotnet` RE# and rust `regex`

Rust `resharp` is written from scratch, there are a number of differences from the original. For starters this works on byte slices (`&[u8]`) and UTF-8 rather than UTF-16. The parser uses the `regex-syntax` crate as a base with 3 extensions described above. The API is also different, there's a different internal representation for the characters and algebra.. etc

This version now uses AVX2 SIMD for literal search, prefix matching, and byte skipping in the match loop, though the optimizations aren't as advanced as the dotnet version yet.

When you should use this as opposed to the default regex in rust is:
- you want to match patterns that require intersection or complement or lookarounds
- you want to match large regexes with high performance, at the expense of memory usage
- you want leftmost longest matches rather than leftmost first matches
- you want to extract all matches rather than just the first match, RE# only supports `find_anchored` and `find_all` but not `find` or `captures`

For tuning, `EngineOptions` controls precompilation threshold, capacity, and lookahead context:

```rust
let opts = resharp::EngineOptions {
    dfa_threshold: 100,           // eagerly compile up to N states
    max_dfa_capacity: 65535,       // max automata states (default: u16::MAX)
    lookahead_context_max: 800,    // max lookahead context distance (default: 800)
};
let re = resharp::Regex::with_options(r"pattern", opts).unwrap();
```

`RE#` matching API is slightly different from `regex`, 
matches will return a `Result<Vec<Match>, Error>`, where the `Error` can be a capacity overflow or a lookahead context overflow. `RE#` will either give you fast matching or fail outright. You can catch these errors and rebuild / adjust your pattern or options accordingly.

## Benchmarks

Throughput comparison with `regex` and `fancy-regex` on an AMD Ryzen 7 5800X, compiled with `--release`. Compile time is excluded; only matching is measured. Run with `cargo bench -- 'readme/' --list`.

| Benchmark | resharp | regex | fancy-regex |
|---|---|---|---|
| dictionary 2663 words (900KB, ~15 matches) | 500 MiB/s | 552 MiB/s | 545 MiB/s |
| dictionary 2663 words (944KB, ~2678 matches) | **449 MiB/s** | 58 MiB/s | 20 MiB/s |
| dictionary `(?i)` 2663 words (900KB) | **503 MiB/s** | 0.03 MiB/s | 0.03 MiB/s |
| lookaround `(?<=\s)[A-Z][a-z]+(?=\s)` (900KB) | **386 MiB/s** | -- | 25 MiB/s |
| literal alternation (900KB) | **12.1 GiB/s** | 11.4 GiB/s | 10.2 GiB/s |
| literal `"Sherlock Holmes"` (900KB) | 33.9 GiB/s | 38.7 GiB/s | 33.7 GiB/s |

**Notes on the results:**

- The first dictionary row is roughly tied - the prose haystack only contains ~15 matches, so the lazy DFA barely explores any states. RE#'s advantage is that its full DFA is smaller, but this isn't visible when most states are never materialized.
- On longer inputs or denser matches, the other engines will degrade - take lazy-dfa benchmarks with a grain of salt, you will not be matching the exact same string over and over in the real world. The seeded dictionary row confirms this: with ~2678 matches, RE# holds at 449 MiB/s vs 58 MiB/s for `regex`.
- The `(?i)` row shows what happens when the pattern forces `regex` to fall back from its DFA to an NFA: throughput drops from 552 MiB/s to 0.03 MiB/s. RE# handles case folding in the DFA and maintains full speed. You can increase `regex`'s DFA threshold to avoid this fallback, but only up to a point.
- RE# compiles lookarounds directly into the automaton - no back-and-forth between forward and backward passes. `regex` doesn't support lookarounds except for anchors; `fancy-regex` handles them via backtracking, which is occasionally much slower.
- Literal and alternation performance relies on explicit AVX2 SIMD - no ARM NEON or other backends yet, so expect slower results on non-x86 platforms.
- If you encounter a bug or a pattern where RE# is >5x slower than `regex` or `fancy-regex`, please [open an issue](https://github.com/ieviev/resharp/issues) - that's worth looking into.

## Crate structure

| Crate | Description |
|-------|-------------|
| `resharp` | engine and public API `(resharp-engine)` |
| `resharp-algebra` | algebraic regex tree, constraint solver, nullability analysis |
| `resharp-parser` | pattern string to AST, extends `regex-syntax` with RE# operators |

And most importantly, have fun! :)
