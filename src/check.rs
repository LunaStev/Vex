use wson_rs::{loads, WsonValue};
use std::fs;

pub fn check() {
    let vex_ws = fs::read_to_string("vex.ws").expect("vex.ws not found");

    match loads(&vex_ws) {
        Ok(data) => {
            println!("✅ Loaded vex.ws");

            if let Some(WsonValue::Bool(is_lib)) = data.get("lib") {
                println!("Project type: {}", if *is_lib { "Library" } else { "Binary" });
            }

            if let Some(WsonValue::Version(ver)) = data.get("version") {
                println!("Version: {}", ver.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("."));
            }
        }
        Err(e) => {
            println!("❌ Failed to parse vex.ws: {e}");
        }
    }
}