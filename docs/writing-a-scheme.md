# Writing a scheme from a maker's document

How to add a family (Murata GRM, TDK C, Vishay CRCW, Nexperia BZX84...) so that it loads, round-trips and decodes what the maker means. The language itself is in [language.md](language.md); the three schemes in `schemes/` (Samsung CL, Yageo RC, Panasonic ERJ) are worked examples of everything below.

## 1. Get the maker's document

1. Search the library first: `tpm lib search <maker> <series>`. Fetch what is missing with `tpm lib fetch <URL>` (a headless Chromium; vendor sites block plain clients, so a 403 from curl says nothing). Never save documents into a scratch folder or into this repo: the repo is public and holds only the facts copied from them.
2. You need the **ordering-code explanation** ("part numbering", "how to order", "global part number"): the positions of the MPN and each position's codes. Murata, TDK and Samsung put it in their catalogs; Yageo, Vishay and Panasonic in each series' data sheet.
3. You also want the **selection or ratings table**: which values exist per size, tolerance and voltage. That becomes the availability rows.
4. Note the document's title, number or revision, the URL it came from and today's date: they go into `sources`.

## 2. Read the numbering, position by position

Write the MPN out as the document does, and give every position a segment:

```
CL  05  B  104  K  O  5  NNN  C          Samsung CL
RC 0402 J  R-07  100K  L                Yageo RC
ERJ - 3GEY J 102 V                      Panasonic ERJ
```

For each position decide its kind (language.md §3):

| the document shows | write |
|---|---|
| fixed letters (series, a separator) | `literal` |
| a list of codes, each with a meaning | `table` with the param it sets |
| a number in a code (`104`, `1002`, `4K7`, `5V1`) | `code` with the kind and `unit` |
| codes the document does not explain, or explains differently from what real part numbers show | `free` with the characters and length |
| a position whose meaning depends on an earlier one (thickness per size) | `alternatives`, each with a `when` |
| a position that is sometimes left out (a dash, a marking letter, a packaging suffix) | `optional = true`, with `absent` when leaving it out means a value |
| two spellings of one field (3-digit codes at ±5 %, 4-digit at ±1 %) | `alternatives`, with `group`s when the choice spans several positions |

Things the first three schemes ran into, and how they are written:

- **Two positions that together mean one parameter** (Yageo's tape type `R`/`K`, the TCR dash and the reel size `07`/`13` make the packaging): one table whose codes span the positions (`"R-07" = "reel_7in_paper"`). A parameter is set by exactly one segment.
- **A letter that only some sizes carry** (Panasonic's marking `Y`, left out for 0402 and smaller): fold it into the size code (`"3GEY" = "0603"`, `"2GE" = "0402"`).
- **The same code meaning different values per size** (Samsung thickness `C` = 0.85 mm, `F` = 1.25 mm): alternatives with `when = { size = "0805" }`.
- **A code whose meaning depends on a value range** (Samsung `F` = ±1 pF below 10 pF, ±1 % above): `when = { capacitance = { max = "9.99pF" } }`.
- **Two size codes with the same case** (Panasonic `1GN` and `1GJ` are both 0201, with different packaging codes): `when = { size = { code = "1GN" } }`.
- **Positions the document explains, but real part numbers use more values than it lists** (Samsung positions 8-10): a `free` segment. Do not invent meanings, and do not leave out real values.
- **Special codes a code kind cannot write** (a jumper `0R`, `R00`, `000`): an alternative with a table.
- **How the maker writes small values**: compare the code kind's output with real part numbers. Samsung writes 0.5 pF `0R5` and 1 pF `010` (`lead_zero`, `pad_whole`); Murata writes `R50` and `1R0` (the defaults).
- **Codes for rare variants you cannot describe** (Yageo's double-power reels, Samsung's aligned-chip tapings): leave them out and say so in the header comment, rather than half-describe them. Parse then reports such an MPN as partial, which is honest.

## 3. Parameters and words

Every `param`, `[implies]` key and example key comes from `mpns list --keys` (`mpn_schemes/src/keys.rs`). Values are written in their text form: `100nF`, `4.7kΩ`, `6.3V`, `0.5mm`, `±10%`, `+80%/-20%`. Packaging is one of the listed words (`reel_7in_paper`, `reel_7in_embossed`, `reel_10in_paper`, `reel_13in_paper`, `reel_13in_embossed`, `bulk`, ...): paper (punched or pressed carrier) or embossed (plastic) tape, and the reel diameter the maker's packaging data gives. If the document does not say the reel size, find the maker's packaging document; never assume one.

A key or a word you need and cannot find: add it to `src/keys.rs` (and to language.md §6) in the same commit, with a line saying where mx3's vocabulary has it.

Fill `[implies]` with what every part of the family is: `technology = "mlcc"`, `series = "CL"`, `aec_q200 = "yes"` for an automotive series.

## 4. Availability

- If the document has a selection table per size, tolerance and voltage, use `availability = "closed"` and copy its rows. Group rows with lists and ranges (`size = ["0402", "0603"]`, `resistance = { min = "1Ω", max = "10MΩ", series = "E24" }`) instead of one row per part. Yageo RC is the example.
- If the table is too big to copy now, keep the scheme `open` and write the limits you do know as `[[excluded]]` rows (Panasonic ERJ: values below 1 Ω and above 10 MΩ). Say in the header comment that the rows are missing.
- Packaging that only some sizes come on: `[[excluded]]` rows (Yageo: no paper tape for 1218, 2010, 2512).
- Give the tables' page in a `note` on the row or segment.

## 5. Examples

Every scheme carries real examples, and an example nobody verified does not go in:

1. The document's own ordering examples first (`RC0402JR-07100KL`, `ERJ3GEYJ102V`, `CL10A106MQ8NNNC`).
2. Then part numbers from the maker's own lineup tables or a supplier listing you looked at (Digi-Key or Mouser product page through `tpm fetch`), with the source in `source`.
3. Cover each kind of position: a small value with `R`, a jumper, each packaging, an optional segment present and absent, each alternative of a `when`.
4. `params` lists every parameter a segment decodes, in the scheme's words. The example must be inside the availability rows.

Never change an example (or a corpus expectation) to make a check pass: find out whether the scheme or the example is wrong. A supplier listing can be wrong too; the maker's document decides.

## 6. Check it

```sh
mpns check schemes/murata/grm.toml        # or: cargo run -p mpn_schemes_cli -- check schemes/murata/grm.toml
mpns --schemes-dir schemes parse GRM155R71C104KA88D
mpns --schemes-dir schemes gen capacitance=100nF dielectric=X7R case_eia=0402 'rated_voltage>=16V' --scheme murata.grm
just test --changed
```

`check` prints every error with its line and segment (language.md §8 lists what it refuses). `parse` shows the reading segment by segment; `gen` shows what the scheme would craft. Look at both for a few real part numbers beyond the examples: a wrong table entry often still round-trips its own examples.

## 7. Commit

- One family per commit: `data(murata): GRM general purpose MLCC (DATA-n)`, with the FEATURES.md item (`tpm work claim DATA` gives the id) and a CHANGELOG line under `[Unreleased]` → `### Added`.
- Changing an existing scheme's meaning (a table entry changed or removed, a segment split, a row narrowed) raises its `version` and gets a CHANGELOG line saying what changed; adding examples or sources does not.
- What the scheme leaves out goes in its header comment and in the FEATURES.md item, so the next person knows where to continue.
