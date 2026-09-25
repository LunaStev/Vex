use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus};

use lockfile::LOCKFILE_NAME;

use crate::paths::{git_cli_path, validate_managed_checkout_path};

pub(crate) fn ensure_repository(
    destination: &Path,
    url: &str,
    name: &str,
    status: &mut dyn FnMut(&str, String),
) -> Result<(), String> {
    validate_managed_checkout_path(destination)?;
    if destination.exists() {
        if !destination.join(".git").is_dir() {
            return Err(format!(
                "managed dependency path `{}` exists but is not a Git checkout",
                destination.display()
            ));
        }
        verify_origin(destination, url)?;
        return Ok(());
    }

    let parent = destination
        .parent()
        .ok_or_else(|| format!("invalid dependency path `{}`", destination.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create `{}`: {error}", parent.display()))?;
    status("Cloning", format!("{name} ({url})"));
    let destination = git_cli_path(destination);
    run(
        command()
            .args(["clone", "--", url])
            .arg(destination.as_ref()),
        "clone Git dependency",
    )
}

pub(crate) fn require_local_repository(
    destination: &Path,
    url: &str,
    name: &str,
    commit: &str,
) -> Result<(), String> {
    validate_managed_checkout_path(destination)?;
    if !destination.join(".git").is_dir() {
        return Err(format!(
            "locked dependency `{name}` is not available locally in offline mode\n\nCaused by:\n  checkout `{}` is missing\n\nhelp: run `vex fetch` while online",
            destination.display()
        ));
    }
    verify_origin(destination, url)?;
    if !has_commit(destination, commit)? {
        return Err(format!(
            "locked dependency `{name}` is incomplete in offline mode\n\nCaused by:\n  commit `{commit}` was not found in `{}`\n\nhelp: run `vex fetch` while online",
            destination.display()
        ));
    }
    Ok(())
}

fn verify_origin(destination: &Path, expected: &str) -> Result<(), String> {
    // Read declarations, not `remote get-url`, which expands user insteadOf rules.
    // NUL delimiters preserve URLs containing whitespace and detect multiple values.
    let output = command_in(destination)
        .args(["config", "--null", "--get-all", "remote.origin.url"])
        .output()
        .map_err(|error| format!("failed to read Git dependency origin: {error}"))?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(git_error(
            "read Git dependency origin",
            output.status,
            &output.stdout,
            &output.stderr,
        ));
    }
    let values: Vec<_> = output
        .stdout
        .strip_suffix(&[0])
        .map(|bytes| bytes.split(|byte| *byte == 0).collect())
        .unwrap_or_default();
    if values.len() == 1 && values[0] == expected.as_bytes() {
        return Ok(());
    }
    let actual = if values.is_empty() {
        "<missing>".to_string()
    } else {
        values
            .iter()
            .map(|value| String::from_utf8_lossy(value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    Err(format!(
        "managed checkout `{}` has origin `{actual}`, expected exactly one origin `{expected}`\nhelp: restore remote.origin.url to the declared source, then run `vex fetch`",
        destination.display()
    ))
}

pub(crate) fn fetch(destination: &Path) -> Result<(), String> {
    run(
        command_in(destination).args(["fetch", "origin", "--tags", "--prune"]),
        "fetch Git dependency",
    )
}

pub(crate) fn refresh_default_branch(destination: &Path) -> Result<(), String> {
    run(
        command_in(destination).args(["remote", "set-head", "origin", "--auto"]),
        "refresh Git dependency default branch",
    )
}

pub(crate) fn has_commit(destination: &Path, commit: &str) -> Result<bool, String> {
    let output = command_in(destination)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .output()
        .map_err(|error| format!("failed to inspect Git dependency commit: {error}"))?;
    Ok(output.status.success())
}

pub(crate) fn require_checkout_at(
    destination: &Path,
    url: &str,
    name: &str,
    commit: &str,
) -> Result<(), String> {
    validate_managed_checkout_path(destination)?;
    if !destination.join(".git").is_dir() {
        return Err(format!(
            "locked Git dependency is not available at `{}`\nhelp: run `vex fetch`",
            destination.display()
        ));
    }
    verify_origin(destination, url)?;
    reject_dirty_checkout(destination, name)?;
    let current = stdout(
        command_in(destination).args(["rev-parse", "HEAD"]),
        "read Git dependency HEAD",
    )?;
    if current != commit {
        return Err(format!(
            "Git dependency at `{}` is checked out at `{current}`, but `{LOCKFILE_NAME}` pins `{commit}`\nhelp: run `vex fetch`",
            destination.display()
        ));
    }
    Ok(())
}

pub(crate) fn checkout_commit(destination: &Path, name: &str, commit: &str) -> Result<(), String> {
    reject_dirty_checkout(destination, name)?;
    let current = stdout(
        command_in(destination).args(["rev-parse", "HEAD"]),
        "read Git dependency HEAD",
    )?;
    if current == commit {
        let head = command_in(destination)
            .args(["symbolic-ref", "--quiet", "HEAD"])
            .output()
            .map_err(|error| format!("failed to inspect Git dependency HEAD: {error}"))?;
        match head.status.code() {
            Some(1) => return Ok(()), // Already detached at the exact locked commit.
            Some(0) => {}             // Matching branch tip still needs detaching.
            _ => {
                return Err(git_error(
                    "inspect Git dependency HEAD",
                    head.status,
                    &head.stdout,
                    &head.stderr,
                ))
            }
        }
    }
    run(
        command_in(destination).args(["checkout", "--detach", commit]),
        "checkout locked Git dependency commit",
    )?;
    reject_dirty_checkout(destination, name)
}

pub(crate) fn reject_dirty_checkout(destination: &Path, name: &str) -> Result<(), String> {
    let dirty = stdout(
        command_in(destination).args([
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ]),
        "inspect Git dependency checkout",
    )?;
    if !dirty.is_empty() {
        return Err(format!(
            "managed Git dependency `{name}` at `{}` has local changes\nhelp: preserve those changes outside the managed checkout, then restore it and rerun `vex fetch`; Vex will not discard your files",
            destination.display()
        ));
    }
    Ok(())
}

pub(crate) fn command_in(destination: &Path) -> Command {
    let mut command = command();
    let path = git_cli_path(destination);
    command.arg("-C").arg(path.as_ref());
    command.arg("--git-dir").arg(path.join(".git"));
    command.arg("--work-tree").arg(path.as_ref());
    command
}

fn command() -> Command {
    let mut command = Command::new("git");
    command.args(["-c", "protocol.ext.allow=never"]);
    // Read-only graph discovery and dry-run status checks must not refresh the
    // live index as a side effect. Explicit checkout/fetch operations still work.
    command.env("GIT_OPTIONAL_LOCKS", "0");
    // A hook or parent tool can export repository-local context that overrides -C.
    // Keep authentication, SSH, proxies, HOME, and user config (including URL
    // rewrites) intact; remove only repository selection/object/index context.
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
        "GIT_SHALLOW_FILE",
        "GIT_IMPLICIT_WORK_TREE",
        "GIT_GRAFT_FILE",
        "GIT_REPLACE_REF_BASE",
    ] {
        command.env_remove(variable);
    }
    command
}

fn run(command: &mut Command, action: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("failed to start git to {action}: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(git_error(
        action,
        output.status,
        &output.stdout,
        &output.stderr,
    ))
}

pub(crate) fn stdout(command: &mut Command, action: &str) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|error| format!("failed to start git to {action}: {error}"))?;
    if !output.status.success() {
        return Err(git_error(
            action,
            output.status,
            &output.stdout,
            &output.stderr,
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_error(action: &str, status: ExitStatus, stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let details = if !stderr.trim().is_empty() {
        stderr.trim()
    } else if !stdout.trim().is_empty() {
        stdout.trim()
    } else {
        "<no output>"
    };
    format!("could not {action} (status {status}): {details}")
}
