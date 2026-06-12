# API reference

## Regex

```rust
use resharp::Regex;

let re = Regex::new(r"pattern")?;
let re = Regex::with_options(r"pattern", opts)?;

let matches: Vec<Match> = re.find_all(input)?;       // leftmost-longest
let found: bool         = re.is_match(input)?;
let anchored: Option<Match> = re.find_anchored(input)?;  // longest match at offset 0
```

Input is `&[u8]`. Matches are byte offsets `[start, end)`.

```rust
pub struct Match { pub start: usize, pub end: usize }
```

### How the APIs agree

The matching APIs answer different questions about one language, so their
answers are mutually constrained. The invariants below are the documented
contract:

- `is_match` is true exactly when `find_all` is non-empty.
- If `find_anchored(...)` returns `Some(_)`, then `is_match` is true; the
  returned span starts at 0 and is the longest match there.
- `find_all` returns leftmost-longest, non-overlapping matches in order.
- `stream` returns leftmost-**shortest** matches (see Streaming below), so its
  spans differ from `find_all` in general; `stream` is empty exactly when
  `find_all` is empty, and when every match is zero-width the two enumerations
  coincide (shortest and longest are the same span).
- `hardened(true)` changes the scan algorithm and its complexity guarantee,
  never the result: default and hardened return identical matches.

Today the `match_invariants` fuzz target asserts the `find_all` ordering, the
`is_match` agreement, and the anchored-at-0 span. The remaining invariants are
stated intent: known cross-API findings still violate some of them, and the
fixes converge on the reading documented here.

## RegexOptions

```rust
use resharp::{RegexOptions, UnicodeMode};

let opts = RegexOptions {
    max_dfa_capacity: 65535,         // cap on DFA states
    lookahead_context_max: 800,      // max lookahead distance
    unicode: UnicodeMode::Default,   // Ascii | Default | Full | Javascript
    case_insensitive: false,         // (?i)
    dot_matches_new_line: false,     // (?s); `_` always matches any byte
    multiline: true,                 // (?m); ON BY DEFAULT
    ignore_whitespace: false,        // (?x)
    hardened: false,                 // worst-case linear, ~5-20x slower
    unbounded_size: false,           // disable parser/algebra size caps
};
```

Builder-style setters chain:

```rust
RegexOptions::default().unicode(UnicodeMode::Ascii).case_insensitive(true)
```

Inline flags (`(?i)`, `(?s)`, `(?-u)`, ...) override the global setting and can be scoped: `(?s:a.b)c.d`.

`multiline` defaults to **on**, unlike most engines. Disable with `.multiline(false)` or `(?-m)`.

For `unicode` see [syntax.md](syntax.md#unicode). For `hardened` see [features.md](features.md).

## escape

```rust
let pat = format!("{}\\d+", resharp::escape("price: $"));
```

`escape_into(text, &mut buf)` appends instead of allocating.

## Error

```rust
pub enum Error {
    Parse(Box<ParseError>),
    Algebra(ResharpError),
    CapacityExceeded,    // hit max_dfa_capacity
    PatternTooLarge,     // hit parser/algebra size cap
    Serialize(String),
}
```

## Streaming (experimental)

> **Experimental, off by default.** Gated behind the `stream` Cargo feature (`features = ["stream"]`). Zero-width and anchored patterns can report phantom matches; see `TODO.md`. Not recommended for production.

Streaming and cursor APIs return **shortest** matches (left-to-right, earliest end), not leftmost-longest. Authoritative source: [`resharp-engine/src/stream.rs`](../resharp-engine/src/stream.rs).

| method | yields |
|---|---|
| `stream` / `stream_with` | `Vec<Match>` / callback `Match` |
| `stream_ends` / `stream_ends_with` | end offsets only (faster, skips reverse pass) |
| `stream_chunk` | end offsets + updated `StreamState` for the next chunk |
| `seek_fwd` | next `(resume_state, end)` from a cursor |
| `seek_rev` | next `(resume_state, start)`, rightmost-first |

`StreamState` carries an absolute byte offset plus a DFA state id. Build with `StreamState::new()`, `::at(pos)`, or `::from_raw(state, pos)` (raw ids are only valid for the producing `Regex`).

## Large files

Memory-map the file and stream it. Memory use stays bounded.

```rust
use memmap2::Mmap;
use resharp::Regex;
use std::fs::File;

let file = File::open("big.log")?;
let mmap = unsafe { Mmap::map(&file)? };
let input: &[u8] = &mmap;

let re = Regex::new(r"\d+")?;
re.stream_with(input, |m| println!("[{}..{})", m.start, m.end))?;
```

`\d+` on `a12b3` yields `[1,2)`, `[2,3)`, `[4,5)` (shortest matches), not the single `[1,3)` you'd get from `find_all`.

### Capturing part of a match

Put the context in lookarounds; the reported span only covers what's between them.

```rust
let re = Regex::new(r#"(?-u)(?<=<row Id=")\d+(?=")"#)?;
```

On `  <row Id="42" Foo="bar"/>  <row Id="99" />` this yields `[11,13)` and `[37,39)`.

### Extending a shortest match

Write the boundary into the pattern: `error.*$` (to end of line), `error.*\n` (include the newline), `error.*?(?=\sat\s)` (lookahead).

### Seeking from an offset

```rust
let re = Regex::new(r"\bERROR\b")?;

if let Some((_, end)) = re.seek_fwd(input, Regex::SEEK_INITIAL, 1_000_000)? {
    println!("next ERROR ends at {end}");
}
if let Some((_, start)) = re.seek_rev(input, Regex::SEEK_INITIAL, 1_000_000)? {
    println!("prev ERROR starts at {start}");
}
```

Pass the returned `resume_state` and offset back in to keep walking. Full mmap example: `resharp-engine/examples/test_seek.rs`.

### Chunked input

If you can't mmap (sockets, decompressed streams), feed bytes to `stream_chunk` and thread the returned `StreamState` between calls.
