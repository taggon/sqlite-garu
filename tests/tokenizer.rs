use garu_core::model::Analyzer;

const MODEL: &[u8] = include_bytes!("../_ref/garu/js/models/base.gmdl");
const CNN2: &[u8] = include_bytes!("../_ref/garu/js/models/cnn2.bin");

fn analyzer() -> Analyzer {
    Analyzer::from_bytes(MODEL, CNN2).expect("Failed to load embedded garu model")
}

fn filtered_tokens(text: &str) -> Vec<String> {
    analyzer()
        .analyze(text)
        .tokens
        .into_iter()
        .filter(|t| !sqlite_garu::is_stop_pos(t.pos))
        .map(|t| t.text)
        .collect()
}

#[test]
fn stop_tags_remove_functional_morphemes() {
    let cases: &[(&str, &[&str])] = &[
        ("달리는 사람", &["달리", "사람"]),
        ("나는 달렸다", &["나", "달리"]),
        ("빠르게 달리고 있다", &["빠르", "달리", "있"]),
        ("우리는 내일 달릴 것이다", &["우리", "달리", "것", "이"]),
        ("사과가 많이 떨어졌다", &["사과", "떨어지"]),
        ("배가 아파서 뛰지 못했다", &["배", "아프", "뛰"]),
    ];
    for (input, expected) in cases {
        assert_eq!(filtered_tokens(input), *expected, "filter '{input}'");
    }
}

#[test]
fn conjugated_forms_produce_same_tokens_달리() {
    let tokens: Vec<Vec<String>> = ["달리는", "달렸다", "달린다", "달리고", "달릴"]
        .into_iter()
        .map(|s| filtered_tokens(s))
        .collect();
    for ts in &tokens {
        assert!(ts.contains(&"달리".to_string()));
    }
    for (a, b) in tokens.iter().zip(tokens.iter().skip(1)) {
        assert_eq!(
            a, b,
            "all conjugations should produce identical filtered tokens"
        );
    }
}

#[test]
fn conjugated_forms_produce_same_tokens_먹() {
    let tokens: Vec<Vec<String>> = ["먹었다", "먹는다", "먹고", "먹을", "먹어라"]
        .into_iter()
        .map(|s| filtered_tokens(s))
        .collect();
    for ts in &tokens {
        assert!(ts.contains(&"먹".to_string()));
    }
    for (a, b) in tokens.iter().zip(tokens.iter().skip(1)) {
        assert_eq!(
            a, b,
            "all conjugations should produce identical filtered tokens"
        );
    }
}

#[test]
fn unrelated_verbs_produce_distinct_tokens() {
    let 달리 = filtered_tokens("달렸다");
    let 뛰 = filtered_tokens("뛰었다");
    assert_eq!(달리, vec!["달리"]);
    assert_eq!(뛰, vec!["뛰"]);
    assert_ne!(달리, 뛰);
}

#[test]
fn foreign_words_and_numbers_are_preserved() {
    assert_eq!(
        filtered_tokens("Bun runtime으로 SQLite 확장 테스트"),
        vec!["Bun", "runtime", "SQLite", "확장", "테스트"]
    );
    assert_eq!(
        filtered_tokens("version 3.51.3 released in 2024"),
        vec!["version", "3.51.3", "released", "in", "2024"]
    );
}

#[test]
fn english_stopwords_are_not_filtered() {
    assert_eq!(
        filtered_tokens("a and the of in on"),
        vec!["a", "and", "the", "of", "in", "on"]
    );
}

#[test]
fn morphological_search_noun() {
    let rows: &[(&str, bool)] = &[
        ("사과가 많이 떨어졌다", true),
        ("배가 아파서 뛰지 못했다", false),
    ];
    for (row, should_match) in rows {
        let found = filtered_tokens(row).contains(&"사과".to_string());
        assert_eq!(found, *should_match, "MATCH '사과' vs '{row}'");
    }
}

// Debug output: `cargo test --test tokenizer -- --nocapture`
#[test]
fn debug_inspect_pos_tags() {
    for text in [
        "달리는 사람",
        "나는 달렸다",
        "빠르게 달리고 있다",
        "우리는 내일 달릴 것이다",
        "배가 아파서 뛰지 못했다",
        "사과가 많이 떨어졌다",
        "Bun runtime으로 SQLite 확장 테스트",
        "version 3.51.3 released in 2024",
        "a and the of in on",
    ] {
        let tokens: Vec<_> = analyzer()
            .analyze(text)
            .tokens
            .iter()
            .map(|t| format!("{}/{}", t.text, t.pos.as_str()))
            .collect();
        println!("{text}: {tokens:?}");
    }
}
