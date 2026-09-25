use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use wson::{loads, WsonMap, WsonValue};

use crate::{LockedPackage, LockedSource, Lockfile, LOCKFILE_NAME, LOCKFILE_VERSION};

pub fn parse_lockfile(raw: &str) -> Result<Lockfile, String> {
    parse_lockfile_inner(raw).map_err(|error| format!("{error}\nhelp: restore a valid `{LOCKFILE_NAME}` from version control; to regenerate intentionally, preserve the invalid file elsewhere and run `vex fetch`"))
}

fn parse_lockfile_inner(raw: &str) -> Result<Lockfile, String> {
    let data =
        loads(raw, "version", 3).map_err(|e| format!("failed to parse `{LOCKFILE_NAME}`: {e}"))?;
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
    if version != 2 && version != LOCKFILE_VERSION {
        return Err(format!(
            "unsupported `{LOCKFILE_NAME}` version `{version}`; expected `{LOCKFILE_VERSION}`"
        ));
    }
    reject_unknown_fields(&data, &["version", "package"], "root")?;

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

    let lockfile = Lockfile { version, packages };
    validate_graph(&lockfile)?;
    let normalized = lockfile.normalized();
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

    let fields: &[&str] = match source.as_str() {
        "path" => &[
            "name",
            "version",
            "source",
            "path",
            "resolved",
            "dependencies",
        ],
        "git" => &[
            "name",
            "version",
            "source",
            "git",
            "branch",
            "tag",
            "rev",
            "commit",
            "resolved",
            "dependencies",
        ],
        _ => &[],
    };
    if !fields.is_empty() {
        reject_unknown_fields(object, fields, &format!("package `{name}`"))?;
    }

    let source = match source.as_str() {
        "path" => LockedSource::Path {
            requested: required_string(object, "path")?,
            resolved: PathBuf::from(required_string(object, "resolved")?),
        },
        "git" => {
            if ["branch", "tag", "rev"]
                .iter()
                .filter(|field| object.contains_key(**field))
                .count()
                > 1
            {
                return Err(format!("lockfile package `{name}` must specify at most one of `branch`, `tag`, or `rev`"));
            }
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

fn reject_unknown_fields(object: &WsonMap, allowed: &[&str], context: &str) -> Result<(), String> {
    for field in object.keys() {
        if !allowed.contains(&field.as_str()) {
            return Err(format!(
                "unknown or source-inapplicable lockfile field `{field}` in {context}"
            ));
        }
    }
    Ok(())
}

fn validate_graph(lockfile: &Lockfile) -> Result<(), String> {
    let mut packages = BTreeMap::new();
    for package in &lockfile.packages {
        if packages.insert(package.name.as_str(), package).is_some() {
            return Err(format!(
                "`{LOCKFILE_NAME}` contains duplicate package `{}`",
                package.name
            ));
        }
    }
    let mut indegree: BTreeMap<&str, usize> = packages.keys().map(|name| (*name, 0)).collect();
    for package in &lockfile.packages {
        let mut seen = BTreeSet::new();
        for dependency in &package.dependencies {
            if !seen.insert(dependency) {
                return Err(format!(
                    "lockfile graph has duplicate edge: {} -> {dependency}",
                    package.name
                ));
            }
            if dependency == &package.name {
                return Err(format!(
                    "lockfile graph has self-dependency: {} -> {dependency}",
                    package.name
                ));
            }
            let count = indegree.get_mut(dependency.as_str()).ok_or_else(|| {
                format!(
                    "lockfile graph references missing package: {} -> {dependency}",
                    package.name
                )
            })?;
            *count += 1;
        }
    }
    // Iterative traversal avoids stack overflow for long hand-edited chains.
    let mut ready: Vec<_> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| *name)
        .collect();
    while let Some(name) = ready.pop() {
        for dependency in &packages[name].dependencies {
            let count = indegree
                .get_mut(dependency.as_str())
                .expect("edges checked above");
            *count -= 1;
            if *count == 0 {
                ready.push(dependency.as_str());
            }
        }
        indegree.remove(name);
    }
    if let Some((&start, _)) = indegree.first_key_value() {
        // Every remaining node has a remaining predecessor. Follow those
        // predecessors until one repeats, then reverse to show directed edges.
        let mut predecessor = BTreeMap::new();
        for &name in indegree.keys() {
            for dependency in &packages[name].dependencies {
                if indegree.contains_key(dependency.as_str()) {
                    predecessor.insert(dependency.as_str(), name);
                }
            }
        }
        let mut positions = BTreeMap::new();
        let mut path = Vec::new();
        let mut current = start;
        loop {
            if let Some(&position) = positions.get(current) {
                let mut cycle = path[position..].to_vec();
                cycle.push(current);
                cycle.reverse();
                return Err(format!(
                    "lockfile graph contains a cycle: {}",
                    cycle.join(" -> ")
                ));
            }
            positions.insert(current, path.len());
            path.push(current);
            current = predecessor[current];
        }
    }
    Ok(())
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
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_git_graph_fixture_preserves_object_formats_and_edges() {
        let raw = include_str!("../../tests/fixtures/lockfile/v2-git-graph.ws");
        let parsed = parse_lockfile(raw).unwrap();
        assert_eq!(parsed.packages[0].dependencies, ["beta"]);
        for (package, length) in parsed.packages.iter().zip([40, 64]) {
            let LockedSource::Git { commit, .. } = &package.source else {
                panic!("expected Git fixture");
            };
            assert_eq!(commit.len(), length);
        }
        let rendered = crate::render::render_lockfile(parsed.clone());
        assert_eq!(parse_lockfile(&rendered).unwrap().packages, parsed.packages);
        assert_eq!(
            crate::render::render_lockfile(parse_lockfile(&rendered).unwrap()),
            rendered
        );
    }

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
