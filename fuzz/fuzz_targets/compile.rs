// `Regex::with_options` must never panic/abort/hang on an arbitrary pattern
// under one `option_sweep()` config; Err (parse/capacity/size) is fine, only a
// crash is a finding. Primary target for the known defect class (intersection
// over alternation/quantifier, nullability asserts), all surfacing at compile.
//
// One compile per unit, one config (first byte picks it): compiling all six
// per unit multiplied a benign sub-second compile by six under ASAN, tripping
// libFuzzer's `-timeout=10` on patterns that aren't actually slow.

#![no_main]

use libfuzzer_sys::fuzz_target;
use resharp::Regex;
use resharp_fuzz::option_sweep;

// longest valid-UTF-8 prefix of `bytes`.
fn decode(bytes: &[u8]) -> &str {
    match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap(),
    }
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let sweep = option_sweep();
    let idx = selector as usize % sweep.len();
    let opts = sweep.into_iter().nth(idx).unwrap();
    // discard the result: `Ok` means it compiled, `Err` is an expected
    // rejection. a crash is what libFuzzer records.
    let _ = Regex::with_options(decode(rest), opts);
});
