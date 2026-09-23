use std::fs;
use std::path::Path;

use crate::{parse, render, Lockfile, LOCKFILE_NAME};

pub fn read_lockfile() -> Result<Option<Lockfile>, String> {
    let path = Path::new(LOCKFILE_NAME);
    if !path.exists() {
        return Ok(None);
    }

    let raw =
        fs::read_to_string(path).map_err(|e| format!("failed to read `{LOCKFILE_NAME}`: {e}"))?;
    parse::parse_lockfile(&raw).map(Some)
}

pub fn write_lockfile(lockfile: &Lockfile) -> Result<(), String> {
    let content = render::render_lockfile(lockfile.clone().normalized());
    fs::write(LOCKFILE_NAME, content).map_err(|e| format!("failed to write `{LOCKFILE_NAME}`: {e}"))
}
