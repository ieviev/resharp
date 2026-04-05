use resharp::{EngineOptions, Regex};
use std::path::Path;

fn load_tests(path: &str) -> Vec<(String, String, Vec<(usize, usize)>)> {
    let content = std::fs::read_to_string(path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| {
            let pattern = t["pattern"].as_str().unwrap().to_string();
            let input = t["input"].as_str().unwrap().to_string();
            let matches: Vec<(usize, usize)> = t["matches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| {
                    let arr = m.as_array().unwrap();
                    (
                        arr[0].as_integer().unwrap() as usize,
                        arr[1].as_integer().unwrap() as usize,
                    )
                })
                .collect();
            (pattern, input, matches)
        })
        .collect()
}

#[test]
fn accel_skip_lazy() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("accel_skip.toml");
    let tests = load_tests(path.to_str().unwrap());
    for (pattern, input, expected) in &tests {
        // println!("pattern: {}",pattern);
        let re = Regex::with_options(
            pattern,
            EngineOptions {
                dfa_threshold: 0,
                max_dfa_capacity: 10000,
                ..Default::default()
            },
        )
        .unwrap();
        let matches = re.find_all(input.as_bytes()).unwrap();
        let result: Vec<(usize, usize)> = matches.iter().map(|m| (m.start, m.end)).collect();
        assert_eq!(
            result, *expected,
            "lazy: pattern={:?}, input={:?}",
            pattern, input
        );
    }
}
