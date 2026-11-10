//! The engine against made-up fixture schemes: loading, parse, generate, overlay, and one broken fixture per
//! refusal of the validator.

use mpn_schemes::{Flag, Prefs, Requirement, SchemeSet, Term};
use std::path::Path;

fn fixtures() -> SchemeSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let texts = ["toy_mlcc.toml", "toy_res.toml"]
        .iter()
        .map(|f| (f.to_string(), std::fs::read_to_string(dir.join(f)).unwrap()));
    let (set, errs) = SchemeSet::from_texts(texts, false);
    assert!(
        errs.is_empty(),
        "{}",
        errs.iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    set
}

fn terms(t: &[&str]) -> Vec<Term> {
    t.iter().map(|s| s.parse().unwrap()).collect()
}

#[test]
fn parse_reads_every_segment() {
    let set = fixtures();
    let p = set.parse("TC05B104KO5NN-C", None);
    assert_eq!(p.len(), 1, "{p:#?}");
    let p = &p[0];
    assert!(p.complete);
    assert_eq!(p.scheme, "toy.mlcc");
    assert_eq!(p.version, 1);
    assert_eq!(p.get("capacitance").unwrap().to_string(), "100nF");
    assert_eq!(p.get("height").unwrap().to_string(), "0.5mm");
    assert_eq!(p.get("technology").unwrap().to_string(), "mlcc");
    let spec = p.segments.iter().find(|s| s.name == "spec").unwrap();
    assert_eq!((spec.start, spec.end, spec.code.as_str()), (11, 13, "NN"));
    assert!(p.flags.is_empty(), "{:?}", p.flags);
}

#[test]
fn parse_flags_without_rejecting() {
    let set = fixtures();
    // 0402 X7R 220 nF is in no availability row
    let p = &set.parse("TC05B224KO5NNC", None)[0];
    assert!(p.complete);
    assert!(p.flags.contains(&Flag::NotInTable));
    assert!(p.flags.contains(&Flag::Absent {
        segment: "dash".into()
    }));
    // 0402 100 nF at 25 V is excluded
    let p = &set.parse("TC05B104KA5NNC", None)[0];
    assert!(p.flags.contains(&Flag::NotInTable));
    // 2.7 nF is E12, 2.4 nF is not
    let p = &set.parse("TC05B242KO5NNC", None)[0];
    assert!(p.flags.contains(&Flag::OutsideSeries {
        segment: "capacitance".into()
    }));
}

#[test]
fn parse_partial_and_hostile_input() {
    let set = fixtures();
    let p = set.parse("TC05B104KO5NN-CXX", None);
    assert_eq!(p.len(), 1);
    assert!(!p[0].complete);
    assert_eq!(p[0].rest, "XX");
    // The thickness code of another size is not read
    let p = set.parse("TC05B104KO8NNC", None);
    assert!(p.iter().all(|p| !p.complete));
    assert!(set.parse("", None).is_empty());
    assert!(set.parse("TC05B104KO5NNÄC", None).is_empty());
    assert!(set.parse(&"TC".repeat(40), None).is_empty());
    assert!(
        set.parse(&format!("TR2J{}", "1".repeat(55)), None)
            .iter()
            .all(|p| !p.complete)
    );
}

#[test]
fn parse_alternatives_and_absent() {
    let set = fixtures();
    let p = &set.parse("TR3F4991", None)[0];
    assert_eq!(p.get("resistance").unwrap().to_string(), "4.99kΩ");
    assert_eq!(p.get("tolerance").unwrap().to_string(), "±1%");
    assert_eq!(p.get("packaging").unwrap().to_string(), "bulk");
    let p = &set.parse("tr2j4r7t", None)[0];
    assert_eq!(p.get("resistance").unwrap().to_string(), "4.7Ω");
}

#[test]
fn maker_hint_ranks_first() {
    let set = fixtures();
    let p = set.parse("TC05B104KO5NNC", Some("toycap"));
    assert_eq!(p[0].maker, "Toy Capacitors");
}

#[test]
fn generate_fixes_what_terms_and_rows_decide() {
    let set = fixtures();
    let req = Requirement {
        class: Some("capacitor".into()),
        terms: terms(&[
            "capacitance=100nF",
            "dielectric=X7R",
            "case_eia=0402",
            "rated_voltage>=16V",
            "tolerance=±10%",
        ]),
        ..Requirement::default()
    };
    let g = set.generate(&req, &Prefs::default());
    let pats: Vec<String> = g.candidates.iter().map(|c| c.pattern.to_string()).collect();
    // 16 V first; 25 V is excluded for 100 nF in 0402; 50 V is in no row. The optional dash comes first.
    assert_eq!(pats, ["TC05B104KO5??-C", "TC05B104KO5??C"], "{g:#?}");
    let c = &g.candidates[0];
    assert_eq!(c.pattern.stem(), "TC05B104KO5");
    assert_eq!(c.min_stem, "TC05");
    assert!(c.pattern.matches("TC05B104KO5NN-C"));
    assert!(
        c.params
            .iter()
            .any(|p| p.key == "height" && p.value.to_string() == "0.5mm")
    );
}

#[test]
fn generate_rows_fix_holes_and_order_by_bound() {
    let set = fixtures();
    let req = Requirement {
        terms: terms(&[
            "capacitance=1uF",
            "case_eia=0603",
            "rated_voltage>=16V",
            "tolerance=±10%",
        ]),
        ..Requirement::default()
    };
    let g = set.generate(&req, &Prefs::default());
    let pats: Vec<String> = g.candidates.iter().map(|c| c.pattern.to_string()).collect();
    // Only X7R exists in 0603; thickness 8 from the row; voltages closest to 16 V first
    assert_eq!(
        &pats[..3],
        ["TC10B105KO8??-C", "TC10B105KO8??C", "TC10B105KA8??-C"],
        "{pats:?}"
    );
    assert!(g.candidates.iter().all(|c| c.scheme == "toy.mlcc"));
}

#[test]
fn generate_holes_and_preferences() {
    let set = fixtures();
    let req = Requirement {
        schemes: vec!["toy.mlcc".into()],
        terms: terms(&[
            "capacitance=10nF",
            "case_eia=0402",
            "dielectric=X5R",
            "rated_voltage=16V",
            "tolerance=±10%",
        ]),
        ..Requirement::default()
    };
    let prefs = Prefs {
        fill_defaults: false,
        ..Prefs::default()
    };
    let g = set.generate(&req, &prefs);
    assert_eq!(g.candidates[0].pattern.to_string(), "TC05A103KO5??-[C|D]");
    let prefs = Prefs {
        prefer: vec![(
            "packaging".into(),
            vec!["reel_13in_paper".parse_value("packaging")],
        )],
        ..Prefs::default()
    };
    let g = set.generate(&req, &prefs);
    assert_eq!(g.candidates[0].pattern.to_string(), "TC05A103KO5??-D");
}

trait ParseValue {
    fn parse_value(&self, key: &str) -> mpn_schemes::Value;
}

impl ParseValue for &str {
    fn parse_value(&self, key: &str) -> mpn_schemes::Value {
        mpn_schemes::Value::parse(mpn_schemes::keys::key(key).unwrap().kind, self).unwrap()
    }
}

#[test]
fn generate_resistor_ranges_from_series() {
    let set = fixtures();
    let req = Requirement {
        class: Some("resistor".into()),
        terms: terms(&["resistance~10kΩ/3%", "tolerance=±1%", "case_eia=0603"]),
        ..Requirement::default()
    };
    let g = set.generate(&req, &Prefs::default());
    let pats: Vec<String> = g.candidates.iter().map(|c| c.pattern.to_string()).collect();
    // Closest first: 10.0k, then 10.2k and 9.76k; the packaging reel before bulk
    assert_eq!(&pats[..2], ["TR3F1002T", "TR3F1002"], "{pats:?}");
    assert!(pats.contains(&"TR3F1022T".to_string()));
    assert!(pats.contains(&"TR3F9761T".to_string()));
    assert!(!pats.iter().any(|p| p.contains("1052")));

    let req = Requirement {
        terms: terms(&[
            "resistance=1kΩ..2kΩ",
            "tolerance=±5%",
            "case_eia=0402",
            "packaging=bulk",
        ]),
        ..Requirement::default()
    };
    let g = set.generate(&req, &Prefs::default());
    let pats: Vec<String> = g.candidates.iter().map(|c| c.pattern.to_string()).collect();
    assert_eq!(
        pats,
        [
            "TR2J102", "TR2J112", "TR2J122", "TR2J132", "TR2J152", "TR2J162", "TR2J182", "TR2J202"
        ]
    );

    // Excluded: 0402 from 1 MΩ
    let req = Requirement {
        class: Some("resistor".into()),
        terms: terms(&["resistance=2.2MΩ", "tolerance=±5%", "case_eia=0402"]),
        ..Requirement::default()
    };
    assert!(set.generate(&req, &Prefs::default()).candidates.is_empty());
}

#[test]
fn generate_reports_unchecked_and_limits() {
    let set = fixtures();
    let req = Requirement {
        class: Some("resistor".into()),
        terms: terms(&[
            "case_eia=0603",
            "tolerance=±5%",
            "resistance=10kΩ",
            "power>=0.1W",
        ]),
        ..Requirement::default()
    };
    let g = set.generate(&req, &Prefs::default());
    assert!(!g.candidates.is_empty());
    assert!(g.candidates.iter().all(|c| c.unchecked == ["power"]));
    // A range on a code without a series cannot be listed
    let req = Requirement {
        terms: terms(&["case_eia=0402", "capacitance>=1nF"]),
        schemes: vec!["toy.mlcc".into()],
        ..Requirement::default()
    };
    let prefs = Prefs {
        max_patterns: 5,
        ..Prefs::default()
    };
    let g = set.generate(&req, &prefs);
    assert_eq!(g.candidates.len(), 5);
}

#[test]
fn overlay_replaces_by_version() {
    let mut set = fixtures();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let text = std::fs::read_to_string(dir.join("toy_res.toml")).unwrap();
    let newer = text.replace("version = 2", "version = 3");
    let (o, e) = SchemeSet::from_texts([("overlay/res.toml".to_string(), newer)], false);
    assert!(e.is_empty());
    let said = set.overlay(o);
    assert_eq!(said, ["toy.res v3 replaces v2 (overlay/res.toml)"]);
    assert_eq!(set.get("toy.res").unwrap().version, 3);
    let (o, _) = SchemeSet::from_texts([("old.toml".to_string(), text)], false);
    assert!(set.overlay(o)[0].contains("ignored"));
}

#[test]
fn terms_parse_and_show() {
    for t in [
        "capacitance=100nF",
        "rated_voltage>=16V",
        "tolerance<=±5%",
        "dielectric=X7R|X5R",
        "resistance=1kΩ..10kΩ",
    ] {
        let term: Term = t.parse().unwrap();
        assert_eq!(term.to_string(), t);
    }
    assert!("capacitance=16V".parse::<Term>().is_err());
    assert!("nonsense=1".parse::<Term>().is_err());
}

/// Each broken fixture names the error it must raise in its first line: `# expect: <text>`
#[test]
fn validator_refuses_each_broken_fixture() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/broken");
    let mut n = 0;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        let text = std::fs::read_to_string(&p).unwrap();
        let want = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("# expect: "))
            .expect("first line # expect:");
        let (set, errs) = SchemeSet::from_texts([(p.display().to_string(), text.clone())], false);
        assert!(set.schemes().is_empty(), "{} loaded", p.display());
        let all: Vec<String> = errs.iter().map(|e| e.to_string()).collect();
        assert!(
            all.iter().any(|e| e.contains(want)),
            "{}: want {want:?}, got {all:#?}",
            p.display()
        );
        assert!(
            errs.iter().all(|e| e.line.is_some()),
            "{}: an error without a line: {all:#?}",
            p.display()
        );
        n += 1;
    }
    assert!(n >= 15, "only {n} broken fixtures");
}
