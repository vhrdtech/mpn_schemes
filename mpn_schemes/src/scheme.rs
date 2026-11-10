//! A loaded, validated scheme (LANG-1..4): the header, the segment tree expanded into flat variants, the
//! availability rows and the examples. Built from a [`RawFile`] by [`compile`], which collects every error of the
//! file rather than stopping at the first.

use crate::codes::{Code, CodeKind, ESeries};
use crate::file::{OneOrMany, RawEntry, RawFile, RawLen, RawRowValue, RawSegment};
use crate::keys::{self, Key};
use crate::value::{Kind, Value, split_unit};
use std::collections::BTreeMap;
use std::fmt;

/// The most variants the optional parts and alternatives of one scheme may expand to
const MAX_VARIANTS: usize = 1024;

/// A load or validation error, with the file, the line and the segment it is about
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: Option<usize>,
    pub segment: Option<String>,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.file)?;
        if let Some(l) = self.line {
            write!(f, ":{l}")?;
        }
        if let Some(s) = &self.segment {
            write!(f, ": segment {s}")?;
        }
        write!(f, ": {}", self.message)
    }
}

impl std::error::Error for Error {}

/// Where a scheme's facts come from: the maker's document
#[derive(Clone, Debug, serde::Serialize)]
pub struct Source {
    pub doc: String,
    pub url: String,
    /// The date the document was fetched, `YYYY-MM-DD`
    pub fetched: String,
    pub pages: Option<String>,
}

/// One family of one maker's part numbers
#[derive(Clone, Debug)]
pub struct Scheme {
    /// Stable id, `<maker>.<family>`: `samsung.cl`
    pub id: String,
    /// Raised on every change of meaning
    pub version: u32,
    pub maker: String,
    pub aliases: Vec<String>,
    /// What kind of part: `capacitor.ceramic`, `resistor.chip`
    pub class: String,
    pub title: String,
    pub sources: Vec<Source>,
    /// The file it was loaded from (`samsung/cl.toml`, or a path for an overlay)
    pub file: String,
    pub(crate) closed: bool,
    pub(crate) min_stem: Option<String>,
    pub(crate) implies: Vec<(&'static Key, Value)>,
    pub(crate) variants: Vec<Variant>,
    pub(crate) available: Vec<Row>,
    pub(crate) excluded: Vec<Row>,
    pub(crate) examples: Vec<Example>,
}

/// One real MPN of the family and the parameters it means
#[derive(Clone, Debug)]
pub struct Example {
    pub mpn: String,
    pub params: Vec<(String, Value)>,
    pub(crate) line: usize,
}

impl Scheme {
    /// The facts every MPN of the family has (`[implies]`)
    pub fn implies(&self) -> impl Iterator<Item = (&'static str, &Value)> {
        self.implies.iter().map(|(k, v)| (k.name, v))
    }

    /// The scheme's examples
    pub fn examples(&self) -> &[Example] {
        &self.examples
    }

    /// The parameter keys the scheme can decode, in segment order, then the implied ones
    pub fn keys(&self) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for v in &self.variants {
            for l in &v.leaves {
                if let Some(k) = l.param
                    && !out.contains(&k.name)
                {
                    out.push(k.name);
                }
            }
        }
        for (k, _) in &self.implies {
            if !out.contains(&k.name) {
                out.push(k.name);
            }
        }
        out
    }

    /// Whether the maker's name or an alias matches `hint` (case and punctuation ignored, either contains the
    /// other)
    pub fn maker_matches(&self, hint: &str) -> bool {
        let norm = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect()
        };
        let h = norm(hint);
        if h.is_empty() {
            return false;
        }
        std::iter::once(&self.maker).chain(&self.aliases).any(|m| {
            let m = norm(m);
            !m.is_empty() && (m.contains(&h) || h.contains(&m))
        })
    }

    /// The leading literal of every variant, upper case (for indexing); empty when a variant starts otherwise
    pub(crate) fn leading_literals(&self) -> Vec<String> {
        self.variants
            .iter()
            .map(|v| match v.leaves.first().map(|l| &l.kind) {
                Some(LeafKind::Literal(s)) => s.to_ascii_uppercase(),
                _ => String::new(),
            })
            .collect()
    }
}

/// A flat sequence of leaf segments: one way through the optional parts and alternatives
#[derive(Clone, Debug)]
pub(crate) struct Variant {
    pub leaves: Vec<Leaf>,
    pub rank: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Leaf {
    pub name: String,
    pub param: Option<&'static Key>,
    pub kind: LeafKind,
    pub when: Vec<Cond>,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum LeafKind {
    Literal(String),
    Table(Vec<Entry>),
    Code {
        code: Code,
        /// The power of ten of the segment's unit (`pF`: -12)
        unit_exp: i32,
        unit: &'static str,
        series: Vec<ESeries>,
    },
    Free(CharSet, usize, usize),
    /// An optional segment left out, with the value that means
    Absent(Option<Value>),
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub code: String,
    pub value: Option<Value>,
    pub default: bool,
}

/// The characters a free segment (or a hole) may hold, as written (`A-Z0-9`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharSet {
    text: String,
    ranges: Vec<(u8, u8)>,
}

impl CharSet {
    pub fn parse(text: &str) -> Option<CharSet> {
        let b = text.as_bytes();
        if b.is_empty() || !text.is_ascii() {
            return None;
        }
        let mut ranges = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if i + 2 < b.len() && b[i + 1] == b'-' {
                if b[i] > b[i + 2] {
                    return None;
                }
                ranges.push((b[i].to_ascii_uppercase(), b[i + 2].to_ascii_uppercase()));
                i += 3;
            } else {
                ranges.push((b[i].to_ascii_uppercase(), b[i].to_ascii_uppercase()));
                i += 1;
            }
        }
        Some(CharSet {
            text: text.to_string(),
            ranges,
        })
    }

    pub fn contains(&self, c: u8) -> bool {
        let c = c.to_ascii_uppercase();
        self.ranges.iter().any(|&(a, b)| c >= a && c <= b)
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// A condition on one segment: in a `when`, an availability row or an excluded row
#[derive(Clone, Debug)]
pub(crate) struct Cond {
    pub segment: String,
    pub test: Test,
}

#[derive(Clone, Debug)]
pub(crate) enum Test {
    /// The segment's code is one of these (segments without a parameter)
    Codes(Vec<String>),
    /// The segment's value is one of these
    Values(Vec<Value>),
    /// The segment's value lies in this range and series
    Range {
        min: Option<Value>,
        max: Option<Value>,
        series: Vec<ESeries>,
    },
    /// A `when` as written, until the param of the segment it names is known
    Raw(RawRowValue),
}

impl Test {
    /// Whether a segment matched as `code`, meaning `value`, passes
    pub fn passes(&self, code: &str, value: Option<&Value>) -> bool {
        match self {
            Test::Codes(c) => c.iter().any(|c| c.eq_ignore_ascii_case(code)),
            Test::Values(vs) => value.is_some_and(|v| vs.iter().any(|x| x.matches(v, None))),
            Test::Raw(_) => false,
            Test::Range { min, max, series } => {
                let Some(v) = value else { return false };
                let lo = min
                    .as_ref()
                    .is_none_or(|m| v.compare(m).is_some_and(|o| o.is_ge()));
                let hi = max
                    .as_ref()
                    .is_none_or(|m| v.compare(m).is_some_and(|o| o.is_le()));
                let ser = series.is_empty()
                    || v.number()
                        .is_some_and(|n| series.iter().any(|s| s.contains(n)));
                lo && hi && ser
            }
        }
    }
}

/// An availability or excluded row: all its conditions hold
#[derive(Clone, Debug)]
pub(crate) struct Row {
    pub conds: Vec<Cond>,
    #[allow(dead_code)]
    pub line: usize,
}

/// Collects the errors of one file
pub(crate) struct Errors<'a> {
    pub file: &'a str,
    pub text: &'a str,
    pub list: Vec<Error>,
}

impl Errors<'_> {
    pub fn line_of(&self, offset: usize) -> usize {
        self.text[..offset.min(self.text.len())]
            .matches('\n')
            .count()
            + 1
    }

    pub fn at(&mut self, line: usize, segment: Option<&str>, message: impl Into<String>) {
        self.list.push(Error {
            file: self.file.to_string(),
            line: Some(line),
            segment: segment.map(str::to_string),
            message: message.into(),
        });
    }
}

/// Build a scheme from a parsed file. `path` is the file's path under `schemes/` when it has one, to check the id
/// against it.
pub(crate) fn compile(raw: RawFile, errs: &mut Errors, path: Option<&str>) -> Option<Scheme> {
    let before = errs.list.len();
    let hline = errs.line_of(raw.scheme.span().start);
    let h = raw.scheme.into_inner();

    // Header
    let id_ok = |s: &str| {
        s.contains('.')
            && s.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c))
    };
    if !id_ok(&h.id) {
        errs.at(
            hline,
            None,
            format!("id {:?}: lower-case `<maker>.<family>`", h.id),
        );
    }
    if let Some(p) = path
        && let Some(stem) = p.strip_suffix(".toml")
    {
        let want = stem.replace('/', ".");
        if want != h.id {
            errs.at(
                hline,
                None,
                format!(
                    "id {:?} does not match the file's path (want {want:?})",
                    h.id
                ),
            );
        }
    }
    if h.version == 0 {
        errs.at(hline, None, "version starts at 1");
    }
    for (what, v) in [
        ("maker", &h.maker),
        ("class", &h.class),
        ("title", &h.title),
    ] {
        if v.trim().is_empty() {
            errs.at(hline, None, format!("{what} is empty"));
        }
    }
    if h.sources.is_empty() {
        errs.at(
            hline,
            None,
            "no sources: name the maker's document the tables come from",
        );
    }
    for s in &h.sources {
        let date = s.fetched.len() == 10
            && s.fetched.bytes().enumerate().all(|(i, c)| {
                if i == 4 || i == 7 {
                    c == b'-'
                } else {
                    c.is_ascii_digit()
                }
            });
        if !date {
            errs.at(
                hline,
                None,
                format!(
                    "source {:?}: fetched {:?} is not YYYY-MM-DD",
                    s.doc, s.fetched
                ),
            );
        }
        if !s.url.starts_with("https://") && !s.url.starts_with("http://") {
            errs.at(
                hline,
                None,
                format!("source {:?}: url {:?} is not a web address", s.doc, s.url),
            );
        }
    }
    let closed = match h.availability.as_deref() {
        None | Some("open") => false,
        Some("closed") => true,
        Some(o) => {
            errs.at(hline, None, format!("availability {o:?}: open or closed"));
            false
        }
    };
    if let Some(s) = &raw.series {
        let line = errs.line_of(s.span().start);
        errs.at(
            line,
            None,
            "[series] is reserved for series facts (LANG-6) and not defined yet",
        );
    }

    // Segments, expanded into variants
    let mut ctx = Expand {
        errs,
        too_many: false,
    };
    let mut variants = vec![(Vec::<Leaf>::new(), 0u32)];
    for seg in &raw.segment {
        let line = ctx.errs.line_of(seg.span().start);
        let opts = ctx.segment(seg.get_ref(), line, None, None);
        variants = ctx.product(variants, opts);
    }
    let too_many = ctx.too_many;
    if raw.segment.is_empty() {
        errs.at(hline, None, "no segments");
    }
    if too_many {
        errs.at(
            hline,
            None,
            format!("the optional parts and alternatives make more than {MAX_VARIANTS} variants"),
        );
    }
    let mut variants: Vec<Variant> = variants
        .into_iter()
        .map(|(leaves, rank)| Variant { leaves, rank })
        .collect();
    resolve_when(&mut variants, errs);

    // Per variant: names, params, `when`, adjacent variable-length segments
    let mut params_of: BTreeMap<String, Option<&'static Key>> = BTreeMap::new();
    for v in &variants {
        let mut seen: Vec<&str> = Vec::new();
        let mut keys_seen: Vec<&str> = Vec::new();
        for (i, l) in v.leaves.iter().enumerate() {
            if seen.contains(&l.name.as_str()) {
                errs.at(
                    l.line,
                    Some(&l.name),
                    "the name is used twice in one way through the scheme",
                );
            }
            if let Some(k) = l.param {
                if keys_seen.contains(&k.name) {
                    errs.at(
                        l.line,
                        Some(&l.name),
                        format!(
                            "param {} is set twice in one way through the scheme",
                            k.name
                        ),
                    );
                }
                keys_seen.push(k.name);
            }
            match params_of.get(&l.name) {
                Some(p) if p.map(|k| k.name) != l.param.map(|k| k.name) => {
                    errs.at(
                        l.line,
                        Some(&l.name),
                        "segments of the same name must set the same param",
                    );
                }
                _ => {
                    params_of.insert(l.name.clone(), l.param);
                }
            }
            for c in &l.when {
                if !seen.contains(&c.segment.as_str()) {
                    errs.at(
                        l.line,
                        Some(&l.name),
                        format!("when names {:?}, which does not come before it", c.segment),
                    );
                }
            }
            seen.push(&l.name);
            if i > 0 && variable(&v.leaves[i - 1]) && variable(l) {
                errs.at(
                    l.line,
                    Some(&l.name),
                    format!(
                        "follows {:?} and both have a variable length: the split is ambiguous",
                        v.leaves[i - 1].name
                    ),
                );
            }
        }
    }
    // The `when` conditions were built without knowing the param of the segment they name: check them now
    for v in &variants {
        for l in &v.leaves {
            for c in &l.when {
                check_cond(c, &variants, errs, l.line);
            }
        }
    }

    // Implied facts
    let mut implies = Vec::new();
    for (k, v) in &raw.implies {
        let line = errs.line_of(v.span().start);
        match keys::key(k) {
            None => errs.at(line, None, format!("[implies] {k}: not a parameter key")),
            Some(key) => {
                if params_of
                    .values()
                    .any(|p| p.is_some_and(|p| p.name == key.name))
                {
                    errs.at(
                        line,
                        None,
                        format!("[implies] {k}: a segment sets it already"),
                    );
                }
                match Value::parse(key.kind, v.get_ref()) {
                    Some(val) if !keys::allows(key, &val) => errs.at(
                        line,
                        None,
                        format!(
                            "[implies] {k} = {:?}: one of {}",
                            v.get_ref(),
                            key.words.join(", ")
                        ),
                    ),
                    Some(val) => implies.push((key, val)),
                    None => errs.at(
                        line,
                        None,
                        format!(
                            "[implies] {k} = {:?}: not a {}",
                            v.get_ref(),
                            kind_name(key.kind)
                        ),
                    ),
                }
            }
        }
    }

    // Availability
    let rows = |list: &[toml::Spanned<BTreeMap<String, RawRowValue>>],
                what: &str,
                errs: &mut Errors|
     -> Vec<Row> {
        let mut out = Vec::new();
        for r in list {
            let line = errs.line_of(r.span().start);
            let mut conds = Vec::new();
            for (k, v) in r.get_ref() {
                if k == "note" {
                    continue;
                }
                match row_cond(k, v, &params_of) {
                    Ok(c) => {
                        check_cond(&c, &variants, errs, line);
                        conds.push(c);
                    }
                    Err(m) => errs.at(line, Some(k), format!("[[{what}]]: {m}")),
                }
            }
            if conds.is_empty() {
                errs.at(line, None, format!("[[{what}]] without conditions"));
            }
            out.push(Row { conds, line });
        }
        out
    };
    let available = rows(&raw.available, "available", errs);
    let excluded = rows(&raw.excluded, "excluded", errs);
    if closed && available.is_empty() {
        errs.at(
            hline,
            None,
            "availability = \"closed\" without [[available]] rows",
        );
    }
    if !closed && !available.is_empty() {
        errs.at(hline, None, "[[available]] rows need availability = \"closed\"; an open scheme only has [[excluded]]");
    }
    if let Some(m) = &h.min_stem
        && !params_of.contains_key(m)
    {
        errs.at(
            hline,
            None,
            format!("min_stem names {m:?}, which is not a segment"),
        );
    }

    // Examples (their round trip runs once the scheme is built, in `check_examples`)
    let mut examples = Vec::new();
    for e in &raw.example {
        let line = errs.line_of(e.span().start);
        let e = e.get_ref();
        let mut params = Vec::new();
        for (k, v) in &e.params {
            match keys::key(k) {
                None => errs.at(
                    line,
                    None,
                    format!("example {}: {k} is not a parameter key", e.mpn),
                ),
                Some(key) => match Value::parse(key.kind, v) {
                    Some(val) => params.push((key.name.to_string(), val)),
                    None => errs.at(
                        line,
                        None,
                        format!(
                            "example {}: {k} = {v:?} is not a {}",
                            e.mpn,
                            kind_name(key.kind)
                        ),
                    ),
                },
            }
        }
        examples.push(Example {
            mpn: e.mpn.clone(),
            params,
            line,
        });
    }
    if examples.is_empty() {
        errs.at(
            hline,
            None,
            "no examples: every family carries real MPNs with their parameters",
        );
    }

    (errs.list.len() == before).then(|| Scheme {
        id: h.id,
        version: h.version,
        maker: h.maker,
        aliases: h.aliases,
        class: h.class,
        title: h.title,
        sources: h
            .sources
            .into_iter()
            .map(|s| Source {
                doc: s.doc,
                url: s.url,
                fetched: s.fetched,
                pages: s.pages,
            })
            .collect(),
        file: errs.file.to_string(),
        closed,
        min_stem: h.min_stem,
        implies,
        variants,
        available,
        excluded,
        examples,
    })
}

/// Whether a leaf can match texts of more than one length
fn variable(l: &Leaf) -> bool {
    match &l.kind {
        LeafKind::Free(_, a, b) => a != b,
        LeafKind::Code { code, .. } => {
            matches!(code.kind, CodeKind::Rkm | CodeKind::Int | CodeKind::Decimal)
                && code.digits.is_none()
        }
        LeafKind::Table(es) => {
            let mut lens = es.iter().map(|e| e.code.len());
            let first = lens.next();
            lens.any(|l| Some(l) != first)
        }
        _ => false,
    }
}

fn kind_name(k: Kind) -> String {
    match k {
        Kind::Quantity(u) => format!("quantity in {u}"),
        Kind::Tolerance => "tolerance (±10%)".into(),
        Kind::Enum => "word".into(),
    }
}

/// A row's or a `when`'s condition on a segment, its texts read with the segment's param
fn row_cond(
    name: &str,
    v: &RawRowValue,
    params_of: &BTreeMap<String, Option<&'static Key>>,
) -> Result<Cond, String> {
    let Some(param) = params_of.get(name) else {
        return Err(format!("{name:?} is not a segment"));
    };
    let read = |t: &str| -> Result<Value, String> {
        let k = param.expect("checked by the caller");
        Value::parse(k.kind, t).ok_or_else(|| format!("{t:?} is not a {}", kind_name(k.kind)))
    };
    let test = match (v, param) {
        (RawRowValue::Code { code }, _) => Test::Codes(code.list()),
        (RawRowValue::One(s), None) => Test::Codes(vec![s.clone()]),
        (RawRowValue::Many(l), None) => Test::Codes(l.clone()),
        (RawRowValue::Range { .. }, None) => {
            return Err("a range needs a segment with a param".into());
        }
        (RawRowValue::One(s), Some(_)) => Test::Values(vec![read(s)?]),
        (RawRowValue::Many(l), Some(_)) => {
            Test::Values(l.iter().map(|s| read(s)).collect::<Result<_, _>>()?)
        }
        (RawRowValue::Range { min, max, series }, Some(k)) => {
            if !matches!(k.kind, Kind::Quantity(_) | Kind::Tolerance) {
                return Err(format!("a range needs a quantity, {} is a word", k.name));
            }
            if min.is_none() && max.is_none() && series.is_none() {
                return Err("a range needs min, max or series".into());
            }
            let min = min.as_deref().map(read).transpose()?;
            let max = max.as_deref().map(read).transpose()?;
            if let (Some(a), Some(b)) = (&min, &max)
                && a.compare(b).is_some_and(|o| o.is_gt())
            {
                return Err("min is above max".into());
            }
            Test::Range {
                min,
                max,
                series: series_list(series.as_ref())?,
            }
        }
    };
    Ok(Cond {
        segment: name.to_string(),
        test,
    })
}

/// A condition may only name values its segment can hold: a table's values, a literal's text
fn check_cond(c: &Cond, variants: &[Variant], errs: &mut Errors, line: usize) {
    let leaves: Vec<&Leaf> = variants
        .iter()
        .flat_map(|v| &v.leaves)
        .filter(|l| l.name == c.segment)
        .collect();
    let holds = |code: Option<&str>, value: Option<&Value>| {
        leaves.iter().any(|l| match &l.kind {
            LeafKind::Table(es) => es.iter().any(|e| {
                code.is_some_and(|c| c.eq_ignore_ascii_case(&e.code))
                    || value.is_some_and(|v| e.value.as_ref().is_some_and(|x| x.matches(v, None)))
            }),
            LeafKind::Literal(s) => code.is_some_and(|c| c.eq_ignore_ascii_case(s)),
            LeafKind::Absent(a) => value.is_some_and(|v| a.as_ref() == Some(v)),
            _ => true,
        })
    };
    match &c.test {
        Test::Codes(cs) => {
            for code in cs {
                if !holds(Some(code), None) {
                    errs.at(
                        line,
                        Some(&c.segment),
                        format!("{code:?} is not a code of the segment"),
                    );
                }
            }
        }
        Test::Values(vs) => {
            for v in vs {
                if !holds(None, Some(v)) {
                    errs.at(
                        line,
                        Some(&c.segment),
                        format!("{v} is not a value the segment can hold"),
                    );
                }
            }
        }
        Test::Range { .. } | Test::Raw(_) => {}
    }
}

fn series_list(s: Option<&OneOrMany>) -> Result<Vec<ESeries>, String> {
    let Some(s) = s else { return Ok(Vec::new()) };
    s.list()
        .iter()
        .map(|n| ESeries::from_name(n).ok_or_else(|| format!("{n:?} is not an E series (E3..E96)")))
        .collect()
}

struct Expand<'a, 'b> {
    errs: &'a mut Errors<'b>,
    too_many: bool,
}

type Options = Vec<(Vec<Leaf>, u32)>;

impl Expand<'_, '_> {
    fn product(&mut self, acc: Options, opts: Options) -> Options {
        if acc.len() * opts.len().max(1) > MAX_VARIANTS {
            self.too_many = true;
            return acc;
        }
        let mut out = Vec::new();
        for (a, ra) in &acc {
            for (o, ro) in &opts {
                let mut v = a.clone();
                v.extend(o.iter().cloned());
                out.push((v, ra + ro));
            }
        }
        out
    }

    /// The ways one segment can be written: each a list of leaves and a rank (0 = first choice)
    fn segment(
        &mut self,
        s: &RawSegment,
        line: usize,
        name: Option<&str>,
        param: Option<&str>,
    ) -> Options {
        let name = s.name.as_deref().or(name);
        let param_name = s.param.as_deref().or(param);
        let bodies = [
            s.literal.is_some(),
            s.table.is_some(),
            s.code.is_some(),
            s.free.is_some(),
            s.group.is_some(),
            s.alternatives.is_some(),
        ]
        .iter()
        .filter(|x| **x)
        .count();
        let label = name.unwrap_or("?");
        if bodies != 1 {
            self.errs.at(
                line,
                Some(label),
                "a segment is exactly one of literal, table, code, free, group, alternatives",
            );
            return vec![(Vec::new(), 0)];
        }
        let param = match param_name {
            None => None,
            Some(p) => match keys::key(p) {
                Some(k) => Some(k),
                None => {
                    self.errs.at(
                        line,
                        Some(label),
                        format!("param {p:?} is not a parameter key"),
                    );
                    return vec![(Vec::new(), 0)];
                }
            },
        };
        if s.group.is_some() && (s.param.is_some() || s.absent.is_some()) {
            self.errs.at(
                line,
                Some(label),
                "a group has no param or absent of its own; its segments do",
            );
        }
        let when = match &s.when {
            None => Vec::new(),
            Some(w) => {
                let mut out = Vec::new();
                for (k, v) in w {
                    // The param of the named segment is known only after the whole tree is read: store texts,
                    // resolved below in `resolve_when`
                    out.push(Cond {
                        segment: k.clone(),
                        test: Test::Raw(v.clone()),
                    });
                }
                out
            }
        };
        let mut opts: Options = if let Some(alts) = &s.alternatives {
            if alts.is_empty() {
                self.errs.at(line, Some(label), "alternatives is empty");
            }
            let mut out = Vec::new();
            for (i, a) in alts.iter().enumerate() {
                let inner_name = if a.group.is_some() { None } else { name };
                if a.optional {
                    self.errs.at(
                        line,
                        Some(label),
                        "an alternative cannot be optional; make the segment optional",
                    );
                }
                for (leaves, r) in self.segment(
                    a,
                    line,
                    inner_name,
                    if a.group.is_some() { None } else { param_name },
                ) {
                    out.push((leaves, r + i as u32));
                }
            }
            out
        } else if let Some(g) = &s.group {
            let mut acc = vec![(Vec::new(), 0)];
            for inner in g {
                let o = self.segment(inner, line, None, None);
                acc = self.product(acc, o);
            }
            acc
        } else {
            let Some(name) = name else {
                self.errs.at(line, None, "a segment needs a name");
                return vec![(Vec::new(), 0)];
            };
            match self.leaf(s, line, name, param) {
                Some(l) => vec![(vec![l], 0)],
                None => vec![(Vec::new(), 0)],
            }
        };
        for (leaves, _) in &mut opts {
            for l in leaves.iter_mut() {
                l.when.extend(when.iter().cloned());
            }
        }
        if s.optional {
            let absent = match (&s.absent, param) {
                (Some(t), Some(k)) => match Value::parse(k.kind, t) {
                    Some(v) if !keys::allows(k, &v) => {
                        self.errs.at(
                            line,
                            Some(label),
                            format!("absent {t:?}: {} is one of {}", k.name, k.words.join(", ")),
                        );
                        None
                    }
                    Some(v) => Some(v),
                    None => {
                        self.errs.at(
                            line,
                            Some(label),
                            format!("absent {t:?} is not a {}", kind_name(k.kind)),
                        );
                        None
                    }
                },
                (Some(_), None) => {
                    self.errs.at(line, Some(label), "absent needs a param");
                    None
                }
                _ => None,
            };
            let n = opts.len() as u32;
            let leaves = match name {
                Some(name) if s.group.is_none() => vec![Leaf {
                    name: name.to_string(),
                    param,
                    kind: LeafKind::Absent(absent),
                    when: Vec::new(),
                    line,
                }],
                _ => Vec::new(),
            };
            opts.push((leaves, n));
        } else if s.absent.is_some() {
            self.errs
                .at(line, Some(label), "absent without optional = true");
        }
        opts
    }

    fn leaf(
        &mut self,
        s: &RawSegment,
        line: usize,
        name: &str,
        param: Option<&'static Key>,
    ) -> Option<Leaf> {
        let mut err = |m: String| self.errs.at(line, Some(name), m);
        let code_opts = s.unit.is_some()
            || s.series.is_some()
            || s.scale.is_some()
            || s.places.is_some()
            || s.letters.is_some()
            || s.lead_zero.is_some()
            || s.pad_whole.is_some();
        if s.code.is_none() && (code_opts || s.digits.is_some()) {
            err("unit, series, digits, scale, places, letters, lead_zero and pad_whole belong to a code segment".into());
        }
        let kind = if let Some(l) = &s.literal {
            if l.is_empty() || !l.is_ascii() {
                err("a literal is non-empty ASCII".into());
                return None;
            }
            if param.is_some() {
                err("a literal sets no param; use a table with one entry".into());
            }
            LeafKind::Literal(l.clone())
        } else if let Some(t) = &s.table {
            if t.is_empty() {
                err("the table is empty".into());
                return None;
            }
            let mut entries = Vec::new();
            let mut defaults = 0;
            for (code, e) in t {
                if code.is_empty() || !code.is_ascii() {
                    err(format!("table code {code:?}: non-empty ASCII"));
                    continue;
                }
                let (text, default) = match e {
                    RawEntry::Value(v) => (Some(v.clone()), false),
                    RawEntry::Full { value, default, .. } => (value.clone(), *default),
                };
                defaults += usize::from(default);
                let value = match (param, text) {
                    (Some(k), Some(t)) => match Value::parse(k.kind, &t) {
                        Some(v) if !keys::allows(k, &v) => {
                            err(format!(
                                "table {code:?} = {t:?}: {} is one of {}",
                                k.name,
                                k.words.join(", ")
                            ));
                            None
                        }
                        Some(v) => Some(v),
                        None => {
                            err(format!(
                                "table {code:?} = {t:?}: not a {}",
                                kind_name(k.kind)
                            ));
                            None
                        }
                    },
                    (Some(_), None) => {
                        err(format!("table {code:?}: no value"));
                        None
                    }
                    // Without a param the texts only describe the codes
                    (None, _) => None,
                };
                entries.push(Entry {
                    code: code.clone(),
                    value,
                    default,
                });
            }
            if defaults > 1 {
                err("more than one default entry".into());
            }
            LeafKind::Table(entries)
        } else if let Some(c) = &s.code {
            let Some(kind) = CodeKind::from_name(c) else {
                err(format!(
                    "code {c:?}: one of eia3, eia4, eia96, rkm, decimal, int"
                ));
                return None;
            };
            let Some(k) = param else {
                err("a code segment needs a param".into());
                return None;
            };
            let Kind::Quantity(base) = k.kind else {
                err(format!(
                    "a code reads a number; {} is not a quantity",
                    k.name
                ));
                return None;
            };
            let (unit_exp, unit) = match &s.unit {
                None => (0, base),
                Some(u) => match split_unit(u) {
                    Some((e, u2)) if u2 == base => (e, u2),
                    _ => {
                        err(format!("unit {u:?} is not a unit of {} ({base})", k.name));
                        return None;
                    }
                },
            };
            let series = match series_list(s.series.as_ref()) {
                Ok(v) => v,
                Err(m) => {
                    err(m);
                    Vec::new()
                }
            };
            let mut code = Code::new(kind);
            code.digits = s.digits;
            code.scale = s.scale;
            code.places = s.places.unwrap_or(0);
            code.lead_zero = s.lead_zero.unwrap_or(false);
            code.pad_whole = s.pad_whole.unwrap_or(false);
            if kind == CodeKind::Rkm {
                code.letters = match &s.letters {
                    Some(l) => {
                        let mut out = Vec::new();
                        for (c, e) in l {
                            let mut ch = c.chars();
                            match (ch.next(), ch.next()) {
                                (Some(x), None) if x.is_ascii_alphabetic() => {
                                    out.push((x.to_ascii_uppercase(), *e))
                                }
                                _ => err(format!("letters: {c:?} is not one letter")),
                            }
                        }
                        out
                    }
                    None => {
                        match (unit, unit_exp) {
                            ("Ω", 0) => vec![('R', 0), ('K', 3), ('M', 6), ('G', 9)],
                            ("V", 0) => vec![('V', 0)],
                            _ => {
                                err("rkm needs letters for this unit, like letters = { R = 0, K = 3 }".into());
                                Vec::new()
                            }
                        }
                    }
                };
            } else if s.letters.is_some() {
                err("letters belong to an rkm code".into());
            }
            if kind != CodeKind::Decimal && (s.scale.is_some() || s.places.is_some()) {
                err("scale and places belong to a decimal code".into());
            }
            if (s.lead_zero.is_some() || s.pad_whole.is_some())
                && !matches!(kind, CodeKind::Eia3 | CodeKind::Eia4)
            {
                err("lead_zero and pad_whole belong to an eia3 or eia4 code".into());
            }
            LeafKind::Code {
                code,
                unit_exp,
                unit,
                series,
            }
        } else if let Some(f) = &s.free {
            let Some(cs) = CharSet::parse(&f.chars) else {
                err(format!("free chars {:?}: like \"A-Z0-9\"", f.chars));
                return None;
            };
            let (a, b) = match f.len {
                RawLen::Exact(n) => (n, n),
                RawLen::Range { min, max } => (min, max),
            };
            if a == 0 || a > b || b > 16 {
                err("free len is 1..16, min not above max".into());
                return None;
            }
            if param.is_some() {
                err("a free segment sets no param".into());
            }
            LeafKind::Free(cs, a, b)
        } else {
            return None;
        };
        Some(Leaf {
            name: name.to_string(),
            param,
            kind,
            when: Vec::new(),
            line,
        })
    }
}

/// Turn the `when` conditions into tests on the named segment's values, now that its param is known
fn resolve_when(scheme_variants: &mut [Variant], errs: &mut Errors) {
    let mut params_of: BTreeMap<String, Option<&'static Key>> = BTreeMap::new();
    for v in scheme_variants.iter() {
        for l in &v.leaves {
            params_of.entry(l.name.clone()).or_insert(l.param);
        }
    }
    for v in scheme_variants.iter_mut() {
        for l in &mut v.leaves {
            for c in &mut l.when {
                let Test::Raw(raw) = &c.test else { continue };
                match row_cond(&c.segment, raw, &params_of) {
                    Ok(done) => c.test = done.test,
                    Err(m) => {
                        errs.at(l.line, Some(&l.name), format!("when: {m}"));
                        c.test = Test::Codes(Vec::new());
                    }
                }
            }
        }
    }
}
