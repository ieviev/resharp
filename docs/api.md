# API reference

## resharp (engine)

### Regex

```rust
use resharp::Regex;

// compile
let re = Regex::new(r"pattern")?;
let re = Regex::with_options(r"pattern", opts)?;

// match
let matches: Vec<Match> = re.find_all(input)?;
let found: bool = re.is_match(input)?;
```

Input is `&[u8]`. Matches are byte-offset ranges `[start, end)`.

```rust
pub struct Match {
    pub start: usize,
    pub end: usize,
}
```

### EngineOptions

```rust
let opts = resharp::EngineOptions {
    dfa_threshold: 0,           // states to eagerly precompile (0 = fully lazy)
    max_dfa_capacity: 65535,    // max DFA states (clamped to u16::MAX)
    lookahead_context_max: 800, // max lookahead distance before AnchorLimit error
};
```

All fields have sensible defaults via `Default::default()`.

- `dfa_threshold`: set >0 to precompile hot states at build time, trading compile cost for faster first match.
- `max_dfa_capacity`: upper bound on cached DFA states. patterns with large state spaces return `Error::CapacityExceeded` instead of allocating unbounded memory.
- `lookahead_context_max`: limits how far ahead the engine tracks lookaround context. increase if patterns with deep lookahead return `AlgebraError::AnchorLimit`.

### Error

```rust
pub enum Error {
    Parse(ResharpError),       // invalid pattern syntax
    Algebra(AlgebraError),     // unsupported pattern or state explosion
    CapacityExceeded,          // DFA exceeded max_dfa_capacity
}
```

`Error` implements `std::error::Error` and `Display`.

### DFA inspection

```rust
let (fwd_states, rev_states) = re.dfa_stats();
```

Returns the number of materialized states in the forward and reverse DFAs. Useful for profiling memory usage.

## resharp-algebra

The algebra crate is used internally by the engine. It is public for advanced use cases like building regex ASTs programmatically.

### RegexBuilder

Constructs regex AST nodes:

```rust
let mut b = resharp_algebra::RegexBuilder::new();
let cat = b.mk_string("cat");
let dog = b.mk_string("dog");
let ts = resharp_algebra::NodeId::TS;  // top-star = _*
let has_cat = b.mk_concat(b.mk_concat(ts, cat), ts);
let has_dog = b.mk_concat(b.mk_concat(ts, dog), ts);
let both = b.mk_inters([has_cat, has_dog].into_iter());
```

Key construction methods:

| method | description |
|--------|-------------|
| `mk_string(s)` | literal string |
| `mk_u8(byte)` | single byte |
| `mk_concat(a, b)` | concatenation |
| `mk_union(a, b)` | alternation |
| `mk_inters(iter)` | intersection |
| `mk_star(a)` | kleene star |
| `mk_plus(a)` | one or more |
| `mk_opt(a)` | optional |
| `mk_repeat(a, lo, hi)` | bounded repeat |
| `mk_compl(a)` | complement |
| `mk_lookahead(body, tail, rel)` | positive lookahead |
| `mk_lookbehind(body, prev)` | positive lookbehind |

### NodeId constants

| constant | meaning |
|----------|---------|
| `NodeId::BOT` | empty language (matches nothing) |
| `NodeId::EPS` | epsilon (empty string) |
| `NodeId::TOP` | any single byte (`_`) |
| `NodeId::TS` | any string (`_*`) |
| `NodeId::BEGIN` | start anchor `^` |
| `NodeId::END` | end anchor `$` |

### AlgebraError

```rust
pub enum AlgebraError {
    AnchorLimit,          // lookahead context distance exceeded
    StateSpaceExplosion,  // infinite derivative expansion
    UnsupportedPattern,   // pattern cannot be compiled to DFA
}
```

## resharp-parser

Parses a pattern string into an algebra node:

```rust
let mut b = resharp_algebra::RegexBuilder::new();
let node = resharp_parser::parse_ast(&mut b, r"(?i)hello")?;
let re = resharp::Regex::from_node(b, node, Default::default())?;
```
