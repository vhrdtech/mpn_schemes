# mpn_schemes

Manufacturer part-number schemes as data. `CL05B104KO5NNNC` is a Samsung MLCC: 0402, X7R, 100 nF, ±10 %, 16 V, on
a 7" paper reel. This crate knows that because a TOML file describes how Samsung composes CL part numbers, segment
by segment, and it can go the other way: from "100 nF X7R 0402 at least 16 V" to the part numbers (or patterns, where
a maker's number holds codes no parameter decides) each maker would use.

- **Parse** an MPN into parameters, with every possible reading ranked and partial matches marked.
- **Generate** MPN patterns for wanted parameters, best first, without enumerating impossible combinations.
- **Validate** scheme files, and round-trip every family's real examples in the tests.

Status: being built. See [FEATURES.md](FEATURES.md) for what works and [docs/](docs/) for the scheme language.

Used by mx3 (the lab's parts database) and better_erc (schematic checks).

## License

MIT, see [LICENSE](LICENSE).
