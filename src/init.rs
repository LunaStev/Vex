use std::{env, fs};
use std::path::Path;

pub fn run(is_lib: bool) {
    let vex_path = Path::new("vex.ws");
    let src_dir = Path::new("src");

    if vex_path.exists() {
        println!("⚠️ 'vex.ws' already exists.");
        return;
    }

    let dir_name = env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| "wave_project".to_string());

    let file_name = if is_lib { "lib.wave" } else { "main.wave" };
    let wave_path = src_dir.join(file_name);

    fs::create_dir_all(src_dir).expect("Failed to create src directory");
    fs::write(&wave_path, "fun main() {\n    println(\"Hello World\");\n}").expect("Failed to create .wave file");

    let content = format!(
        r#"{{
    name = "{dir_name}",
    version = 0.1.0,
    lib = {is_lib},

    // library
    dependencies = []
}}"#
    );

    fs::write(vex_path, content.trim()).expect("Failed to create vex.ws");

    println!("✅ Project initialized successfully: {file_name}");
}