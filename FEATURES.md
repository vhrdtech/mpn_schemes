# mpn_schemes features and roadmap

The single source of truth for what mpn_schemes does, which scheme families exist, what is broken and what is
planned. [CHANGELOG.md](CHANGELOG.md) records what changed and when.

Statuses: ✅ done, 🚧 in progress (the note says what is missing), 🐛 known bug, 📋 planned, 💡 idea, ⛔ blocked,
🔍 needs a check. IDs are stable: never renumber or reuse one; new ones come from `tpm work claim AREA`.

The design these items come from: mx3 `docs/mpn-schemes.md` (the overview on mx3's side). The language reference is
[docs/language.md](docs/language.md), how to write a family [docs/writing-a-scheme.md](docs/writing-a-scheme.md).
Planned and built 10 Nov 2026 (tpm P2527#10nov-1405).

## Language (`LANG`)

Scheme files: header, segments, tables, availability, examples; the loader and validator.

- ✅ LANG-1 scheme-files: TOML per family, `[scheme]` header, segments `literal`, `table`, `code`, `free`, `group`, `alternatives`, `optional` with `absent`, `when` (values, lists, ranges, `{ code = }`); expanded into flat variants, at most 1024 (language.md §1-3).
- ✅ LANG-2 availability-rows: `availability = open | closed`, `[[available]]` and `[[excluded]]` rows on segment values, lists, ranges with an E series, or codes (language.md §5).
- ✅ LANG-3 examples: every scheme carries real MPNs with their parameters; the loader round-trips each both ways (language.md §7).
- ✅ LANG-4 validator: every error of a file with its line and segment; one broken fixture per refusal in `tests/fixtures/broken` (language.md §8).
- ✅ LANG-5 parameter-keys: the key list with kinds (quantity with unit, tolerance, word with allowed words for packaging, technology, aec_q200), following mx3's curated names; exact decimal values with one stable text form.
- 📋 LANG-6 series-facts: a `[series]` section for the facts better_erc reads (technology, qualification, leakage, derating; better_erc CFG-7, passive-selection §6.2). Reserved: a file with `[series]` is refused until the format is defined with better_erc. `[implies]` already carries per-family facts like `aec_q200`.
- ✅ LANG-7 scheme-set: the built-in files (embedded by the build script from `schemes/`), a directory, and an overlay where the same id with a higher version replaces (`SchemeSet::embedded`, `load_dir`, `overlay`).
- 🐛 LANG-9 package-schemes-path: the build script reads `../schemes`; `cargo package` cannot include files outside the crate, so publishing needs the schemes copied or linked into `mpn_schemes/` first (the build script also looks in `mpn_schemes/schemes`).

## Code kinds (`CODE`)

Numeric value codes with a parser and a formatter each: eia3, eia4, eia96, rkm, decimal, int.

- ✅ CODE-1 code-kinds: `eia3`, `eia4` (with `lead_zero`, `pad_whole`), `eia96`, `rkm` (letters per unit), `decimal` (`scale`, `places`), `int` (`digits`); each reads every length it can at a position and writes what reads back (language.md §4).
- ✅ CODE-2 e-series: E3, E6, E12, E24, E48, E96 membership and listing a range; parse flags a value outside a segment's series, generate lists ranges from it.

## Parse and generate (`ENG`)

Matching an MPN against the schemes; generating patterns for a requirement.

- ✅ ENG-1 parse: backtracking per variant with a step limit, MPNs capped at 64 ASCII characters; readings ranked (inside the rows, maker hint, in series, fewer free characters); partial readings with the unread rest only when nothing reads the MPN whole; every reading carries scheme id and version, per segment span, code and value, flags.
- ✅ ENG-2 generate: domains filtered by SRCH-7-style terms (eq with relative tolerance, range, ge, le, enum_in) and ordered closest to the bound; preferred values, then the table default, then the other entries; holes for undecided segments; best-first walk checking `when` and rows, which fix holes; unchecked terms reported; `min_stem` per candidate.
- ✅ ENG-3 patterns: fixed text and holes, `stem()`, `matches()` on a whole MPN, `exact()`, text form `GRM155R71C104KA??D`.
- ✅ ENG-4 values: `Dec` (exact decimal), `Quantity`, `Tolerance`, `Value` with a stable text form that `Value::parse` reads back; own type rather than rvariant (chrono, indexmap and strum, and si_dynamic is not on crates.io, which would block publishing).

## Command line (`CLI`)

`mpns parse|gen|check|list`.

- ✅ CLI-1 parse: `mpns parse MPN... [--maker] [--all] [--json]`, segment by segment; exit 1 when one does not parse whole.
- ✅ CLI-2 gen: `mpns gen TERM... [--class] [--maker] [--scheme] [--prefer key=a|b] [--holes] [--max] [--json]`.
- ✅ CLI-3 check: `mpns check [FILES|DIRS] [--json]`, the built-in set without arguments; exit 1 on any error.
- ✅ CLI-4 list: `mpns list [--keys] [--json]`; `--schemes-dir` overlay, `--color`, `completions`, `--version` with the commit.

## Scheme families (`DATA`)

One item per family file, with its maker's source document.

- 🚧 DATA-1 samsung-cl: `schemes/samsung/cl.toml`, Samsung MLCC catalog (Jan 2024) part numbering; 9 examples (the catalog's own and Digi-Key listings). mx3 corpus (10 Nov 2026): 94 of 95 CL parts read, 376 values agree with Digi-Key, none disagree. Open: no availability rows yet (the catalog's product lineup lists every part and could become closed rows), so generate leaves the thickness a hole; thickness codes the lineup uses beyond the code table (0805 6/7/8/9/E, 1206 9/K, 1210 D/G/T...) are read without a height; voltage code M (63 V at Digi-Key, CL32B106KMVNNWE) is in no Samsung document at hand; sizes R1/L5/L6/01/19, dielectric J (JIS-B), one-sided tolerances H/L/U/V and the special tapings J/Z/Y/2/7/R/W are left out.
- ✅ DATA-2 yageo-rc: `schemes/yageo/rc.toml`, RC_L data sheet V.14, closed rows from the resistance-range and packing tables; jumpers decode without a tolerance; 9 examples. mx3 corpus: 116 RC parts read, 290 values agree, none disagree. Left out: RC0075 and the ESD reel S/7N, the reel codes 7W/3W (double power) and 7D (double quantity), the last code P (RC0402JR-070RP at Digi-Key, not in the data sheet); F-coded jumpers (RC0402FR-070RL) read with an outside-series flag.
- ✅ DATA-3 panasonic-erj-g: `schemes/panasonic/erj_g.toml`, ERJ general purpose ±5 % (AOA0000C301), reels from Panasonic's taping pages (AOA0000C90: 180 mm); open with excluded rows for the resistance range; 4 examples. mx3 corpus: the 3 ERJ ±5 % parts read, values agree. The precision ERJ series (±0.5 %, ±1 %, AOA0000C304; most ERJ parts in the corpus) is another family for the data batches.

## Dropped and superseded
