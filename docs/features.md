# features

## untrusted mode

untrusted mode makes `find_all` linear-time even on adversarial input. you get the same matches you'd get without it - identical positions, identical lengths, same leftmost-longest semantics. it just can't be made slow on purpose. internally it runs an O(n·S) forward scan (n = input length, S = DFA states) instead of the normal scan which can go quadratic when there are many overlapping match candidates.

this is not "earliest match" or any other altered semantics. some engines achieve linear-time `find_all` by switching to earliest-match (e.g. Hyperscan, rust `regex`), which returns different (shorter) matches than you'd normally get. untrusted mode doesn't do that - it returns exactly the same leftmost-longest matches as normal mode. there is no semantic tradeoff, only a constant-factor speed tradeoff on non-adversarial input.

this only matters for `find_all`. `is_match` and anchored matching are already linear regardless of mode.

```rust
let re = Regex::with_options(pattern, EngineOptions::default().untrusted(true))?;
```

patterns with lookaround are currently rejected in untrusted mode. this restriction may be lifted in a future version.

### benchmarks

normal text (`en-sampled.txt`, ~0.5 MiB):

| pattern | normal | untrusted | ratio |
|---|---|---|---|
| `[A-Z][a-z]+` | 2.2ms | 6.5ms | 3.0x |
| `[A-Za-z]{8,13}` | 1.7ms | 7.6ms | 4.4x |
| `\w{3,8}` | 2.6ms | 22ms | 8.7x |
| `\d+` | 1.3ms | 5.2ms | 3.9x |
| `[A-Z]{2,}` | 0.7ms | 4.7ms | 6.7x |

pathological input (`.*[^A-Z]|[A-Z]` on repeated `A`s):

| input size | normal | untrusted | ratio |
|---|---|---|---|
| 1,000 | 0.7ms | 28us | 0.04x |
| 5,000 | 18ms | 146us | 0.008x |
| 10,000 | 73ms | 303us | 0.004x |
| 50,000 | 1.8s | 1.6ms | 0.0009x |

typical patterns see 5-50x overhead on normal text. on pathological input normal mode is quadratic while untrusted stays linear. note that the quadratic blowup in normal mode requires both the pattern and the input to be adversarial - it won't happen with just one or the other. this is why the faster default is kept and untrusted mode is opt-in. if you control the pattern (hardcoded in your source), you almost certainly don't need it. it's meant for services that accept user-supplied regexes where an attacker could control both the pattern and the input.
