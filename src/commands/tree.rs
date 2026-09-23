// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::collections::{BTreeMap, BTreeSet};

use crate::ui;
use lockfile::{LockedPackage, LockedSource};
use manifest::Manifest;
use resolver::{resolve, ResolveOptions, UpdatePolicy};

#[derive(Debug, Default, Eq, PartialEq)]
struct TreeOptions {
    locked: bool,
    offline: bool,
}

pub fn tree(args: &[String]) {
    if matches!(args, [help] if help == "-h" || help == "--help") {
        println!("usage: vex tree [--locked] [--offline]");
        return;
    }
    if let Err(error) = run_tree(args) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run_tree(args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let manifest = Manifest::load()?;
    let root_dependencies = manifest
        .dependencies
        .iter()
        .map(|dependency| dependency.name.clone())
        .collect::<Vec<_>>();
    let resolution = resolve(
        &manifest,
        ResolveOptions {
            dry_run: false,
            update: UpdatePolicy::ReuseLocked,
            locked: options.locked,
            offline: options.offline,
        },
        ui::status,
    )?;
    print!(
        "{}",
        render_tree(
            &manifest.name,
            &manifest.version,
            &root_dependencies,
            resolution.packages(),
        )?
    );
    Ok(())
}

fn parse_options(args: &[String]) -> Result<TreeOptions, String> {
    let mut options = TreeOptions::default();
    for argument in args {
        match argument.as_str() {
            "--locked" => options.locked = true,
            "--offline" => options.offline = true,
            _ => {
                return Err(format!(
                    "unknown Vex option `{argument}`\nusage: vex tree [--locked] [--offline]"
                ));
            }
        }
    }
    Ok(options)
}

fn render_tree(
    root_name: &str,
    root_version: &str,
    root_dependencies: &[String],
    packages: &[LockedPackage],
) -> Result<String, String> {
    let packages = packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut roots = root_dependencies.to_vec();
    roots.sort();
    roots.dedup();

    let mut output = format!("{root_name} v{root_version}\n");
    let mut expanded = BTreeSet::new();
    let mut repeated = false;
    for (index, dependency) in roots.iter().enumerate() {
        render_package(
            dependency,
            "",
            index + 1 == roots.len(),
            &packages,
            &mut expanded,
            &mut repeated,
            &mut output,
        )?;
    }
    if repeated {
        output.push_str("\n(*) package dependencies already shown\n");
    }
    Ok(output)
}

fn render_package(
    name: &str,
    prefix: &str,
    last: bool,
    packages: &BTreeMap<&str, &LockedPackage>,
    expanded: &mut BTreeSet<String>,
    repeated: &mut bool,
    output: &mut String,
) -> Result<(), String> {
    let package = packages.get(name).copied().ok_or_else(|| {
        format!("dependency graph references missing package `{name}` in vex.lock")
    })?;
    let already_expanded = !expanded.insert(name.to_string());

    output.push_str(prefix);
    output.push_str(if last { "└── " } else { "├── " });
    output.push_str(&package_label(package));
    if already_expanded && !package.dependencies.is_empty() {
        output.push_str(" (*)");
        *repeated = true;
    }
    output.push('\n');

    if already_expanded {
        return Ok(());
    }

    let mut dependencies = package.dependencies.clone();
    dependencies.sort();
    dependencies.dedup();
    let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
    for (index, dependency) in dependencies.iter().enumerate() {
        render_package(
            dependency,
            &child_prefix,
            index + 1 == dependencies.len(),
            packages,
            expanded,
            repeated,
            output,
        )?;
    }
    Ok(())
}

fn package_label(package: &LockedPackage) -> String {
    match &package.source {
        LockedSource::Path { requested, .. } => {
            format!("{} v{} (path {requested})", package.name, package.version)
        }
        LockedSource::Git {
            url,
            branch,
            tag,
            rev,
            commit,
            ..
        } => {
            let reference = branch
                .as_ref()
                .map(|value| format!(" branch {value}"))
                .or_else(|| tag.as_ref().map(|value| format!(" tag {value}")))
                .or_else(|| rev.as_ref().map(|value| format!(" rev {value}")))
                .unwrap_or_default();
            let short_commit = &commit[..commit.len().min(7)];
            format!(
                "{} v{} (git {url}{reference} @ {short_commit})",
                package.name, package.version
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn path_package(name: &str, version: &str, dependencies: &[&str]) -> LockedPackage {
        LockedPackage {
            name: name.to_string(),
            version: version.to_string(),
            source: LockedSource::Path {
                requested: format!("../{name}"),
                resolved: PathBuf::from(format!("../{name}")),
            },
            dependencies: dependencies
                .iter()
                .map(|dependency| dependency.to_string())
                .collect(),
        }
    }

    #[test]
    fn renders_sources_commits_and_shared_dependencies() {
        let packages = vec![
            LockedPackage {
                name: "alpha".to_string(),
                version: "1.0.0".to_string(),
                source: LockedSource::Git {
                    url: "https://example.com/alpha.git".to_string(),
                    branch: Some("main".to_string()),
                    tag: None,
                    rev: None,
                    commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
                    resolved: PathBuf::from(".vex/deps/alpha"),
                },
                dependencies: vec!["shared".to_string()],
            },
            path_package("beta", "2.0.0", &["shared"]),
            path_package("shared", "0.2.0", &["leaf"]),
            path_package("leaf", "0.1.0", &[]),
        ];

        let rendered = render_tree(
            "app",
            "0.1.0",
            &["beta".to_string(), "alpha".to_string()],
            &packages,
        )
        .unwrap();
        assert_eq!(
            rendered,
            concat!(
                "app v0.1.0\n",
                "├── alpha v1.0.0 (git https://example.com/alpha.git branch main @ 0123456)\n",
                "│   └── shared v0.2.0 (path ../shared)\n",
                "│       └── leaf v0.1.0 (path ../leaf)\n",
                "└── beta v2.0.0 (path ../beta)\n",
                "    └── shared v0.2.0 (path ../shared) (*)\n",
                "\n",
                "(*) package dependencies already shown\n",
            )
        );
    }

    #[test]
    fn rejects_missing_graph_edges() {
        let error = render_tree("app", "0.1.0", &["missing".to_string()], &[]).unwrap_err();
        assert!(error.contains("missing package `missing`"), "{error}");
    }

    #[test]
    fn parses_locked_and_offline_options() {
        assert_eq!(
            parse_options(&["--locked".to_string(), "--offline".to_string()]).unwrap(),
            TreeOptions {
                locked: true,
                offline: true,
            }
        );
    }
}
