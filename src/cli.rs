use colorex::Colorize;

use crate::commands::build::{build, BuildMode};
use crate::commands::check::check;
use crate::commands::fetch::fetch;
use crate::commands::info::info;
use crate::commands::init::init;
use crate::commands::run::run as run_package;
use crate::commands::setup::setup;
use crate::commands::tree::tree;
use crate::commands::update::update;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> i32 {
    match run_reported() {
        Ok(code) => code,
        Err(error) => {
            crate::ui::error(&error);
            error.category.code()
        }
    }
}

fn run_reported() -> Result<i32, diagnostic::Error> {
    use crate::{
        messages::Messages,
        outcome::{self, Outcome},
    };
    use diagnostic::Error;
    use serde_json::json;
    let mut raw = std::env::args_os().skip(1).peekable();
    let path = if raw.peek().is_some_and(|arg| arg == "--message-file") {
        raw.next();
        let path = raw
            .next()
            .filter(|p| !p.is_empty() && !p.to_string_lossy().starts_with('-'))
            .ok_or_else(|| Error::usage("missing path for --message-file"))?;
        Some(std::path::PathBuf::from(path))
    } else {
        None
    };
    let args = raw
        .map(|a| {
            a.into_string()
                .map_err(|_| Error::usage("command arguments must be UTF-8"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if path.is_some() && args.first().is_some_and(|s| s == "--message-file") {
        return Err(Error::usage("--message-file may only be specified once"));
    }
    let command = args.first().map(String::as_str).unwrap_or("help");
    let dry_run = matches!(command, "build" | "check" | "run")
        && args
            .iter()
            .skip(1)
            .take_while(|s| s.as_str() != "--")
            .any(|s| s == "--dry-run");
    let mut messages = Messages::open(path.as_deref(), command, dry_run)?;
    messages.emit(json!({"event":"started"}))?;
    let result = process::install_handlers()
        .map_err(Error::environment)
        .and_then(|()| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                dispatch(&args, &mut messages)
            }))
            .unwrap_or_else(|_| Err(Error::internal("unexpected Vex internal failure")))
        });
    let result = result.map_err(outcome::supervised);
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            crate::ui::error(&error);
            if let Err(log_error) = messages.diagnostic(&error) {
                crate::ui::error(&log_error);
                return Ok(if error.category == diagnostic::Category::Cancelled {
                    error.category.code()
                } else {
                    log_error.category.code()
                });
            }
            Outcome::error(&error)
        }
    };
    if let Err(error) = messages.emit(json!({"event":"finished", "success":outcome.code == 0,
        "origin":outcome.origin, "category":outcome.category, "exit_code":outcome.code, "signal":outcome.signal})) {
        if outcome.origin == "program" { crate::ui::warning(&error); } else { crate::ui::error(&error); }
        // Once the user program has executed, logging cannot replace its outcome.
        if outcome.origin != "program" && outcome.category != "cancelled" { return Ok(error.category.code()); }
    }
    Ok(outcome.code)
}

fn dispatch(
    args: &[String],
    messages: &mut crate::messages::Messages,
) -> Result<crate::outcome::Outcome, diagnostic::Error> {
    use crate::outcome::Outcome;
    use diagnostic::Error;
    #[cfg(debug_assertions)]
    if std::env::var_os("VEX_TEST_INTERNAL_FAILURE").is_some() {
        return Err(Error::internal("injected internal failure"));
    }
    if args.is_empty() {
        print_help();
        return Ok(Outcome::success());
    }
    match args[0].as_str() {
        "init" => init(&args[1..]).map(|()| Outcome::success()),
        "build" => build(BuildMode::Build, &args[1..], messages),
        "run" => run_package(&args[1..], messages),
        "check" => check(&args[1..], messages),
        "fetch" => fetch(&args[1..]).map(|()| Outcome::success()),
        "update" => update(&args[1..]).map(|()| Outcome::success()),
        "info" => info(&args[1..]).map(|()| Outcome::success()),
        "tree" => tree(&args[1..]).map(|()| Outcome::success()),
        "setup" => setup(&args[1..]).map(|()| Outcome::success()),
        "--version" | "-V" | "version" if args.len() == 1 => {
            print_version();
            Ok(Outcome::success())
        }
        "--help" | "-h" | "help" if args.len() == 1 => {
            print_help();
            Ok(Outcome::success())
        }
        "--version" | "-V" | "version" | "--help" | "-h" | "help" => {
            Err(Error::usage(format!("unexpected argument `{}`", args[1])))
        }
        unknown => Err(Error::usage(format!(
            "unknown command `{unknown}`\nhelp: run vex --help"
        ))),
    }
}

fn print_version() {
    println!("{} {}", "vex".color("2,161,47"), VERSION.color("2,161,47"));
}

fn print_help() {
    println!("Vex - Wave package manager");
    println!();
    println!("Usage:");
    println!("  vex [--message-file <new-path>] <command> [options]");
    println!("  vex init [--lib]");
    println!("  vex build [--target <triple>] [--release] [--dry-run] [--locked] [--offline]");
    println!(
        "  vex run [--target <triple>] [--release] [--dry-run] [--locked] [--offline] [-- <args...>]"
    );
    println!("  vex check [--target <triple>] [--release] [--dry-run] [--locked] [--offline]");
    println!("  vex fetch [--locked] [--offline]");
    println!("  vex update [<package>...]");
    println!("  vex info");
    println!("  vex tree [--locked] [--offline]");
    println!("  vex setup wavec [--version <version>] [--script-fallback]");
    println!("  vex --version");
}
