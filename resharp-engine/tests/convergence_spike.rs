#[test]
fn convergence_spike() {
    let patterns = [
        r"\bBREADTH\b",
        r"\bBREADTH",
        r"BREADTH\b",
        r"(?<=\w)BREADTH(?=\w)",
        r"(?<=foo)BREADTH",
        r"(?<=\s)BREADTH",
        r"(?<!\d)a",
        r"^BREADTH\b",
        r"BREADTH",
        r"HELLO.*WORLD",
    ];
    for pat in patterns {
        let re = resharp::Regex::new(pat).unwrap();
        match re.find_convergence_node(4) {
            Some((node, depth)) => println!("pat={:<30} depth={} conv={}", pat, depth, node),
            None => println!("pat={:<30} NO CONVERGENCE", pat),
        }
    }
}
