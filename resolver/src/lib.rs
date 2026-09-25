use std::collections::BTreeSet;

use lockfile::{
    read_lockfile, LockedPackage, LockedSource, Lockfile, LOCKFILE_NAME, LOCKFILE_VERSION,
};
use manifest::Manifest;

mod git;
mod graph;
mod paths;
mod transaction;

pub fn recover_project(guard: &state::Guard, dry_run: bool) -> Result<(), String> {
    transaction::recover(guard.root(), dry_run)
}

#[derive(Clone, Debug, Default)]
pub struct ResolveOptions {
    pub dry_run: bool,
    pub update: UpdatePolicy,
    pub locked: bool,
    pub offline: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum UpdatePolicy {
    #[default]
    ReuseLocked,
    UpdateAll,
    UpdateSelected(BTreeSet<String>),
}

impl UpdatePolicy {
    fn is_update(&self) -> bool {
        !matches!(self, Self::ReuseLocked)
    }

    pub(crate) fn updates(&self, package: &str) -> bool {
        match self {
            Self::ReuseLocked => false,
            Self::UpdateAll => true,
            Self::UpdateSelected(packages) => packages.contains(package),
        }
    }
}

#[derive(Debug)]
pub struct Resolution {
    packages: Vec<LockedPackage>,
    _guard: state::Guard,
}

impl Resolution {
    pub fn dependency_args(&self) -> Vec<String> {
        let mut args = vec!["--dep-root=.vex/deps".to_string()];
        for package in &self.packages {
            let path = match &package.source {
                LockedSource::Path { resolved, .. } | LockedSource::Git { resolved, .. } => {
                    resolved
                }
            };
            args.push(format!("--dep={}={}", package.name, path.to_string_lossy()));
        }
        args
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    pub fn packages(&self) -> &[LockedPackage] {
        &self.packages
    }
}

pub fn resolve<F>(
    manifest: &Manifest,
    options: ResolveOptions,
    mut status: F,
) -> Result<Resolution, String>
where
    F: FnMut(&str, String),
{
    if !manifest.dependencies.is_empty() {
        status(
            "Resolving",
            format!("dependencies for {} v{}", manifest.name, manifest.version),
        );
    }

    if options.update.is_update() && options.locked {
        return Err("`--locked` cannot be used while updating dependencies".to_string());
    }
    if options.update.is_update() && options.offline {
        return Err("`--offline` cannot be used while updating Git dependencies".to_string());
    }

    let guard = state::Guard::acquire(options.dry_run, &mut status)?;
    recover_project(&guard, options.dry_run)?;
    let existing = read_lockfile()?;
    if options.locked {
        let lockfile = existing.as_ref().ok_or_else(|| {
            format!(
                "`{LOCKFILE_NAME}` is required by `--locked`\nhelp: run `vex fetch` and commit `{LOCKFILE_NAME}`"
            )
        })?;
        if lockfile.version != 2 && lockfile.version != LOCKFILE_VERSION {
            return Err(format!(
                "`{LOCKFILE_NAME}` version {} cannot be used with `--locked`; expected version {LOCKFILE_VERSION}\nhelp: run `vex fetch` to regenerate the lockfile",
                lockfile.version
            ));
        }
    }
    let existing = existing.unwrap_or_else(Lockfile::empty);
    let root = paths::env_root()?;
    let dep_root = root.join(".vex/deps");
    let root_manifest = if manifest.source_path.is_absolute() {
        manifest.source_path.clone()
    } else {
        root.join(&manifest.source_path)
    };
    let locked = options.locked;
    let dry_run = options.dry_run;
    paths::validate_managed_root(&root, &dep_root)?;

    if let UpdatePolicy::UpdateSelected(selected) = &options.update {
        let mut preflight = graph::Resolver::new(
            ResolveOptions {
                dry_run: true,
                update: UpdatePolicy::UpdateSelected(selected.clone()),
                locked: false,
                offline: true,
            },
            root.clone(),
            dep_root.clone(),
            None,
            manifest.name.clone(),
            root_manifest.clone(),
            &existing,
            &mut status,
        );
        // Local discovery must reuse pinned sources even for selected names.
        preflight.local_preflight();
        preflight.resolve_manifest_dependencies(manifest).map_err(|e| format!("cannot validate update names from the local current graph: {e}\nhelp: run `vex fetch` first"))?;
        preflight.validate_selected_packages()?;
    }
    let transaction = if dry_run {
        None
    } else {
        Some(transaction::Transaction::new(&root)?)
    };
    let physical_deps = transaction
        .as_ref()
        .map(|t| t.stage.clone())
        .unwrap_or(dep_root);
    let packages = {
        let mut resolver = graph::Resolver::new(
            options,
            root,
            physical_deps,
            transaction.as_ref(),
            manifest.name.clone(),
            root_manifest,
            &existing,
            &mut status,
        );
        resolver.resolve_manifest_dependencies(manifest)?;
        resolver.validate_selected_packages()?;
        resolver.into_packages()
    };
    let resolved = Lockfile {
        version: if (locked || dry_run) && existing.version == 2 {
            2
        } else {
            LOCKFILE_VERSION
        },
        packages,
    }
    .normalized();

    if resolved != existing {
        if locked {
            return Err(format!(
                "`{LOCKFILE_NAME}` needs to be updated, but `--locked` prevents changes\nhelp: run `vex fetch` and commit the updated `{LOCKFILE_NAME}`"
            ));
        }
        if dry_run {
            return Err(format!(
                "dependency graph differs from `{LOCKFILE_NAME}`\nhelp: run `vex fetch` to resolve and lock dependencies"
            ));
        }
        status(
            "Locking",
            format!(
                "{} package{} to exact sources",
                resolved.packages.len(),
                if resolved.packages.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
        );
    }

    if let Some(transaction) = transaction {
        transaction.publish((resolved != existing).then(|| lockfile::encode(resolved.clone())))?;
    }

    Ok(Resolution {
        packages: resolved.packages,
        _guard: guard,
    })
}
