use resharp_algebra::RegexBuilder;

fn main() {
    let pattern = std::env::args()
        .nth(1)
        .expect("usage: test_date <pattern> [input]");
    let input = std::env::args().nth(2).unwrap_or_default();
    let t0 = std::time::Instant::now();
    let mut b = RegexBuilder::new();
    let t1 = std::time::Instant::now();
    let node = resharp_parser::parse_ast(&mut b, &pattern).unwrap();
    let t2 = std::time::Instant::now();
    eprintln!(
        "builder: {:?}  parse: {:?}  nodes={}",
        t1 - t0,
        t2 - t1,
        b.num_nodes()
    );

    let _rev = b.reverse(node).unwrap();
    let t3 = std::time::Instant::now();
    eprintln!("reverse: {:?}  nodes={}", t3 - t2, b.num_nodes());

    let re = resharp::Regex::from_node(b, node, resharp::EngineOptions::default()).unwrap();
    let t4 = std::time::Instant::now();
    eprintln!("compile: {:?}", t4 - t3);
    if !input.is_empty() {
        match re.find_all(input.as_bytes()) {
            Ok(matches) => {
                let t5 = std::time::Instant::now();
                eprintln!("match: {:?}  {} matches", t5 - t4, matches.len());
            }
            Err(e) => eprintln!("match err: {}", e),
        }
    }
}
