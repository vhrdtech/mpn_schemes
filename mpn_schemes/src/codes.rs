//! Code kinds (CODE-1): how a number is written in a few characters of an MPN, each with a reader and a writer,
//! and the E series (CODE-2) a value may have to belong to.
//!
//! A code kind is a format, never a maker's table: `eia3` is the same rule at Samsung and at Panasonic. The number
//! a code reads is in the segment's unit (`pF` for Samsung's `104`), the engine turns it into a base-unit value.

use crate::value::Dec;
use std::fmt;

/// The code kinds a `code` segment can name
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodeKind {
    /// Two significant digits and a multiplier digit 0..7 (`104` = 10·10⁴), or `R` as the decimal point (`1R5`,
    /// `R47`); always three characters
    Eia3,
    /// Three significant digits and a multiplier digit 0..7 (`1002` = 100·10²), or `R` (`49R9`, `4R70`); four
    /// characters
    Eia4,
    /// EIA-96: two digits indexing E96 and a multiplier letter (`01C` = 100·100 = 10 k)
    Eia96,
    /// IEC 60062 RKM: a letter as the decimal point and multiplier (`4K7`, `10K`, `R047`, `1M`, `0R`, `5V1`)
    Rkm,
    /// A decimal number: with `scale`, digits with an implied point (`050` at scale -2 = 0.50); without, digits
    /// with an optional point (`5.0`), written with `places` digits after the point
    Decimal,
    /// An integer, zero-padded to `digits` when given
    Int,
}

impl CodeKind {
    pub const ALL: &[(&str, CodeKind)] = &[
        ("eia3", CodeKind::Eia3),
        ("eia4", CodeKind::Eia4),
        ("eia96", CodeKind::Eia96),
        ("rkm", CodeKind::Rkm),
        ("decimal", CodeKind::Decimal),
        ("int", CodeKind::Int),
    ];

    pub fn from_name(name: &str) -> Option<CodeKind> {
        Self::ALL.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, k)| *k == self)
            .map(|(n, _)| *n)
            .unwrap_or("?")
    }
}

/// The longest code a variable-length kind reads
const MAX_VARIABLE: usize = 12;

/// A code kind with the options of its segment
#[derive(Clone, Debug)]
pub struct Code {
    pub kind: CodeKind,
    /// Exact length in characters (`int`, `decimal`, `rkm`); none: variable
    pub digits: Option<usize>,
    /// `decimal`: the implied power of ten of the last digit
    pub scale: Option<i32>,
    /// `decimal` without scale: digits after the point when written
    pub places: usize,
    /// `rkm`: the letters and the power of ten each stands for
    pub letters: Vec<(char, i32)>,
    /// `eia3`/`eia4` below 1: write `0R5` rather than `R50` (Samsung does, Murata does not)
    pub lead_zero: bool,
}

impl Code {
    pub fn new(kind: CodeKind) -> Code {
        Code {
            kind,
            digits: None,
            scale: None,
            places: 0,
            letters: Vec::new(),
            lead_zero: false,
        }
    }

    /// Every way a code of this kind can start `text`: (length, number), shortest first
    pub fn read(&self, text: &str) -> Vec<(usize, Dec)> {
        let b = text.as_bytes();
        let mut out = Vec::new();
        match self.kind {
            CodeKind::Eia3 | CodeKind::Eia4 => {
                let n = if self.kind == CodeKind::Eia3 { 3 } else { 4 };
                if let Some(v) = b.get(..n).and_then(read_eia) {
                    out.push((n, v));
                }
            }
            CodeKind::Eia96 => {
                if let Some(v) = b.get(..3).and_then(read_eia96) {
                    out.push((3, v));
                }
            }
            CodeKind::Rkm => {
                for len in 2..=b.len().min(MAX_VARIABLE) {
                    if self.digits.is_some_and(|d| d != len) {
                        continue;
                    }
                    if let Some(v) = self.read_rkm(&b[..len]) {
                        out.push((len, v));
                    }
                }
            }
            CodeKind::Int => {
                for len in 1..=b.len().min(MAX_VARIABLE) {
                    if !b[len - 1].is_ascii_digit() {
                        break;
                    }
                    if self.digits.is_none_or(|d| d == len) {
                        let s = std::str::from_utf8(&b[..len]).unwrap_or("x");
                        if let Some(v) = Dec::parse(s) {
                            out.push((len, v));
                        }
                    }
                }
            }
            CodeKind::Decimal => {
                let mut dot = false;
                for len in 1..=b.len().min(MAX_VARIABLE) {
                    let c = b[len - 1];
                    if c == b'.' && self.scale.is_none() && !dot && len > 1 {
                        dot = true;
                        continue;
                    }
                    if !c.is_ascii_digit() {
                        break;
                    }
                    if self.digits.is_some_and(|d| d != len) {
                        continue;
                    }
                    let s = std::str::from_utf8(&b[..len]).unwrap_or("x");
                    if let Some(v) = Dec::parse(s) {
                        out.push((len, v.scale(self.scale.unwrap_or(0))));
                    }
                }
            }
        }
        out
    }

    /// The code for a number, if this kind can write it
    pub fn write(&self, v: Dec) -> Option<String> {
        if v.mantissa() < 0 {
            return None;
        }
        let s = match self.kind {
            CodeKind::Eia3 => write_eia(v, 2, self.lead_zero)?,
            CodeKind::Eia4 => write_eia(v, 3, self.lead_zero)?,
            CodeKind::Eia96 => write_eia96(v)?,
            CodeKind::Rkm => self.write_rkm(v)?,
            CodeKind::Int => {
                let i = v.to_i64()?;
                format!("{i:0w$}", w = self.digits.unwrap_or(0))
            }
            CodeKind::Decimal => match self.scale {
                Some(sc) => {
                    let i = v.scale(-sc).to_i64()?;
                    format!("{i:0w$}", w = self.digits.unwrap_or(0))
                }
                None => {
                    if -v.exp() > self.places as i32 {
                        return None;
                    }
                    let i = v.scale(self.places as i32).to_i64()?;
                    let t = format!("{i:0w$}", w = self.places + 1);
                    if self.places == 0 {
                        t
                    } else {
                        let (a, b) = t.split_at(t.len() - self.places);
                        format!("{a}.{b}")
                    }
                }
            },
        };
        // What is written must read back as the same number
        let back = self
            .read(&s)
            .into_iter()
            .any(|(l, x)| l == s.len() && x == v);
        back.then_some(s)
    }

    fn read_rkm(&self, b: &[u8]) -> Option<Dec> {
        let pos = b.iter().position(|c| !c.is_ascii_digit())?;
        let letter = b[pos].to_ascii_uppercase() as char;
        let exp = self.letters.iter().find(|(c, _)| *c == letter)?.1;
        let (int, frac) = (&b[..pos], &b[pos + 1..]);
        if int.is_empty() && frac.is_empty() || !frac.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let s = format!(
            "{}.{}",
            std::str::from_utf8(int).ok()?,
            std::str::from_utf8(frac).ok()?
        );
        let s = if int.is_empty() { format!("0{s}") } else { s };
        let s = s.trim_end_matches('.');
        Some(Dec::parse(s)?.scale(exp))
    }

    fn write_rkm(&self, v: Dec) -> Option<String> {
        let mut letters = self.letters.clone();
        letters.sort_by_key(|(_, e)| *e);
        let (letter, exp) = if v.is_zero() {
            *letters.iter().find(|(_, e)| *e == 0).or(letters.first())?
        } else {
            *letters
                .iter()
                .rev()
                .find(|(_, e)| v.magnitude() >= *e)
                .or(letters.first())?
        };
        let m = v.scale(-exp).to_string();
        let (int, frac) = m.split_once('.').unwrap_or((&m, ""));
        let int = if int == "0" && !frac.is_empty() {
            ""
        } else {
            int
        };
        let mut s = format!("{int}{letter}{frac}");
        if let Some(d) = self.digits {
            while s.len() < d {
                s.push('0');
            }
        }
        Some(s)
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.kind.name())
    }
}

/// `104`, `1R5`, `1002`, `49R9`: significant digits and a multiplier digit, or `R` as the point
fn read_eia(c: &[u8]) -> Option<Dec> {
    let n = c.len();
    let rs = c.iter().filter(|&&x| x == b'R' || x == b'r').count();
    if rs == 1 {
        let pos = c.iter().position(|&x| x == b'R' || x == b'r')?;
        if !c
            .iter()
            .enumerate()
            .all(|(i, x)| i == pos || x.is_ascii_digit())
        {
            return None;
        }
        let s = format!(
            "{}.{}",
            std::str::from_utf8(&c[..pos]).ok()?,
            std::str::from_utf8(&c[pos + 1..]).ok()?
        );
        let s = if pos == 0 { format!("0{s}") } else { s };
        return Dec::parse(s.trim_end_matches('.'));
    }
    if rs > 1 || !c.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let sig: i64 = std::str::from_utf8(&c[..n - 1]).ok()?.parse().ok()?;
    let mult = (c[n - 1] - b'0') as i32;
    if mult > 7 || sig == 0 {
        return None;
    }
    Some(Dec::new(sig, mult))
}

/// Write with `sig` significant digits (2 for eia3, 3 for eia4)
fn write_eia(v: Dec, sig: u32, lead_zero: bool) -> Option<String> {
    if v.is_zero() || v.digits() > sig {
        return None;
    }
    let len = sig as i32 + 1;
    let mag = v.magnitude();
    if mag >= sig as i32 - 1 {
        // digits and a multiplier
        let mult = mag - (sig as i32 - 1);
        if mult > 7 {
            return None;
        }
        let n = v.scale(-mult).to_i64()?;
        return Some(format!("{n}{mult}"));
    }
    // R as the point: `sig` digits in all
    let frac = if mag >= 0 {
        sig as i32 - (mag + 1)
    } else if lead_zero && -v.exp() < sig as i32 {
        sig as i32 - 1
    } else {
        sig as i32
    };
    let n = v.scale(frac).to_i64()?;
    let digits = format!("{n:0w$}", w = sig as usize);
    if digits.len() != sig as usize {
        return None;
    }
    let (i, fr) = digits.split_at(sig as usize - frac as usize);
    let s = format!("{i}R{fr}");
    (s.len() as i32 == len).then_some(s)
}

const EIA96_MULT: &[(u8, i32)] = &[
    (b'Z', -3),
    (b'Y', -2),
    (b'R', -2),
    (b'X', -1),
    (b'S', -1),
    (b'A', 0),
    (b'B', 1),
    (b'H', 1),
    (b'C', 2),
    (b'D', 3),
    (b'E', 4),
    (b'F', 5),
];

fn read_eia96(c: &[u8]) -> Option<Dec> {
    if !c[..2].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let idx = ((c[0] - b'0') * 10 + (c[1] - b'0')) as usize;
    let base = *E96.get(idx.checked_sub(1)?)?;
    let exp = EIA96_MULT
        .iter()
        .find(|(l, _)| *l == c[2].to_ascii_uppercase())?
        .1;
    Some(Dec::new(i64::from(base), exp))
}

fn write_eia96(v: Dec) -> Option<String> {
    if v.is_zero() || v.digits() > 3 {
        return None;
    }
    let exp = v.magnitude() - 2;
    let n = v.scale(-exp).to_i64()?;
    let idx = E96.iter().position(|&x| i64::from(x) == n)? + 1;
    let letter = EIA96_MULT.iter().find(|(_, e)| *e == exp)?.0 as char;
    Some(format!("{idx:02}{letter}"))
}

/// The E series of IEC 60063 a value may be required to belong to
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ESeries {
    E3,
    E6,
    E12,
    E24,
    E48,
    E96,
}

const E24: [u16; 24] = [
    100, 110, 120, 130, 150, 160, 180, 200, 220, 240, 270, 300, 330, 360, 390, 430, 470, 510, 560,
    620, 680, 750, 820, 910,
];

const E96: [u16; 96] = [
    100, 102, 105, 107, 110, 113, 115, 118, 121, 124, 127, 130, 133, 137, 140, 143, 147, 150, 154,
    158, 162, 165, 169, 174, 178, 182, 187, 191, 196, 200, 205, 210, 215, 221, 226, 232, 237, 243,
    249, 255, 261, 267, 274, 280, 287, 294, 301, 309, 316, 324, 332, 340, 348, 357, 365, 374, 383,
    392, 402, 412, 422, 432, 442, 453, 464, 475, 487, 499, 511, 523, 536, 549, 562, 576, 590, 604,
    619, 634, 649, 665, 681, 698, 715, 732, 750, 768, 787, 806, 825, 845, 866, 887, 909, 931, 953,
    976,
];

impl ESeries {
    pub const ALL: &[(&str, ESeries)] = &[
        ("E3", ESeries::E3),
        ("E6", ESeries::E6),
        ("E12", ESeries::E12),
        ("E24", ESeries::E24),
        ("E48", ESeries::E48),
        ("E96", ESeries::E96),
    ];

    pub fn from_name(name: &str) -> Option<ESeries> {
        Self::ALL.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
    }

    pub fn name(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(_, s)| *s == self)
            .map(|(n, _)| *n)
            .unwrap_or("?")
    }

    /// The values of one decade, as three-digit numbers 100..999
    pub fn decade(self) -> Vec<u16> {
        let step = |src: &[u16], n: usize| src.iter().step_by(n).copied().collect();
        match self {
            ESeries::E3 => step(&E24, 8),
            ESeries::E6 => step(&E24, 4),
            ESeries::E12 => step(&E24, 2),
            ESeries::E24 => E24.to_vec(),
            ESeries::E48 => step(&E96, 2),
            ESeries::E96 => E96.to_vec(),
        }
    }

    /// Whether the value is in the series (any decade)
    pub fn contains(self, v: Dec) -> bool {
        if v.is_zero() || v.mantissa() < 0 || v.digits() > 3 {
            return false;
        }
        let n = v.scale(2 - v.magnitude()).to_i64();
        n.is_some_and(|n| self.decade().iter().any(|&x| i64::from(x) == n))
    }

    /// The values of the series from `min` to `max`, ascending
    pub fn between(self, min: Dec, max: Dec) -> Vec<Dec> {
        let mut out = Vec::new();
        if max < min || max.mantissa() <= 0 {
            return out;
        }
        let lo = if min.mantissa() > 0 {
            min.magnitude()
        } else {
            max.magnitude() - 12
        };
        for decade in lo..=max.magnitude() {
            for &x in &self.decade() {
                let v = Dec::new(i64::from(x), decade - 2);
                if v >= min && v <= max {
                    out.push(v);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::parse(s).unwrap()
    }

    fn rkm(letters: &[(char, i32)]) -> Code {
        Code {
            letters: letters.to_vec(),
            ..Code::new(CodeKind::Rkm)
        }
    }

    const OHMS: &[(char, i32)] = &[('R', 0), ('K', 3), ('M', 6), ('G', 9)];

    #[test]
    fn eia3_both_ways() {
        let c = Code::new(CodeKind::Eia3);
        for (code, v) in [
            ("104", "100000"),
            ("1R5", "1.5"),
            ("R47", "0.47"),
            ("R50", "0.5"),
            ("100", "10"),
            ("479", ""),
            ("475", "4700000"),
            ("220", "22"),
            ("8R2", "8.2"),
        ] {
            let got = c.read(code);
            if v.is_empty() {
                assert!(got.is_empty(), "{code}");
                continue;
            }
            assert_eq!(got, vec![(3, d(v))], "{code}");
            assert_eq!(c.write(d(v)).as_deref(), Some(code), "{v}");
        }
        assert_eq!(c.write(d("1.25")), None);
        let z = Code {
            lead_zero: true,
            ..Code::new(CodeKind::Eia3)
        };
        assert_eq!(z.write(d("0.5")).as_deref(), Some("0R5"));
        assert_eq!(z.write(d("0.47")).as_deref(), Some("R47"));
        assert_eq!(c.read("0R5"), vec![(3, d("0.5"))]);
        assert_eq!(c.write(d("0")), None);
    }

    #[test]
    fn eia4_both_ways() {
        let c = Code::new(CodeKind::Eia4);
        for (code, v) in [
            ("1002", "10000"),
            ("49R9", "49.9"),
            ("4R70", "4.7"),
            ("R470", "0.47"),
            ("1000", "100"),
            ("4991", "4990"),
            ("10R0", "10"),
        ] {
            assert_eq!(c.read(code), vec![(4, d(v))], "{code}");
            assert_eq!(c.write(d(v)).as_deref(), Some(code), "{v}");
        }
    }

    #[test]
    fn eia96_both_ways() {
        let c = Code::new(CodeKind::Eia96);
        for (code, v) in [
            ("01C", "10000"),
            ("68X", "49.9"),
            ("96A", "976"),
            ("01Z", "0.1"),
        ] {
            assert_eq!(c.read(code), vec![(3, d(v))], "{code}");
            assert_eq!(c.write(d(v)).as_deref(), Some(code), "{v}");
        }
    }

    #[test]
    fn rkm_both_ways() {
        let c = rkm(OHMS);
        for (code, v) in [
            ("4K7", "4700"),
            ("10K", "10000"),
            ("R047", "0.047"),
            ("1M", "1000000"),
            ("0R", "0"),
            ("100R", "100"),
            ("4K99", "4990"),
            ("2R2", "2.2"),
            ("1G", "1000000000"),
        ] {
            assert!(c.read(code).contains(&(code.len(), d(v))), "{code}");
            assert_eq!(c.write(d(v)).as_deref(), Some(code), "{v}");
        }
        // Readings of a longer text: every length that is a valid code
        assert_eq!(
            c.read("4K70X"),
            vec![(2, d("4000")), (3, d("4700")), (4, d("4700"))]
        );
        assert!(c.read("47").is_empty());
        let v = rkm(&[('V', 0)]);
        assert_eq!(v.write(d("5.1")).as_deref(), Some("5V1"));
        assert_eq!(v.read("5V1"), vec![(2, d("5")), (3, d("5.1"))]);
    }

    #[test]
    fn decimal_and_int() {
        let mut c = Code::new(CodeKind::Decimal);
        c.scale = Some(-2);
        c.digits = Some(3);
        assert_eq!(c.read("050X"), vec![(3, d("0.5"))]);
        assert_eq!(c.write(d("0.5")).as_deref(), Some("050"));
        let mut c = Code::new(CodeKind::Decimal);
        c.places = 1;
        assert_eq!(c.write(d("5")).as_deref(), Some("5.0"));
        assert!(c.read("5.0A").contains(&(3, d("5"))));
        let mut i = Code::new(CodeKind::Int);
        i.digits = Some(2);
        assert_eq!(i.write(d("5")).as_deref(), Some("05"));
        assert_eq!(i.read("05A"), vec![(2, d("5"))]);
    }

    #[test]
    fn e_series() {
        assert!(ESeries::E24.contains(d("4700")));
        assert!(ESeries::E24.contains(d("0.47")));
        assert!(!ESeries::E24.contains(d("4990")));
        assert!(ESeries::E96.contains(d("4990")));
        assert!(ESeries::E12.contains(d("82")));
        assert!(!ESeries::E12.contains(d("91")));
        assert_eq!(ESeries::E6.decade(), vec![100, 150, 220, 330, 470, 680]);
        assert_eq!(ESeries::E3.decade(), vec![100, 220, 470]);
        assert_eq!(ESeries::E48.decade().len(), 48);
        let v = ESeries::E6.between(d("1000"), d("10000"));
        assert_eq!(v.first(), Some(&d("1000")));
        assert_eq!(v.last(), Some(&d("10000")));
        assert_eq!(v.len(), 7);
    }
}
