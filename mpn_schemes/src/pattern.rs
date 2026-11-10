//! Patterns (ENG-3): an MPN with holes, what generate yields. `GRM155R71C104KA??D` is fixed text and a hole of
//! two characters where Murata's spec code goes; the supplier's search on the stem fills it in.

use serde::Serialize;
use std::fmt;

/// An MPN pattern: fixed text and holes, in order
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Pattern {
    pub parts: Vec<Part>,
}

/// A piece of a pattern
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "part", rename_all = "snake_case")]
pub enum Part {
    Fixed { text: String },
    Hole(Hole),
}

/// A segment generate did not decide
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Hole {
    /// The scheme's segment
    pub segment: String,
    /// The codes it may hold; empty: any text of `chars` and a length in `min_len..=max_len`
    pub choices: Vec<String>,
    /// The characters it may hold, as a set like `A-Z0-9`
    pub chars: String,
    pub min_len: usize,
    pub max_len: usize,
}

/// Steps one `matches` call may spend
const MATCH_STEPS: usize = 10_000;

impl Pattern {
    /// The fixed text before the first hole: what a supplier's prefix search asks for
    pub fn stem(&self) -> String {
        let mut s = String::new();
        for p in &self.parts {
            match p {
                Part::Fixed { text } => s.push_str(text),
                Part::Hole(_) => break,
            }
        }
        s
    }

    /// The pattern has no holes: it is one MPN
    pub fn is_exact(&self) -> bool {
        self.parts.iter().all(|p| matches!(p, Part::Fixed { .. }))
    }

    /// The MPN when the pattern has no holes
    pub fn exact(&self) -> Option<String> {
        self.is_exact().then(|| self.stem())
    }

    /// The holes
    pub fn holes(&self) -> impl Iterator<Item = &Hole> {
        self.parts.iter().filter_map(|p| match p {
            Part::Hole(h) => Some(h),
            _ => None,
        })
    }

    /// Whether a whole MPN fits the pattern (ASCII case ignored, surrounding spaces trimmed)
    pub fn matches(&self, mpn: &str) -> bool {
        let t = mpn.trim();
        if !t.is_ascii() || t.len() > crate::parse::MAX_MPN_LEN {
            return false;
        }
        let mut steps = MATCH_STEPS;
        fits(&self.parts, t.as_bytes(), &mut steps)
    }
}

fn fits(parts: &[Part], text: &[u8], steps: &mut usize) -> bool {
    if *steps == 0 {
        return false;
    }
    *steps -= 1;
    let Some((first, rest)) = parts.split_first() else {
        return text.is_empty();
    };
    match first {
        Part::Fixed { text: f } => {
            text.len() >= f.len()
                && text[..f.len()].eq_ignore_ascii_case(f.as_bytes())
                && fits(rest, &text[f.len()..], steps)
        }
        Part::Hole(h) if !h.choices.is_empty() => h.choices.iter().any(|c| {
            text.len() >= c.len()
                && text[..c.len()].eq_ignore_ascii_case(c.as_bytes())
                && fits(rest, &text[c.len()..], steps)
        }),
        Part::Hole(h) => {
            let set = crate::scheme::CharSet::parse(&h.chars);
            let ok = text
                .iter()
                .take(h.max_len)
                .take_while(|&&c| set.as_ref().is_none_or(|s| s.contains(c)))
                .count();
            (h.min_len..=ok.min(h.max_len)).any(|n| fits(rest, &text[n..], steps))
        }
    }
}

/// `CL05B104KO5NNNC`, `GRM155R71C104KA??D`; a hole of variable length is `*`, one with choices `[A|B]`
impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.parts {
            match p {
                Part::Fixed { text } => f.write_str(text)?,
                Part::Hole(h) if !h.choices.is_empty() && h.choices.len() <= 4 => {
                    write!(f, "[{}]", h.choices.join("|"))?
                }
                Part::Hole(h) if h.min_len == h.max_len => f.write_str(&"?".repeat(h.min_len))?,
                Part::Hole(_) => f.write_str("*")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(t: &str) -> Part {
        Part::Fixed { text: t.into() }
    }

    #[test]
    fn stems_and_matching() {
        let p = Pattern {
            parts: vec![
                fixed("GRM155R71C104KA"),
                Part::Hole(Hole {
                    segment: "spec".into(),
                    choices: vec![],
                    chars: "0-9A-Z".into(),
                    min_len: 2,
                    max_len: 2,
                }),
                fixed("D"),
            ],
        };
        assert_eq!(p.to_string(), "GRM155R71C104KA??D");
        assert_eq!(p.stem(), "GRM155R71C104KA");
        assert!(p.matches("GRM155R71C104KA88D"));
        assert!(p.matches("grm155r71c104ka01d"));
        assert!(!p.matches("GRM155R71C104KA8D"));
        assert!(!p.matches("GRM155R71C104KA88J"));
        let q = Pattern {
            parts: vec![
                fixed("RC0402"),
                Part::Hole(Hole {
                    segment: "tol".into(),
                    choices: vec!["F".into(), "J".into()],
                    chars: String::new(),
                    min_len: 1,
                    max_len: 1,
                }),
                fixed("R-0710KL"),
            ],
        };
        assert_eq!(q.to_string(), "RC0402[F|J]R-0710KL");
        assert!(q.matches("RC0402JR-0710KL"));
        assert!(!q.matches("RC0402DR-0710KL"));
        assert!(!q.is_exact());
    }
}
