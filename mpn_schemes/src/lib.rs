//! Manufacturer part-number schemes as data.
//!
//! A scheme describes how one maker composes the part numbers of one family (Samsung CL, Murata GRM, Yageo RC, ...)
//! as a sequence of segments, each mapped to a parameter through a table, a numeric code kind, a literal or a free
//! field, with availability rows for the combinations that exist. The crate loads and validates scheme files, parses
//! an MPN into parameters and generates MPN patterns for wanted parameters. It knows nothing about databases or
//! suppliers; mx3 and better_erc build on it. The design is in `docs/` and mx3's `docs/mpn-schemes.md`.
