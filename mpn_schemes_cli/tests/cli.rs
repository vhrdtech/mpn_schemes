//! `mpns` end to end, on the library's fixture schemes laid over the built-in ones

use std::process::Command;

fn mpns(args: &[&str]) -> (i32, String, String) {
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/../mpn_schemes/tests/fixtures");
    let out = Command::new(env!("CARGO_BIN_EXE_mpns"))
        .args(["--color", "never", "--schemes-dir", fixtures])
        .args(args)
        .output()
        .expect("mpns runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn parse_shows_segments_and_fails_on_unknown() {
    let (code, out, _) = mpns(&["parse", "TC05B104KO5NN-C"]);
    assert_eq!(code, 0);
    assert!(out.contains("capacitance = 100nF"), "{out}");
    let (code, out, _) = mpns(&["parse", "NOTAPART"]);
    assert_eq!(code, 1);
    assert!(out.contains("no scheme reads it"));
}

#[test]
fn parse_json_lines() {
    let (code, out, _) = mpns(&["parse", "--json", "TR3F4991", "TR2J103T"]);
    assert_eq!(code, 0);
    let lines: Vec<serde_json::Value> = out
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    let p = &lines[0]["parses"][0];
    assert_eq!(p["scheme"], "toy.res");
    assert_eq!(p["version"], 2);
    assert_eq!(p["params"][2]["value"], "4.99kΩ");
    assert_eq!(p["segments"][3]["code"], "4991");
}

#[test]
fn gen_patterns_and_usage_errors() {
    let (code, out, _) = mpns(&[
        "gen",
        "--scheme",
        "toy.res",
        "--class",
        "resistor",
        "resistance=10kΩ",
        "tolerance=±5%",
        "case_eia=0402",
    ]);
    assert_eq!(code, 0);
    assert!(out.lines().next().unwrap().starts_with("TR2J103T"), "{out}");
    let (code, _, err) = mpns(&["gen", "capacitance=16V"]);
    assert_eq!(code, 2);
    assert!(err.contains("not a valid value"), "{err}");
}

#[test]
fn check_reports_broken_files() {
    let broken = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../mpn_schemes/tests/fixtures/broken/no_sources.toml"
    );
    let (code, _, err) = mpns(&["check", broken]);
    assert_eq!(code, 1);
    assert!(err.contains("no sources"), "{err}");
    let (code, out, _) = mpns(&["check"]);
    assert_eq!(code, 0, "the built-in schemes check clean");
    let _ = out;
}

#[test]
fn version_names_the_commit() {
    let (code, out, _) = mpns(&["--version"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("mpns 0."), "{out}");
    assert!(out.contains("built"));
}
