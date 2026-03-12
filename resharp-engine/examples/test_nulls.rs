use resharp::{EngineOptions, Regex};

fn dump(pattern: &str, input: &str) {
    let re = Regex::with_options(
        pattern,
        EngineOptions {
            dfa_threshold: 1000,
            max_dfa_capacity: 65535,
            ..Default::default()
        },
    )
    .unwrap();
    eprintln!("pattern: {}", pattern);
    eprintln!("input:   {:?} (len={})", input, input.len());
    eprintln!("effects:\n{}", re.effects_debug());
    let nulls = re.collect_rev_nulls_debug(input.as_bytes());
    eprintln!("nulls: {} {:?}", nulls.len(), nulls);
    eprintln!("rev_states: {}", re.dfa_stats().1);

    // also check: what states are begin vs center
    // eprintln!("begin_debug:\n{}", re.begin_debug());
    eprintln!();
}

fn main() {
    // simple test first
    dump(r"ab", "xabx");
    dump(r"(?<=é)x", "éx");
}
