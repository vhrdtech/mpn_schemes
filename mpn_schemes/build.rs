// Embeds every `schemes/<maker>/<family>.toml` of the repository (or of the package, once published) into the
// library: writes `$OUT_DIR/embedded.rs` with one `(path, include_str!(...))` per file, sorted by path.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dir = [manifest.join("../schemes"), manifest.join("schemes")]
        .into_iter()
        .find(|d| d.is_dir());
    let mut files = Vec::new();
    if let Some(dir) = &dir {
        println!("cargo:rerun-if-changed={}", dir.display());
        collect(dir, dir, &mut files);
    }
    files.sort();
    let mut out =
        String::from("/// The scheme files built into the crate: (path under `schemes/`, text)\n");
    out.push_str("pub(crate) const EMBEDDED: &[(&str, &str)] = &[\n");
    for (rel, abs) in &files {
        out.push_str(&format!("    ({rel:?}, include_str!({abs:?})),\n"));
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("embedded.rs");
    std::fs::write(dest, out).expect("write embedded.rs");
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(root, &p, out);
        } else if p.extension().is_some_and(|x| x == "toml") {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let abs = p
                .canonicalize()
                .unwrap_or(p.clone())
                .to_string_lossy()
                .into_owned();
            out.push((rel, abs));
        }
    }
}
