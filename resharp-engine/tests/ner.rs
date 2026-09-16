use resharp::RegexSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entity {
    Date,
    Money,
    Percent,
    Email,
    Url,
    ProperNoun,
    Verb,
    Adjective,
    Adverb,
    Number,
}

const LEXICON: &[(Entity, &str)] = &[
    (Entity::Date, r"[0-9]{4}-[0-9]{2}-[0-9]{2}"),
    (Entity::Money, r"\$[0-9]+(?:\.[0-9]{2})?"),
    (Entity::Percent, r"[0-9]+(?:\.[0-9]+)?%"),
    (Entity::Email, r"[a-z.]+@[a-z]+\.[a-z]+"),
    (Entity::Url, r"https?://[^ ]+"),
    (Entity::ProperNoun, r"[A-Z][a-z]+\b&~(On|In|At|To|For|Of|The|An|And|Via|Was|Is)"),
    (Entity::Verb, r"[a-z]+ing\b"),
    (Entity::Adjective, r"[a-z]+(?:ful|less|ous|ive)\b"),
    (Entity::Adverb, r"[a-z]+ly\b"),
    (Entity::Number, r"[0-9]+"),
];

fn tagger() -> RegexSet {
    RegexSet::new(LEXICON.iter().map(|(_, p)| *p)).unwrap()
}

fn tag(set: &RegexSet, text: &str) -> Vec<(String, Entity)> {
    set.categorize_all(text.as_bytes())
        .unwrap()
        .iter()
        .map(|m| (text[m.start..m.end].to_string(), LEXICON[m.tag].0))
        .collect()
}

#[test]
fn ner_tags_a_sentence() {
    let set = tagger();
    let text = "On 2024-01-15 Alice was carefully sending $12.50 to bob.smith@example.com, \
                a generous 30% tip, via https://pay.example.org/x for 3 wonderful things.";
    let tagged = tag(&set, text);
    let expect: Vec<(&str, Entity)> = vec![
        ("2024-01-15", Entity::Date),
        ("Alice", Entity::ProperNoun),
        ("carefully", Entity::Adverb),
        ("sending", Entity::Verb),
        ("$12.50", Entity::Money),
        ("bob.smith@example.com", Entity::Email),
        ("generous", Entity::Adjective),
        ("30%", Entity::Percent),
        ("https://pay.example.org/x", Entity::Url),
        ("3", Entity::Number),
        ("wonderful", Entity::Adjective),
    ];
    let got: Vec<(&str, Entity)> = tagged.iter().map(|(s, e)| (s.as_str(), *e)).collect();
    assert_eq!(got, expect);
}

#[test]
fn ner_longest_span_beats_shorter_sibling() {
    let set = tagger();
    assert_eq!(tag(&set, "12.5%"), vec![("12.5%".to_string(), Entity::Percent)]);
    assert_eq!(tag(&set, "12.5"), vec![("12".to_string(), Entity::Number), ("5".to_string(), Entity::Number)]);
    assert_eq!(tag(&set, "$7"), vec![("$7".to_string(), Entity::Money)]);
}

#[test]
fn ner_which_entity_kinds_occur() {
    let set = tagger();
    let kinds = |text: &str| -> Vec<Entity> {
        set.matched(text.as_bytes()).unwrap().iter().map(|&i| LEXICON[i].0).collect()
    };
    assert_eq!(kinds("running quickly"), vec![Entity::Verb, Entity::Adverb]);
    assert_eq!(kinds("Bob paid $5"), vec![Entity::Money, Entity::ProperNoun, Entity::Number]);
    assert_eq!(kinds("..."), vec![]);
}
