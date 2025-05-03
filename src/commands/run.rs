use wson_rs::{loads};
use std::fs;
use std::path::Path;
use wavec::compile_and_run;

pub fn run() {
    let vex_ws = fs::read_to_string("vex.ws").expect("vex.ws not found");

    match loads(&vex_ws) {
        Ok(..) => unsafe {
            let src_dir = Path::new("src");
            let mut main_file: Option<String> = None;

            if src_dir.is_dir() {
                for entry in fs::read_dir(src_dir).expect("failed to read src dir") {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("wave") {
                            if let Ok(contents) = fs::read_to_string(&path) {
                                if contents.contains("fun main()") {
                                    main_file = Some(path.to_string_lossy().to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if let Some(main_path) = main_file {
                println!("Running {main_path}...");
                compile_and_run(main_path.as_ref());
            } else {
                println!("No file with `fn main()` found in src/");
            }
        }
        Err(e) => {
            println!("❌ Failed to parse vex.ws: {e}");
        }
    }
}