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

### `_` -- universal wildcard

Matches any single byte including newlines. `_*` means "any string".

Standard `.` does **not** match `\n`. Use `_` when you need to cross line boundaries.

```
_       matches any single byte
_*      matches any byte string (including empty)
_{5,10} matches any byte string of 5-10 bytes
_*cat_* any string containing "cat"
```

Prefer `_` over `.*` with complement -- `~(.*xyz.*)` means "does not contain xyz on the same line", while `~(_*xyz_*)` means "does not contain xyz" unconditionally.

### `&` -- intersection

Both sides must match. The result is the intersection of two regular languages.

```
_*cat_*&_*dog_*           contains both "cat" and "dog"
_*cat_*&_*dog_*&_{5,30}   ...and is 5-30 characters long
```

Intersection has higher precedence than alternatives: `a|b&c` is parsed as `a|(b&c)`.

### `~(...)` -- complement

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

RE# operates on raw bytes. Complement inverts at the byte level, so `~(pattern)` can match arbitrary byte sequences -- including invalid UTF-8. Intersect with `\p{utf8}` to stay in valid UTF-8 space:

```
~(_*abc_*)&\p{utf8}                 does not contain "abc", valid UTF-8 only
~(_*\d\d_*)&\p{utf8}               no consecutive digits, valid UTF-8 only
```

Without `&\p{utf8}`, a complement pattern will match any byte string that doesn't match the inner pattern, including byte sequences that aren't valid UTF-8. This matters when your input is guaranteed UTF-8 and you want the engine to respect that.

`\p{utf8}` matches `(ascii | [C0-DF][80-BF] | [E0-EF][80-BF]{2} | [F0-F7][80-BF]{3})*` -- the set of all valid UTF-8 byte strings. There's no special UTF-8 mode; the constraint falls out of intersection over byte-level automata. See the [blog post](https://iev.ee/blog/symbolic-derivatives-and-the-rust-rewrite-of-resharp/) for details.

> `\W`, `\D`, `\S` already intersect with valid UTF-8 internally, so they never match invalid byte sequences. The `&\p{utf8}` constraint is only needed when using `~(...)` complement directly.

## Unicode

`\w`, `\d`, `\s` and `\b` are Unicode-aware by default, but **scoped to 2-byte UTF-8** sequences (U+0000..U+07FF): ASCII, Latin Extended, Greek, Cyrillic, Hebrew, Arabic, and other scripts through NKo.

### Why not full Unicode `\w`?

RE# lazily compiles automaton states on demand, but each new state requires deriving transitions for every character class in the pattern. Full Unicode `\w` covers ~140,000 codepoints across hundreds of disjoint byte ranges - deriving through that is expensive every time a new state is built. `\S`, by contrast, is the complement of just 6 whitespace codepoints. If you're using `\w` to mean "non-whitespace token character", `\S` is both more precise and orders of magnitude cheaper to derive.

RE# defaults to 2-byte coverage (~1,600 codepoints) as a practical middle ground: it covers ASCII plus Latin, Greek, Cyrillic, Hebrew, Arabic, and other scripts through NKo - enough for most `\w` use cases without the derivation cost of full Unicode.

Once states are compiled, match throughput is not significantly affected. But be prepared for milliseconds to seconds of compilation time for large patterns using full Unicode `\w` via `\p{Letter}` - the cost is entirely in building states, not in scanning input.

`\b` uses this same 2-byte `\w` definition - characters outside that range are treated as non-word for boundary purposes.

Scripts encoded as 3+ byte UTF-8 (U+0800+) - Devanagari, Thai, CJK, etc. - are not included in `\w`, `\d`, `\s`. For these, use `\p{Class}` which covers the full Unicode range:

| Shorthand | Covers | Full-range alternative |
|-----------|--------|----------------------|
| `\w` | word chars up to U+07FF | `\p{Letter}` \| `\p{Number}` \| `\_` |
| `\d` | digits up to U+07FF | `\p{Number}` |
| `\s` | whitespace up to U+07FF | `\p{White_Space}` |
| `\W` | non-word (UTF-8 safe) | |
| `\D` | non-digit (UTF-8 safe) | |
| `\S` | non-whitespace (UTF-8 safe) | |

`\p{Class}` expands to the full Unicode range via `regex_syntax`, with no 2-byte limit. Any [Unicode general category or script name](https://www.unicode.org/reports/tr44/#General_Category_Values) works:

```
\p{Letter}           all Unicode letters (L)
\p{Number}           all Unicode numbers (N)
\p{White_Space}      all Unicode whitespace
\p{Devanagari}       Devanagari script (U+0900..U+097F)
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
| `\d` | digit (unicode; `[0-9]` for ascii) |
| `\D` | non-digit |
| `\w` | word character (unicode; `[A-Za-z0-9_]` for ascii) |
| `\W` | non-word character |
| `\s` | whitespace (unicode; `[\t\n\v\f\r ]` for ascii) |
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

Lookarounds are compiled directly into the automaton -- no backtracking.

Lookarounds combine with intersection and complement:

```
(?<=author).*&.*and.*   after "author", containing "and"
(?<=\s)_*(?=\.)         preceded by whitespace, followed by "."
```

**Restriction: no nested lookarounds.** RE# normalizes all lookarounds into the form `(?<=R1)R2(?=R3)`, where R1, R2, and R3 are regular expressions that themselves cannot contain lookbehinds. This is what allows RE# to encode lookaround information directly into DFA states and maintain linear-time matching.

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
