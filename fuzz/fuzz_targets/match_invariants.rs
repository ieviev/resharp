// self-consistency of matching: for any pattern that compiles and any haystack,
// `find_all` / `find_anchored` / `is_match` must satisfy the invariants the API
// documents, with no oracle required.
//
//   * every match is a valid `[start, end)` slice of the haystack
//     (`start <= end <= len`);
//   * `find_all` returns leftmost-longest, non-overlapping matches
//     (`match[i].start >= match[i-1].end`);
//   * `find_all` is non-empty  <=>  `is_match` is true;
//   * `find_anchored`, when it matches, matches at offset 0.
//
// both the default and hardened engines are checked, since hardened swaps in a
// different forward-scan algorithm that must agree on these invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use resharp::{Regex, RegexOptions};
use resharp_fuzz::hex;

fuzz_target!(|input: (&str, &[u8])| {
    let (pattern, haystack) = input;

    for opts in [
        RegexOptions::default(),
        RegexOptions::default().hardened(true),
    ] {
        let re = match Regex::with_options(pattern, opts) {
            Ok(re) => re,
            Err(_) => continue,
        };

        if let Ok(matches) = re.find_all(haystack) {
            let mut prev_end = 0usize;
            for (i, m) in matches.iter().enumerate() {
                assert!(
                    m.start <= m.end && m.end <= haystack.len(),
                    "out-of-bounds match {m:?} (haystack len {}) \
                     pattern={pattern:?} haystack_hex={}",
                    haystack.len(),
                    hex(haystack),
                );
                if i > 0 {
                    assert!(
                        m.start >= prev_end,
                        "overlapping / unsorted match {m:?} after end {prev_end} \
                         pattern={pattern:?} haystack_hex={}",
                        hex(haystack),
                    );
                }
                prev_end = m.end;
            }

            if let Ok(is_match) = re.is_match(haystack) {
                assert_eq!(
                    !matches.is_empty(),
                    is_match,
                    "find_all/is_match disagree (find_all len {}, is_match {is_match}) \
                     pattern={pattern:?} haystack_hex={}",
                    matches.len(),
                    hex(haystack),
                );
            }
        }

        if let Ok(Some(m)) = re.find_anchored(haystack) {
            assert!(
                m.start == 0 && m.end <= haystack.len(),
                "find_anchored returned non-anchored / out-of-bounds match {m:?} \
                 (haystack len {}) pattern={pattern:?} haystack_hex={}",
                haystack.len(),
                hex(haystack),
            );
        }
    }
});
