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
        run(is_lib);
    } else if args.len() >= 2 && args[1] == "info" {
        info();
    } else {
        println!("❌ Unknown command.");
    }
}
