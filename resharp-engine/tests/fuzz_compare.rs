// Runs the regex crate (multiline mode) against the fuzz corpus and writes
// data/fuzz/regex-crate/*-regex-crate.json for use by fuzz_vs_regex_crate.rs.
//
// Usage:
//   cargo test --test fuzz_compare <test_name> -- --ignored --nocapture

use serde::{Deserialize, Serialize};
use std::panic;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Deserialize)]
struct FuzzEntry {
    pattern: String,
    input: String,
}

#[derive(Serialize)]
struct RegexCrateEntry {
    pattern: String,
    input: String,
    matches: Option<Vec<[usize; 2]>>,
}

fn regex_find_multiline(pattern: &str, input: &[u8]) -> Option<Vec<[usize; 2]>> {
    let re = regex::RegexBuilder::new(pattern)
        .multi_line(true)
        .build()
        .ok()?;
    let input_str = std::str::from_utf8(input).ok()?;
    Some(re.find_iter(input_str).map(|m| [m.start(), m.end()]).collect())
}

fn bounded_run<F, T>(f: F) -> Option<T>
where
    F: FnOnce() -> Option<T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || {
            let r = panic::catch_unwind(panic::AssertUnwindSafe(f));
            let _ = tx.send(r);
        });
    match rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(v)) => v,
        _ => None,
    }
}

fn fuzz_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/fuzz")
}

fn record_regex_crate(filename: &str) {
    let path = fuzz_dir().join(filename);
    if !path.exists() {
        eprintln!("skip: {} not found", path.display());
        return;
    }

    let stem = filename.strip_suffix(".json").unwrap_or(filename);
    let out_dir = fuzz_dir().join("regex-crate");
    std::fs::create_dir_all(&out_dir).unwrap();
    let out_path = out_dir.join(format!("{}-regex-crate.json", stem));

    let content = std::fs::read_to_string(&path).unwrap();
    let entries: Vec<FuzzEntry> = serde_json::from_str(&content).unwrap();

    let mut results: Vec<RegexCrateEntry> = Vec::with_capacity(entries.len());
    let mut null = 0usize;

    for (i, entry) in entries.iter().enumerate() {
        let pat = entry.pattern.clone();
        let inp = entry.input.as_bytes().to_vec();
        let matches = bounded_run(move || regex_find_multiline(&pat, &inp));
        if matches.is_none() { null += 1; }
        results.push(RegexCrateEntry {
            pattern: entry.pattern.clone(),
            input: entry.input.clone(),
            matches,
        });
        if (i + 1) % 5000 == 0 {
            eprintln!("  [{}/{}] null={}", i + 1, entries.len(), null);
        }
    }

    std::fs::write(&out_path, serde_json::to_string(&results).unwrap()).unwrap();
    eprintln!("wrote {} ({} entries, {} null)", out_path.display(), results.len(), null);
}

macro_rules! fuzz_test {
    ($name:ident, $file:literal) => {
        #[test]
        #[ignore]
        fn $name() {
            record_regex_crate($file);
        }
    };
}

fuzz_test!(fuzz_npm, "npm-uniquePatterns.json");
fuzz_test!(fuzz_pypi, "pypi-uniquePatterns.json");
fuzz_test!(fuzz_regexlib, "internetSources-regExLib.json");
fuzz_test!(fuzz_stackoverflow, "internetSources-stackoverflow.json");
fuzz_test!(fuzz_uniq, "uniq-regexes-8.json");
