use wson_rs::{loads, WsonValue};
use std::fs;

pub fn info() {
    let vex_ws = fs::read_to_string("vex.ws");

    match vex_ws {
        Ok(raw) => match loads(&raw) {
            Ok(data) => {
                println!("📦 Vex Project Info");

                if let Some(WsonValue::String(name)) = data.get("name") {
                    println!("• Name: {}", name);
                }
                if let Some(WsonValue::Version(version)) = data.get("version") {
                    let ver = version.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(".");
                    println!("• Version: {}", ver);
                }
                if let Some(WsonValue::Bool(lib)) = data.get("lib") {
                    println!("• Type: {}", if *lib { "Library" } else { "Binary" });
                }

                if let Some(WsonValue::Array(deps)) = data.get("dependencies") {
                    println!("• Dependencies: {}", deps.len());
                    for dep in deps {
                        if let WsonValue::Object(map) = dep {
                            let name = match map.get("name") {
                                Some(WsonValue::String(n)) => n.clone(),
                                _ => "<unnamed>".to_string(),
                            };
                            let version = match map.get("version") {
                                Some(WsonValue::Version(v)) => {
                                    v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(".")
                                }
                                _ => "?".to_string(),
                            };
                            println!("    - {} v{}", name, version);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to parse vex.ws: {e}");
            }
        },
        Err(_) => eprintln!("❌ Cannot read vex.ws (file not found?)"),
    }
}
