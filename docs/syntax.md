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
- Unions of lookbehinds: `(?<=abc)de|(?<=def)gh`

## Extensions

### `_` -- universal wildcard

Matches any single byte including newlines. `_*` means "any string".

Standard `.` does **not** match `\n`. Use `_` when you need to cross line boundaries.

```
_       matches any single character
_*      matches any string (including empty)
_{5,10} matches any string of 5-10 characters
_*cat_* any string containing "cat"
```

Prefer `_` over `.*` with complement -- `~(.*xyz.*)` means "does not contain xyz on the same line", while `~(_*xyz_*)` means "does not contain xyz" unconditionally.

### `&` -- intersection

Both sides must match. The result is the intersection of two regular languages.

```
_*cat_*&_*dog_*           contains both "cat" and "dog"
_*cat_*&_*dog_*&_{5,30}   ...and is 5-30 characters long
```

Intersection has higher precedence than alternation: `a|b&c` is parsed as `a|(b&c)`.

### `~(...)` -- complement

Matches everything the inner pattern does **not** match. Parentheses are required.

```
~(_*\d\d_*)     no consecutive digits
~(_*\n\n_*)     no double newlines
~(_*xyz_*)      does not contain "xyz"
```

### Combining operators

```
F.*&~(.*Finn)                       starts with F, doesn't end with "Finn"
~(_*\d\d_*)&[a-zA-Z\d]{8,}         8+ alphanumeric, no consecutive digits
~(_*\n\n_*)&_*keyword_*&\S_*\S     paragraph containing "keyword"
```

## Standard syntax

### Character classes

| Pattern | Description |
|---------|-------------|
| `[abc]` | any of a, b, c |
| `[^abc]` | any character except a, b, c |
| `[a-z]` | range: a through z |
| `\d` | digit (unicode, hundreds of digits; `[0-9]` for ascii (10 digits)) |
| `\D` | non-digit |
| `\w` | word character (unicode, 10000+ chars) |
| `\W` | non-word character |
| `\s` | whitespace, `[\t\r\n ]` |
| `\S` | non-whitespace, `[^\t\r\n ]` |
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
| `\b` | word boundary |

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
(?<=ab).*&~(_*and_*)    after "ab", not containing "and"
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

Matches are **leftmost-longest**. This differs from most regex engines which use leftmost-first (greedy or lazy). Lazy quantifiers (`*?`, `+?`, `??`, `{n,m}?`) are not supported and will produce a parse error.
