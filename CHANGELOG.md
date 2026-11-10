# Changelog

All notable changes to mpn_schemes are recorded here, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). [FEATURES.md](FEATURES.md) holds the current status of
every feature and scheme family; this file only records what changed. IDs in parentheses refer to FEATURES.md.

## [Unreleased]

### Added

- Scheme `panasonic.erj_d` (Panasonic ERJ-D1/D2 low TCR high power current sensing chip resistors, wide terminal type), with examples from the data sheet and a Digi-Key listing (DATA-17).

## [0.1.4] - 2026-11-10

### Added

- `schemes/nexperia/bzx84.toml`: Nexperia BZX84 zener diodes, with examples from Nexperia's orderable-parts pages (DATA-12).
- Parameter key `package` (word) and packaging word `reel_11in_embossed` (DATA-12, LANG-5).
- `schemes/nexperia/bat54.toml`: Nexperia BAT54 Schottky diodes, single and dual configurations in SOT23/SOT323 (DATA-13).
- Parameter key `configuration` (word) and package word `sot323` (DATA-13, LANG-5).
- `schemes/nexperia/bc8x7.toml`: Nexperia BC847/BC857 general-purpose transistors, ungraded and hFE groups A/B/C (DATA-14).
- Parameter keys `polarity` and `gain_group` (DATA-14, LANG-5).
- `schemes/nexperia/2n7002.toml`: Nexperia 2N7002 N-channel MOSFET and the ESD-protected 2N7002K (DATA-15).
- Parameter key `esd_protected` (DATA-15, LANG-5).
- `schemes/onsemi/mmsz.toml`: onsemi MMSZ52xx zener diodes, closed with availability rows for the ±2 % subset (DATA-16).
- Parameter key `aec_q101` and package word `sod123` (DATA-16, LANG-5).
- `schemes/littelfuse/smaj.toml`, `schemes/littelfuse/smbj.toml`, `schemes/littelfuse/smcj.toml`: Littelfuse SMAJ/SMBJ/SMCJ 400/600/1500 W TVS diodes, stand-off voltage 5 V to 440 V (DATA-17).
- Parameter keys `standoff_voltage` and `bidirectional`, and package words `do214ac`, `do214aa`, `do214ab` (DATA-17, LANG-5).

## [0.1.3] - 2026-11-10

### Added

- Scheme `vishay.crcw` (Vishay Dale D/CRCW e3 standard thick film chip resistors), with examples from the data sheet and Digi-Key listings (DATA-4).
- Scheme `panasonic.erj_precision` (Panasonic ERJ ±0.5 % and ±1 % precision thick film chip resistors), with examples from the data sheet and Digi-Key listings (DATA-5).
- Scheme `yageo.ac_resistor` (Yageo AC automotive grade anti-sulfurated chip resistors), with examples from the data sheet and Farnell listings (DATA-6).
- Scheme `uni-royal.chip` (UNI-ROYAL generic thick film chip resistors), with examples from the data sheet and LCSC listings (DATA-7).

### Changed

- `yageo.rc` (v1 → v2): the default-code segment now also reads `P` (the RC_P, total-lead-free spec sheet), not only `L`; `series` is decoded from that code (`RC_L` or `RC_P`) instead of always implied `RC_L` (DATA-2).

## [0.1.2] - 2026-11-10

### Added

- Schemes `yageo.cc` (Yageo CC commercial MLCC), `yageo.ac` (Yageo AC automotive-grade MLCC), `kemet.c` (KEMET C
  commercial MLCC) and `walsin.mlcc` (Walsin general-purpose and mid/high-voltage MLCC), with examples from the
  makers' data sheets, Digi-Key and LCSC listings (DATA-13..16).

## [0.1.1] - 2026-11-10

### Added

- Schemes `murata.grm` (Murata GRM general purpose MLCC), `murata.gcm` (Murata GCM/GCJ automotive MLCC), `tdk.c` (TDK C series general purpose MLCC) and `tdk.cga` (TDK CGA series automotive MLCC), with examples from the makers' catalogs and Digi-Key listings (DATA-8..11).

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
