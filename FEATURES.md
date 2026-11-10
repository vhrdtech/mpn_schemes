# mpn_schemes features and roadmap

The single source of truth for what mpn_schemes does, which scheme families exist, what is broken and what is
planned. [CHANGELOG.md](CHANGELOG.md) records what changed and when.

Statuses: ✅ done, 🚧 in progress (the note says what is missing), 🐛 known bug, 📋 planned, 💡 idea, ⛔ blocked,
🔍 needs a check. IDs are stable: never renumber or reuse one; new ones come from `tpm work claim AREA`.

The design these items come from: mx3 `docs/mpn-schemes.md` (the overview on mx3's side) until `docs/` here holds
the language reference. Planned 10 Nov 2026 (tpm P2527#10nov-1405).

## Language (`LANG`)

Scheme files: header, segments, tables, availability, examples; the loader and validator.

## Code kinds (`CODE`)

Numeric value codes with a parser and a formatter each: eia3, eia4, eia96, rkm, decimal, int.

## Parse and generate (`ENG`)

Matching an MPN against the schemes; generating patterns for a requirement.

## Command line (`CLI`)

`mpns parse|gen|check|list`.

## Scheme families (`DATA`)

One item per family file, with its maker's source document.

## Dropped and superseded
