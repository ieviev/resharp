# Features

## Hardened mode

Guarantees **linear matching for all patterns** - O(N·S) where N is input length and S is DFA states. The default engine can go quadratic when a pattern produces dense reverse-scan candidates.

```rust
let re = resharp::Regex::with_options(
    r"pattern",
    resharp::EngineOptions::default().hardened(true),
).unwrap();
```

- ~3-20x slower on average - still fast, just not as fast as RE# in the default mode
- lookarounds not supported - returns `UnsupportedPattern`

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

