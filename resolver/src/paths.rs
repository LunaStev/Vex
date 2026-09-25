use std::borrow::Cow;
#[cfg(test)]
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) fn env_root() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("failed to determine project directory: {error}"))
        .map(|path| path.canonicalize().unwrap_or(path))
}

pub(crate) fn resolve_path(path: &str, manifest_path: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(candidate)
    };
    resolved.canonicalize().unwrap_or(resolved)
}

pub(crate) fn relative_to_root(path: &Path, root: &Path) -> PathBuf {
    if let Ok(relative) = path.strip_prefix(root) {
        return relative.to_path_buf();
    }

    let path_components = path.components().collect::<Vec<_>>();
    let root_components = root.components().collect::<Vec<_>>();
    let common = path_components
        .iter()
        .zip(&root_components)
        .take_while(|(path_component, root_component)| path_component == root_component)
        .count();

    let same_root = common > 0
        && !matches!(
            (path_components.get(common), root_components.get(common)),
            (Some(Component::Prefix(_)), _) | (_, Some(Component::Prefix(_)))
        );
    if !same_root {
        return path.to_path_buf();
    }

    let mut relative = PathBuf::new();
    for component in &root_components[common..] {
        if matches!(component, Component::Normal(_)) {
            relative.push("..");
        }
    }
    for component in &path_components[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative
    }
}

pub(crate) fn validate_managed_root(root: &Path, dep_root: &Path) -> Result<(), String> {
    reject_symbolic_link(&root.join(".vex"), "managed Vex directory")?;
    reject_symbolic_link(dep_root, "managed dependency directory")
}

pub(crate) fn validate_managed_checkout_path(destination: &Path) -> Result<(), String> {
    reject_symbolic_link(destination, "managed dependency checkout")?;
    reject_symbolic_link(
        &destination.join(".git"),
        "managed dependency Git directory",
    )
}

fn reject_symbolic_link(path: &Path, label: &str) -> Result<(), String> {
    state::reject_link(path).map_err(|e| format!("{label}: {e}"))
}

pub(crate) fn git_cli_path(path: &Path) -> Cow<'_, Path> {
    #[cfg(windows)]
    {
        use std::path::Prefix;

        let mut components = path.components();
        let prefix = match components.next() {
            Some(Component::Prefix(prefix)) => prefix,
            _ => return Cow::Borrowed(path),
        };
        let mut normalized = match prefix.kind() {
            Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", drive as char)),
            Prefix::VerbatimUNC(server, share) => {
                let mut normalized = PathBuf::from(r"\\");
                normalized.push(server);
                normalized.push(share);
                normalized
            }
            _ => return Cow::Borrowed(path),
        };
        for component in components {
            if !matches!(component, Component::RootDir) {
                normalized.push(component.as_os_str());
            }
        }
        Cow::Owned(normalized)
    }

    #[cfg(not(windows))]
    {
        Cow::Borrowed(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_outside_the_project_are_locked_relative_to_the_project() {
        let root = Path::new("/workspace/app");
        assert_eq!(
            relative_to_root(Path::new("/workspace/dep"), root),
            PathBuf::from("../dep")
        );
        assert_eq!(
            relative_to_root(Path::new("/workspace/app/.vex/deps/remote"), root),
            PathBuf::from(".vex/deps/remote")
        );
    }

    #[cfg(unix)]
    #[test]
    fn managed_dependency_links_are_rejected() {
        use std::os::unix::fs::symlink;
        use std::time::{SystemTime, UNIX_EPOCH};

        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock must be after the Unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("vex-managed-link-test-{}-{id}", std::process::id()));
        let outside = root.with_extension("outside");
        fs::create_dir_all(&root).expect("test project must be created");
        fs::create_dir_all(&outside).expect("outside directory must be created");
        symlink(&outside, root.join(".vex")).expect("test link must be created");

        let error = validate_managed_root(&root, &root.join(".vex/deps"))
            .expect_err("managed root symlinks must be rejected");
        assert!(error.contains("must not be a symbolic link"), "{error}");

        fs::remove_file(root.join(".vex")).expect("test link must be removed");
        fs::remove_dir_all(root).expect("test project must be removed");
        fs::remove_dir_all(outside).expect("outside directory must be removed");
    }

    #[cfg(windows)]
    #[test]
    fn git_cli_path_removes_verbatim_disk_prefix() {
        let path = Path::new(r"\\?\C:\workspace\project\.vex\deps\package");
        assert_eq!(
            git_cli_path(path).as_ref(),
            Path::new(r"C:\workspace\project\.vex\deps\package")
        );
    }

    #[cfg(windows)]
    #[test]
    fn git_cli_path_removes_verbatim_unc_prefix() {
        let path = Path::new(r"\\?\UNC\server\share\project\.vex\deps\package");
        assert_eq!(
            git_cli_path(path).as_ref(),
            Path::new(r"\\server\share\project\.vex\deps\package")
        );
    }
}
