# Features

## Extended support for lookarounds compared to original RE# spec

This crate accepts some patterns that the RE# spec (and the .NET
engine) reject, because they're actually fine and useful, for example:

- lookarounds inside a union, like `(?<=a)x|y`, as long as the branches are distinguishable
- a lookbehind-carrying union combined with `&`, like `(^abc|def)&.*`.

such patterns are covered by heuristic checks in the parser

## Hardened mode

Guarantees **linear matching for all patterns and all match-collecting APIs** (`find_all`, etc.) in O(N·S), where N is input length and S is DFA states. The default engine is linear on the vast majority of patterns but can go quadratic on `find_all` when a pattern produces dense reverse-scan candidates. Hardened mode rules that out unconditionally.

`is_match` is **always linear** in the default engine too. It short-circuits on the first match and never runs the reverse pass, so hardened mode is only relevant when you are collecting matches.

```rust
let re = resharp::Regex::with_options(
    r"pattern",
    resharp::RegexOptions::default().hardened(true),
).unwrap();
```

Note: RE# auto-hardens many common patterns at compile time, so the default engine is already linear on the bulk of pathological real-world regexes. Use `hardened(true)` only when worst-case linearity is a hard requirement (e.g. running untrusted patterns).

- With hardened mode, in the worst cases you can expect performance roughly around 100MBs on consumer hardware. Having it disabled can sometimes be more than 10x faster

> hardened mode on `.*[^A-Z]|[A-Z]` with input of `"A" * N` (N=10,000):

| input size | normal | hardened | speedup w/ hardened |
|---|---|---|---|
| 1,000 | 0.7ms | 28us | 25x |
| 5,000 | 18ms | 146us | 123x |
| 10,000 | 73ms | 303us | 241x |
| 50,000 | 1.8s | 1.6ms | 1,125x |

> hardened mode on normal patterns on english prose

| pattern | normal | hardened | ratio |
|---|---|---|---|
| `[A-Z][a-z]+` | 2.2ms | 6.5ms | 3.0x slower |
| `[A-Za-z]{8,13}` | 1.7ms | 7.6ms | 4.4x slower |
| `\w{3,8}` | 2.6ms | 22ms | 8.7x slower |
| `\d+` | 1.3ms | 5.2ms | 3.9x slower |
| `[A-Z]{2,}` | 0.7ms | 4.7ms | 6.7x slower |

