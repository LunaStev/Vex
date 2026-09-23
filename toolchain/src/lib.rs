use std::env;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

pub fn install_wavec(version: Option<&str>) -> Result<(), String> {
    let installer_args = installer_args(version);

    #[cfg(unix)]
    let result = unix::install(&installer_args);
    #[cfg(windows)]
    let result = windows::install(&installer_args);
    #[cfg(not(any(unix, windows)))]
    let result = Err("wavec setup is not supported on this platform".to_string());

    result
}

pub fn requested_version(version: Option<&str>) -> &str {
    version.unwrap_or("latest")
}

fn installer_args(version: Option<&str>) -> Vec<String> {
    match version {
        Some(version) => vec!["--version".to_string(), version.to_string()],
        None => vec!["latest".to_string()],
    }
}

fn create_installer_file(extension: &str) -> Result<(PathBuf, File), String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..100 {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "vex-wave-install-{}-{timestamp}-{id}.{extension}",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "failed to create temporary wavec installer `{}`: {error}",
                    path.display()
                ));
            }
        }
    }
    Err("failed to allocate a unique temporary path for the wavec installer".to_string())
}

fn remove_installer(path: &PathBuf) -> Result<(), String> {
    fs::remove_file(path).map_err(|error| {
        format!(
            "failed to remove temporary wavec installer `{}`: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_arguments_preserve_an_explicit_version() {
        assert_eq!(installer_args(None), ["latest"]);
        assert_eq!(
            installer_args(Some("0.2.0-pre-beta")),
            ["--version", "0.2.0-pre-beta"]
        );
    }

    #[test]
    fn installer_temporary_files_are_unique_and_exclusive() {
        let (first_path, first_file) =
            create_installer_file("test").expect("first temporary installer must be created");
        let (second_path, second_file) =
            create_installer_file("test").expect("second temporary installer must be created");
        assert_ne!(first_path, second_path);
        drop(first_file);
        drop(second_file);
        fs::remove_file(first_path).expect("first temporary installer must be removed");
        fs::remove_file(second_path).expect("second temporary installer must be removed");
    }
}
