# API reference

## Regex

```rust
use resharp::Regex;

let re = Regex::new(r"pattern")?;
let re = Regex::with_options(r"pattern", opts)?;

let matches: Vec<Match>     = re.find_all(input)?;       // leftmost-longest
let found: bool             = re.is_match(input)?;
let anchored: Option<Match> = re.find_anchored(input)?;  // longest match at offset 0
let caps: Vec<Captures>     = re.captures_all(input)?;   // find_all, with groups
```

Input is `&[u8]`, matches are byte offsets `[start, end)`.

```rust
pub struct Match { pub start: usize, pub end: usize }
```

## RegexOptions

```rust
use resharp::{RegexOptions, UnicodeMode};

let opts = RegexOptions {
    max_dfa_capacity: 65535,         // cap on DFA states
    lookahead_context_max: 800,      // max lookahead distance
    unicode: UnicodeMode::Default,   // Ascii | Default | Full | Javascript
    case_insensitive: false,         // (?i)
    dot_matches_new_line: false,     // (?s); `.` matches `\n`
    multiline: true,                 // (?m); on by default, disable with (?-m)
    ignore_whitespace: false,        // (?x)
    implicit_captures: false,        // make every bare (...) capture
    hardened: false,                 // true: linear find_all, slower
    unbounded_size: false,           // disable parser/algebra size caps
    ..Default::default()
};
```

Setters chain: `RegexOptions::default().unicode(UnicodeMode::Ascii).case_insensitive(true)`.

Inline flags (`(?i)`, `(?s)`, `(?-u)`, ...) override the global setting and can be scoped: `(?s:a.b)c.d`.

`unicode`: [syntax.md](syntax.md#unicode). `hardened`: [features.md](features.md#hardened-mode).

## Capture groups (experimental)

**Experimental, not recommended for production, feature-gated:**
`resharp = { features = ["experimental_capture_groups"] }`.
For one group, a lookaround is faster and stable: the match is the group.

`(?<name>...)`/`(??...)` capture; `(?:...)`/bare `(...)` don't unless
`implicit_captures(true)`. Slot 0 is the whole match, then groups in source order.

```rust
let re = Regex::new(r"(?<user>[a-z]+)@(?<host>[a-z.]+)")?;
let caps = re.captures_all(b"joe@ex.com")?;
assert_eq!(caps[0].spans(), &[Some((0, 10)), Some((0, 3)), Some((4, 10))]);
assert_eq!(caps[0].name("host"), caps[0].get(2));
assert_eq!(re.capture_index_for_name("host"), Some(2));
```

| accessor | gives |
|---|---|
| `get(i)`, `name(n)` | `Option<Match>`, `None` if the group didn't participate |
| `spans()` | `&[Option<(usize, usize)>]`, slot 0 = whole match |
| `capture_names()` | names by slot, `None` for slot 0 and unnamed groups |

`captures_all` only, one `Captures` per `find_all` match, no single-match form.

Looping a capture is rejected (`(?<hex>[a-z0-9]{3})+` -> `Err`): only the last
iteration's span would be kept, a foot-gun. Optional captures are fine.
Captures inside a lookaround are also rejected: `(?=(?<c>a))b` -> `Err`.

`|` is unordered union: a group participates if it can, in any accepting run.
Not PCRE (backtracking, arm order) or POSIX/glibc (arm-order tie-break by
subexpression number) semantics: `a|(?<g0>.)` on `"a"` fills `g0`, and
swapping to `(?<g0>.)|a` doesn't change the result.

## escape

```rust
let pat = format!("{}\\d+", resharp::escape("price: $"));
```

`escape_into(text, &mut buf)` appends instead of allocating.

## Error

```rust
#[non_exhaustive]
pub enum Error {
    Parse(Box<ParseError>),
    Algebra(ResharpError),
    CapacityExceeded,        // hit max_dfa_capacity
    PatternTooLarge,         // hit parser/algebra size cap
    Serialize(String),
    InternalError(&'static str),
}
```
