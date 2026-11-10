//! Parse (ENG-1): match an MPN against a scheme's variants, left to right with backtracking, and rank the
//! results. Untrusted input: the MPN's length is capped and every scheme gets a fixed number of steps.

use crate::scheme::{Cond, LeafKind, Row, Scheme, Variant};
use crate::value::{Quantity, Value};
use serde::Serialize;

/// The longest MPN parse looks at
pub const MAX_MPN_LEN: usize = 64;
/// Matching steps one scheme may spend on one MPN
const STEPS_PER_SCHEME: usize = 50_000;

/// One reading of an MPN by one scheme
#[derive(Clone, Debug, Serialize)]
pub struct Parse {
    /// The scheme's id and version: record both with a decoded value
    pub scheme: String,
    pub version: u32,
    pub maker: String,
    pub class: String,
    /// The whole MPN was read. A partial parse read only `segments` and left `rest`.
    pub complete: bool,
    /// What a partial parse could not read; empty when complete
    pub rest: String,
    /// The parameters, from the segments in order, then the scheme's implied facts
    pub params: Vec<Param>,
    pub segments: Vec<SegmentMatch>,
    pub flags: Vec<Flag>,
}

/// A decoded parameter
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Param {
    pub key: &'static str,
    pub value: Value,
    /// The segment it was read from; none for an implied fact
    pub segment: Option<String>,
}

/// One segment of a parse: where it is in the MPN and what it means
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SegmentMatch {
    pub name: String,
    /// Byte offsets into the MPN as given (ASCII, after trimming)
    pub start: usize,
    pub end: usize,
    /// The text of the segment, as in the MPN
    pub code: String,
    pub key: Option<&'static str>,
    pub value: Option<Value>,
    /// Read by a free segment (no meaning decoded)
    pub free: bool,
}

/// Something a parse found worth a look, without rejecting it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "flag", rename_all = "snake_case")]
pub enum Flag {
    /// No availability row covers the combination, or an excluded row does: the scheme does not list this part
    /// (catalogues grow faster than schemes)
    NotInTable,
    /// A coded value outside the E series the segment names
    OutsideSeries { segment: String },
    /// An optional segment was left out
    Absent { segment: String },
}

impl Parse {
    /// The value of a parameter
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.params.iter().find(|p| p.key == key).map(|p| &p.value)
    }

    /// The number of characters read by free segments (fewer is a better reading)
    pub fn free_chars(&self) -> usize {
        self.segments
            .iter()
            .filter(|s| s.free)
            .map(|s| s.end - s.start)
            .sum()
    }
}

/// The MPN as matched: trimmed, ASCII only, capped
pub(crate) fn clean(mpn: &str) -> Option<&str> {
    let t = mpn.trim();
    (!t.is_empty() && t.len() <= MAX_MPN_LEN && t.is_ascii()).then_some(t)
}

/// A segment matched during the search
#[derive(Clone)]
struct Hit {
    leaf: usize,
    start: usize,
    end: usize,
    value: Option<Value>,
    outside_series: bool,
}

struct Search<'a> {
    text: &'a str,
    steps: usize,
    complete: Vec<(usize, Vec<Hit>)>,
    best_partial: Option<(usize, Vec<Hit>, usize)>,
}

/// Every complete parse of `mpn` by this scheme, best first; when there is none, the longest partial one
pub(crate) fn parse_scheme(scheme: &Scheme, mpn: &str) -> Vec<Parse> {
    let Some(text) = clean(mpn) else {
        return Vec::new();
    };
    let mut s = Search {
        text,
        steps: STEPS_PER_SCHEME,
        complete: Vec::new(),
        best_partial: None,
    };
    for (vi, v) in scheme.variants.iter().enumerate() {
        let mut hits = Vec::new();
        walk(&mut s, v, vi, 0, 0, &mut hits);
        if s.steps == 0 {
            break;
        }
    }
    let mut out: Vec<(u32, Parse)> = Vec::new();
    for (vi, hits) in &s.complete {
        let v = &scheme.variants[*vi];
        let p = build(scheme, v, text, hits, true);
        if !out.iter().any(|(_, q)| q.segments == p.segments) {
            out.push((v.rank, p));
        }
    }
    if out.is_empty()
        && let Some((vi, hits, _)) = &s.best_partial
    {
        let v = &scheme.variants[*vi];
        out.push((v.rank, build(scheme, v, text, hits, false)));
    }
    out.sort_by_key(|(rank, p)| {
        let outside = p
            .flags
            .iter()
            .any(|f| matches!(f, Flag::OutsideSeries { .. }));
        (
            p.flags.contains(&Flag::NotInTable),
            outside,
            p.free_chars(),
            *rank,
        )
    });
    out.into_iter().map(|(_, p)| p).collect()
}

fn walk(s: &mut Search, v: &Variant, vi: usize, li: usize, pos: usize, hits: &mut Vec<Hit>) {
    if s.steps == 0 {
        return;
    }
    s.steps -= 1;
    if li == v.leaves.len() {
        if pos == s.text.len() {
            s.complete.push((vi, hits.clone()));
        } else {
            note_partial(s, vi, hits, pos);
        }
        return;
    }
    let leaf = &v.leaves[li];
    if !leaf.when.iter().all(|c| cond_holds(c, v, s.text, hits)) {
        note_partial(s, vi, hits, pos);
        return;
    }
    let rest = &s.text[pos..];
    let mut tries: Vec<(usize, Option<Value>, bool)> = Vec::new();
    match &leaf.kind {
        LeafKind::Literal(l) => {
            if starts_ci(rest, l) {
                tries.push((l.len(), None, false));
            }
        }
        LeafKind::Table(es) => {
            for e in es {
                if starts_ci(rest, &e.code) {
                    tries.push((e.code.len(), e.value.clone(), false));
                }
            }
            // Longest code first: `10` before `1`
            tries.sort_by_key(|t| std::cmp::Reverse(t.0));
        }
        LeafKind::Code {
            code,
            unit_exp,
            unit,
            series,
        } => {
            for (len, n) in code.read(rest).into_iter().rev() {
                let outside = !series.is_empty() && !series.iter().any(|e| e.contains(n));
                let q = Quantity {
                    value: n.scale(*unit_exp),
                    unit,
                };
                tries.push((len, Some(Value::Quantity(q)), outside));
            }
        }
        LeafKind::Free(cs, min, max) => {
            let ok = rest
                .bytes()
                .take(*max)
                .take_while(|&c| cs.contains(c))
                .count();
            for len in (*min..=ok.min(*max)).rev() {
                tries.push((len, None, false));
            }
        }
        LeafKind::Absent(v) => tries.push((0, v.clone(), false)),
    }
    if tries.is_empty() {
        note_partial(s, vi, hits, pos);
    }
    for (len, value, outside_series) in tries {
        hits.push(Hit {
            leaf: li,
            start: pos,
            end: pos + len,
            value,
            outside_series,
        });
        walk(s, v, vi, li + 1, pos + len, hits);
        hits.pop();
        if s.steps == 0 {
            return;
        }
    }
}

fn note_partial(s: &mut Search, vi: usize, hits: &[Hit], pos: usize) {
    // Only a reading past the first two segments says anything about the MPN
    let real = hits.iter().filter(|h| h.end > h.start).count();
    if real < 2 {
        return;
    }
    if s.best_partial.as_ref().is_none_or(|(_, _, p)| pos > *p) {
        s.best_partial = Some((vi, hits.to_vec(), pos));
    }
}

fn starts_ci(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// A `when` condition against the segments matched so far
fn cond_holds(c: &Cond, v: &Variant, text: &str, hits: &[Hit]) -> bool {
    hits.iter()
        .find(|h| v.leaves[h.leaf].name == c.segment)
        .is_some_and(|h| c.test.passes(&text[h.start..h.end], h.value.as_ref()))
}

/// Whether a row holds for a complete reading: every segment it names was read and passes
pub(crate) fn row_holds(
    row: &Row,
    v: &Variant,
    text: &str,
    hits: &[(usize, usize, usize, Option<Value>)],
) -> bool {
    row.conds.iter().all(|c| {
        hits.iter()
            .find(|h| v.leaves[h.0].name == c.segment)
            .is_some_and(|h| c.test.passes(&text[h.1..h.2], h.3.as_ref()))
    })
}

fn build(scheme: &Scheme, v: &Variant, text: &str, hits: &[Hit], complete: bool) -> Parse {
    let mut segments = Vec::new();
    let mut params = Vec::new();
    let mut flags = Vec::new();
    for h in hits {
        let leaf = &v.leaves[h.leaf];
        if matches!(leaf.kind, LeafKind::Absent(_)) {
            flags.push(Flag::Absent {
                segment: leaf.name.clone(),
            });
        }
        if h.outside_series {
            flags.push(Flag::OutsideSeries {
                segment: leaf.name.clone(),
            });
        }
        if let (Some(k), Some(val)) = (leaf.param, &h.value) {
            params.push(Param {
                key: k.name,
                value: val.clone(),
                segment: Some(leaf.name.clone()),
            });
        }
        segments.push(SegmentMatch {
            name: leaf.name.clone(),
            start: h.start,
            end: h.end,
            code: text[h.start..h.end].to_string(),
            key: leaf.param.map(|k| k.name),
            value: h.value.clone(),
            free: matches!(leaf.kind, LeafKind::Free(..)),
        });
    }
    for (k, val) in &scheme.implies {
        params.push(Param {
            key: k.name,
            value: val.clone(),
            segment: None,
        });
    }
    if complete {
        let flat: Vec<_> = hits
            .iter()
            .map(|h| (h.leaf, h.start, h.end, h.value.clone()))
            .collect();
        let listed = !scheme.closed
            || scheme
                .available
                .iter()
                .any(|r| row_holds(r, v, text, &flat));
        let excluded = scheme.excluded.iter().any(|r| row_holds(r, v, text, &flat));
        if !listed || excluded {
            flags.insert(0, Flag::NotInTable);
        }
    }
    let end = hits.last().map_or(0, |h| h.end);
    Parse {
        scheme: scheme.id.clone(),
        version: scheme.version,
        maker: scheme.maker.clone(),
        class: scheme.class.clone(),
        complete,
        rest: if complete {
            String::new()
        } else {
            text[end..].to_string()
        },
        params,
        segments,
        flags,
    }
}
