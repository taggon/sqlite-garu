# sqlite-garu

> **주의**
> 이 프로젝트는 아직 프로덕션 환경에서 사용하기에 적합하지 않은 개발 단계의 프로젝트입니다.

[English](docs/README.en.md)

SQLite FTS5 용 한국어 형태소 분석 토크나이저 확장 기능. [garu](https://github.com/ongjin/garu) 형태소 분석기를 사용합니다.

## 특징

- **형태소 기반 전문 검색** — "달렸다"로 검색하면 "달리는 사람", "나는 달렸다" 등 모든 활용형이 매칭됩니다.
- **임베디드 모델** — garu base 모델(1.7MB)이 바이너리에 포함되어 별도 파일 없이 동작합니다.
- **Nori 스타일 불용태그 필터** — 조사, 어미, 기능어 등을 자동 제거하여 검색 정확도를 높입니다.
- **외국어/숫자 지원** — 영어, 숫자, 한자 등은 검색 대상으로 유지됩니다.

## 빌드

```sh
cargo build --release --features loadable
```

빌드 결과: `target/release/libsqlite_garu.dylib` (macOS) / `.so` (Linux) / `.dll` (Windows)

## 사용법

### 확장 로드

```sql
.load libsqlite_garu
```

### FTS5 테이블 생성

```sql
CREATE VIRTUAL TABLE docs USING fts5(content, tokenize='garu');
```

### 색인 및 검색

```sql
INSERT INTO docs VALUES ('달리는 사람');
INSERT INTO docs VALUES ('나는 달렸다');
INSERT INTO docs VALUES ('그녀도 달린다');

-- 활용형 검색: "달렸다"로 검색해도 모든 행이 매칭됩니다
SELECT * FROM docs WHERE docs MATCH '달렸다';
-- → 달리는 사람
-- → 나는 달렸다
-- → 그녀도 달린다
```

## 테스트

```sh
cargo test
```

> **참고:** `cargo test`는 `loadable` feature 없이 실행합니다. `--features loadable`을 붙이면 테스트가 실패합니다.

## 의존성

- [garu](https://github.com/ongjin/garu) — 브라우저/임베디드 환경을 위한 초경량 한국어 형태소 분석기 (git submodule)
- [rusqlite](https://github.com/rusqlite/rusqlite) — Rust용 SQLite 바인딩 (bundled)

## 라이선스

MIT
