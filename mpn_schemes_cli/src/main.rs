//! `mpns`: the command line of mpn_schemes (CLI-1..4). Parse MPNs, generate patterns for a requirement, check
//! scheme files and list the loaded schemes.

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use mpn_schemes::{Error, Parse, Prefs, Requirement, SchemeSet, Term, Value, keys};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_SHA"),
    ", built ",
    env!("BUILD_TIME"),
    ")"
);

/// Manufacturer part-number schemes: decode an MPN into parameters, craft MPN patterns for wanted parameters
#[derive(Parser)]
#[command(name = "mpns", version = VERSION, after_help = EXAMPLES)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
    /// Lay the schemes of this directory over the built-in ones (same id with a higher version replaces)
    #[arg(long, global = true, value_name = "DIR")]
    schemes_dir: Option<PathBuf>,
    /// Colour in the output
    #[arg(long, global = true, value_enum, default_value_t = Color::Auto)]
    color: Color,
}

const EXAMPLES: &str = "Examples:
  mpns parse CL05B104KO5NNNC
  mpns parse RC0402FR-0710KL --maker Yageo --json
  mpns gen capacitance=100nF dielectric=X7R case_eia=0402 'rated_voltage>=16V'
  mpns gen 'resistance~4.7kΩ/1%' tolerance=±1% case_eia=0603 --class resistor
  mpns check schemes/
  mpns list --keys";

#[derive(Clone, Copy, ValueEnum)]
enum Color {
    /// Colour when stdout is a terminal and NO_COLOR is not set
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
enum Cmd {
    /// Decode MPNs into parameters, segment by segment (exit 1 when one does not parse whole)
    Parse {
        /// The part numbers
        #[arg(required = true)]
        mpns: Vec<String>,
        /// The maker, to rank its schemes first
        #[arg(long)]
        maker: Option<String>,
        /// Show every reading, not only the best
        #[arg(long)]
        all: bool,
        /// JSON lines: one object per MPN with all its readings
        #[arg(long)]
        json: bool,
    },
    /// Generate MPN patterns for wanted parameters, best first (holes are `?`, `*` or `[A|B]`)
    #[command(name = "gen")]
    Generate {
        /// Terms: key=value, key>=value, key<=value, key=a..b, key=a|b, key~value/1%
        #[arg(required = true)]
        terms: Vec<String>,
        /// The class of part (`capacitor`, `resistor.chip`); a class matches its sub-classes
        #[arg(long)]
        class: Option<String>,
        /// Only these makers (name or alias); repeatable
        #[arg(long)]
        maker: Vec<String>,
        /// Only these schemes (id); repeatable
        #[arg(long)]
        scheme: Vec<String>,
        /// Preferred values for a parameter no term sets: key=a|b; repeatable
        #[arg(long)]
        prefer: Vec<String>,
        /// Leave segments no term decides as holes instead of the table's default
        #[arg(long)]
        holes: bool,
        /// The most patterns
        #[arg(long, default_value_t = 50)]
        max: usize,
        #[arg(long)]
        json: bool,
    },
    /// Validate scheme files and round-trip their examples (none given: the built-in schemes)
    Check {
        /// Files or directories
        paths: Vec<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// List the loaded schemes, or with --keys the parameter keys a scheme may use
    List {
        #[arg(long)]
        keys: bool,
        #[arg(long)]
        json: bool,
    },
    /// Print a shell completion script
    Completions { shell: clap_complete::Shell },
}

struct Out {
    color: bool,
}

impl Out {
    fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
    fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
    fn warn(&self, s: &str) -> String {
        self.paint("33", s)
    }
    fn bad(&self, s: &str) -> String {
        self.paint("31", s)
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let color = match cli.color {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
    };
    let out = Out { color };
    if let Cmd::Completions { shell } = cli.cmd {
        clap_complete::generate(shell, &mut Cli::command(), "mpns", &mut std::io::stdout());
        return ExitCode::SUCCESS;
    }
    if let Cmd::Check { paths, json } = &cli.cmd {
        return check(&out, paths, *json);
    }
    let set = match load(&out, cli.schemes_dir.as_ref()) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match cli.cmd {
        Cmd::Parse {
            mpns,
            maker,
            all,
            json,
        } => parse(&out, &set, &mpns, maker.as_deref(), all, json),
        Cmd::Generate {
            terms,
            class,
            maker,
            scheme,
            prefer,
            holes,
            max,
            json,
        } => {
            let mut req = Requirement {
                class,
                makers: maker,
                schemes: scheme,
                ..Requirement::default()
            };
            for t in &terms {
                match t.parse::<Term>() {
                    Ok(t) => req.terms.push(t),
                    Err(e) => return usage(&out, &e),
                }
            }
            let mut prefs = Prefs {
                fill_defaults: !holes,
                max_patterns: max,
                ..Prefs::default()
            };
            for p in &prefer {
                match read_prefer(p) {
                    Ok(p) => prefs.prefer.push(p),
                    Err(e) => return usage(&out, &e),
                }
            }
            generate(&out, &set, &req, &prefs, json)
        }
        Cmd::List { keys, json } => list(&out, &set, keys, json),
        Cmd::Check { .. } | Cmd::Completions { .. } => unreachable!("handled above"),
    }
}

fn usage(out: &Out, msg: &str) -> ExitCode {
    eprintln!("{} {msg}", out.bad("error:"));
    ExitCode::from(2)
}

fn show_errors(out: &Out, errs: &[Error]) {
    for e in errs {
        eprintln!("{} {e}", out.bad("error:"));
    }
}

fn load(out: &Out, dir: Option<&PathBuf>) -> Result<SchemeSet, ExitCode> {
    let mut set = match SchemeSet::embedded() {
        Ok(s) => s,
        Err(errs) => {
            show_errors(out, &errs);
            return Err(ExitCode::FAILURE);
        }
    };
    if let Some(dir) = dir {
        let (o, errs) = SchemeSet::load_dir(dir);
        show_errors(out, &errs);
        for line in set.overlay(o) {
            eprintln!("{}", out.dim(&line));
        }
    }
    Ok(set)
}

fn read_prefer(p: &str) -> Result<(String, Vec<Value>), String> {
    let (k, v) = p.split_once('=').ok_or("--prefer key=a|b")?;
    let key = keys::key(k.trim()).ok_or_else(|| format!("{k:?} is not a parameter key"))?;
    let vals = v
        .split('|')
        .map(|t| {
            Value::parse(key.kind, t).ok_or_else(|| format!("{k}: {t:?} is not a valid value"))
        })
        .collect::<Result<_, _>>()?;
    Ok((key.name.to_string(), vals))
}

#[derive(serde::Serialize)]
struct ParseLine<'a> {
    mpn: &'a str,
    parses: &'a [Parse],
}

fn parse(
    out: &Out,
    set: &SchemeSet,
    mpns: &[String],
    maker: Option<&str>,
    all: bool,
    json: bool,
) -> ExitCode {
    let mut failed = false;
    for mpn in mpns {
        let parses = set.parse(mpn, maker);
        failed |= !parses.first().is_some_and(|p| p.complete);
        if json {
            let line = ParseLine {
                mpn,
                parses: &parses,
            };
            println!("{}", serde_json::to_string(&line).expect("serializable"));
            continue;
        }
        if parses.is_empty() {
            println!("{}  {}", out.bold(mpn), out.bad("no scheme reads it"));
            continue;
        }
        let shown = if all { parses.len() } else { 1 };
        for (i, p) in parses.iter().take(shown).enumerate() {
            let head = if p.complete {
                out.bold(mpn)
            } else {
                out.warn(mpn)
            };
            let what = if p.complete {
                String::new()
            } else {
                out.warn(&format!("  partial, not read: {:?}", p.rest))
            };
            let n = if parses.len() > 1 {
                format!("  ({} of {})", i + 1, parses.len())
            } else {
                String::new()
            };
            println!(
                "{head}  {} v{}  {}{n}{what}",
                p.scheme,
                p.version,
                out.dim(&p.maker)
            );
            for s in &p.segments {
                let code = if s.code.is_empty() {
                    "-".to_string()
                } else {
                    s.code.clone()
                };
                let meaning = match (&s.key, &s.value) {
                    (Some(k), Some(v)) => format!("{k} = {v}"),
                    (Some(k), None) => format!("{k} = ?"),
                    _ => String::new(),
                };
                println!("  {:<8} {:<14} {meaning}", code, out.dim(&s.name));
            }
            for prm in p.params.iter().filter(|x| x.segment.is_none()) {
                println!(
                    "  {:<8} {:<14} {} = {}",
                    "",
                    out.dim("implied"),
                    prm.key,
                    prm.value
                );
            }
            for f in &p.flags {
                let t = match f {
                    mpn_schemes::Flag::NotInTable => {
                        "not in the scheme's availability table".to_string()
                    }
                    mpn_schemes::Flag::OutsideSeries { segment } => {
                        format!("{segment}: outside its E series")
                    }
                    mpn_schemes::Flag::Absent { segment } => format!("{segment}: left out"),
                };
                println!("  {}", out.warn(&format!("note: {t}")));
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn generate(out: &Out, set: &SchemeSet, req: &Requirement, prefs: &Prefs, json: bool) -> ExitCode {
    let g = set.generate(req, prefs);
    if json {
        println!("{}", serde_json::to_string(&g).expect("serializable"));
    } else {
        for c in &g.candidates {
            let fixed: Vec<String> = c
                .params
                .iter()
                .filter(|p| p.segment.is_some() && !req.terms.iter().any(|t| t.key == p.key))
                .map(|p| format!("{}={}", p.key, p.value))
                .collect();
            let unchecked = if c.unchecked.is_empty() {
                String::new()
            } else {
                out.warn(&format!("  unchecked: {}", c.unchecked.join(", ")))
            };
            println!(
                "{:<24} {:<14} {}{unchecked}",
                out.bold(&c.pattern.to_string()),
                c.scheme,
                out.dim(&fixed.join(" "))
            );
        }
        for n in &g.notes {
            eprintln!("{}", out.warn(&format!("note: {n}")));
        }
    }
    if g.candidates.is_empty() {
        if !json {
            eprintln!("no scheme yields a pattern for this requirement");
        }
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[derive(serde::Serialize)]
struct CheckLine {
    file: String,
    line: Option<usize>,
    segment: Option<String>,
    message: String,
}

fn check(out: &Out, paths: &[PathBuf], json: bool) -> ExitCode {
    let (set, errs) = if paths.is_empty() {
        SchemeSet::from_texts(
            SchemeSet::embedded_files()
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string())),
            true,
        )
    } else {
        let mut set = SchemeSet::default();
        let mut errs = Vec::new();
        for p in paths {
            let (s, mut e) = if p.is_dir() {
                SchemeSet::load_dir(p)
            } else {
                match std::fs::read_to_string(p) {
                    Ok(t) => SchemeSet::from_texts([(p.display().to_string(), t)], false),
                    Err(e) => (
                        SchemeSet::default(),
                        vec![Error {
                            file: p.display().to_string(),
                            line: None,
                            segment: None,
                            message: e.to_string(),
                        }],
                    ),
                }
            };
            errs.append(&mut e);
            set.overlay(s);
        }
        (set, errs)
    };
    if json {
        for e in &errs {
            let l = CheckLine {
                file: e.file.clone(),
                line: e.line,
                segment: e.segment.clone(),
                message: e.message.clone(),
            };
            println!("{}", serde_json::to_string(&l).expect("serializable"));
        }
    } else {
        show_errors(out, &errs);
        for s in set.schemes() {
            println!(
                "{} {} v{}  {} examples",
                out.paint("32", "ok"),
                s.id,
                s.version,
                s.examples().len()
            );
        }
    }
    if errs.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[derive(serde::Serialize)]
struct ListLine<'a> {
    id: &'a str,
    version: u32,
    maker: &'a str,
    class: &'a str,
    title: &'a str,
    file: &'a str,
    keys: Vec<&'static str>,
    sources: &'a [mpn_schemes::Source],
}

fn list(out: &Out, set: &SchemeSet, keys_only: bool, json: bool) -> ExitCode {
    if keys_only {
        for k in keys::KEYS {
            let kind = match k.kind {
                mpn_schemes::Kind::Quantity(u) => format!("quantity {u}"),
                mpn_schemes::Kind::Tolerance => "tolerance".into(),
                mpn_schemes::Kind::Enum => "word".into(),
            };
            if json {
                println!(
                    "{}",
                    serde_json::json!({"key": k.name, "kind": kind, "doc": k.doc})
                );
            } else {
                println!("{:<16} {:<12} {}", out.bold(k.name), kind, out.dim(k.doc));
            }
        }
        return ExitCode::SUCCESS;
    }
    for s in set.schemes() {
        if json {
            let l = ListLine {
                id: &s.id,
                version: s.version,
                maker: &s.maker,
                class: &s.class,
                title: &s.title,
                file: &s.file,
                keys: s.keys(),
                sources: &s.sources,
            };
            println!("{}", serde_json::to_string(&l).expect("serializable"));
        } else {
            println!(
                "{:<20} v{:<3} {:<20} {:<26} {}",
                out.bold(&s.id),
                s.version,
                s.class,
                s.maker,
                out.dim(&s.title)
            );
        }
    }
    ExitCode::SUCCESS
}
