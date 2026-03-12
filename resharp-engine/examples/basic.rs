use resharp::Regex;

fn show(re: &Regex, input: &[u8]) {
    let matches = re.find_all(input).unwrap();
    for m in &matches {
        if m.start == m.end {
            continue;
        }
        let text = &input[m.start..m.end];
        let preview = if text.len() > 70 { &text[..70] } else { text };
        println!(
            "  [{}..{}) {:?}",
            m.start,
            m.end,
            String::from_utf8_lossy(preview)
        );
    }
}

fn main() {
    // intersection: contains both "cat" and "dog"
    let re = Regex::new("_*cat_*&_*dog_*").unwrap();
    println!("cat & dog:");
    show(&re, b"the cat chased the dog");
    show(&re, b"the cat sat on the mat");

    // intersection with length constraint
    let re = Regex::new("_*cat_*&_*dog_*&_{5,30}").unwrap();
    println!("\ncat & dog & 5-30 chars:");
    show(&re, b"the cat chased the dog");

    // complement: segments without consecutive digits
    let re = Regex::new(r"~(_*\d\d_*)&_+").unwrap();
    println!("\nno consecutive digits:");
    show(&re, b"abc12def456gh7ij");

    // complement: split on double newlines (paragraph boundaries)
    let re = Regex::new(r"~(_*\n\n_*)&\S_*\S").unwrap();
    println!("\nsingle paragraphs:");
    show(
        &re,
        b"first paragraph\nstill first\n\nsecond paragraph\n\nthird part",
    );

    // starts with F, does not end with "Finn"
    let re = Regex::new("F.*&~(.*Finn)").unwrap();
    println!("\nF.* & ~(.*Finn):");
    show(&re, b"Finn, Huckleberry Finn, and Tom");

    // password validation: 8+ alphanumeric, has upper, has lower,
    // has digit, no consecutive digits
    let re = Regex::new(r"[a-zA-Z\d]{8,}&_*[A-Z]_*&_*[a-z]_*&_*\d_*&~(_*\d\d_*)").unwrap();
    println!("\npassword validation:");
    for pw in ["Abcdefg1", "abcdefg1", "ABCDEFG1", "Ab1cd2ef", "Ab12cdef"] {
        println!("  {:12} {}", pw, re.is_match(pw.as_bytes()).unwrap());
    }

    // paragraph extraction: no double newlines, contains "swap"
    let re = Regex::new(r"~(_*\n\n_*)&_*swap_*&\S_*\S").unwrap();
    let doc = b"we can swap values\nusing temp vars\n\nno changes here\n\nalso swap this\nand more";
    println!("\nparagraphs containing 'swap':");
    show(&re, doc);

    // lookahead: digits followed by am/pm
    let re = Regex::new(r"\d+(?=\s*[aApP]\.?[mM]\.?)").unwrap();
    println!("\ntimes (lookahead):");
    show(&re, b"meeting at 10am, lunch at 12 p.m.");

    // lookbehind: text after "author:"
    let re = Regex::new(r"(?<=author:\s).*").unwrap();
    println!("\nafter 'author:' (lookbehind):");
    show(&re, b"author: Jane Doe");

    // negative lookbehind: words not preceded by a digit
    let re = Regex::new(r"(?<!\d)[a-z]+").unwrap();
    println!("\nwords not after digit:");
    show(&re, b"3abc def 7ghi jkl");

    // large alternation: dictionary of 25 long words
    // resharp handles this via symbolic derivatives; backtracking engines
    // degrade exponentially on patterns this size
    let words = [
        "acknowledgement",
        "accomplishments",
        "characteristics",
        "confidentiality",
        "congratulations",
        "disproportionate",
        "electromagnetic",
        "experimentation",
        "extraordinarily",
        "fundamentalists",
        "implementations",
        "inconsistencies",
        "instrumentation",
        "interchangeable",
        "internationally",
        "misunderstanding",
        "professionalism",
        "representatives",
        "responsibilities",
        "straightforward",
        "telecommunications",
        "troubleshooting",
        "unconstitutional",
        "understandingly",
        "unquestioningly",
    ];
    let pattern = words.join("|");
    let re = Regex::new(&pattern).unwrap();
    let input = b"the representatives discussed telecommunications \
                  and responsibilities at the internationally recognized \
                  conference on professionalism";
    println!("\ndictionary search (25 words):");
    show(&re, input);

    // dictionary + context window (.{0,20} on each side) to show what is around the matches
    let ctx = format!(".{{0,20}}({}).{{0,20}}", words.join("|"));
    let re = Regex::new(&ctx).unwrap();
    println!("\ndictionary + context window:");
    show(&re, input);
}
