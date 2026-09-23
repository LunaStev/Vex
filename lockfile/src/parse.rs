use std::path::PathBuf;

use wson_rs::{loads, WsonMap, WsonValue};

use crate::{LockedPackage, LockedSource, Lockfile, LOCKFILE_NAME, LOCKFILE_VERSION};

const URL_SEPARATOR_SENTINEL: &str = "__VEX_LOCK_URL_SEPARATOR__";

pub(crate) fn parse_lockfile(raw: &str) -> Result<Lockfile, String> {
    let protected = raw.replace("://", URL_SEPARATOR_SENTINEL);
    let data = loads(&protected).map_err(|e| format!("failed to parse `{LOCKFILE_NAME}`: {e}"))?;
    let version = match data.get("version") {
        Some(WsonValue::Int(value)) => *value,
        _ => {
            return Err(format!(
                "`{LOCKFILE_NAME}` field `version` must be an integer"
            ))
        }
    };

    if version == 1 {
        return Ok(Lockfile {
            version,
            packages: Vec::new(),
        });
    }
    if version != LOCKFILE_VERSION {
        return Err(format!(
            "unsupported `{LOCKFILE_NAME}` version `{version}`; expected `{LOCKFILE_VERSION}`"
        ));
    }

    let packages = match data.get("package") {
        Some(WsonValue::Array(items)) => items
            .iter()
            .map(parse_package)
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(format!(
                "`{LOCKFILE_NAME}` field `package` must be an array"
            ))
        }
        None => Vec::new(),
    };

    let normalized = Lockfile { version, packages }.normalized();
    for pair in normalized.packages.windows(2) {
        if pair[0].name == pair[1].name {
            return Err(format!(
                "`{LOCKFILE_NAME}` contains duplicate package `{}`",
                pair[0].name
            ));
        }
    }
    Ok(normalized)
}

fn parse_package(value: &WsonValue) -> Result<LockedPackage, String> {
    let WsonValue::Object(object) = value else {
        return Err("lockfile package entry must be an object".to_string());
    };
    let name = required_string(object, "name")?;
    let version = required_string(object, "version")?;
    let source = required_string(object, "source")?;
    let dependencies = optional_string_array(object, "dependencies")?;

    let source = match source.as_str() {
        "path" => LockedSource::Path {
            requested: required_string(object, "path")?,
            resolved: PathBuf::from(required_string(object, "resolved")?),
        },
        "git" => {
            let commit = required_string(object, "commit")?;
            validate_commit(&commit, &name)?;
            LockedSource::Git {
                url: required_string(object, "git")?,
                branch: optional_string(object, "branch")?,
                tag: optional_string(object, "tag")?,
                rev: optional_string(object, "rev")?,
                commit: commit.to_ascii_lowercase(),
                resolved: PathBuf::from(required_string(object, "resolved")?),
            }
        }
        other => {
            return Err(format!(
                "unknown lockfile source `{other}` for package `{name}`"
            ))
        }
    };
    Ok(LockedPackage {
        name,
        version,
        source,
        dependencies,
    })
}

fn validate_commit(commit: &str, package: &str) -> Result<(), String> {
    if matches!(commit.len(), 40 | 64) && commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(format!(
        "lockfile package `{package}` has invalid Git commit `{commit}`; expected a full hexadecimal object ID"
    ))
}

fn required_string(object: &WsonMap, key: &str) -> Result<String, String> {
    match object.get(key) {
        Some(WsonValue::String(value)) => Ok(restore_url(value)),
        _ => Err(format!("lockfile field `{key}` must be a string")),
    }
}

fn optional_string(object: &WsonMap, key: &str) -> Result<Option<String>, String> {
    match object.get(key) {
        Some(WsonValue::String(value)) => Ok(Some(restore_url(value))),
        Some(_) => Err(format!("lockfile field `{key}` must be a string")),
        None => Ok(None),
    }
}

fn optional_string_array(object: &WsonMap, key: &str) -> Result<Vec<String>, String> {
    match object.get(key) {
        Some(WsonValue::Array(values)) => values
            .iter()
            .map(|value| match value {
                WsonValue::String(value) => Ok(restore_url(value)),
                _ => Err(format!("lockfile field `{key}` must contain only strings")),
            })
            .collect(),
        Some(_) => Err(format!("lockfile field `{key}` must be an array")),
        None => Ok(Vec::new()),
    }
}

fn restore_url(value: &str) -> String {
    value.replace(URL_SEPARATOR_SENTINEL, "://")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_lock(commit: &str) -> String {
        format!(
            r#"{{ version = 2, package = [{{
                name = "dep", version = "1.0.0", source = "git",
                git = "https://example.com/dep.git", commit = "{commit}",
                resolved = ".vex/deps/dep", dependencies = []
            }}] }}"#
        )
    }

    #[test]
    fn version_one_lockfile_is_treated_as_unresolved() {
        let parsed = parse_lockfile("{ version = 1, package = [] }")
            .expect("legacy lockfile should trigger regeneration");
        assert!(parsed.packages.is_empty());
    }

    #[test]
    fn rejects_non_object_id_git_commits() {
        let error =
            parse_lockfile(&git_lock("--help")).expect_err("Git commits must be full object IDs");
        assert!(error.contains("invalid Git commit"), "{error}");
    }

    #[test]
    fn normalizes_sha1_and_sha256_git_commits_to_lowercase() {
        for spelling in [
            "0123456789ABCDEF0123456789ABCDEF01234567",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
        ] {
            let parsed = parse_lockfile(&git_lock(spelling)).expect("object ID must parse");
            let LockedSource::Git { commit, .. } = &parsed.packages[0].source else {
                unreachable!();
            };
            assert_eq!(commit, &spelling.to_ascii_lowercase());
        }
    }

    #[test]
    fn rejects_abbreviated_invalid_and_unsupported_length_commits() {
        for invalid in [
            "0123456789abcdef",
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789abcdef0123456789abcdef012345678",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0",
            "0123456789abcdef0123456789abcdef0123456g",
            "0123456789ABCDEF0123456789ABCDEF0123456G",
            "0123456789abcdef0123456789abcdef012345 7",
        ] {
            let result = parse_lockfile(&git_lock(invalid));
            assert!(result.is_err(), "commit `{invalid}` must be rejected");
            let error = result.unwrap_err();
            assert!(error.contains("invalid Git commit"), "{error}");
        }
    }
}
