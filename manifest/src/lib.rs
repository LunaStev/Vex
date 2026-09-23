use std::fs;
use std::path::{Path, PathBuf};

mod parse;
mod render;

pub use render::render_new_manifest;

pub const MANIFEST_FILE: &str = "vex.ws";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DependencySource {
    Path {
        path: String,
    },
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Dependency {
    pub name: String,
    pub version: Option<String>,
    pub source: DependencySource,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub lib: bool,
    pub description: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<Dependency>,
    pub source_path: PathBuf,
}

impl Manifest {
    pub fn load() -> Result<Self, String> {
        let source_path = Path::new(MANIFEST_FILE);
        if !source_path.is_file() {
            let directory = std::env::current_dir()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());
            return Err(format!(
                "could not find `{MANIFEST_FILE}` in `{directory}`\nhelp: run `vex init` to create a package"
            ));
        }
        Self::load_from(source_path)
    }

    pub fn load_from(source_path: impl AsRef<Path>) -> Result<Self, String> {
        let source_path = source_path.as_ref().to_path_buf();
        if !source_path.is_file() {
            return Err(format!(
                "manifest not found at `{}`",
                source_path.to_string_lossy()
            ));
        }

        let raw = fs::read_to_string(&source_path)
            .map_err(|e| format!("failed to read `{}`: {e}", source_path.to_string_lossy()))?;

        parse::parse_manifest(&raw, source_path.clone()).map_err(|err| {
            format!(
                "failed to load manifest `{}`: {err}",
                source_path.to_string_lossy()
            )
        })
    }

    pub fn default_entry_path(&self) -> PathBuf {
        if self.lib {
            PathBuf::from("src/lib.wave")
        } else {
            PathBuf::from("src/main.wave")
        }
    }
}
