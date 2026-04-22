# RE# syntax

RE# supports standard regex syntax plus three extensions: intersection (`&`), complement (`~`), and a universal wildcard (`_`).

## Intuition

```
_*              any string
a_*             any string that starts with 'a'
_*a             any string that ends with 'a'
_*a_*           any string that contains 'a'
~(_*a_*)        any string that does NOT contain 'a'
(_*a_*)&~(_*b_*)  contains 'a' AND does not contain 'b'
(?<=b)_*&_*(?=a)  preceded by 'b' AND followed by 'a'
```

You combine all of these with `&` to get more complex patterns.

## Unsupported features

- Group captures: `(...)` is always non-capturing. For extracting sub-matches, use lookarounds or a separate engine post-match.
- Lazy quantifiers: `*?`, `+?`, `??`, `{n,m}?` produce a parse error.
- Backreferences: `\1`, `\2`, etc.
- Nested lookarounds: `(?=(?<=a)b)` or `(?<=(?=a)b)c`
- Lookbehinds in different alternatives: `(?<=abc)de|(?<=def)gh`

## Extensions

### `_`: universal wildcard

Matches any single byte including newlines. `_*` means "any string".

Standard `.` does **not** match `\n`. Use `_` when you need to cross line boundaries.

```
_       matches any single byte
_*      matches any byte string (including empty)
_{5,10} matches any byte string of 5-10 bytes
_*cat_* any string containing "cat"
```

Prefer `_*` over `.*` with complement. `~(.*xyz.*)` means "does not contain xyz on the same line", while `~(_*xyz_*)` means "does not contain xyz" unconditionally.

### `&`: intersection

Both sides must match. The result is the intersection of two regular languages.

```
_*cat_*&_*dog_*           contains both "cat" and "dog"
_*cat_*&_*dog_*&_{5,30}   ...and is 5-30 characters long
```

Intersection has higher precedence than alternatives: `a|b&c` is parsed as `a|(b&c)`.

### `~(...)`: complement

Matches everything the inner pattern does **not** match. Parentheses are required.

```
~(_*\d\d_*)     no consecutive digits
~(_*\n\n_*)     no double newlines
~(_*xyz_*)      does not contain "xyz"
```

### Combining operators

```
F.*&~(_*Finn)                       starts with F, doesn't end with "Finn"
~(_*\d\d_*)&[a-zA-Z\d]{8,}         8+ alphanumeric, no consecutive digits
~(_*\n\n_*)&_*keyword_*&\S_*\S     paragraph containing "keyword"
```

### Complement and UTF-8

RE# operates on raw bytes. Complement inverts at the byte level, so `~(pattern)` can match arbitrary byte sequences, including invalid UTF-8. Intersect with `\p{utf8}` to stay in valid UTF-8 space:

```
~(_*abc_*)&\p{utf8}                 does not contain "abc", valid UTF-8 only
~(_*\d\d_*)&\p{utf8}               no consecutive digits, valid UTF-8 only
```

Without `&\p{utf8}`, a complement pattern will match any byte string that doesn't match the inner pattern, including byte sequences that aren't valid UTF-8. This matters when your input is guaranteed UTF-8 and you want the engine to respect that.

`\p{utf8}` matches `(ascii | [C0-DF][80-BF] | [E0-EF][80-BF]{2} | [F0-F7][80-BF]{3})*`, the set of all valid UTF-8 byte strings. There's no special UTF-8 mode; the constraint falls out of intersection over byte-level automata. See the [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) for details.

> `\W`, `\D`, `\S` already intersect with valid UTF-8 internally, so they never match invalid byte sequences. The `&\p{utf8}` constraint is only needed when using `~(...)` complement directly.

## Unicode

| Shorthand | Covers | Full-range alternative |
|-----------|--------|----------------------|
| `\w` | word chars up to 2-byte UTF-8 (U+07FF) | `\p{Letter}` \| `\p{Number}` \| `\_` |
| `\d` | ASCII `[0-9]` only | `\p{Number}` |
| `\s` | ASCII `[\t-\r ]` | `\p{White_Space}` |
| `\W` | non-word | |
| `\D` | non-digit | |
| `\S` | non-whitespace | |

`\w` and `\b` cover U+0000..U+07FF (ASCII, Latin Extended, Greek, Cyrillic, Hebrew, Arabic, through NKo). Scripts in 3+ byte UTF-8 (Devanagari, Thai, CJK, …) need `\p{Class}`.

`\d` and `\s` are ASCII-only. Non-ASCII digits and non-ASCII whitespace are rare and their inclusion hurts DFA size and prefix acceleration. Use `\p{Number}` / `\p{White_Space}` for the full Unicode sets.

### Rationale

The goal of the default configuration is not strict conformance to the Unicode spec, it's to reduce unintended performance foot-guns where possible while still covering what real patterns actually use. Full Unicode coverage is available via `UnicodeMode::Full` or explicit `\p{Class}` escapes.

`UnicodeMode` has four settings:

- `Ascii`: `\w`=`[a-zA-Z0-9_]`, `\d`=`[0-9]`, `.` and negated classes step byte-by-byte. Fastest.
- `Default`: common Unicode. `\w` is same as `Full` but only up to 2-byte coverage of UTF-8 (U+0000..U+07FF, through NKo); `\d`=`[0-9]` and `\s`=`[\t-\r ]`.
- `Full`: full Unicode spec. `\w`, `\d`, and `\s` cover the full Unicode word/digit/whitespace sets including 3- and 4-byte UTF-8 codepoints (CJK, historic scripts, etc.). Matches the full Unicode spec at the cost of larger build times.
- `Javascript`: ASCII `\w`/`\d`/`\s`, but `.`, `[^...]`, `\W`/`\D`/`\S` match one full UTF-8 codepoint. Matches default JS `RegExp` behavior (no `u` flag); intended for WASM/JavaScript usage.

Full Unicode `\w` covers ~140,000 codepoints across hundreds of byte ranges. Including all of that in `\w` makes pattern build time significantly worse (ms to seconds on large patterns); match time stays roughly the same.

2-byte coverage (~1,600 codepoints: ASCII through NKo) handles most real `\w` uses at a fraction of the build cost. For wider coverage use either `Full` unicode mode or `\p{Letter}` / `\p{Number}` explicitly. If you mean "non-whitespace token", `\S` is usually what you want: it's the complement of 6 codepoints and far cheaper.

`\b` uses the same 2-byte `\w`; characters beyond U+07FF are treated as non-word for boundary purposes.

For `\d`, the only non-ASCII digits that fit in 2 bytes are Arabic-Indic (U+0660..U+0669), Extended Arabic-Indic (U+06F0..U+06F9), and NKo (U+07C0..U+07C9). These are essentially nonexistent in real corpora (even Arabic/Persian digital text overwhelmingly uses ASCII digits), but including them adds three extra 2-byte branches to every `\d`, which breaks single-byte SIMD prefix acceleration and enlarges the DFA for patterns like `\d+`, `\d{n}`, or `[\w\d]+`.

`\p{Class}` expands to the full Unicode range via `regex_syntax`, with no 2-byte limit. Any [Unicode general category or script name](https://www.unicode.org/reports/tr44/#General_Category_Values) works:

```
\p{Letter}           all Unicode letters (L)
\p{Number}           all Unicode numbers (N)
\p{White_Space}      all Unicode whitespace
\p{Devanagari}       Devanagari script
\p{Greek}            Greek script
\p{Han}              CJK Unified Ideographs
\p{Uppercase}        uppercase letters
```

You can also use explicit ranges: `[\u{0900}-\u{097F}]`.

### Special properties

| Pattern | Description |
|---------|-------------|
| `\p{utf8}` | valid UTF-8 byte strings (for constraining complement) |
| `\p{ascii}` | ASCII bytes (0x00..0x7F) |
| `\p{hex}` | hexadecimal digits (`[0-9a-fA-F]`) |

## Standard syntax

### Character classes

| Pattern | Description |
|---------|-------------|
| `[abc]` | any of a, b, c |
| `[^abc]` | any character except a, b, c |
| `[a-z]` | range: a through z |
| `\d` | digit (ASCII `[0-9]`; use `\p{Number}` for full Unicode) |
| `\D` | non-digit (`[^0-9]`) |
| `\w` | word character (2-byte Unicode by default; `[A-Za-z0-9_]` for ascii, full Unicode via `UnicodeMode::Full` or `\p{Letter}`) |
| `\W` | non-word character |
| `\s` | whitespace (ASCII `[\t\n\v\f\r ]`; use `\p{White_Space}` or `UnicodeMode::Full` for full Unicode) |
| `\S` | non-whitespace |
| `.` | any character except `\n` |

### Quantifiers

| Pattern | Description |
|---------|-------------|
| `*` | 0 or more |
| `+` | 1 or more |
| `?` | 0 or 1 |
| `{n}` | exactly n |
| `{n,}` | n or more |
| `{n,m}` | between n and m |

### Anchors

| Pattern | Description |
|---------|-------------|
| `^` | start of line |
| `$` | end of line |
| `\A` | start of string |
| `\z` | end of string |
| `\b` | word boundary (unicode, see below) |

### Lookarounds

| Pattern | Description |
|---------|-------------|
| `(?=...)` | positive lookahead |
| `(?!...)` | negative lookahead |
| `(?<=...)` | positive lookbehind |
| `(?<!...)` | negative lookbehind |

Lookarounds are compiled directly into the automaton: no backtracking.

Lookarounds combine with intersection as expected:

```
(?<=author).*&.*and.*   after "author", containing "and"
(?<=\s)_*(?=\.)         preceded by whitespace, followed by "."
```

**Restrictions:**

- No nested lookarounds. RE# normalizes every pattern into `(?<=R1)R2(?=R3)`, where R1, R2, R3 are plain regular expressions with no lookbehinds of their own. This is what lets RE# encode lookaround state directly into DFA states and stay linear-time.
- No lookarounds inside complement (`~(...)`) or stars `*`

### Flags

| Flag | Meaning |
|------|---------|
| `(?i)` | case-insensitive |
| `(?s)` | dot matches newline |
| `(?m)` | multiline anchors |
| `(?x)` | extended (ignore whitespace) |

Flags apply from the point they appear until the end of the enclosing group.

## Match semantics

Matches are **leftmost-longest**. This differs from most regex engines which use leftmost-greedy (PCRE). Lazy quantifiers (`*?`, `+?`, `??`, `{n,m}?`) are not supported and will produce a parse error.

Alternation order does not affect what gets matched; only length does. For `y|yes|n|no` against `yes please`:

| Engine | Match |
|--------|-------|
| RE# (leftmost-longest) | `yes` |
| PCRE / Rust `regex` (leftmost-greedy) | `y` |

For predictable longest-match behavior, alternative order is irrelevant: `yes|y|no|n` and `y|yes|n|no` both match `yes` in RE#.
