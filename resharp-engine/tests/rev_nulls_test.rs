use resharp::Regex;
use std::path::Path;

struct TestCase {
    name: String,
    pattern: String,
    ignore: bool,
    input: String,
    rev_nulls: Vec<usize>,
}

fn load_tests() -> Vec<TestCase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("rev_nulls.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    tests
        .iter()
        .map(|t| TestCase {
            name: t["name"].as_str().unwrap().to_string(),
            pattern: t["pattern"].as_str().unwrap().to_string(),
            ignore: t.get("ignore").and_then(|v| v.as_bool()).unwrap_or(false),
            input: t
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            rev_nulls: t["rev_nulls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_integer().unwrap() as usize)
                .collect(),
        })
        .collect()
}

#[test]
fn test_rev_nulls_toml() {
    for tc in load_tests() {
        if tc.ignore {
            continue;
        }
        let re = Regex::new(&tc.pattern).unwrap_or_else(|e| {
            panic!("name={} pattern={:?}: compile error: {}", tc.name, tc.pattern, e)
        });
        let got = re.collect_rev_nulls_debug(tc.input.as_bytes());
        for i in 1..got.len() {
            assert!(
                got[i] <= got[i - 1],
                "rev nulls not sorted descending at [{}]: {} > {} (name={}, pattern={:?}, got={:?})",
                i, got[i], got[i - 1], tc.name, tc.pattern, got
            );
        }
        assert_eq!(
            got, tc.rev_nulls,
            "name={} pattern={:?} input={:?}",
            tc.name, tc.pattern, tc.input
        );
    }
}
