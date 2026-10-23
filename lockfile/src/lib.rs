use std::path::PathBuf;

mod parse;
mod render;
mod store;

pub use parse::parse_lockfile as decode;
pub use render::render_lockfile as encode;
pub use store::{read_lockfile, write_lockfile};

pub const LOCKFILE_NAME: &str = "vex.lock";
pub const LOCKFILE_VERSION: i64 = 4;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LockedSource {
    Path {
        requested: String,
        resolved: PathBuf,
    },
    Git {
        url: String,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        commit: String,
        resolved: PathBuf,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LockedPackage {
    /// Import binding, distinct from the declared package name.
    pub package: Option<String>,
    pub id: Option<String>,
    pub subdir: Option<String>,
    pub name: String,
    pub version: String,
    pub source: LockedSource,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Lockfile {
    pub version: i64,
    pub packages: Vec<LockedPackage>,
}

impl LockedPackage {
    pub fn package_name(&self) -> &str {
        self.package.as_deref().unwrap_or(&self.name)
    }
    pub fn directory(&self) -> PathBuf {
        let (LockedSource::Path { resolved, .. } | LockedSource::Git { resolved, .. }) =
            &self.source;
        self.subdir
            .as_ref()
            .map_or_else(|| resolved.clone(), |subdir| resolved.join(subdir))
    }
}

impl Lockfile {
    pub fn empty() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            packages: Vec::new(),
        }
    }

    pub fn normalized(mut self) -> Self {
        for package in &mut self.packages {
            package.dependencies.sort();
            package.dependencies.dedup();
        }
        self.packages.sort_by(|a, b| a.name.cmp(&b.name));
        self
    }

    pub fn package(&self, name: &str) -> Option<&LockedPackage> {
        self.packages.iter().find(|package| package.name == name)
    }
}

pub fn package_identity(
    name: &str,
    version: &str,
    source: &LockedSource,
    subdir: Option<&str>,
) -> String {
    use sha2::{Digest, Sha256};
    // Concrete source identity excludes selectors and import aliases.
    let identity = match source {
        LockedSource::Path { resolved, .. } => {
            #[cfg(windows)]
            let path = resolved.to_string_lossy().replace('\\', "/");
            #[cfg(not(windows))]
            let path = resolved.to_string_lossy();
            format!("path:{path}")
        }
        LockedSource::Git { url, commit, .. } => format!("git:{}@{commit}", source::identity(url)),
    };
    format!(
        "{:x}",
        Sha256::digest(format!("{name:?}|{version:?}|{identity:?}|{subdir:?}").as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_round_trip_preserves_git_commits_and_edges() {
        let lockfile = Lockfile {
            version: LOCKFILE_VERSION,
            packages: vec![
                LockedPackage {
                    package: None,
                    id: None,
                    subdir: None,
                    name: "core".to_string(),
                    version: "1.0.0".to_string(),
                    source: LockedSource::Path {
                        requested: "../core".to_string(),
                        resolved: PathBuf::from("../core"),
                    },
                    dependencies: vec![],
                },
                LockedPackage {
                    package: None,
                    id: None,
                    subdir: None,
                    name: "math".to_string(),
                    version: "1.2.3".to_string(),
                    source: LockedSource::Git {
                        url: "https://example.com/math.git".to_string(),
                        branch: Some("main".to_string()),
                        tag: None,
                        rev: None,
                        commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
                        resolved: PathBuf::from(".vex/deps/math"),
                    },
                    dependencies: vec!["core".to_string()],
                },
            ],
        };

        let rendered = crate::render::render_lockfile(lockfile.clone());
        let parsed = crate::parse::parse_lockfile(&rendered).expect("rendered lockfile must parse");
        assert_eq!(parsed, lockfile);
    }
}
