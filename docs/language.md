# The scheme language

A scheme file describes how one maker composes the part numbers (MPNs) of one family: Samsung CL, Yageo RC, Panasonic ERJ. It is TOML, one family per file, at `schemes/<maker>/<family>.toml`, and its `id` is `<maker>.<family>` (the path with `/` as `.`). The library reads it, validates it and uses it both ways: **parse** an MPN into parameters, **generate** MPN patterns for wanted parameters. How to write one from a datasheet: [writing-a-scheme.md](writing-a-scheme.md).

A whole file, with every part explained below:

```toml
[scheme]
id = "samsung.cl"
version = 1
maker = "Samsung Electro-Mechanics"
aliases = ["SEMCO", "Samsung"]
class = "capacitor.ceramic"
title = "Samsung MLCC, general (CL)"
sources = [{ doc = "MLCC catalog", url = "https://...", fetched = "2026-11-10", pages = "6-7" }]
availability = "open"            # or "closed"
min_stem = "size"                # optional

[implies]
technology = "mlcc"

[[segment]]
name = "series"
literal = "CL"

[[segment]]
name = "size"
param = "case_eia"
table = { "05" = "0402", "10" = "0603" }

[[segment]]
name = "capacitance"
param = "capacitance"
code = "eia3"
unit = "pF"

[[excluded]]
size = "0402"
capacitance = { min = "10uF" }

[[example]]
mpn = "CL05B104KO5NNNC"
params = { case_eia = "0402", capacitance = "100nF" }
```

Unknown fields are errors, never ignored: a typo in a field name stops the file from loading.

## 1. Header: `[scheme]`

| field | required | meaning |
|---|---|---|
| `id` | yes | `<maker>.<family>`, lower case, `a-z 0-9 . _ -`; must match the file's path. Stable: decoded values record it. |
| `version` | yes | Starts at 1. Raised on every change of meaning (a table entry changed or removed, a segment split, a row narrowed). Adding examples or sources does not raise it. |
| `maker` | yes | The maker's name as the maker writes it. |
| `aliases` | no | Other names (short names, former names). A maker hint matches the name or an alias, ignoring case and punctuation, either containing the other. |
| `class` | yes | What kind of part, dotted: `capacitor.ceramic`, `resistor.chip`, `diode.zener`. A requirement for `capacitor` selects `capacitor.ceramic`. |
| `title` | yes | One line for people. |
| `sources` | yes, ≥ 1 | The maker's documents the tables come from: `doc` (title and number), `url`, `fetched` (`YYYY-MM-DD`), optional `pages` and `note`. |
| `availability` | no | `open` (default): every combination exists unless an `[[excluded]]` row says otherwise. `closed`: only combinations an `[[available]]` row covers exist. |
| `min_stem` | no | A segment name: a supplier prefix search for this family should include at least the text through this segment. Default: through the first segment a term set. |
| `note` | no | Free text. |

## 2. `[implies]`: facts of every MPN

Parameters every part of the family has, keyed by parameter key: `technology = "mlcc"`, `aec_q200 = "yes"`. Parse adds them to every reading (with no segment); generate refuses the scheme when a term contradicts them. A key a segment sets cannot also be implied.

`[series]` is reserved for the series facts better_erc will read (LANG-6: leakage, derating, qualification); a file that uses it today is refused.

## 3. Segments: `[[segment]]`

The MPN is a sequence of segments, matched left to right. Each `[[segment]]` has a `name` (unique, used by rows, `when` and in parse output), optionally a `param` (the parameter key it sets, see §6), and exactly **one body**:

| body | example | matches |
|---|---|---|
| `literal = "CL"` | series letters, `-` | exactly this text |
| `table = { code = value, ... }` | `{ "05" = "0402" }` | one of the codes; with a `param`, each code means that value |
| `code = "<kind>"` | `code = "eia3"`, `unit = "pF"` | a number written in a code kind (§4) |
| `free = { chars = "A-Z0-9", len = 3 }` | spec and revision codes | characters that carry nothing we decode; `len` is a number or `{ min, max }` |
| `group = [ {...}, {...} ]` | | a sub-sequence of segments (inline tables, each with its own `name`) |
| `alternatives` | | any one of several bodies (§3.2) |

Matching is ASCII case-insensitive; generated text uses the scheme's spelling. Every segment may also carry `note`.

### 3.1 Tables

```toml
table = { C = { value = "reel_7in_paper", default = true }, D = "reel_13in_paper" }
```

An entry is `code = "value"` or `code = { value = "...", default = true, note = "..." }`. The value is read with the param's kind (§6): `"100nF"`, `"±10%"`, `"X7R"`. One entry may be the `default`: generate uses it when no term or preference decides the segment (the maker's standard reel). A table without a `param` only lists the codes that may appear; its values are descriptions and are not checked.

Codes may have different lengths (`"1"` and `"10"`); parse tries the longest first and backtracks.

### 3.2 Optional segments and alternatives

```toml
[[segment]]
name = "packaging"
param = "packaging"
optional = true
absent = "bulk"                 # the value when the segment is left out
table = { T = "reel_7in_paper" }
```

`optional = true` lets the segment be left out; `absent` is the value that means (needs a `param`). Generate writes the segment first and the shorter spelling after it.

```toml
[[segment]]
name = "value"
[[segment.alternatives]]
group = [
  { name = "tolerance", param = "tolerance", table = { J = "±5%" } },
  { name = "resistance", param = "resistance", code = "eia3", series = "E24" },
]
[[segment.alternatives]]
group = [
  { name = "tolerance", param = "tolerance", table = { F = "±1%" } },
  { name = "resistance", param = "resistance", code = "eia4", series = ["E24", "E96"] },
]
```

`alternatives` is a list of bodies; exactly one matches. A plain alternative (a table, a code...) takes the segment's `name` and `param`; a `group` alternative names its own segments. Segments of the same name must set the same param. The first alternative is the preferred spelling.

### 3.3 `when`: a meaning that depends on an earlier segment

```toml
[[segment]]
name = "thickness"
param = "height"
[[segment.alternatives]]
when = { size = "0402" }
table = { "5" = "0.5mm" }
[[segment.alternatives]]
when = { size = ["0603", "0805"] }
table = { "8" = "0.8mm", "A" = "0.65mm" }
```

`when` holds conditions on segments **before** this one, by segment name, written like an availability row's (§5): one value, a list, a range (`{ max = "9.99pF" }`) or the codes themselves (`{ code = ["1GN", "1GJ"] }`, when several codes share a value but not their meaning). The segment (or alternative) only applies when they hold. Use it when a maker reuses a code with a different meaning per case size or series:

```toml
[[segment.alternatives]]
when = { capacitance = { max = "9.99pF" } }    # Samsung: F is ±1 pF below 10 pF...
table = { F = "±1pF" }
[[segment.alternatives]]
when = { capacitance = { min = "10pF" } }      # ...and ±1 % from 10 pF
table = { F = "±1%" }
```

### 3.4 How the engine sees it

The loader expands optional parts and alternatives into flat **variants** (one way through the file each, at most 1024 per scheme). Parse tries every variant with backtracking; generate walks every variant's choices. Within one variant a name and a param appear once. Two adjacent segments that both have a variable length (a free segment with a `len` range, an `rkm`, `int` or `decimal` code without `digits`, a table with codes of different lengths) are refused: their split would be a guess. Put a fixed-length segment between them, or give one of them a fixed length.

## 4. Code kinds

A `code` segment reads a number written in a code kind, multiplies it by its `unit` (default: the param's base unit) and sets the param. Each kind reads and writes, so the same segment serves parse and generate.

| kind | text | examples | options |
|---|---|---|---|
| `eia3` | 2 significant digits + multiplier digit 0-7, or `R` as the decimal point; 3 characters | `104` = 10·10⁴, `1R5` = 1.5, `R47` = 0.47, `010` = 1 | `lead_zero = true` writes `0R5` instead of `R50` below 1; `pad_whole = true` writes whole values below 10 as `010` instead of `1R0` |
| `eia4` | 3 significant digits + multiplier, or `R`; 4 characters | `1002` = 10 000, `49R9`, `4R70` | `lead_zero`, `pad_whole` |
| `eia96` | E96 index `01`-`96` + multiplier letter (Z Y/R X/S A B/H C D E F) | `01C` = 10 000, `68X` = 49.9 | |
| `rkm` | IEC 60062: digits with a letter as decimal point and multiplier | `4K7`, `10K`, `R047`, `1M`, `0R`, `5V1` | `letters = { R = 0, K = 3, M = 6 }` (default for Ω: R K M G; for V: V); `digits` for a fixed length |
| `decimal` | digits with an implied point (`scale`) or with a `.` | `050` at `scale = -2` = 0.50, `5.0` | `scale`, `digits`, `places` (digits after the point when written) |
| `int` | an integer | `05` | `digits` (zero-padded fixed length) |

`unit` is the unit of the written number: Samsung's `104` is in `pF`, so `unit = "pF"` and it reads 100 nF. For `rkm` the letters carry the multiplier, so the unit is usually the base unit.

`series = "E24"` or `["E24", "E96"]` (E3, E6, E12, E24, E48, E96) says the values a maker makes. Parse still reads a value outside it but flags it (`outside_series`); generate only writes values in it, and it is what lets generate list the values of a range (`resistance = 1kΩ..2kΩ`). A coded segment without a series answers an equal term but not a range.

Codes a kind cannot express (a zero-ohm jumper written `000` or `0R00`) go in an alternative with a table.

## 5. Availability: `[[available]]` and `[[excluded]]`

Rows say which combinations exist, copied from the maker's selection table. A row is a set of conditions on segments, by segment name:

```toml
[[available]]
size = "0402"                                   # one value
dielectric = ["X7R", "X5R"]                     # a list
capacitance = { min = "10nF", max = "100nF" }   # a range (min, max or both)
voltage = ["6.3V", "10V", "16V"]
thickness = "0.5mm"
note = "catalog p. 12"
```

- Values are the segment's **param values** (`"0.5mm"`, not the code `"5"`); for a segment without a param they are codes. `{ code = "1GN" }` or `{ code = [...] }` tests the code of a segment with a param.
- A range may add `series = "E12"`: the value must also be in that series.
- A row's conditions all hold together; segments a row does not name are free.
- `closed` schemes: a combination exists when some `[[available]]` row holds. `open` schemes have no `[[available]]` rows.
- `[[excluded]]` rows (either mode): a combination is out when one holds.

Parse never rejects an MPN for availability: it reads it and flags it `not_in_table` (catalogues grow faster than schemes). Generate only yields combinations the rows allow, and a row can fix a segment no term set: the row above makes generate write thickness `5` for 0402 X7R 100 nF instead of leaving a hole.

There is deliberately no expression language. A datasheet's selection table is rows of values and ranges; anything rows cannot say is a sign the family should be two schemes.

## 6. Parameter keys and values

`param`, `[implies]` and example keys come from the crate's key list (`mpns list --keys`, `src/keys.rs`), which follows mx3's curated parameter names. A key has a kind:

| kind | keys | text form |
|---|---|---|
| quantity | `capacitance` (F), `resistance` (Ω), `inductance` (H), `rated_voltage`, `zener_voltage` (V), `power` (W), `rated_current` (A), `height` (m) | a number with an SI prefix and the unit: `100nF`, `4.7kΩ`, `6.3V`, `62.5mW`, `0.5mm`. `ohm` and `u` are read as `Ω` and `µ`. |
| tolerance | `tolerance`, `tcr` | `±10%`, `±0.25pF`, `+80%/-20%`, `±100ppm/K` |
| word | `dielectric`, `case_eia`, `case_metric`, `packaging`, `technology`, `aec_q200`, `series` | as written: `X7R`, `0402`, `reel_7in_paper` |

Values are exact decimals, so `0.1uF` and `100nF` are the same value. Every value has one text form, the one parse and generate output: engineering prefixes (`100nF`, `4.7kΩ`, `62.5mW`), lengths in mm, `Ω` and `µ` (U+00B5). A key that is missing is added to `src/keys.rs` in the same commit as the first scheme that needs it.

Packaging words: `reel_7in_paper`, `reel_7in_embossed`, `reel_13in_paper`, `reel_13in_embossed`, `bulk`, `bulk_case`, `tube`, `tray`. Add a word here before using a new one.

## 7. Examples: `[[example]]`

```toml
[[example]]
mpn = "CL05B104KO5NNNC"
params = { case_eia = "0402", dielectric = "X7R", capacitance = "100nF", tolerance = "±10%", rated_voltage = "16V", height = "0.5mm", packaging = "reel_7in_paper" }
source = "catalog p. 6"
```

Real MPNs only: from the maker's document or a supplier listing someone checked. `params` lists every parameter a segment decodes (implied ones may be left out). The loader checks each example both ways:

1. `parse(mpn)` reads it whole, and its best reading has exactly these parameters (no fewer, no more, no other reading with different parameters);
2. the value of a coded segment is in its series, and the combination is not outside the availability rows;
3. `generate` with these parameters as equal terms yields a pattern that matches the MPN.

A scheme without examples is refused.

## 8. What the validator refuses

Every error names the file, the line and the segment; all errors of a file are reported together. A file is refused when:

- the TOML does not parse, or has an unknown field;
- a header field is missing or malformed (id, version, sources with a URL and a `YYYY-MM-DD` date, availability);
- a segment has no body or two, an unknown `param`, a table value that is not of the param's kind, two defaults, an unknown code kind, a unit that is not the param's, an unknown series, code options on a non-code segment, a literal with a param, `absent` without `optional`;
- a name or a param appears twice in one variant, or segments of one name set different params;
- a `when` names a segment that does not come before it, or a value it cannot hold;
- two adjacent segments both have a variable length;
- a row names a segment that does not exist or a value its table cannot hold, a closed scheme has no `[[available]]` rows, or an open one has some;
- `[implies]` names an unknown key or one a segment sets; `[series]` is present;
- the optional parts and alternatives expand to more than 1024 variants;
- there are no examples, or an example fails its round trip (§7).

`mpns check [FILES|DIRS]` runs exactly this; `just test` runs it on every built-in scheme.

## 9. Parse and generate in short

**Parse** (`SchemeSet::parse(mpn, maker_hint)`): schemes whose leading literal starts the MPN are tried; each variant is matched with backtracking within a step limit. Complete readings come first: inside the availability rows, then the hinted maker, then fewer characters read by free segments. Only when no scheme reads the MPN whole, each scheme's longest partial reading is returned, with the text it could not read. Every reading carries the scheme id and version, each segment's span, code and value, the parameters and flags (`not_in_table`, `outside_series`, `absent`).

**Generate** (`SchemeSet::generate(requirement, prefs)`): for each scheme of the requirement's class (and makers, schemes), each segment's choices are filtered by the terms on its param and ordered closest to the bound first (`rated_voltage >= 16V`: 16 V, then 25 V...). A segment no term sets takes the preferred value (`Prefs::prefer`), else its table default (unless `fill_defaults` is off), else it becomes a **hole**. Combinations are walked best first and checked against `when` and the rows, which may fix holes. The result is patterns: fixed text and holes (`?` per character, `*` for a variable length, `[A|B]` for a few choices), each with its parameters, the terms it could not check (keys the scheme does not encode) and a `min_stem` for supplier prefix searches.
