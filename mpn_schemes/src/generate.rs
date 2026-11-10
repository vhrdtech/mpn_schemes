//! Generate (ENG-2): the MPN patterns that would have the wanted parameters, best first.
//!
//! Each segment's domain is filtered by the requirement's terms and ordered closest-to-the-bound first; segments no
//! term, preference or default decides become holes. The combinations are walked best first (lowest sum of ranks)
//! and checked against the `when` conditions and availability rows, which may also fix a hole.

use crate::codes::ESeries;
use crate::keys;
use crate::parse::Param;
use crate::pattern::{Hole, Part, Pattern};
use crate::scheme::{Leaf, LeafKind, Row, Scheme, Test, Variant};
use crate::value::{Dec, Quantity, Value};
use serde::Serialize;
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashSet};
use std::fmt;
use std::str::FromStr;

/// What is wanted: the class of part, terms on parameters, and optionally which makers or schemes to use
#[derive(Clone, Debug, Default)]
pub struct Requirement {
    /// `capacitor.ceramic`; a class also matches its sub-classes (`capacitor` matches `capacitor.ceramic`). None:
    /// every class.
    pub class: Option<String>,
    pub terms: Vec<Term>,
    /// Maker names or aliases; empty: every maker
    pub makers: Vec<String>,
    /// Scheme ids; empty: every scheme
    pub schemes: Vec<String>,
}

/// A condition on one parameter, as in mx3's search terms (SRCH-7)
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub key: String,
    pub op: Op,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Equal; for quantities within a relative tolerance when one is given (0.01 = 1 %)
    Eq { value: Value, rel_tol: Option<f64> },
    /// From `min` to `max`, both included
    Range { min: Value, max: Value },
    /// At least
    Ge(Value),
    /// At most
    Le(Value),
    /// One of these, in this order of preference
    EnumIn(Vec<Value>),
}

impl Op {
    /// Whether a value satisfies the term
    pub fn accepts(&self, v: &Value) -> bool {
        let cmp = |b: &Value| v.compare(b);
        match self {
            Op::Eq { value, rel_tol } => v.matches(value, *rel_tol),
            Op::Range { min, max } => {
                cmp(min).is_some_and(Ordering::is_ge) && cmp(max).is_some_and(Ordering::is_le)
            }
            Op::Ge(b) => cmp(b).is_some_and(Ordering::is_ge),
            Op::Le(b) => cmp(b).is_some_and(Ordering::is_le),
            Op::EnumIn(l) => l.iter().any(|x| v.matches(x, None)),
        }
    }

    /// How good an accepted value is (lower is better): closest to the bound first
    fn rank(&self, v: &Value) -> (i64, Option<Dec>, Option<Reverse<Dec>>) {
        let num = |x: &Value| match x {
            Value::Quantity(q) => Some(q.value),
            Value::Tolerance(t) => Some(t.plus.max(t.minus)),
            Value::Text(_) => None,
        };
        match self {
            Op::Eq { value, .. } => {
                let d = match (num(v), num(value)) {
                    (Some(a), Some(b)) => {
                        let f = (a.to_f64() - b.to_f64()).abs()
                            / b.to_f64().abs().max(f64::MIN_POSITIVE);
                        (f * 1e9) as i64
                    }
                    _ => 0,
                };
                (d, None, None)
            }
            Op::Range { .. } | Op::Ge(_) => (0, num(v), None),
            Op::Le(_) => (0, None, num(v).map(Reverse)),
            Op::EnumIn(l) => (
                l.iter()
                    .position(|x| v.matches(x, None))
                    .unwrap_or(usize::MAX) as i64,
                None,
                None,
            ),
        }
    }
}

impl Term {
    /// A term on a key, its value text read with the key's kind
    pub fn new(key: &str, op: &str, value: &str) -> Result<Term, String> {
        let k = keys::key(key).ok_or_else(|| format!("{key:?} is not a parameter key"))?;
        let read = |t: &str| {
            Value::parse(k.kind, t).ok_or_else(|| format!("{key}: {t:?} is not a valid value"))
        };
        let op = match op {
            "=" => match value.split_once("..") {
                Some((a, b)) => Op::Range {
                    min: read(a)?,
                    max: read(b)?,
                },
                None if value.contains('|') => {
                    Op::EnumIn(value.split('|').map(read).collect::<Result<_, _>>()?)
                }
                None => Op::Eq {
                    value: read(value)?,
                    rel_tol: None,
                },
            },
            "~" => {
                let (v, t) = value
                    .split_once('/')
                    .ok_or("`key~value/1%`: a value and a relative tolerance")?;
                let t = t
                    .trim()
                    .strip_suffix('%')
                    .ok_or("the tolerance after `/` is in %")?;
                let t = Dec::parse(t.trim())
                    .ok_or("the tolerance is a number")?
                    .to_f64()
                    / 100.0;
                Op::Eq {
                    value: read(v)?,
                    rel_tol: Some(t),
                }
            }
            ">=" => Op::Ge(read(value)?),
            "<=" => Op::Le(read(value)?),
            o => return Err(format!("operator {o:?}: =, ~, >= or <=")),
        };
        Ok(Term {
            key: k.name.to_string(),
            op,
        })
    }
}

/// `capacitance=100nF`, `rated_voltage>=16V`, `resistance~10kΩ/1%` (within 1 %), `dielectric=X7R|X5R`,
/// `resistance=1kΩ..10kΩ`
impl FromStr for Term {
    type Err = String;

    fn from_str(s: &str) -> Result<Term, String> {
        for op in [">=", "<=", "~", "="] {
            if let Some((k, v)) = s.split_once(op) {
                return Term::new(k.trim(), op, v.trim());
            }
        }
        Err(format!(
            "{s:?}: key=value, key>=value, key<=value, key=a..b, key=a|b or key~value/1%"
        ))
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let k = &self.key;
        match &self.op {
            Op::Eq {
                value,
                rel_tol: None,
            } => write!(f, "{k}={value}"),
            Op::Eq {
                value,
                rel_tol: Some(t),
            } => write!(f, "{k}~{value}/{}%", t * 100.0),
            Op::Range { min, max } => write!(f, "{k}={min}..{max}"),
            Op::Ge(v) => write!(f, "{k}>={v}"),
            Op::Le(v) => write!(f, "{k}<={v}"),
            Op::EnumIn(l) => {
                let t: Vec<String> = l.iter().map(Value::to_string).collect();
                write!(f, "{k}={}", t.join("|"))
            }
        }
    }
}

/// How to choose what the requirement leaves open
#[derive(Clone, Debug)]
pub struct Prefs {
    /// For parameters no term sets: the values to use, best first (`packaging`: `reel_7in_paper`)
    pub prefer: Vec<(String, Vec<Value>)>,
    /// A segment no term or preference decides takes its table's default entry (true), or stays a hole (false)
    pub fill_defaults: bool,
    /// The most patterns returned, over all schemes
    pub max_patterns: usize,
    /// The most values a range enumerates for one coded segment
    pub max_values: usize,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            prefer: Vec::new(),
            fill_defaults: true,
            max_patterns: 200,
            max_values: 64,
        }
    }
}

/// One generated pattern and what it means
#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub scheme: String,
    pub version: u32,
    pub maker: String,
    pub pattern: Pattern,
    /// The parameters the pattern fixes, then the scheme's implied facts
    pub params: Vec<Param>,
    /// Keys of terms the scheme neither encodes nor implies: the pattern says nothing about them
    pub unchecked: Vec<String>,
    /// Lower is better: the sum of each segment's choice rank (0 = its best value)
    pub rank: u32,
    /// Which way through the optional parts and alternatives (0 = everything as first written); ranks below
    /// `rank`: the same values spelled another way come before the next value
    pub variant: u32,
    /// The shortest prefix a supplier search should use: through the scheme's `min_stem` segment, else through
    /// the first segment a term set; never longer than the stem
    pub min_stem: String,
}

/// The result of [`crate::SchemeSet::generate`]
#[derive(Clone, Debug, Default, Serialize)]
pub struct Generated {
    pub candidates: Vec<Candidate>,
    /// What generate could not do (a range on a coded segment without a series, a step limit reached)
    pub notes: Vec<String>,
}

/// Steps the best-first walk may spend per scheme
const STEPS: usize = 200_000;

/// The choices of each leaf of a variant, and the keys of terms it leaves unchecked
type Domains = (Vec<Vec<Choice>>, Vec<String>);
/// Codes a hole may hold, with what each means
type Codes = Vec<(String, Option<Value>)>;
/// Holes (by leaf index) a row narrows, and to which codes
type Narrowing = Vec<(usize, Codes)>;

#[derive(Clone, Debug)]
enum Choice {
    Fixed {
        code: String,
        value: Option<Value>,
    },
    /// Codes it may hold (with their values), or any text when `codes` is empty
    Hole {
        codes: Vec<(String, Option<Value>)>,
        chars: String,
        min: usize,
        max: usize,
    },
}

/// Whether a scheme is one the requirement asks about
pub(crate) fn selects(scheme: &Scheme, req: &Requirement) -> bool {
    let class = req
        .class
        .as_ref()
        .is_none_or(|c| scheme.class == *c || scheme.class.starts_with(&format!("{c}.")));
    let maker = req.makers.is_empty() || req.makers.iter().any(|m| scheme.maker_matches(m));
    let id = req.schemes.is_empty() || req.schemes.contains(&scheme.id);
    class && maker && id
}

/// The patterns of one scheme for a requirement, best first, at most `limit`
pub(crate) fn generate_scheme(
    scheme: &Scheme,
    req: &Requirement,
    prefs: &Prefs,
    limit: usize,
    notes: &mut Vec<String>,
) -> Vec<Candidate> {
    // Per variant: the domains, or why it cannot serve
    let mut domains: Vec<Option<Domains>> = Vec::new();
    for v in &scheme.variants {
        domains.push(variant_domains(scheme, v, req, prefs, notes));
    }
    let mut heap = BinaryHeap::new();
    let mut seen: HashSet<(usize, Vec<u16>)> = HashSet::new();
    for (vi, d) in domains.iter().enumerate() {
        if let Some((d, _)) = d
            && d.iter().all(|c| !c.is_empty())
        {
            let idx = vec![0u16; d.len()];
            seen.insert((vi, idx.clone()));
            heap.push(Reverse((0u32, scheme.variants[vi].rank, vi, idx)));
        }
    }
    let mut out: Vec<Candidate> = Vec::new();
    let mut steps = STEPS;
    while let Some(Reverse((cost, vrank, vi, idx))) = heap.pop() {
        if out.len() >= limit {
            break;
        }
        if steps == 0 {
            notes.push(format!("{}: stopped after {STEPS} steps", scheme.id));
            break;
        }
        steps -= 1;
        let (d, unchecked) = domains[vi]
            .as_ref()
            .expect("only variants with domains are queued");
        let v = &scheme.variants[vi];
        let picks: Vec<&Choice> = idx.iter().zip(d).map(|(&i, c)| &c[i as usize]).collect();
        for c in evaluate(scheme, v, &picks, req, cost, unchecked) {
            if !out.iter().any(|o| o.pattern == c.pattern) {
                out.push(c);
            }
        }
        for k in 0..idx.len() {
            if (idx[k] as usize) + 1 < d[k].len() {
                let mut n = idx.clone();
                n[k] += 1;
                if seen.insert((vi, n.clone())) {
                    heap.push(Reverse((cost + 1, vrank, vi, n)));
                }
            }
        }
    }
    out.truncate(limit);
    out
}

fn variant_domains(
    scheme: &Scheme,
    v: &Variant,
    req: &Requirement,
    prefs: &Prefs,
    notes: &mut Vec<String>,
) -> Option<Domains> {
    // Terms on keys this variant does not encode: an implied fact must satisfy them, else they stay unchecked
    let mut unchecked = Vec::new();
    for t in &req.terms {
        if v.leaves
            .iter()
            .any(|l| l.param.is_some_and(|k| k.name == t.key))
        {
            continue;
        }
        match scheme.implies.iter().find(|(k, _)| k.name == t.key) {
            Some((_, val)) if !t.op.accepts(val) => return None,
            Some(_) => {}
            None => {
                if !unchecked.contains(&t.key) {
                    unchecked.push(t.key.clone());
                }
            }
        }
    }
    let mut out = Vec::new();
    for l in &v.leaves {
        let terms: Vec<&Term> = l
            .param
            .map(|k| req.terms.iter().filter(|t| t.key == k.name).collect())
            .unwrap_or_default();
        out.push(domain(scheme, l, &terms, prefs, notes));
    }
    Some((out, unchecked))
}

fn order_by_terms(vals: &mut Vec<(String, Option<Value>, bool)>, terms: &[&Term]) {
    vals.retain(|(_, v, _)| {
        v.as_ref()
            .is_some_and(|v| terms.iter().all(|t| t.op.accepts(v)))
    });
    if let Some(t) = terms.first() {
        vals.sort_by_key(|(_, v, _)| v.as_ref().map(|v| t.op.rank(v)));
    }
}

fn domain(
    scheme: &Scheme,
    l: &Leaf,
    terms: &[&Term],
    prefs: &Prefs,
    notes: &mut Vec<String>,
) -> Vec<Choice> {
    let fixed = |code: &str, value: Option<Value>| Choice::Fixed {
        code: code.to_string(),
        value,
    };
    let pref = l
        .param
        .and_then(|k| prefs.prefer.iter().find(|(key, _)| key == k.name))
        .map(|(_, v)| v);
    match &l.kind {
        LeafKind::Literal(s) => vec![fixed(s, None)],
        LeafKind::Absent(v) => {
            if terms.is_empty()
                || v.as_ref()
                    .is_some_and(|v| terms.iter().all(|t| t.op.accepts(v)))
            {
                vec![fixed("", v.clone())]
            } else {
                Vec::new()
            }
        }
        LeafKind::Table(es) => {
            let mut all: Vec<(String, Option<Value>, bool)> = es
                .iter()
                .map(|e| (e.code.clone(), e.value.clone(), e.default))
                .collect();
            if !terms.is_empty() {
                order_by_terms(&mut all, terms);
                return all.into_iter().map(|(c, v, _)| fixed(&c, v)).collect();
            }
            if let Some(p) = pref {
                let mut chosen = Vec::new();
                for want in p {
                    for (c, v, _) in &all {
                        if v.as_ref().is_some_and(|v| v.matches(want, None)) {
                            chosen.push(fixed(c, v.clone()));
                        }
                    }
                }
                if !chosen.is_empty() {
                    return chosen;
                }
            }
            if prefs.fill_defaults
                && let Some((c, v, _)) = all.iter().find(|e| e.2)
            {
                return vec![fixed(c, v.clone())];
            }
            if all.len() == 1 {
                return vec![fixed(&all[0].0, all[0].1.clone())];
            }
            let min = all.iter().map(|e| e.0.len()).min().unwrap_or(1);
            let max = all.iter().map(|e| e.0.len()).max().unwrap_or(1);
            vec![Choice::Hole {
                codes: all.into_iter().map(|(c, v, _)| (c, v)).collect(),
                chars: String::new(),
                min,
                max,
            }]
        }
        LeafKind::Code {
            code,
            unit_exp,
            unit,
            series,
        } => {
            let (chars, min, max) = code_shape(code);
            let hole = Choice::Hole {
                codes: Vec::new(),
                chars,
                min,
                max,
            };
            if terms.is_empty() {
                return vec![hole];
            }
            let numbers = candidates(terms, series, prefs.max_values);
            let Some(numbers) = numbers else {
                notes.push(format!(
                    "{}: segment {}: a range on a code needs `series` to list values",
                    scheme.id, l.name
                ));
                return Vec::new();
            };
            let mut vals: Vec<(String, Option<Value>, bool)> = Vec::new();
            for n in numbers {
                let v = Value::Quantity(Quantity { value: n, unit });
                if !series.is_empty() && !series.iter().any(|s| s.contains(n)) {
                    continue;
                }
                if let Some(c) = code.write(n.scale(-unit_exp))
                    && !vals.iter().any(|x| x.0 == c)
                {
                    vals.push((c, Some(v), false));
                }
            }
            order_by_terms(&mut vals, terms);
            vals.into_iter().map(|(c, v, _)| fixed(&c, v)).collect()
        }
        LeafKind::Free(cs, min, max) => vec![Choice::Hole {
            codes: Vec::new(),
            chars: cs.as_str().to_string(),
            min: *min,
            max: *max,
        }],
    }
}

/// The characters and lengths a coded segment can have, for a hole
fn code_shape(code: &crate::codes::Code) -> (String, usize, usize) {
    use crate::codes::CodeKind::*;
    match code.kind {
        Eia3 => ("0-9R".into(), 3, 3),
        Eia4 => ("0-9R".into(), 4, 4),
        Eia96 => ("0-9A-Z".into(), 3, 3),
        Rkm => {
            let letters: String = code.letters.iter().map(|(c, _)| *c).collect();
            let n = code.digits.unwrap_or(0);
            (
                format!("0-9{letters}"),
                if n > 0 { n } else { 2 },
                if n > 0 { n } else { 12 },
            )
        }
        Int | Decimal => {
            let n = code.digits.unwrap_or(0);
            let chars = if code.kind == Decimal && code.scale.is_none() {
                "0-9."
            } else {
                "0-9"
            };
            (
                chars.into(),
                if n > 0 { n } else { 1 },
                if n > 0 { n } else { 12 },
            )
        }
    }
}

/// The numbers (in the base unit) a coded segment should try for its terms; None when a range cannot be listed
fn candidates(terms: &[&Term], series: &[ESeries], max_values: usize) -> Option<Vec<Dec>> {
    let t = terms[0];
    let num = |v: &Value| v.number();
    let listed = |lo: Dec, hi: Dec| -> Vec<Dec> {
        let mut all: Vec<Dec> = series.iter().flat_map(|s| s.between(lo, hi)).collect();
        all.sort();
        all.dedup();
        all
    };
    match &t.op {
        Op::Eq { value, rel_tol } => {
            let n = num(value)?;
            match rel_tol {
                Some(tol) if !series.is_empty() => {
                    let f = n.to_f64();
                    let lo = Dec::parse(&format!("{:.12}", f * (1.0 - tol)))?;
                    let hi = Dec::parse(&format!("{:.12}", f * (1.0 + tol)))?;
                    let mut v = listed(lo, hi);
                    if !v.contains(&n) {
                        v.push(n);
                    }
                    Some(v)
                }
                _ => Some(vec![n]),
            }
        }
        Op::EnumIn(l) => Some(l.iter().filter_map(num).collect()),
        _ if series.is_empty() => None,
        Op::Range { min, max } => {
            let mut v = listed(num(min)?, num(max)?);
            v.truncate(max_values);
            Some(v)
        }
        Op::Ge(min) => {
            let lo = num(min)?;
            let mut v = listed(lo, lo.scale(6));
            v.truncate(max_values);
            Some(v)
        }
        Op::Le(max) => {
            let hi = num(max)?;
            let mut v = listed(hi.scale(-6), hi);
            v.reverse();
            v.truncate(max_values);
            Some(v)
        }
    }
}

/// What a leaf is in one combination, for checking conditions
enum State<'a> {
    Known(&'a str, Option<&'a Value>),
    /// A hole with listed codes (narrowed by rows), or any text
    Hole(Vec<(String, Option<Value>)>, bool),
}

/// Check one combination; yields its candidates (several when availability rows fix a hole differently)
fn evaluate(
    scheme: &Scheme,
    v: &Variant,
    picks: &[&Choice],
    req: &Requirement,
    cost: u32,
    unchecked: &[String],
) -> Vec<Candidate> {
    let states: Vec<State> = picks
        .iter()
        .map(|c| match c {
            Choice::Fixed { code, value } => State::Known(code, value.as_ref()),
            Choice::Hole { codes, .. } => State::Hole(codes.clone(), codes.is_empty()),
        })
        .collect();
    let index = |name: &str| v.leaves.iter().position(|l| l.name == name);
    // `when`: a fixed segment must pass, a hole is narrowed to the codes that pass
    for l in &v.leaves {
        for c in &l.when {
            let Some(i) = index(&c.segment) else {
                return Vec::new();
            };
            if let State::Known(code, val) = &states[i]
                && !c.test.passes(code, *val)
            {
                return Vec::new();
            }
        }
    }
    // Excluded rows: out when every segment they name is fixed and passes
    for r in &scheme.excluded {
        let all_known = r
            .conds
            .iter()
            .all(|c| index(&c.segment).is_some_and(|i| matches!(states[i], State::Known(..))));
        if all_known && row_eval(r, &index, &states).is_some() {
            return Vec::new();
        }
    }
    // Availability rows: in a closed scheme one must hold; each may narrow holes
    let mut narrowings: Vec<Narrowing> = Vec::new();
    if scheme.closed {
        for r in &scheme.available {
            if let Some(n) = row_eval(r, &index, &states) {
                narrowings.push(n);
            }
        }
        if narrowings.is_empty() {
            return Vec::new();
        }
        if narrowings.iter().any(Vec::is_empty) {
            narrowings = vec![Vec::new()];
        }
    } else {
        narrowings.push(Vec::new());
    }
    let mut out = Vec::new();
    for n in narrowings {
        out.push(candidate(scheme, v, picks, &n, req, cost, unchecked));
    }
    out
}

/// A row against one combination: None when it cannot hold, else the narrowed codes of the holes it names
fn row_eval(r: &Row, index: &dyn Fn(&str) -> Option<usize>, states: &[State]) -> Option<Narrowing> {
    let mut narrow = Vec::new();
    for c in &r.conds {
        let i = index(&c.segment)?;
        match &states[i] {
            State::Known(code, val) => {
                if !c.test.passes(code, *val) {
                    return None;
                }
            }
            State::Hole(codes, any) => {
                if *any {
                    if let Test::Codes(cs) = &c.test {
                        narrow.push((i, cs.iter().map(|c| (c.clone(), None)).collect()));
                    }
                    continue;
                }
                let kept: Vec<_> = codes
                    .iter()
                    .filter(|(cd, v)| c.test.passes(cd, v.as_ref()))
                    .cloned()
                    .collect();
                if kept.is_empty() {
                    return None;
                }
                if kept.len() < codes.len() {
                    narrow.push((i, kept));
                }
            }
        }
    }
    Some(narrow)
}

fn candidate(
    scheme: &Scheme,
    v: &Variant,
    picks: &[&Choice],
    narrow: &[(usize, Codes)],
    req: &Requirement,
    cost: u32,
    unchecked: &[String],
) -> Candidate {
    let mut parts: Vec<Part> = Vec::new();
    let mut params = Vec::new();
    let push_fixed = |parts: &mut Vec<Part>, t: &str| {
        if t.is_empty() {
            return;
        }
        if let Some(Part::Fixed { text }) = parts.last_mut() {
            text.push_str(t);
        } else {
            parts.push(Part::Fixed {
                text: t.to_string(),
            });
        }
    };
    // Where the stem may end: after the min_stem segment, else after the first segment a term set
    let target = scheme.min_stem.clone().or_else(|| {
        v.leaves
            .iter()
            .find(|l| {
                l.param
                    .is_some_and(|k| req.terms.iter().any(|t| t.key == k.name))
            })
            .map(|l| l.name.clone())
    });
    let mut fixed_len = 0usize;
    let mut open = true;
    let mut min_stem_len: Option<usize> = None;
    for (i, (l, c)) in v.leaves.iter().zip(picks).enumerate() {
        let (code, value): (Option<String>, Option<Value>) = match c {
            Choice::Fixed { code, value } => (Some(code.clone()), value.clone()),
            Choice::Hole {
                codes,
                chars,
                min,
                max,
            } => {
                let codes = narrow
                    .iter()
                    .find(|(j, _)| *j == i)
                    .map(|(_, c)| c)
                    .unwrap_or(codes);
                if codes.len() == 1 {
                    (Some(codes[0].0.clone()), codes[0].1.clone())
                } else {
                    let first = codes.first().and_then(|c| c.1.clone());
                    let same = first.is_some() && codes.iter().all(|c| c.1 == first);
                    let (mn, mx) = if codes.is_empty() {
                        (*min, *max)
                    } else {
                        (
                            codes.iter().map(|c| c.0.len()).min().unwrap_or(1),
                            codes.iter().map(|c| c.0.len()).max().unwrap_or(1),
                        )
                    };
                    parts.push(Part::Hole(Hole {
                        segment: l.name.clone(),
                        choices: codes.iter().map(|c| c.0.clone()).collect(),
                        chars: chars.clone(),
                        min_len: mn,
                        max_len: mx,
                    }));
                    open = false;
                    (None, if same { first } else { None })
                }
            }
        };
        if let Some(code) = &code {
            push_fixed(&mut parts, code);
            if open {
                fixed_len += code.len();
            }
        }
        if open && target.as_deref() == Some(l.name.as_str()) {
            min_stem_len = Some(fixed_len);
        }
        if let (Some(k), Some(val)) = (l.param, value) {
            params.push(Param {
                key: k.name,
                value: val,
                segment: Some(l.name.clone()),
            });
        }
    }
    for (k, val) in &scheme.implies {
        params.push(Param {
            key: k.name,
            value: val.clone(),
            segment: None,
        });
    }
    let pattern = Pattern { parts };
    let stem = pattern.stem();
    let min_stem = stem[..min_stem_len.unwrap_or(stem.len()).min(stem.len())].to_string();
    Candidate {
        scheme: scheme.id.clone(),
        version: scheme.version,
        maker: scheme.maker.clone(),
        pattern,
        params,
        unchecked: unchecked.to_vec(),
        rank: cost,
        variant: v.rank,
        min_stem,
    }
}
