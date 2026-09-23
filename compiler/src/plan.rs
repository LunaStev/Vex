use serde_json::Value;

pub(crate) fn validate_dry_run_json_output(stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let candidate = extract_json_object(&stdout)
        .or_else(|| extract_json_object(&stderr))
        .ok_or_else(|| "wavec dry-run did not return a JSON object plan".to_string())?;
    let value: Value = serde_json::from_str(candidate)
        .map_err(|error| format!("invalid wavec dry-run JSON plan: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "wavec dry-run plan must be a JSON object".to_string())?;

    match object.get("schema_version").and_then(Value::as_u64) {
        Some(1) => {}
        Some(found) => {
            return Err(format!(
                "unsupported wavec dry-run schema_version `{found}`; expected `1`"
            ));
        }
        None => return Err("dry-run JSON is missing numeric key `schema_version`".to_string()),
    }
    for key in ["mode", "target", "emit"] {
        require_string(object, key)?;
    }
    for key in ["emit_kinds", "inputs", "emit_jobs", "compile"] {
        require_array(object, key)?;
    }
    require_string_or_null(object, "control_mode")?;
    require_string_or_null(object, "forced_input_type")?;
    require_link_or_null(object.get("link"))?;
    require_execute_or_null(object.get("execute"))?;
    Ok(())
}

fn require_string(object: &serde_json::Map<String, Value>, key: &str) -> Result<(), String> {
    match object.get(key).and_then(Value::as_str) {
        Some(_) => Ok(()),
        None => Err(format!("dry-run JSON is missing string key `{key}`")),
    }
}

fn require_string_or_null(
    object: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<(), String> {
    match object.get(key) {
        Some(value) if value.is_null() || value.as_str().is_some() => Ok(()),
        Some(_) => Err(format!("dry-run JSON key `{key}` must be string or null")),
        None => Err(format!("dry-run JSON is missing key `{key}`")),
    }
}

fn require_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("dry-run JSON is missing array key `{key}`"))
}

fn require_link_or_null(value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Err("dry-run JSON is missing key `link`".to_string());
    };
    if value.is_null() {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "dry-run JSON key `link` must be object or null".to_string())?;
    require_string(object, "output")?;
    require_array(object, "inputs")?;
    require_string(object, "program")?;
    require_array(object, "args")?;
    Ok(())
}

fn require_execute_or_null(value: Option<&Value>) -> Result<(), String> {
    let Some(value) = value else {
        return Err("dry-run JSON is missing key `execute`".to_string());
    };
    if value.is_null() {
        return Ok(());
    }
    let object = value
        .as_object()
        .ok_or_else(|| "dry-run JSON key `execute` must be object or null".to_string())?;
    require_string(object, "program")?;
    require_array(object, "args")?;
    Ok(())
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| &text[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_plan() -> &'static str {
        r#"{
            "schema_version": 1,
            "mode": "build",
            "target": "x86_64-unknown-linux-gnu",
            "emit": "bin",
            "emit_kinds": ["bin"],
            "control_mode": null,
            "forced_input_type": null,
            "inputs": [{"path":"src/main.wave","kind":"wave"}],
            "emit_jobs": [],
            "compile": [{"input":"src/main.wave","kind":"wave","output":"target/main.o","command":"wavec <internal>"}],
            "link": {"output":"target/main","inputs":["target/main.o"],"program":"ld.lld","args":["target/main.o"]},
            "execute": null
        }"#
    }

    #[test]
    fn dry_run_schema_v1_is_validated() {
        validate_dry_run_json_output(valid_plan().as_bytes(), b"")
            .expect("schema v1 dry-run plan must be accepted");
        let missing_execute = valid_plan().replace(",\n            \"execute\": null", "");
        let error = validate_dry_run_json_output(missing_execute.as_bytes(), b"")
            .expect_err("execute is required by the v1 contract");
        assert!(error.contains("execute"), "{error}");
        let schema_two = valid_plan().replace("\"schema_version\": 1", "\"schema_version\": 2");
        let error = validate_dry_run_json_output(schema_two.as_bytes(), b"")
            .expect_err("unknown schema version must be rejected");
        assert!(error.contains("schema_version"), "{error}");
    }

    #[test]
    fn dry_run_json_can_be_extracted_from_noisy_output() {
        let output = format!("debug line\n{}\n", valid_plan());
        validate_dry_run_json_output(output.as_bytes(), b"")
            .expect("JSON object should be extracted from mixed output");
    }
}
