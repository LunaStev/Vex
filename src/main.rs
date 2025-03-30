use crate::check::check;
use crate::info::info;
use crate::init::init;
use crate::run::run;

mod init;
mod run;
mod info;

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
    } else {
        println!("❌ Unknown command.");
    }
}
