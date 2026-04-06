fn main() {
    let pattern = r"((?:ASIA|AKIA|AROA|AIDA)([A-Z0-7]{16}))";
    let re = resharp::Regex::new(pattern).unwrap();
    re.find_all(b"AKIAIOSFODNN7EXAMPLE").ok();
    eprintln!(
        "compiled OK, is_match test={}",
        re.is_match(b"AKIAIOSFODNN7EXAMPLE").unwrap()
    );
    eprintln!("is_match empty={}", re.is_match(b"hello world").unwrap());
}
