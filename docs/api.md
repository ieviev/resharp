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

### RegexOptions

```rust
use resharp::{RegexOptions, UnicodeMode};

let opts = RegexOptions {
    dfa_threshold: 0,                 // states to eagerly precompile (0 = fully lazy)
    max_dfa_capacity: 65535,          // max DFA states (clamped to u16::MAX)
    lookahead_context_max: 800,       // max lookahead distance before AnchorLimit error
    hardened: false,                  // O(N·S) forward scan, slower but worst-case safe
    unicode: UnicodeMode::Default,    // \w/\d coverage (Ascii | Default | Full | Javascript)
    case_insensitive: false,          // global case-insensitive matching
    dot_matches_new_line: false,      // . matches \n (behaves like _)
    ignore_whitespace: false,         // allow whitespace and # comments in pattern
};
```

All fields have sensible defaults via `Default::default()`. Builder-style setters are available for chaining:

```rust
let re = Regex::with_options(
    r"\w+@\w+\.\w+",
    RegexOptions::default().unicode(UnicodeMode::Ascii).case_insensitive(true),
)?;
```

#### engine tuning

- `dfa_threshold`: set >0 to precompile hot states at build time, trading compile cost for faster first match.
- `max_dfa_capacity`: upper bound on cached DFA states. patterns with large state spaces return `Error::CapacityExceeded` instead of allocating unbounded memory.
- `lookahead_context_max`: limits how far ahead the engine tracks lookaround context. increase if patterns with deep lookahead return `ResharpError::AnchorLimit`.
- `hardened`: use O(n·S) hardened forward scan, preventing quadratic blowup even when both pattern and input are adversarial.

#### pattern flags

- `unicode`: `UnicodeMode` enum controlling `\w`/`\d`/`\s` coverage. `Ascii` is equivalent to inline `(?-u)` (`\w` = `[a-zA-Z0-9_]`, `\d` = `[0-9]`, `\s` = `[\t\n\v\f\r ]`); `Default` covers up to 2-byte UTF-8; `Full` covers all Unicode; `Javascript` matches default JS `RegExp` semantics. See [syntax.md](syntax.md#unicode).
- `case_insensitive`: equivalent to inline `(?i)`.
- `dot_matches_new_line`: makes `.` match `\n`. equivalent to inline `(?s)`. note that `_` always matches any byte regardless of this flag.
- `ignore_whitespace`: equivalent to inline `(?x)`.

inline flags (`(?i)`, `(?s)`, `(?-u)`, etc.) override the global setting and can be scoped with groups: `(?s:a.b)c.d` - dot inside the group matches newline, dot outside does not.

### escape

```rust
let pattern = format!("{}\\d+", resharp::escape("price: $"));
let re = Regex::new(&pattern)?;
```

`resharp::escape(text)` backslash-escapes all resharp meta characters in `text`, returning a pattern that matches the literal string. `resharp::escape_into(text, &mut buf)` appends to an existing `String`.

### Error

```rust
pub enum Error {
    Parse(Box<ParseError>),    // invalid pattern syntax
    Algebra(ResharpError),     // unsupported pattern or state explosion
    CapacityExceeded,          // DFA exceeded max_dfa_capacity
    PatternTooLarge,           // pattern produced too many algebra nodes
    Serialize(String),         // serialization/deserialization failure
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

### ResharpError

```rust
pub enum ResharpError {
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
