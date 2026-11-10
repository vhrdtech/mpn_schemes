# Changelog

All notable changes to mpn_schemes are recorded here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). [FEATURES.md](FEATURES.md) holds the current status of
every feature and scheme family; this file only records what changed. IDs in parentheses refer to FEATURES.md.

## [Unreleased]

## [0.1.0] - 2026-11-10

### Added

- The repository: Cargo workspace (`mpn_schemes`, `mpns`), AGENTS.md, FEATURES.md, licenses.
- The scheme language: TOML per family with literal, table, code, free, group and alternative segments, optional parts, `when`, availability and excluded rows, implied facts and examples; `[series]` reserved (LANG-1..3, LANG-6). Described in `docs/language.md`, with `docs/writing-a-scheme.md` for writing one from a maker's document.
- The validator: all errors of a file with line and segment; every example round-trips both ways at load (LANG-4).
- Parameter keys and values: exact decimals with units, tolerances, words with allowed lists for packaging, technology and aec_q200 (LANG-5, ENG-4).
- The built-in scheme set, directory loading and overlays by version (LANG-7).
- Code kinds eia3, eia4, eia96, rkm, decimal and int, and the E series E3 to E96 (CODE-1, CODE-2).
- `SchemeSet::parse`: ranked complete readings, partial ones with the unread rest, flags (ENG-1).
- `SchemeSet::generate`: patterns with holes for a requirement, best first, with availability rows fixing holes (ENG-2, ENG-3).
- `mpns parse`, `gen`, `check`, `list`, `completions` (CLI-1..4).
- Schemes `samsung.cl` (Samsung MLCC CL), `yageo.rc` (Yageo RC_L) and `panasonic.erj_g` (Panasonic ERJ ±5 %), with examples from the makers' documents and Digi-Key listings (DATA-1..3).
- The corpus test (`MPN_CORPUS_DIR`) reports per scheme how many MPNs parsed and how many values agreed with the supplier's.
