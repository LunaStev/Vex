use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use lockfile::{LockedPackage, LockedSource, Lockfile, LOCKFILE_NAME};
use manifest::{Dependency, DependencySource, Manifest, MANIFEST_FILE};

use crate::git;
use crate::paths::{relative_to_root, resolve_path};
use crate::{ResolveOptions, UpdatePolicy};

pub(crate) struct Resolver<'a> {
    options: ResolveOptions,
    root: PathBuf,
    dep_root: PathBuf,
    root_name: String,
    root_manifest: PathBuf,
    existing: &'a Lockfile,
    packages: BTreeMap<String, LockedPackage>,
    requests: HashMap<String, RequestKey>,
    visiting: Vec<String>,
    status: &'a mut dyn FnMut(&str, String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RequestKey {
    Path {
        resolved: PathBuf,
        version: Option<String>,
    },
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        version: Option<String>,
    },
}

impl<'a> Resolver<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        options: ResolveOptions,
        root: PathBuf,
        dep_root: PathBuf,
        root_name: String,
        root_manifest: PathBuf,
        existing: &'a Lockfile,
        status: &'a mut dyn FnMut(&str, String),
    ) -> Self {
        Self {
            options,
            root,
            dep_root,
            root_name,
            root_manifest,
            existing,
            packages: BTreeMap::new(),
            requests: HashMap::new(),
            visiting: Vec::new(),
            status,
        }
    }

    pub(crate) fn into_packages(self) -> Vec<LockedPackage> {
        self.packages.into_values().collect()
    }

    pub(crate) fn validate_selected_packages(&self) -> Result<(), String> {
        let UpdatePolicy::UpdateSelected(selected) = &self.options.update else {
            return Ok(());
        };

        let available = self
            .packages
            .values()
            .filter(|package| matches!(package.source, LockedSource::Git { .. }))
            .map(|package| package.name.as_str())
            .collect::<BTreeSet<_>>();
        let unavailable = selected
            .iter()
            .filter(|name| !available.contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();

        if unavailable.is_empty() {
            return Ok(());
        }

        let requested = unavailable
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let package_label = if unavailable.len() == 1 {
            "package"
        } else {
            "packages"
        };
        let available = if available.is_empty() {
            "<none>".to_string()
        } else {
            available.into_iter().collect::<Vec<_>>().join(", ")
        };
        Err(format!(
            "cannot update {package_label} {requested}: one or more requested names are not Git dependencies in the current graph\nhelp: available Git packages: {available}\nhelp: run `vex update <package>...` using one or more available package names"
        ))
    }

    pub(crate) fn resolve_manifest_dependencies(
        &mut self,
        manifest: &Manifest,
    ) -> Result<Vec<String>, String> {
        let mut dependencies = Vec::new();
        for dependency in &manifest.dependencies {
            self.resolve_dependency(dependency, &manifest.source_path)
                .map_err(|error| {
                    format!(
                        "failed to resolve dependency `{}` from `{}`\n\nCaused by:\n  {}",
                        dependency.name,
                        manifest.source_path.display(),
                        indent_lines(&error)
                    )
                })?;
            dependencies.push(dependency.name.clone());
        }
        dependencies.sort();
        Ok(dependencies)
    }

    fn resolve_dependency(
        &mut self,
        dependency: &Dependency,
        parent_manifest: &Path,
    ) -> Result<(), String> {
        if dependency.name == self.root_name {
            let source = match &dependency.source {
                DependencySource::Path { path } => resolve_path(path, parent_manifest)
                    .to_string_lossy()
                    .into_owned(),
                DependencySource::Git { url, .. } => url.clone(),
            };
            return Err(format!(
                "dependency `{}` declared in `{}` from source `{source}` reuses root package name `{}` from `{}`",
                dependency.name,
                parent_manifest.display(),
                self.root_name,
                self.root_manifest.display()
            ));
        }

        let key = match &dependency.source {
            DependencySource::Path { path } => RequestKey::Path {
                resolved: resolve_path(path, parent_manifest),
                version: dependency.version.clone(),
            },
            DependencySource::Git {
                url,
                branch,
                tag,
                rev,
            } => RequestKey::Git {
                url: url.clone(),
                branch: branch.clone(),
                tag: tag.clone(),
                rev: rev.clone(),
                version: dependency.version.clone(),
            },
        };

        if let Some(previous) = self.requests.get(&dependency.name) {
            if previous != &key {
                return Err(format!(
                    "package name `{}` refers to more than one source or version requirement",
                    dependency.name
                ));
            }
            if self.packages.contains_key(&dependency.name) {
                return Ok(());
            }
        } else {
            self.requests.insert(dependency.name.clone(), key);
        }

        if let Some(index) = self
            .visiting
            .iter()
            .position(|name| name == &dependency.name)
        {
            let mut cycle = self.visiting[index..].to_vec();
            cycle.push(dependency.name.clone());
            return Err(format!("dependency cycle detected: {}", cycle.join(" -> ")));
        }

        let (resolved_path, locked_source) = match &dependency.source {
            DependencySource::Path { path } => {
                let resolved = resolve_path(path, parent_manifest);
                let source = LockedSource::Path {
                    requested: path.clone(),
                    resolved: relative_to_root(&resolved, &self.root),
                };
                (resolved, source)
            }
            DependencySource::Git {
                url,
                branch,
                tag,
                rev,
            } => {
                let destination = self.dep_root.join(&dependency.name);
                let commit = self.resolve_git_commit(dependency, &destination)?;
                let source = LockedSource::Git {
                    url: url.clone(),
                    branch: branch.clone(),
                    tag: tag.clone(),
                    rev: rev.clone(),
                    commit,
                    resolved: relative_to_root(&destination, &self.root),
                };
                (destination, source)
            }
        };

        let manifest_path = resolved_path.join(MANIFEST_FILE);
        let package_manifest = Manifest::load_from(&manifest_path)?;
        if package_manifest.name != dependency.name {
            return Err(format!(
                "dependency is named `{}` but `{}` declares package `{}`",
                dependency.name,
                manifest_path.display(),
                package_manifest.name
            ));
        }
        if !package_manifest.lib {
            return Err(format!(
                "dependency `{}` is not a library package\nhelp: set `lib = true` in `{}` and provide `src/lib.wave`",
                dependency.name,
                manifest_path.display()
            ));
        }
        let library_entry = resolved_path.join(package_manifest.default_entry_path());
        if !library_entry.is_file() {
            return Err(format!(
                "dependency `{}` has no canonical library entry `{}`\nhelp: library packages must expose `src/lib.wave`",
                dependency.name,
                library_entry.display()
            ));
        }
        if let Some(required) = dependency.version.as_deref() {
            if package_manifest.version != required {
                return Err(format!(
                    "dependency `{}` requires version `{required}` but source contains version `{}`",
                    dependency.name, package_manifest.version
                ));
            }
        }

        self.visiting.push(dependency.name.clone());
        let dependencies = self.resolve_manifest_dependencies(&package_manifest)?;
        self.visiting.pop();

        self.packages.insert(
            dependency.name.clone(),
            LockedPackage {
                name: dependency.name.clone(),
                version: package_manifest.version,
                source: locked_source,
                dependencies,
            },
        );
        Ok(())
    }

    fn resolve_git_commit(
        &mut self,
        dependency: &Dependency,
        destination: &Path,
    ) -> Result<String, String> {
        let DependencySource::Git {
            url,
            branch,
            tag,
            rev,
        } = &dependency.source
        else {
            return Err("internal error: expected Git dependency".to_string());
        };

        let locked = if self.options.update.updates(&dependency.name) {
            None
        } else {
            self.existing
                .package(&dependency.name)
                .and_then(|package| match &package.source {
                    LockedSource::Git {
                        url: locked_url,
                        branch: locked_branch,
                        tag: locked_tag,
                        rev: locked_rev,
                        commit,
                        ..
                    } if locked_url == url
                        && locked_branch == branch
                        && locked_tag == tag
                        && locked_rev == rev =>
                    {
                        Some(commit.clone())
                    }
                    _ => None,
                })
        };

        if self.options.locked && locked.is_none() {
            return Err(format!(
                "`{LOCKFILE_NAME}` does not match Git dependency `{}`\nhelp: run `vex fetch` to update the lockfile",
                dependency.name
            ));
        }

        if self.options.dry_run {
            let commit = locked.ok_or_else(|| {
                format!(
                    "Git dependency `{}` is not pinned in `{LOCKFILE_NAME}`\nhelp: run `vex fetch` first",
                    dependency.name
                )
            })?;
            git::require_checkout_at(destination, url, &dependency.name, &commit)?;
            return Ok(commit);
        }

        if self.options.offline {
            let commit = locked.ok_or_else(|| {
                format!(
                    "Git dependency `{}` is not pinned for offline use\nhelp: run `vex fetch` while online",
                    dependency.name
                )
            })?;
            git::require_local_repository(destination, url, &dependency.name, &commit)?;
            git::checkout_commit(destination, &dependency.name, &commit)?;
            return Ok(commit);
        }

        git::ensure_repository(destination, url, &dependency.name, &mut *self.status)?;
        git::reject_dirty_checkout(destination, &dependency.name)?;

        if let Some(commit) = locked {
            if !git::has_commit(destination, &commit)? {
                (self.status)("Fetching", format!("{} ({url})", dependency.name));
                git::fetch(destination)?;
            }
            git::checkout_commit(destination, &dependency.name, &commit)?;
            return Ok(commit);
        }

        (self.status)("Fetching", format!("{} ({url})", dependency.name));
        git::fetch(destination)?;
        let reference = if let Some(branch) = branch {
            format!("refs/remotes/origin/{branch}^{{commit}}")
        } else if let Some(tag) = tag {
            format!("refs/tags/{tag}^{{commit}}")
        } else if let Some(rev) = rev {
            format!("{rev}^{{commit}}")
        } else {
            git::refresh_default_branch(destination)?;
            "refs/remotes/origin/HEAD^{commit}".to_string()
        };
        let commit = git::stdout(
            git::command_in(destination).args([
                "rev-parse",
                "--verify",
                "--end-of-options",
                &reference,
            ]),
            "resolve Git dependency reference",
        )?;
        git::checkout_commit(destination, &dependency.name, &commit)?;
        Ok(commit)
    }
}

fn indent_lines(message: &str) -> String {
    message.replace('\n', "\n  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_identity_uses_resolved_location() {
        let first = RequestKey::Path {
            resolved: PathBuf::from("/tmp/package"),
            version: Some("1.0.0".to_string()),
        };
        let second = RequestKey::Path {
            resolved: PathBuf::from("/tmp/package"),
            version: Some("1.0.0".to_string()),
        };
        assert_eq!(first, second);
    }
}
