//! The parameter keys a scheme may set (LANG-5), with the kind of value each holds.
//!
//! The names follow mx3's curated parameter vocabulary, so a decoded value lands on the same definition there.
//! A key that is missing here is added in the same commit as the first scheme that needs it.

use crate::value::Kind;

/// One parameter key
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub name: &'static str,
    pub kind: Kind,
    pub doc: &'static str,
}

const fn q(name: &'static str, unit: &'static str, doc: &'static str) -> Key {
    Key {
        name,
        kind: Kind::Quantity(unit),
        doc,
    }
}

const fn e(name: &'static str, doc: &'static str) -> Key {
    Key {
        name,
        kind: Kind::Enum,
        doc,
    }
}

/// Every key a scheme may name in `param`, `[implies]` and examples
pub const KEYS: &[Key] = &[
    q("capacitance", "F", "capacitance"),
    q("resistance", "Ω", "resistance"),
    q("inductance", "H", "inductance"),
    q("rated_voltage", "V", "rated (working) voltage"),
    q("zener_voltage", "V", "zener or breakdown voltage"),
    q("power", "W", "rated power"),
    q("rated_current", "A", "rated current"),
    q("height", "m", "height or thickness of the body"),
    Key {
        name: "tolerance",
        kind: Kind::Tolerance,
        doc: "tolerance of the main value: ±10%, ±0.25pF",
    },
    Key {
        name: "tcr",
        kind: Kind::Tolerance,
        doc: "temperature coefficient: ±100ppm/K",
    },
    e(
        "dielectric",
        "ceramic dielectric (temperature characteristic): C0G, X5R, X7R",
    ),
    e("case_eia", "case size, EIA inch code: 0402, 0603"),
    e("case_metric", "case size, metric code: 1005, 1608"),
    e(
        "packaging",
        "packaging: reel_7in_paper, reel_7in_embossed, reel_13in_paper, bulk",
    ),
    e("technology", "construction: mlcc, thick_film, thin_film"),
    e("aec_q200", "qualified to AEC-Q200: yes or no"),
    e("series", "the maker's series name: CL, RC, ERJ-3G"),
];

/// The key of this name
pub fn key(name: &str) -> Option<&'static Key> {
    KEYS.iter().find(|k| k.name == name)
}
