mod cli;
mod commands;
mod messages;
mod outcome;
mod ui;

fn main() {
    std::process::exit(cli::run());
}
