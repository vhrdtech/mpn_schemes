//! Parameter values: exact decimals with a unit, tolerances and enumerated text.
//!
//! Every value has one stable text form (`Display`), which is also what [`Value::parse`] reads back: mx3 maps
//! these texts into its own value type through its parsers, so the form must not change without a CHANGELOG entry.

use std::cmp::Ordering;
use std::fmt;

/// An exact decimal number: `mantissa · 10^exp`, normalized so the mantissa has no trailing zeros
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dec {
    mantissa: i64,
    exp: i32,
}

impl Dec {
    /// The number `mantissa · 10^exp`
    pub fn new(mantissa: i64, exp: i32) -> Dec {
        let (mut m, mut e) = (mantissa, exp);
        if m == 0 {
            return Dec {
                mantissa: 0,
                exp: 0,
            };
        }
        while m % 10 == 0 {
            m /= 10;
            e += 1;
        }
        Dec {
            mantissa: m,
            exp: e,
        }
    }

    pub const ZERO: Dec = Dec {
        mantissa: 0,
        exp: 0,
    };

    pub fn mantissa(self) -> i64 {
        self.mantissa
    }

    pub fn exp(self) -> i32 {
        self.exp
    }

    pub fn is_zero(self) -> bool {
        self.mantissa == 0
    }

    /// Multiply by a power of ten
    pub fn scale(self, by: i32) -> Dec {
        Dec::new(self.mantissa, self.exp.saturating_add(by))
    }

    /// Parse `12`, `4.7`, `0.025`, `-3`; at most 18 significant digits
    pub fn parse(text: &str) -> Option<Dec> {
        let (neg, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
        if int.is_empty() && frac.is_empty() {
            return None;
        }
        if !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
            return None;
        }
        let all: String = format!("{int}{frac}");
        let all = all.trim_start_matches('0');
        if all.len() > 18 {
            return None;
        }
        let m: i64 = if all.is_empty() { 0 } else { all.parse().ok()? };
        let exp = -(frac.len() as i32);
        Some(Dec::new(if neg { -m } else { m }, exp))
    }

    /// Number of significant digits of the mantissa
    pub fn digits(self) -> u32 {
        if self.mantissa == 0 {
            1
        } else {
            self.mantissa.unsigned_abs().ilog10() + 1
        }
    }

    /// The power of ten of the leading digit: 470 → 2, 0.047 → -2
    pub fn magnitude(self) -> i32 {
        self.exp + self.digits() as i32 - 1
    }

    /// An approximation for tolerance comparisons only
    pub fn to_f64(self) -> f64 {
        self.mantissa as f64 * 10f64.powi(self.exp)
    }

    /// The value as an integer when it is one and fits
    pub fn to_i64(self) -> Option<i64> {
        if self.exp < 0 {
            return None;
        }
        self.mantissa
            .checked_mul(10i64.checked_pow(self.exp as u32)?)
    }
}

impl Ord for Dec {
    fn cmp(&self, other: &Self) -> Ordering {
        let (a, b) = (self.mantissa, other.mantissa);
        if (a < 0) != (b < 0) || a == 0 || b == 0 {
            return a.cmp(&b);
        }
        // Same sign, both non-zero: compare magnitudes first, then the aligned mantissas
        let (ma, mb) = (self.magnitude(), other.magnitude());
        let pos = a > 0;
        if ma != mb {
            return if pos { ma.cmp(&mb) } else { mb.cmp(&ma) };
        }
        let e = self.exp.min(other.exp);
        let sa = i128::from(a) * 10i128.pow((self.exp - e) as u32);
        let sb = i128::from(b) * 10i128.pow((other.exp - e) as u32);
        sa.cmp(&sb)
    }
}

impl PartialOrd for Dec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.mantissa < 0;
        let digits = self.mantissa.unsigned_abs().to_string();
        let sign = if neg { "-" } else { "" };
        if self.exp >= 0 {
            write!(f, "{sign}{digits}{}", "0".repeat(self.exp as usize))
        } else {
            let point = digits.len() as i32 + self.exp;
            if point > 0 {
                let (i, fr) = digits.split_at(point as usize);
                write!(f, "{sign}{i}.{fr}")
            } else {
                write!(f, "{sign}0.{}{digits}", "0".repeat((-point) as usize))
            }
        }
    }
}

/// SI prefixes a quantity may be written with, and their power of ten
const PREFIXES: &[(&str, i32)] = &[
    ("p", -12),
    ("n", -9),
    ("µ", -6),
    ("μ", -6),
    ("u", -6),
    ("m", -3),
    ("k", 3),
    ("K", 3),
    ("M", 6),
    ("G", 9),
];

/// The base units a parameter can have. Lengths are shown in mm, everything else with an engineering prefix.
pub const UNITS: &[&str] = &["F", "Ω", "H", "V", "W", "A", "m", "Hz", "%", "ppm/K"];

/// Split `pF` into (-12, "F"), `Ω` into (0, "Ω"); `ohm` is read as `Ω`
pub fn split_unit(text: &str) -> Option<(i32, &'static str)> {
    let norm = |u: &str| -> Option<&'static str> {
        match u {
            "ohm" | "Ohm" | "OHM" | "Ω" | "\u{2126}" => Some("Ω"),
            "R" => None,
            _ => UNITS.iter().find(|x| **x == u).copied(),
        }
    };
    if let Some(u) = norm(text) {
        return Some((0, u));
    }
    for (p, e) in PREFIXES {
        if let Some(rest) = text.strip_prefix(p)
            && let Some(u) = norm(rest)
            && u != "%"
            && u != "ppm/K"
        {
            return Some((*e, u));
        }
    }
    None
}

/// A number with a base unit: 100 nF is `Quantity { value: 1·10⁻⁷, unit: "F" }`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Quantity {
    pub value: Dec,
    pub unit: &'static str,
}

impl Quantity {
    /// Parse `100nF`, `4.7 kΩ`, `16V`, `0.5mm`, `10ohm`; the unit must be `want` when given
    pub fn parse(text: &str, want: Option<&str>) -> Option<Quantity> {
        let t = text.trim();
        let end = t
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
            .unwrap_or(t.len());
        let (num, unit) = t.split_at(end);
        let value = Dec::parse(num)?;
        let (exp, unit) = split_unit(unit.trim())?;
        if want.is_some_and(|w| w != unit) {
            return None;
        }
        Some(Quantity {
            value: value.scale(exp),
            unit,
        })
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value.is_zero() {
            return write!(f, "0{}", self.unit);
        }
        if self.unit == "m" {
            return write!(f, "{}mm", self.value.scale(3));
        }
        if self.unit == "%" || self.unit == "ppm/K" {
            return write!(f, "{}{}", self.value, self.unit);
        }
        let mag = self.value.magnitude();
        let e3 = (mag.div_euclid(3) * 3).clamp(-12, 9);
        let prefix = match e3 {
            -12 => "p",
            -9 => "n",
            -6 => "µ",
            -3 => "m",
            0 => "",
            3 => "k",
            6 => "M",
            9 => "G",
            _ => return write!(f, "{}{}", self.value, self.unit),
        };
        write!(f, "{}{prefix}{}", self.value.scale(-e3), self.unit)
    }
}

/// A tolerance: `±10%`, `±0.25pF`, `+80%/-20%`. `minus` is stored as a positive number.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tolerance {
    pub plus: Dec,
    pub minus: Dec,
    /// `%`, `ppm/K` or the base unit of an absolute tolerance (`F` for ±0.25 pF)
    pub unit: &'static str,
}

impl Tolerance {
    pub fn parse(text: &str) -> Option<Tolerance> {
        let t = text.trim();
        let sym = ["±", "+/-", "+-"]
            .iter()
            .find_map(|p| t.strip_prefix(p))
            .map(str::trim);
        if let Some(rest) = sym {
            let (v, unit) = Self::amount(rest)?;
            return Some(Tolerance {
                plus: v,
                minus: v,
                unit,
            });
        }
        // +80%/-20%
        let (p, m) = t.split_once('/')?;
        let (plus, u1) = Self::amount(p.trim().strip_prefix('+')?)?;
        let (minus, u2) = Self::amount(m.trim().strip_prefix('-')?)?;
        (u1 == u2).then_some(Tolerance {
            plus,
            minus,
            unit: u1,
        })
    }

    fn amount(text: &str) -> Option<(Dec, &'static str)> {
        if let Some(n) = text.strip_suffix('%') {
            return Some((Dec::parse(n.trim())?, "%"));
        }
        if let Some(n) = text.strip_suffix("ppm/K") {
            return Some((Dec::parse(n.trim())?, "ppm/K"));
        }
        let q = Quantity::parse(text, None)?;
        Some((q.value, q.unit))
    }

    fn fmt_amount(&self, v: Dec, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.unit {
            "%" | "ppm/K" => write!(f, "{v}{}", self.unit),
            u => write!(f, "{}", Quantity { value: v, unit: u }),
        }
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.plus == self.minus {
            f.write_str("±")?;
            self.fmt_amount(self.plus, f)
        } else {
            f.write_str("+")?;
            self.fmt_amount(self.plus, f)?;
            f.write_str("/-")?;
            self.fmt_amount(self.minus, f)
        }
    }
}

/// The kind of value a parameter key holds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A number with this base unit
    Quantity(&'static str),
    /// A tolerance, relative or absolute
    Tolerance,
    /// A word from a set (`X7R`, `0402`, `reel_7in_paper`, `yes`)
    Enum,
}

/// A parameter value
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Quantity(Quantity),
    Tolerance(Tolerance),
    Text(String),
}

impl Value {
    /// Read a value of the given kind from its text
    pub fn parse(kind: Kind, text: &str) -> Option<Value> {
        match kind {
            Kind::Quantity(unit) => Quantity::parse(text, Some(unit)).map(Value::Quantity),
            Kind::Tolerance => Tolerance::parse(text).map(Value::Tolerance),
            Kind::Enum => {
                let t = text.trim();
                (!t.is_empty()).then(|| Value::Text(t.to_string()))
            }
        }
    }

    /// The number of a quantity
    pub fn number(&self) -> Option<Dec> {
        match self {
            Value::Quantity(q) => Some(q.value),
            _ => None,
        }
    }

    /// Equal, or for quantities within a relative tolerance (0.01 = 1 %)
    pub fn matches(&self, other: &Value, rel_tol: Option<f64>) -> bool {
        match (self, other, rel_tol) {
            (Value::Quantity(a), Value::Quantity(b), Some(t)) if a.unit == b.unit => {
                let (x, y) = (a.value.to_f64(), b.value.to_f64());
                (x - y).abs() <= t * y.abs()
            }
            (Value::Text(a), Value::Text(b), _) => a.eq_ignore_ascii_case(b),
            _ => self == other,
        }
    }

    /// Order of two values of the same kind: quantities by number, tolerances by their larger side
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        match (self, other) {
            (Value::Quantity(a), Value::Quantity(b)) if a.unit == b.unit => {
                Some(a.value.cmp(&b.value))
            }
            (Value::Tolerance(a), Value::Tolerance(b)) if a.unit == b.unit => {
                Some(a.plus.max(a.minus).cmp(&b.plus.max(b.minus)))
            }
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Quantity(q) => q.fmt(f),
            Value::Tolerance(t) => t.fmt(f),
            Value::Text(s) => f.write_str(s),
        }
    }
}

/// Values are written as their text form, the one [`Value::parse`] reads
impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(t: &str) -> String {
        Quantity::parse(t, None).unwrap().to_string()
    }

    #[test]
    fn dec_parse_and_show() {
        assert_eq!(Dec::parse("4.70").unwrap(), Dec::new(47, -1));
        assert_eq!(Dec::parse("0.025").unwrap().to_string(), "0.025");
        assert_eq!(Dec::parse("1200").unwrap().to_string(), "1200");
        assert_eq!(Dec::parse("0").unwrap(), Dec::ZERO);
        assert!(Dec::parse("1e3").is_none());
        assert!(Dec::parse(".").is_none());
        assert!(Dec::new(47, -1) < Dec::new(5, 0));
        assert!(Dec::new(-47, -1) < Dec::new(-4, 0));
        assert!(Dec::new(1, 3) > Dec::new(999, 0));
    }

    #[test]
    fn quantities_round_trip() {
        assert_eq!(q("100nF"), "100nF");
        assert_eq!(q("0.1uF"), "100nF");
        assert_eq!(q("4700Ω"), "4.7kΩ");
        assert_eq!(q("4.7 kohm"), "4.7kΩ");
        assert_eq!(q("0.5mm"), "0.5mm");
        assert_eq!(q("1.25mm"), "1.25mm");
        assert_eq!(q("6.3V"), "6.3V");
        assert_eq!(q("0.0625W"), "62.5mW");
        assert_eq!(q("0Ω"), "0Ω");
        assert_eq!(q("10MΩ"), "10MΩ");
        assert_eq!(q("0.047Ω"), "47mΩ");
        assert!(Quantity::parse("16V", Some("F")).is_none());
    }

    #[test]
    fn tolerances() {
        let t = |s: &str| Tolerance::parse(s).unwrap().to_string();
        assert_eq!(t("±10%"), "±10%");
        assert_eq!(t("+/-0.25pF"), "±0.25pF");
        assert_eq!(t("+80%/-20%"), "+80%/-20%");
        assert_eq!(t("±100ppm/K"), "±100ppm/K");
        assert!(Tolerance::parse("10%").is_none());
    }

    #[test]
    fn relative_match() {
        let a = Value::parse(Kind::Quantity("Ω"), "10k").is_none();
        assert!(a, "a bare prefix is not a unit");
        let a = Value::parse(Kind::Quantity("Ω"), "10kΩ").unwrap();
        let b = Value::parse(Kind::Quantity("Ω"), "10.05kΩ").unwrap();
        assert!(a.matches(&b, Some(0.01)));
        assert!(!a.matches(&b, None));
    }
}
