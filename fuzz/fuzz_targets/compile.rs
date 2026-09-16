// `Regex::with_options` must not panic/abort/hang on any pattern under one
// `option_sweep()` config. Err is fine, a crash is a finding. one compile per unit, the
// first byte picks the config: compiling all six per unit tripped libFuzzer's
// `-timeout=10` under ASAN on patterns that aren't slow.

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
