//! `mpns`: the command line of mpn_schemes. Subcommands come with the engine (parse, gen, check, list).

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version" | "-V") => println!(
            "mpns {} ({}, built {})",
            env!("CARGO_PKG_VERSION"),
            env!("GIT_SHA"),
            env!("BUILD_TIME")
        ),
        _ => {
            eprintln!("mpns: no commands yet (see FEATURES.md)");
            std::process::exit(2);
        }
    }
}
