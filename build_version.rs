// Shared by the build scripts of mx3_backend, mx3_cli and mx3_ui (`include!`d): sets `GIT_SHA` (short, `-dirty`
// with local changes) and `BUILD_TIME` (the commit's time, UTC) for `--version` and `--self-check`, so any
// installed build can be traced back to its commit. Both fall back to `unknown` without git.
//
// Nothing here follows the clock or a path that may be missing (ops#12oct-0023): either makes the package dirty
// on every cargo call, and every test binary links again. `BUILD_TIME` is the commit's time, so two builds of
// one commit are the same build. The script runs again when HEAD moves (read from git's real directories: in a
// worktree `.git` is a file, and the members' scripts run in `crates/<name>/`) and when what is compiled in
// changes, which builds the crate anyway: so `-dirty` is right without a look at the index, which `git status`
// and `git add` rewrite.

use std::path::Path;
use std::process::Command;

pub fn emit() {
    watch();
    println!("cargo:rustc-env=GIT_SHA={}", git_sha());
    println!("cargo:rustc-env=BUILD_TIME={}", build_time());
}

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `rerun-if-changed` for what moves the SHA (HEAD and its log of this checkout, a worktree has its own; the
/// branch's ref in the repo, loose once it moved) and `-dirty` (the package's sources and the lock file; cargo
/// follows this script's own source). Only paths that exist: a missing one is always "changed".
fn watch() {
    let dir = |flag: &str| git(&["rev-parse", "--path-format=absolute", flag]);
    let mut paths: Vec<String> = ["src", "Cargo.toml"].map(String::from).into();
    if let (Some(own), Some(common)) = (dir("--git-dir"), dir("--git-common-dir")) {
        paths.extend([format!("{own}/HEAD"), format!("{own}/logs/HEAD")]);
        if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
            paths.push(format!("{common}/{branch}"));
        }
    }
    if let Some(top) = dir("--show-toplevel") {
        paths.push(format!("{top}/Cargo.lock"));
    }
    for p in paths.iter().filter(|p| Path::new(p).exists()) {
        println!("cargo:rerun-if-changed={p}");
    }
}

fn git_sha() -> String {
    let Some(sha) = git(&["rev-parse", "--short", "HEAD"]) else {
        return "unknown".into();
    };
    match git(&["status", "--porcelain"]) {
        Some(s) if !s.is_empty() => format!("{sha}-dirty"),
        _ => sha,
    }
}

/// The commit's time as "3 Oct 2026 18:20 UTC", without a date crate
fn build_time() -> String {
    let Some(secs) = git(&["log", "-1", "--format=%ct"]).and_then(|s| s.parse::<i64>().ok()) else {
        return "unknown".into();
    };
    let (days, rest) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{day} {} {year} {:02}:{:02} UTC",
        MONTHS[(month - 1) as usize],
        rest / 3600,
        rest % 3600 / 60
    )
}
