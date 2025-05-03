use wavec::version_wave;
use commands::check::check;
use commands::info::info;
use commands::init::init;
use commands::run::run;
use crate::version::version_vex;

mod commands;
mod version;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let is_lib = args.iter().any(|arg| arg == "--lib");

    if args.len() >= 2 && args[1] == "init" {
        init(is_lib);
    } else if args.len() >= 2 && args[1] == "info" {
        info();
    } else if args.len() >= 2 && args[1] == "run" {
        run();
    } else if args.len() >= 2 && args[1] == "check" {
        check();
    } else if args.len() >= 2 && (args[1] == "--version" || args[1] == "-V") {
        version_vex();
        version_wave();
    } else {
        println!("❌ Unknown command.");
    }
}
