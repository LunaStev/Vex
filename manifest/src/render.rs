pub fn render_new_manifest(project_name: &str, author: &str, is_lib: bool) -> String {
    format!(
        r#"{{
    name = "{name}",
    version = 0.1.0,
    lib = {is_lib},
    description = "{name} Project",
    author = "{author}",
    license = "Unknown",
    dependencies = []
}}
"#,
        name = escape_wson_string(project_name),
        author = escape_wson_string(author)
    )
}

fn escape_wson_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
