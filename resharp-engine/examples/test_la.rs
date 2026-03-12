use resharp::Regex;

fn test(pattern: &str, input: &str) {
    match Regex::new(pattern) {
        Ok(re) => {
            let m = re.find_all(input.as_bytes()).unwrap();
            eprintln!(
                "  {:<35} on {:<20} => {:?}",
                pattern,
                format!("{:?}", input),
                m.iter().map(|m| (m.start, m.end)).collect::<Vec<_>>()
            );
        }
        Err(e) => {
            eprintln!(
                "  {:<35} on {:<20} => ERR: {}",
                pattern,
                format!("{:?}", input),
                e
            );
        }
    }
}

fn main() {
    eprintln!("--- intersection ---");
    test(".*(?=.*def)&.*def", "abcdefdef");
    test(".*(?=.*-)&.*", "a-");
    test(".*def&.*(?=.*def)", "abcdefdef");

    eprintln!("--- intersection+complement ---");
    test(".*(?=.*E)&~(.*and.*)", "___and__E");

    eprintln!("--- LA outside intersection ---");
    test("(.*a.*&.*c.*)(?=.*def)", "abcdef");

    eprintln!("--- dash ---");
    test(".*(?=.*-)&.*", "a-");
    test(".*(?=.*-)&\\S.*\\S", "-aaaa-");

    eprintln!("--- date ---");
    test(
        "[0-9]{2}[/.-][0-9]{2}[/.-]([0-9]{4}|[0-9]{2})&.*$",
        "01.01.2023\n",
    );
}
