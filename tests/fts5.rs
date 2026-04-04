use rusqlite::Connection;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    unsafe {
        assert_eq!(
            sqlite_garu::register_tokenizer(conn.handle()),
            rusqlite::ffi::SQLITE_OK as i32,
            "failed to register garu tokenizer"
        );
    }
    conn
}

fn rows(conn: &Connection, sql: &str, params: &[&str]) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = params
        .iter()
        .map(|p| Box::new(*p) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    stmt.query_map(param_refs.as_slice(), |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

#[test]
fn register_tokenizer_without_error() {
    let _ = db();
}

#[test]
fn search_korean_text() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE docs USING fts5(title, tokenize='garu');
         INSERT INTO docs VALUES ('한국어 형태소 분석기 테스트');
         INSERT INTO docs VALUES ('SQLite 전문 검색 확장 기능');
         INSERT INTO docs VALUES ('자연어 처리 라이브러리 비교');",
    )
    .unwrap();

    let mut results = rows(
        &conn,
        "SELECT title FROM docs WHERE docs MATCH ?",
        &["형태소"],
    );
    results.sort();
    assert_eq!(results, vec!["한국어 형태소 분석기 테스트"]);
}

#[test]
fn morphological_noun_search() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE articles USING fts5(body, tokenize='garu');
         INSERT INTO articles VALUES ('사과가 많이 떨어졌다');
         INSERT INTO articles VALUES ('나는 사과를 좋아한다');
         INSERT INTO articles VALUES ('이 사건은 아직 미해결이다');",
    )
    .unwrap();

    let mut results = rows(
        &conn,
        "SELECT body FROM articles WHERE articles MATCH ?",
        &["사과"],
    );
    results.sort();
    assert_eq!(
        results,
        vec!["나는 사과를 좋아한다", "사과가 많이 떨어졌다"]
    );
}

#[test]
fn foreign_language_search() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE foreign_docs USING fts5(content, tokenize='garu');
         INSERT INTO foreign_docs VALUES ('Bun runtime으로 SQLite 확장 테스트');
         INSERT INTO foreign_docs VALUES ('Rust tokenizer 구현');
         INSERT INTO foreign_docs VALUES ('Python script 예제');",
    )
    .unwrap();

    let sqlite_results = rows(
        &conn,
        "SELECT content FROM foreign_docs WHERE foreign_docs MATCH ?",
        &["SQLite"],
    );
    assert_eq!(sqlite_results, vec!["Bun runtime으로 SQLite 확장 테스트"]);

    let runtime_results = rows(
        &conn,
        "SELECT content FROM foreign_docs WHERE foreign_docs MATCH ?",
        &["runtime"],
    );
    assert_eq!(runtime_results, vec!["Bun runtime으로 SQLite 확장 테스트"]);
}

#[test]
fn numeric_search() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE version_docs USING fts5(content, tokenize='garu');
         INSERT INTO version_docs VALUES ('version 3.51.3 released in 2024');
         INSERT INTO version_docs VALUES ('version 3.45.0 released in 2023');",
    )
    .unwrap();

    let year_results = rows(
        &conn,
        "SELECT content FROM version_docs WHERE version_docs MATCH ?",
        &["2024"],
    );
    assert_eq!(year_results, vec!["version 3.51.3 released in 2024"]);

    // garu >= 0.6.x treats decimal numbers like 3.51.3 as a single SN token
    let full_version_results = rows(
        &conn,
        "SELECT content FROM version_docs WHERE version_docs MATCH ?",
        &["\"3.51.3\""],
    );
    assert_eq!(
        full_version_results,
        vec!["version 3.51.3 released in 2024"]
    );
}

#[test]
fn english_stopwords_are_searchable() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE stopword_docs USING fts5(content, tokenize='garu');
         INSERT INTO stopword_docs VALUES ('a and the of in on');
         INSERT INTO stopword_docs VALUES ('quick brown fox');",
    )
    .unwrap();

    let and_results = rows(
        &conn,
        "SELECT content FROM stopword_docs WHERE stopword_docs MATCH ?",
        &["\"and\""],
    );
    assert_eq!(and_results, vec!["a and the of in on"]);

    let the_results = rows(
        &conn,
        "SELECT content FROM stopword_docs WHERE stopword_docs MATCH ?",
        &["\"the\""],
    );
    assert_eq!(the_results, vec!["a and the of in on"]);
}

#[test]
fn empty_string() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE t USING fts5(content, tokenize='garu');
         INSERT INTO t VALUES ('');",
    )
    .unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM t", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn phrase_query() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE phrase USING fts5(content, tokenize='garu');
         INSERT INTO phrase VALUES ('한국어 형태소 분석');
         INSERT INTO phrase VALUES ('영어 형태소 분석');
         INSERT INTO phrase VALUES ('한국어 음운론 연구');",
    )
    .unwrap();

    let results = rows(
        &conn,
        "SELECT content FROM phrase WHERE phrase MATCH ?",
        &["\"한국어 형태소\""],
    );
    assert_eq!(results, vec!["한국어 형태소 분석"]);
}

#[test]
fn morphological_verb_search() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE verbs USING fts5(content, tokenize='garu');
         INSERT INTO verbs VALUES ('달리는 사람');
         INSERT INTO verbs VALUES ('나는 달렸다');
         INSERT INTO verbs VALUES ('그녀도 달린다');
         INSERT INTO verbs VALUES ('빠르게 달리고 있다');
         INSERT INTO verbs VALUES ('우리는 내일 달릴 것이다');
         INSERT INTO verbs VALUES ('배가 아파서 뛰지 못했다');",
    )
    .unwrap();

    let expected: Vec<&str> = vec![
        "달리는 사람",
        "나는 달렸다",
        "그녀도 달린다",
        "빠르게 달리고 있다",
        "우리는 내일 달릴 것이다",
    ];
    let negative = "배가 아파서 뛰지 못했다";

    for query in &["달렸다", "달린다", "달리고"] {
        let mut results = rows(
            &conn,
            "SELECT content FROM verbs WHERE verbs MATCH ?",
            &[query],
        );
        results.sort();
        assert_eq!(results.len(), 5, "MATCH '{query}' should match 5 rows");
        for row in &expected {
            assert!(
                results.contains(&row.to_string()),
                "MATCH '{query}' should match '{row}'"
            );
        }
        assert!(
            !results.contains(&negative.to_string()),
            "MATCH '{query}' should not match '{negative}'"
        );
    }
}

#[test]
fn morphological_verb_search_먹() {
    let conn = db();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE eat USING fts5(content, tokenize='garu');
         INSERT INTO eat VALUES ('먹었다');
         INSERT INTO eat VALUES ('먹는다');
         INSERT INTO eat VALUES ('먹고 싶다');
         INSERT INTO eat VALUES ('먹을 것이다');
         INSERT INTO eat VALUES ('먹어라');
         INSERT INTO eat VALUES ('배가 아파서 뛰지 못했다');",
    )
    .unwrap();

    let expected: Vec<&str> = vec!["먹었다", "먹는다", "먹고 싶다", "먹을 것이다", "먹어라"];
    let negative = "배가 아파서 뛰지 못했다";

    for query in &["먹었다", "먹는다", "먹을"] {
        let mut results = rows(&conn, "SELECT content FROM eat WHERE eat MATCH ?", &[query]);
        results.sort();
        assert_eq!(results.len(), 5, "MATCH '{query}' should match 5 rows");
        for row in &expected {
            assert!(
                results.contains(&row.to_string()),
                "MATCH '{query}' should match '{row}'"
            );
        }
        assert!(
            !results.contains(&negative.to_string()),
            "MATCH '{query}' should not match '{negative}'"
        );
    }
}
