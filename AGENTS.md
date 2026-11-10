# Working on mpn_schemes

Guidance for AI agents and contributors. Read this before changing code or scheme files.

mpn_schemes describes, as data, how manufacturers compose their part numbers (MPNs): one TOML file per part family
(Samsung CL, Murata GRM, Yageo RC, Panasonic ERJ, Nexperia BZX84, ...). The library loads and validates the files,
parses an MPN into parameters (value, tolerance, case, voltage, dielectric, packaging, ...) and generates MPN
patterns for wanted parameters. It knows nothing about databases or suppliers. Two internal tools build on it: mx3
(the lab's parts database: decoded parameters as facts, BOM checks, crafting MPNs and checking them at Digi-Key and
Mouser) and better_erc (schematic checks that need a part's values and its series). Published on crates.io once
the user says so; the scheme files ship inside the crate.

## FEATURES.md is the source of truth

[FEATURES.md](FEATURES.md) lists every feature and every scheme family with its status, every known bug, and what
is planned, with stable IDs per area (`LANG-3`, `CODE-2`, `DATA-5`, ...).

- **Read the relevant area before starting.**
- **Name IDs with a short slug when talking to the user**: `LANG-2 availability-rows`, never a bare `LANG-2`.
  Commit messages, CHANGELOG and code `TODO`s keep the bare ID.
- **Update it in the same commit** as the code or data: mark items ✅/🚧/🐛 with a pointer, add bugs you find but
  don't fix (the next free ID comes from `tpm work claim AREA` while other sessions run), move obsolete items to
  *Dropped and superseded*. Never renumber or reuse IDs.
- Don't track status anywhere else. Code `TODO`s that matter reference an ID: `// TODO(LANG-4): ...`.

## CHANGELOG.md records every change

- Every change a user of the crate would notice gets an entry under `## [Unreleased]` in the same commit:
  `### Breaking changes`, `### Added`, `### Changed`, `### Fixed`, `### Removed`, with the ID in parentheses.
- Always record: a changed scheme language (a file that loaded before and no longer does, or means something else),
  a scheme whose `version` was raised and what changed in its meaning, a new family, changed cargo features.
- Work commits never touch the version; `tpm land` writes it.

## Scheme files are data that real decisions depend on

- A scheme's `version` is raised on every change of meaning (a table entry changed or removed, a segment split, an
  availability row narrowed). Adding examples or sources does not raise it. mx3 records the id and version with each
  decoded value and re-decodes on a raise.
- Every table, code and availability row comes from the maker's own ordering-code or selection document, named in
  the file's `sources` with URL and fetch date. Fetch documents through the library (`tpm lib search`, `tpm lib
  fetch`), never into a scratch folder; vendor sites that block plain clients go through `tpm fetch`.
- Every family carries real examples (MPN and the parameters it means). An example nobody verified against a
  document or a supplier listing does not go in.
- Never adjust an example or a corpus expectation to make a test pass; find out which side is wrong.
- Vendor documents themselves never go into this repo (public): only the facts copied from them, with the source.

## Layout

Cargo workspace, edition 2024, one shared version.

- `mpn_schemes/` — the library: language types, loader, validator, code kinds, parse, generate.
- `mpn_schemes_cli/` — the `mpns` binary: parse, gen, check, list.
- `schemes/<maker>/<family>.toml` — the scheme files, embedded in the library.
- `docs/` — the scheme language and how to write a scheme from a datasheet.

## Commands

```sh
just test            # every test (nextest, doctests); `just test --changed` before marking work ready
just lint            # clippy -D warnings, fmt --check
just fmt
just install         # build in the deploy profile and install mpns through tpm install
MPN_CORPUS_DIR=~/sync/mx3-corpus just test   # also the corpus round-trip on real parts exported by mx3 (mx3 MPN-2)
```

Before declaring a change done: build, clippy without warnings, fmt, tests.

## Code conventions

- Untrusted input (an MPN from a BOM or a supplier) never panics and does bounded work: the matcher has a step
  limit and lengths are checked before indexing.
- No maker, series or code table in Rust. What is code: the code kinds (formats like `eia3`, `rkm`) and the engine.
- Errors name the file, the segment and the line; the validator collects all errors of a file, not just the first.
- Dependencies with `default-features = false` and only the features in use; pure Rust, no `-sys` crates. Keep
  `cargo tree -d` clean and check new dependencies with `cargo machete`.
- The CLI follows the CLI UX guide, `~/git/vhrd_cli/docs/ux-guide.md`
  (https://github.com/vhrdtech/vhrd_cli/blob/main/docs/ux-guide.md): help, completion, `--json`, colour, exit codes,
  `--version` with the git SHA and build time (`build_version.rs`).

## Tests

- Every scheme's examples both ways: `parse(mpn)` gives exactly the parameters, `generate(parameters)` yields the
  MPN or a pattern that matches it.
- The validator: one broken fixture per refusal.
- Code kinds against tables of known values (E24, E96, IEC 60062).
- The corpus test reads JSON lines `{maker, mpn, params: {key: text}}` from `MPN_CORPUS_DIR` when set and reports per
  scheme how many parsed, agreed and disagreed; without the variable it says it skipped. Lab data never goes into
  this repo.

## Commits

Conventional Commits with a scope: `feat(lang): ...`, `fix(parse): ...`, `data(murata): ...`, `feat(cli): ...`,
`test: ...`, `docs: ...`. Short imperative summary, blank line, body with what and why, feature IDs referenced.
Work happens on a session branch (`tpm work new SLUG`); `tpm land` puts it on main.
