// Differential `is_match` against `regex`, over the subset where the two
// share semantics (see `DiffPattern`); a disagreement is a finding (almost
// always in resharp -- `regex` is the more battle-tested oracle). Only
// existence is compared, not offsets: resharp is leftmost-longest vs regex's
// leftmost-greedy, so positions may differ while `is_match` must not. Both
// run byte-oriented (`UnicodeMode::Ascii` / `.unicode(false)`).

#![no_main]

use libfuzzer_sys::fuzz_target;
use resharp::{Regex, RegexOptions, UnicodeMode};
use resharp_fuzz::{hex, DiffPattern};

#[derive(Debug, arbitrary::Arbitrary)]
struct DiffInput {
    pattern: DiffPattern,
    haystack: Vec<u8>,
}

fuzz_target!(|input: DiffInput| {
    let pattern = &input.pattern.0;
    let haystack = &input.haystack;

    let rs = match Regex::with_options(
        pattern,
        RegexOptions::default().unicode(UnicodeMode::Ascii),
    ) {
        Ok(rs) => rs,
        Err(_) => return,
    };
    let re = match regex::bytes::RegexBuilder::new(pattern).unicode(false).build() {
        Ok(re) => re,
        Err(_) => return,
    };

    let rs_match = match rs.is_match(haystack) {
        Ok(b) => b,
        Err(_) => return,
    };
    let re_match = re.is_match(haystack);

    assert_eq!(
        rs_match, re_match,
        "is_match divergence: resharp={rs_match} regex={re_match} \
         pattern={pattern:?} haystack_hex={}",
        hex(haystack),
    );
});
