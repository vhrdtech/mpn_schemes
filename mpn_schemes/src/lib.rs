//! Manufacturer part-number schemes as data.
//!
//! A scheme describes how one maker composes the part numbers of one family (Samsung CL, Murata GRM, Yageo RC, ...)
//! as a sequence of segments, each mapped to a parameter through a table, a numeric code kind, a literal or a free
//! field, with availability rows for the combinations that exist. The crate loads and validates scheme files, parses
//! an MPN into parameters and generates MPN patterns for wanted parameters, and tells two spellings of one MPN apart
//! from two parts ([spelling]). It knows nothing about databases or
//! suppliers; mx3 and better_erc build on it. The scheme language is described in `docs/language.md`.
//!
//! ```
//! use mpn_schemes::{Prefs, Requirement, SchemeSet};
//!
//! let set = SchemeSet::embedded().expect("the built-in schemes load");
//! let parses = set.parse("CL05B104KO5NNNC", None);
//! let best = &parses[0];
//! assert!(best.complete);
//! assert_eq!(best.get("capacitance").unwrap().to_string(), "100nF");
//!
//! let req = Requirement {
//!     class: Some("capacitor".into()),
//!     terms: vec![
//!         "capacitance=100nF".parse().unwrap(),
//!         "dielectric=X7R".parse().unwrap(),
//!         "case_eia=0402".parse().unwrap(),
//!         "rated_voltage>=16V".parse().unwrap(),
//!     ],
//!     ..Requirement::default()
//! };
//! let found = set.generate(&req, &Prefs::default());
//! assert!(found.candidates.iter().any(|c| c.pattern.matches("CL05B104KO5NNNC")));
//! ```

mod file;
mod generate;
mod parse;
mod pattern;
mod scheme;
mod set;

pub mod codes;
pub mod keys;
pub mod spelling;
pub mod value;

pub use generate::{Candidate, Generated, Op, Prefs, Requirement, Term};
pub use parse::{Flag, MAX_MPN_LEN, Param, Parse, SegmentMatch};
pub use pattern::{Hole, Part, Pattern};
pub use scheme::{CharSet, Error, Example, Scheme, Source};
pub use set::SchemeSet;
pub use value::{Dec, Kind, Quantity, Tolerance, Value};
