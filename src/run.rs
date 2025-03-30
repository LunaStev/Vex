use wson_rs::{loads};
use std::fs;

pub fn run() {
    let vex_ws = fs::read_to_string("vex.ws").expect("vex.ws not found");

    match loads(&vex_ws) {
        Ok(..) => {
            let src_dir = Path::new("src");
            let mut main_file: Option<String> = None;

            if let Some(WsonValue::Bool(is_lib)) = data.get("lib") {
                println!("Project type: {}", if *is_lib { "Library" } else { "Binary" });
            }

            if let Some(main_path) = main_file {
                println!("🚀 Running {main_path}...");
                let status = Command::new("wave")
                    .arg("run")
                    .arg(&main_path)
                    .status()
                    .expect("failed to run wave");

                if !status.success() {
                    println!("❌ Wave execution failed");
                }
            } else {
                println!("❌ No file with `fn main()` found in src/");
            }
        }
        Err(e) => {
            println!("❌ Failed to parse vex.ws: {e}");
        }
    }
}