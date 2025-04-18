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

            if let Some(WsonValue::String(author)) = data.get("author") {
                println!("Author: {}", author);
            }

            if let Some(WsonValue::String(license)) = data.get("license") {
                println!("License: {}", license);
            }

            if let Some(WsonValue::Array(deps)) = data.get("dependencies") {
                println!("Dependencies:");

                for dep in deps {
                    if let WsonValue::Object(obj) = dep {
                        let name = match obj.get("name") {
                            Some(WsonValue::String(s)) => s,
                            _ => "<unknown>",
                        };

                        let version = match obj.get("version") {
                            Some(WsonValue::Version(v)) => v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("."),
                            _ => "<unknown>".to_string(),
                        };

                        println!("- {} v{}", name, version);
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to parse vex.ws: {e}");
        }
    }
}