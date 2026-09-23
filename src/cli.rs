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

pub fn run() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    if args.is_empty() {
        print_help();
        return;
    }

    match args[0].as_str() {
        "init" => init(&args[1..]),
        "build" => build(BuildMode::Build, &args[1..]),
        "run" => run_package(&args[1..]),
        "check" => check(&args[1..]),
        "fetch" => fetch(&args[1..]),
        "update" => update(&args[1..]),
        "info" => info(&args[1..]),
        "tree" => tree(&args[1..]),
        "setup" => setup(&args[1..]),
        "--version" | "-V" | "version" if args.len() == 1 => print_version(),
        "--help" | "-h" | "help" if args.len() == 1 => print_help(),
        "--version" | "-V" | "version" | "--help" | "-h" | "help" => {
            eprintln!("error: unexpected argument `{}`", args[1]);
            std::process::exit(2);
        }
        unknown => {
            eprintln!("error: unknown command `{unknown}`");
            print_help();
            std::process::exit(2);
        }
    }
}

fn print_version() {
    println!("{} {}", "vex".color("2,161,47"), VERSION.color("2,161,47"));
}

fn print_help() {
    println!("Vex - Wave package manager");
    println!();
    println!("Usage:");
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
    println!("  vex setup wavec [--version <version>]");
    println!("  vex --version");
}
