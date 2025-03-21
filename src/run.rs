use std::fs;

pub fn run() {
    let vex_ws = fs::read_to_string("vex.ws").expect("vex.ws not found");

    if vex_ws.contains("lib = true") {}
}