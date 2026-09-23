use std::fs;
use std::process::{Command, Stdio};

use crate::{create_installer_file, remove_installer};

const INSTALLER_URL: &str = "https://wave-lang.dev/install.sh";

pub(crate) fn install(args: &[String]) -> Result<(), String> {
    let (script, output) = create_installer_file("sh")?;
    let status = Command::new("curl")
        .args(["-fsSL", INSTALLER_URL])
        .stdin(Stdio::null())
        .stdout(Stdio::from(output))
        .status();
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            let _ = fs::remove_file(&script);
            return Err(format!("failed to start curl for wavec installer: {error}"));
        }
    };
    if !status.success() {
        let _ = fs::remove_file(&script);
        return Err(format!("failed to download wavec installer: {status}"));
    }

    let mut child = match Command::new("bash")
        .arg(&script)
        .args(args)
        .stdin(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = fs::remove_file(&script);
            return Err(format!("failed to start wavec installer: {error}"));
        }
    };
    let status = child.wait();
    remove_installer(&script)?;
    let status = status.map_err(|error| format!("failed to wait for wavec installer: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("wavec installation failed with status: {status}"))
    }
}
