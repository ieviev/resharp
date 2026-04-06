use resharp::Regex;
use serde::{Deserialize, Serialize};
use std::panic;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Deserialize, Clone)]
struct FuzzEntry {
    pattern: String,
    input: String,
    matches: Vec<[usize; 2]>,
}

#[derive(Serialize)]
struct CompareResult {
    file: String,
    engine: String,
    tested: usize,
    passed: usize,
    compile_fail: usize,
    match_fail: usize,
    panicked: usize,
    timed_out: usize,
    failures: Vec<Failure>,
}

#[derive(Serialize)]
struct Failure {
    index: usize,
    pattern: String,
    input: String,
    expected: Vec<[usize; 2]>,
    actual: Vec<[usize; 2]>,
}

fn escape_resharp_ops(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let bs = i - start;
            for _ in 0..bs {
                out.push('\\');
            }
            // odd backslashes: last one escapes the next char
            if bs % 2 == 1 && i < bytes.len() {
                out.push(bytes[i] as char);
                i += 1;
            }
        } else if bytes[i] == b'_' || bytes[i] == b'&' {
            out.push('\\');
            out.push(bytes[i] as char);
            i += 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn resharp_find(pattern: &str, input: &[u8]) -> Option<Vec<[usize; 2]>> {
    let pat = escape_resharp_ops(pattern);
    let re = Regex::new(&pat).ok()?;
    let result = re.find_all(input).ok()?;
    Some(result.iter().map(|m| [m.start, m.end]).collect())
}

fn rust_regex_find(pattern: &str, input: &[u8]) -> Option<Vec<[usize; 2]>> {
    let re = regex::Regex::new(pattern).ok()?;
    let input_str = std::str::from_utf8(input).ok()?;
    Some(
        re.find_iter(input_str)
            .map(|m| [m.start(), m.end()])
            .collect(),
    )
}

fn bounded_run<F, T>(f: F, timeout: Duration) -> Result<T, &'static str>
where
    F: FnOnce() -> Option<T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let builder = thread::Builder::new().stack_size(8 * 1024 * 1024);
    let _handle = builder
        .spawn(move || {
            let r = panic::catch_unwind(panic::AssertUnwindSafe(f));
            let _ = tx.send(r);
        })
        .map_err(|_| "spawn")?;

    match rx.recv_timeout(timeout) {
        Ok(Ok(Some(v))) => Ok(v),
        Ok(Ok(None)) => Err("compile/runtime"),
        Ok(Err(_)) => Err("panic"),
        Err(_) => Err("timeout"),
    }
}

fn fuzz_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("data/fuzz")
}

fn run_all_engines(filename: &str) {
    let path = fuzz_dir().join(filename);
    if !path.exists() {
        eprintln!("skip: {} not found", path.display());
        return;
    }

    let content = std::fs::read_to_string(&path).unwrap();
    let entries: Vec<FuzzEntry> = serde_json::from_str(&content).unwrap();
    let timeout = Duration::from_secs(5);
    let stem = filename.strip_suffix(".json").unwrap_or(filename);

    // 1. run resharp on all entries, collect results
    eprintln!("running resharp on {}", filename);
    let mut resharp_results: Vec<Option<Vec<[usize; 2]>>> = Vec::with_capacity(entries.len());
    let mut resharp_cmp = CompareResult {
        file: filename.to_string(),
        engine: "resharp".to_string(),
        tested: 0,
        passed: 0,
        compile_fail: 0,
        match_fail: 0,
        panicked: 0,
        timed_out: 0,
        failures: Vec::new(),
    };

    for (i, entry) in entries.iter().enumerate() {
        let pat = entry.pattern.clone();
        let inp = entry.input.as_bytes().to_vec();
        match bounded_run(move || resharp_find(&pat, &inp), timeout) {
            Ok(actual) => {
                resharp_cmp.tested += 1;
                if actual == entry.matches {
                    resharp_cmp.passed += 1;
                } else {
                    resharp_cmp.match_fail += 1;
                    resharp_cmp.failures.push(Failure {
                        index: i,
                        pattern: entry.pattern.clone(),
                        input: entry.input.clone(),
                        expected: entry.matches.clone(),
                        actual: actual.clone(),
                    });
                }
                resharp_results.push(Some(actual));
            }
            Err("compile/runtime") => {
                resharp_cmp.compile_fail += 1;
                resharp_results.push(None);
            }
            Err("panic") => {
                resharp_cmp.panicked += 1;
                resharp_results.push(None);
            }
            Err("timeout") => {
                resharp_cmp.timed_out += 1;
                resharp_results.push(None);
            }
            _ => {
                resharp_results.push(None);
            }
        }
        if (i + 1) % 5000 == 0 {
            eprintln!(
                "  resharp [{}/{}] passed={} fail={}",
                i + 1,
                entries.len(),
                resharp_cmp.passed,
                resharp_cmp.match_fail
            );
        }
    }

    // 2. run rust regex on all entries, compare to both oracle and resharp
    eprintln!("running regex crate on {}", filename);
    let mut regex_vs_oracle = CompareResult {
        file: filename.to_string(),
        engine: "regex-crate-vs-oracle".to_string(),
        tested: 0,
        passed: 0,
        compile_fail: 0,
        match_fail: 0,
        panicked: 0,
        timed_out: 0,
        failures: Vec::new(),
    };
    let mut regex_vs_resharp = CompareResult {
        file: filename.to_string(),
        engine: "regex-crate-vs-resharp".to_string(),
        tested: 0,
        passed: 0,
        compile_fail: 0,
        match_fail: 0,
        panicked: 0,
        timed_out: 0,
        failures: Vec::new(),
    };

    for (i, entry) in entries.iter().enumerate() {
        let pat = entry.pattern.clone();
        let inp = entry.input.as_bytes().to_vec();
        match bounded_run(move || rust_regex_find(&pat, &inp), timeout) {
            Ok(actual) => {
                // vs oracle (resharp-rust)
                regex_vs_oracle.tested += 1;
                if actual == entry.matches {
                    regex_vs_oracle.passed += 1;
                } else {
                    regex_vs_oracle.match_fail += 1;
                    regex_vs_oracle.failures.push(Failure {
                        index: i,
                        pattern: entry.pattern.clone(),
                        input: entry.input.clone(),
                        expected: entry.matches.clone(),
                        actual: actual.clone(),
                    });
                }

                // vs resharp (current engine)
                if let Some(ref resharp_actual) = resharp_results[i] {
                    regex_vs_resharp.tested += 1;
                    if actual == *resharp_actual {
                        regex_vs_resharp.passed += 1;
                    } else {
                        regex_vs_resharp.match_fail += 1;
                        regex_vs_resharp.failures.push(Failure {
                            index: i,
                            pattern: entry.pattern.clone(),
                            input: entry.input.clone(),
                            expected: actual,
                            actual: resharp_actual.clone(),
                        });
                    }
                }
            }
            Err("compile/runtime") => {
                regex_vs_oracle.compile_fail += 1;
            }
            Err("panic") => {
                regex_vs_oracle.panicked += 1;
            }
            Err("timeout") => {
                regex_vs_oracle.timed_out += 1;
            }
            _ => {}
        }
        if (i + 1) % 5000 == 0 {
            eprintln!(
                "  regex [{}/{}] vs-oracle: passed={} fail={} | vs-resharp: passed={} fail={}",
                i + 1,
                entries.len(),
                regex_vs_oracle.passed,
                regex_vs_oracle.match_fail,
                regex_vs_resharp.passed,
                regex_vs_resharp.match_fail,
            );
        }
    }

    // save all results
    for (suffix, result) in [
        ("resharp-vs-oracle", &resharp_cmp),
        ("regex-vs-oracle", &regex_vs_oracle),
        ("regex-vs-resharp", &regex_vs_resharp),
    ] {
        let out_path = fuzz_dir().join(format!("{}-{}.json", stem, suffix));
        let json = serde_json::to_string_pretty(result).unwrap();
        std::fs::write(&out_path, json).unwrap();
        eprintln!(
            "{} {}: tested={} passed={} fail={} compile_fail={} panicked={} timeout={}",
            filename,
            suffix,
            result.tested,
            result.passed,
            result.match_fail,
            result.compile_fail,
            result.panicked,
            result.timed_out
        );
    }
}

macro_rules! fuzz_test {
    ($name:ident, $file:literal) => {
        #[test]
        #[ignore]
        fn $name() {
            run_all_engines($file);
        }
    };
}

fuzz_test!(fuzz_npm, "npm-uniquePatterns.json");
fuzz_test!(fuzz_pypi, "pypi-uniquePatterns.json");
fuzz_test!(fuzz_regexlib, "internetSources-regExLib.json");
fuzz_test!(fuzz_stackoverflow, "internetSources-stackoverflow.json");
fuzz_test!(fuzz_uniq, "uniq-regexes-8.json");
