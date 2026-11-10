//! The built-in scheme files: every one loads, its id follows its path, and its examples round-trip (the loader
//! checks that). With `MPN_CORPUS_DIR` set, also the corpus of real parts exported by mx3.

use mpn_schemes::SchemeSet;
use std::collections::BTreeMap;

#[test]
fn every_builtin_scheme_loads_and_round_trips() {
    let files = SchemeSet::embedded_files();
    assert!(!files.is_empty(), "no scheme files were built in");
    let (set, errs) = SchemeSet::from_texts(
        files.iter().map(|(p, t)| (p.to_string(), t.to_string())),
        true,
    );
    let msg: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
    assert!(errs.is_empty(), "\n{}", msg.join("\n"));
    assert_eq!(set.schemes().len(), files.len());
    for s in set.schemes() {
        assert!(!s.examples().is_empty(), "{}: no examples", s.id);
        assert!(!s.sources.is_empty(), "{}: no sources", s.id);
    }
}

#[test]
fn each_example_parses_to_its_own_scheme_first() {
    let set = SchemeSet::embedded().unwrap();
    for s in set.schemes() {
        for e in s.examples() {
            let p = set.parse(&e.mpn, None);
            assert_eq!(
                p.first().map(|p| p.scheme.as_str()),
                Some(s.id.as_str()),
                "{}",
                e.mpn
            );
        }
    }
}

/// One line of the corpus mx3 exports (`mx3 mpn corpus`): a part's maker, MPN and supplier-given values
#[derive(serde::Deserialize)]
struct Line {
    maker: String,
    mpn: String,
    #[serde(default)]
    params: BTreeMap<String, String>,
}

#[derive(Default)]
struct Tally {
    parsed: usize,
    agreed: usize,
    disagreed: BTreeMap<String, usize>,
}

/// Reports per scheme how many corpus MPNs parsed and how many values agreed. It fails only on unreadable input:
/// a disagreement is either a scheme bug or a supplier's, and a person decides which.
#[test]
fn corpus_round_trip() {
    let Ok(dir) = std::env::var("MPN_CORPUS_DIR") else {
        eprintln!("corpus_round_trip: skipped, MPN_CORPUS_DIR is not set");
        return;
    };
    let set = SchemeSet::embedded().unwrap();
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut lines = 0usize;
    let mut unparsed = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("MPN_CORPUS_DIR is a directory")
        .flatten()
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "jsonl") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (n, raw) in text
            .lines()
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty())
        {
            let line: Line = serde_json::from_str(raw)
                .unwrap_or_else(|e| panic!("{}:{}: {e}", path.display(), n + 1));
            lines += 1;
            let parses = set.parse(&line.mpn, Some(&line.maker));
            let Some(p) = parses.first().filter(|p| p.complete) else {
                unparsed += 1;
                continue;
            };
            let t = tallies.entry(p.scheme.clone()).or_default();
            t.parsed += 1;
            for (k, v) in &line.params {
                let (Some(got), Some(key)) = (p.get(k), mpn_schemes::keys::key(k)) else {
                    continue;
                };
                match mpn_schemes::Value::parse(key.kind, v) {
                    // mx3 exports numbers through f32 on the way (2.2 µF as 0.0000022000002 F): 1e-6 relative is equal
                    Some(want) if want.matches(got, Some(1e-6)) => t.agreed += 1,
                    Some(_) => *t.disagreed.entry(k.clone()).or_default() += 1,
                    None => {}
                }
            }
        }
    }
    eprintln!("corpus: {lines} MPNs, {unparsed} not read whole by any scheme");
    for (id, t) in &tallies {
        eprintln!(
            "  {id}: {} parsed, {} values agreed, disagreed {:?}",
            t.parsed, t.agreed, t.disagreed
        );
    }
}
