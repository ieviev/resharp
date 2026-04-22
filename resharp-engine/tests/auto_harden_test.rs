use resharp::{EngineOptions, Regex};
use std::path::Path;

#[test]
fn auto_harden_toml() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("auto_harden.toml");
    let content = std::fs::read_to_string(&path).unwrap();
    let table: toml::Value = content.parse().unwrap();
    let tests = table["test"].as_array().unwrap();
    for t in tests {
        let pattern = t["pattern"].as_str().unwrap();
        let expected = t["hardened"].as_bool().unwrap();
        let re = Regex::new(pattern).expect("pattern compiles");
        assert_eq!(
            re.is_hardened(),
            expected,
            "pattern={:?}: expected is_hardened={}, got {}",
            pattern,
            expected,
            re.is_hardened()
        );

        if expected {
            let hardened =
                Regex::with_options(pattern, EngineOptions::default().hardened(true)).unwrap();
            let inputs: &[&[u8]] = &[b"", b"aaaaaaaa", b"abcdefg", b"|  |\n| a |\n|  |"];
            for input in inputs {
                assert_eq!(
                    re.find_all(input).unwrap(),
                    hardened.find_all(input).unwrap(),
                    "pattern={:?} input={:?}",
                    pattern,
                    input
                );
            }
        }
    }
}
