//! The set of loaded schemes (LANG-7): the files built into the crate, a directory, or both with one laid over
//! the other; parse and generate over the whole set.

use crate::file::RawFile;
use crate::generate::{self, Generated, Op, Prefs, Requirement, Term};
use crate::parse::{self, Parse};
use crate::scheme::{self, Error, Errors, Scheme};
use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));

/// Loaded and validated schemes
#[derive(Clone, Debug, Default)]
pub struct SchemeSet {
    schemes: Vec<Scheme>,
}

impl SchemeSet {
    /// The schemes built into the crate. The crate's tests prove they load; an error here means a broken build.
    pub fn embedded() -> Result<SchemeSet, Vec<Error>> {
        let (set, errs) = SchemeSet::from_texts(
            EMBEDDED.iter().map(|(p, t)| (p.to_string(), t.to_string())),
            true,
        );
        if errs.is_empty() { Ok(set) } else { Err(errs) }
    }

    /// The paths and texts of the files built into the crate
    pub fn embedded_files() -> &'static [(&'static str, &'static str)] {
        EMBEDDED
    }

    /// Load schemes from (name, text) pairs. With `in_tree` the names are paths under `schemes/` and the ids must
    /// follow them. A file with errors is left out and its errors are returned; the others load.
    pub fn from_texts(
        files: impl IntoIterator<Item = (String, String)>,
        in_tree: bool,
    ) -> (SchemeSet, Vec<Error>) {
        let mut set = SchemeSet::default();
        let mut errors = Vec::new();
        for (name, text) in files {
            match load_one(&name, &text, in_tree) {
                Ok(s) => {
                    if let Some(other) = set.get(&s.id) {
                        errors.push(Error {
                            file: name.clone(),
                            line: None,
                            segment: None,
                            message: format!("id {} is also in {}", s.id, other.file),
                        });
                    } else {
                        set.schemes.push(s);
                    }
                }
                Err(e) => errors.extend(e),
            }
        }
        (set, errors)
    }

    /// Load every `*.toml` under a directory, recursively, in path order
    pub fn load_dir(dir: &Path) -> (SchemeSet, Vec<Error>) {
        let mut files = Vec::new();
        let mut errors = Vec::new();
        collect(dir, &mut files, &mut errors);
        files.sort();
        let texts = files
            .into_iter()
            .filter_map(|p| match std::fs::read_to_string(&p) {
                Ok(t) => Some((p.display().to_string(), t)),
                Err(e) => {
                    errors.push(Error {
                        file: p.display().to_string(),
                        line: None,
                        segment: None,
                        message: e.to_string(),
                    });
                    None
                }
            });
        let texts: Vec<_> = texts.collect();
        let (set, mut more) = SchemeSet::from_texts(texts, false);
        errors.append(&mut more);
        (set, errors)
    }

    /// Lay `other` over this set: a scheme with the same id and a higher version replaces this one, a new id is
    /// added. Returns what happened to each of `other`'s schemes, one line each.
    pub fn overlay(&mut self, other: SchemeSet) -> Vec<String> {
        let mut said = Vec::new();
        for s in other.schemes {
            match self.schemes.iter_mut().find(|x| x.id == s.id) {
                Some(x) if s.version > x.version => {
                    said.push(format!(
                        "{} v{} replaces v{} ({})",
                        s.id, s.version, x.version, s.file
                    ));
                    *x = s;
                }
                Some(x) => said.push(format!(
                    "{} v{} from {} ignored: v{} is loaded already",
                    s.id, s.version, s.file, x.version
                )),
                None => {
                    said.push(format!("{} v{} added ({})", s.id, s.version, s.file));
                    self.schemes.push(s);
                }
            }
        }
        said
    }

    pub fn schemes(&self) -> &[Scheme] {
        &self.schemes
    }

    pub fn get(&self, id: &str) -> Option<&Scheme> {
        self.schemes.iter().find(|s| s.id == id)
    }

    /// Every reading of an MPN, best first: complete parses (inside the availability rows first, then values in
    /// their E series, then the maker `maker_hint` names, then fewer free characters); only when no scheme reads it whole, each scheme's longest
    /// partial reading. Never panics; an MPN longer than 64 characters or not ASCII gives nothing.
    pub fn parse(&self, mpn: &str, maker_hint: Option<&str>) -> Vec<Parse> {
        let Some(text) = parse::clean(mpn) else {
            return Vec::new();
        };
        let upper = text.to_ascii_uppercase();
        let mut all: Vec<(usize, Parse)> = Vec::new();
        for (i, s) in self.schemes.iter().enumerate() {
            let lits = s.leading_literals();
            if !lits
                .iter()
                .any(|l| l.is_empty() || upper.starts_with(l.as_str()))
            {
                continue;
            }
            all.extend(parse::parse_scheme(s, text).into_iter().map(|p| (i, p)));
        }
        let any_complete = all.iter().any(|(_, p)| p.complete);
        all.retain(|(_, p)| p.complete == any_complete);
        let hint = |i: usize| maker_hint.is_some_and(|h| self.schemes[i].maker_matches(h));
        all.sort_by_key(|(i, p)| {
            (
                p.flags.contains(&parse::Flag::NotInTable),
                p.flags
                    .iter()
                    .any(|f| matches!(f, parse::Flag::OutsideSeries { .. })),
                !hint(*i),
                std::cmp::Reverse(text.len() - p.rest.len()),
                p.free_chars(),
            )
        });
        all.into_iter().map(|(_, p)| p).collect()
    }

    /// The patterns for a requirement over every scheme it selects, best (lowest rank) first, at most
    /// `prefs.max_patterns`
    pub fn generate(&self, req: &Requirement, prefs: &Prefs) -> Generated {
        let mut g = Generated::default();
        for s in &self.schemes {
            if generate::selects(s, req) {
                let mut c =
                    generate::generate_scheme(s, req, prefs, prefs.max_patterns, &mut g.notes);
                g.candidates.append(&mut c);
            }
        }
        g.candidates.sort_by_key(|c| (c.rank, c.variant));
        g.candidates.truncate(prefs.max_patterns);
        g
    }
}

fn collect(dir: &Path, out: &mut Vec<std::path::PathBuf>, errors: &mut Vec<Error>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            errors.push(Error {
                file: dir.display().to_string(),
                line: None,
                segment: None,
                message: e.to_string(),
            });
            return;
        }
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out, errors);
        } else if p.extension().is_some_and(|x| x == "toml") {
            out.push(p);
        }
    }
}

/// Read, build and check one file, examples included
fn load_one(name: &str, text: &str, in_tree: bool) -> Result<Scheme, Vec<Error>> {
    let mut errs = Errors {
        file: name,
        text,
        list: Vec::new(),
    };
    let raw: RawFile = match toml::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            let line = e.span().map(|s| errs.line_of(s.start));
            return Err(vec![Error {
                file: name.to_string(),
                line,
                segment: None,
                message: e.message().to_string(),
            }]);
        }
    };
    let scheme = scheme::compile(raw, &mut errs, in_tree.then_some(name));
    let Some(scheme) = scheme else {
        return Err(errs.list);
    };
    check_examples(&scheme, &mut errs);
    if errs.list.is_empty() {
        Ok(scheme)
    } else {
        Err(errs.list)
    }
}

/// Every example both ways: parse gives exactly its parameters, and generate for them yields a pattern that
/// matches it
fn check_examples(s: &Scheme, errs: &mut Errors) {
    for e in &s.examples {
        let line = e.line;
        let parses: Vec<Parse> = parse::parse_scheme(s, &e.mpn)
            .into_iter()
            .filter(|p| p.complete)
            .collect();
        let Some(best) = parses.first() else {
            errs.at(line, None, format!("example {}: does not parse", e.mpn));
            continue;
        };
        // Ambiguous: another reading as good as the best (same flags, as many free characters) that means
        // something else. A reading through a fallback free segment ranks lower and is no ambiguity.
        let rank = |p: &Parse| {
            let outside = p
                .flags
                .iter()
                .any(|f| matches!(f, parse::Flag::OutsideSeries { .. }));
            (
                p.flags.contains(&parse::Flag::NotInTable),
                outside,
                p.free_chars(),
            )
        };
        let differ: Vec<&Parse> = parses
            .iter()
            .filter(|p| p.params != best.params && rank(p) == rank(best))
            .collect();
        if !differ.is_empty() {
            errs.at(
                line,
                None,
                format!(
                    "example {}: ambiguous, {} readings with different parameters",
                    e.mpn,
                    differ.len() + 1
                ),
            );
        }
        let mut wrong = Vec::new();
        for (k, v) in &e.params {
            match best.get(k) {
                Some(got) if got == v => {}
                Some(got) => wrong.push(format!("{k} is {got}, the example says {v}")),
                None => wrong.push(format!("{k} is not decoded, the example says {v}")),
            }
        }
        for p in best.params.iter().filter(|p| p.segment.is_some()) {
            if !e.params.iter().any(|(k, _)| k == p.key) {
                wrong.push(format!(
                    "{} = {} is decoded but not in the example",
                    p.key, p.value
                ));
            }
        }
        if !wrong.is_empty() {
            errs.at(
                line,
                None,
                format!("example {}: {}", e.mpn, wrong.join("; ")),
            );
            continue;
        }
        if let Some(parse::Flag::OutsideSeries { segment }) = best
            .flags
            .iter()
            .find(|f| matches!(f, parse::Flag::OutsideSeries { .. }))
        {
            errs.at(
                line,
                Some(segment),
                format!(
                    "example {}: the value is outside the segment's series",
                    e.mpn
                ),
            );
            continue;
        }
        if best.flags.contains(&parse::Flag::NotInTable) {
            errs.at(
                line,
                None,
                format!("example {}: no availability row covers it", e.mpn),
            );
            continue;
        }
        let req = Requirement {
            class: None,
            terms: e
                .params
                .iter()
                .map(|(k, v)| Term {
                    key: k.clone(),
                    op: Op::Eq {
                        value: v.clone(),
                        rel_tol: None,
                    },
                })
                .collect(),
            makers: Vec::new(),
            schemes: vec![s.id.clone()],
        };
        let mut notes = Vec::new();
        let prefs = Prefs {
            fill_defaults: false,
            ..Prefs::default()
        };
        let cands = generate::generate_scheme(s, &req, &prefs, 1000, &mut notes);
        if !cands.iter().any(|c| c.pattern.matches(&e.mpn)) {
            let got: Vec<String> = cands
                .iter()
                .take(3)
                .map(|c| c.pattern.to_string())
                .collect();
            errs.at(
                line,
                None,
                format!(
                    "example {}: generate does not yield it (got {})",
                    e.mpn,
                    if got.is_empty() {
                        "nothing".into()
                    } else {
                        got.join(", ")
                    }
                ),
            );
        }
    }
}
