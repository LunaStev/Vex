use diagnostic::Error;
use manifest::MANIFEST_FILE;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Selection {
    pub manifest_path: Option<PathBuf>,
}

pub struct Project {
    pub manifest_path: PathBuf,
    previous_directory: PathBuf,
}

impl Selection {
    pub fn enter(&self) -> Result<Project, Error> {
        let previous_directory = std::env::current_dir().map_err(Error::environment)?;
        let start = previous_directory
            .canonicalize()
            .map_err(Error::environment)?;
        let selected = match &self.manifest_path {
            Some(path) => {
                if path.file_name().is_none_or(|name| name != MANIFEST_FILE) {
                    return Err(Error::usage("--manifest-path must name a vex.ws file"));
                }
                let path = if path.is_absolute() {
                    path.clone()
                } else {
                    start.join(path)
                };
                if !path.is_file() {
                    return Err(Error::environment(format!(
                        "manifest not found at `{}`",
                        path.display()
                    )));
                }
                path
            }
            None => discover(&start)?,
        };
        let manifest_path = selected.canonicalize().map_err(Error::environment)?;
        if manifest_path
            .file_name()
            .is_none_or(|name| name != MANIFEST_FILE)
        {
            return Err(Error::usage(
                "the selected manifest must resolve to a vex.ws file",
            ));
        }
        let root = manifest_path
            .parent()
            .ok_or_else(|| Error::environment("manifest has no parent directory"))?;
        std::env::set_current_dir(root).map_err(Error::environment)?;
        Ok(Project {
            manifest_path,
            previous_directory,
        })
    }
}

fn discover(start: &Path) -> Result<PathBuf, Error> {
    for directory in start.ancestors() {
        let path = directory.join(MANIFEST_FILE);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                if !path.is_file() {
                    return Err(Error::environment(format!(
                        "selected manifest `{}` is not a file",
                        path.display()
                    )));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(Error::environment(format!(
                    "cannot inspect `{}`: {error}",
                    path.display()
                )))
            }
        }
    }
    Err(Error::environment(format!("could not find `{MANIFEST_FILE}` in `{}` or its ancestors\nhelp: run `vex init` to create a package", start.display())))
}

impl Drop for Project {
    fn drop(&mut self) {
        // A command owns this scope until its compiler and user program exit.
        let _ = std::env::set_current_dir(&self.previous_directory);
    }
}
