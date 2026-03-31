# sqlite-garu

[한국어](../README.md)

A Korean morphological tokenizer extension for SQLite FTS5, powered by [garu](https://github.com/ongjin/garu).

## Features

- **Morphological full-text search** — Searching "달렸다" matches "달리는 사람", "나는 달렸다", and all other conjugations of the verb "달리다".
- **Embedded model** — The garu base model (1.7MB) is compiled into the binary. No external model files needed.
- **Nori-style stop tag filter** — Automatically filters out particles, endings, and functional morphemes for better search precision.
- **Foreign language support** — English, numbers, Hanja, and other non-Korean text are preserved as searchable tokens.

## Build

```sh
cargo build --release --features loadable
```

Output: `target/release/libsqlite_garu.dylib` (macOS) / `.so` (Linux) / `.dll` (Windows)

## Usage

### Load the extension

```sql
.load libsqlite_garu
```

### Create an FTS5 table

```sql
CREATE VIRTUAL TABLE docs USING fts5(content, tokenize='garu');
```

### Index and search

```sql
INSERT INTO docs VALUES ('달리는 사람');
INSERT INTO docs VALUES ('나는 달렸다');
INSERT INTO docs VALUES ('그녀도 달린다');

-- Morphological search: "달렸다" matches all conjugated forms
SELECT * FROM docs WHERE docs MATCH '달렸다';
-- → 달리는 사람
-- → 나는 달렸다
-- → 그녀도 달린다
```

## Testing

```sh
cargo test
```

> **Note:** Run `cargo test` without the `loadable` feature. Adding `--features loadable` will cause test failures.

## How it works

1. **Tokenization** — Input text is analyzed by garu-core, which produces lemma forms for each morpheme. For example, "달렸다" → `[달리/VV, 었/EP, 다/EF]`.

2. **Stop tag filtering** — Functional morphemes (particles, endings, adverbs, punctuation) are removed based on a POS tag filter inspired by Elasticsearch Nori's `KoreanPartOfSpeechStopFilter`. Only content morphemes (nouns, verbs, adjectives, etc.) are indexed.

3. **FTS5 matching** — Both indexed content and search queries pass through the same tokenizer. Since "달렸다" and "달리는" both produce the lemma "달리", searching either form matches both documents.

### Filtered POS tags

| Category | Tags |
|---|---|
| Endings | EP, EF, EC, ETN, ETM |
| Particles | JKS, JKC, JKG, JKO, JKB, JKV, JKQ, JX, JC |
| Modifiers | MAG, MAJ, MM |
| Punctuation | SF, SP, SS, SE, SO, SW |
| Affixes | XPN, XSN, XSV, XSA |
| Interjection | IC |

Foreign languages (SL), numbers (SN), and Hanja (SH) are **not** filtered and remain searchable.

## Dependencies

- [garu](https://github.com/ongjin/garu) — Ultra-lightweight Korean morphological analyzer for browser/embedded environments (git submodule)
- [rusqlite](https://github.com/rusqlite/rusqlite) — Rust SQLite bindings (bundled)

## License

MIT
