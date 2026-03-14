fn test_pattern(label: &str, pattern: &str, input: &[u8]) -> bool {
    match resharp::Regex::new(pattern) {
        Ok(re) => {
            match re.find_all(input) {
                Ok(matches) => {
                    let regex_re = regex::bytes::Regex::new(pattern).unwrap();
                    let regex_matches: Vec<_> = regex_re.find_iter(input).collect();
                    if matches.len() != regex_matches.len() {
                        println!("  MISMATCH {}: resharp={} regex={} (len={})",
                            label, matches.len(), regex_matches.len(), input.len());
                        return false;
                    }
                    for (i, (rm, em)) in matches.iter().zip(regex_matches.iter()).enumerate() {
                        if rm.start != em.start() || rm.end != em.end() {
                            println!("  MISMATCH {}: match[{}] resharp=[{},{}) regex=[{},{})",
                                label, i, rm.start, rm.end, em.start(), em.end());
                            return false;
                        }
                    }
                    true
                }
                Err(e) => { println!("  ERROR {}: {}", label, e); false }
            }
        }
        Err(e) => { println!("  PARSE ERROR {}: {}", label, e); false }
    }
}

fn main() {
    let mut failures = 0;
    let mut total = 0;

    // original cloudflare pattern
    println!("=== cloudflare ===");
    let full = r#"(?:(?:"|'|\]|\}|\\|\d|(?:nan|infinity|true|false|null|undefined|symbol|math)|`|-|\+)+[)]*;?((?:\s|-|\~|!|\{\}|\|\||\+)*.*(?:.*=.*)))"#;
    let input = b"math x=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    total += 1;
    if !test_pattern("cloudflare", full, input) { failures += 1; }

    // math.*=.* across many lengths
    println!("=== math.*=.* (0..500) ===");
    for n in 0..500 {
        let input_str = format!("math x={}", "x".repeat(n));
        total += 1;
        if !test_pattern(&format!("n={}", n), r"math.*=.*", input_str.as_bytes()) { failures += 1; }
    }

    // various patterns with skip-triggering inputs
    println!("=== other patterns ===");
    for n in 0..200 {
        let input_str = format!("hello world {}", "a".repeat(n));
        total += 1;
        if !test_pattern(&format!("hello n={}", n), r"hello.*world", input_str.as_bytes()) { failures += 1; }
    }

    for n in 0..200 {
        let input_str = format!("ab{}cd", "x".repeat(n));
        total += 1;
        if !test_pattern(&format!("abcd n={}", n), r"ab.*cd", input_str.as_bytes()) { failures += 1; }
    }

    println!("\n{}/{} passed, {} failures", total - failures, total, failures);
}
