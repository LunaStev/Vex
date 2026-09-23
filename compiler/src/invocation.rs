use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::plan;

pub fn run_build_with_dry_run(args: &[String], user_requested_dry_run: bool) -> Result<(), String> {
    let mut dry_run_args = args.to_vec();
    if !contains_dry_run_flag(&dry_run_args) {
        insert_build_flag(&mut dry_run_args, "--dry-run");
    }
    insert_build_flag(&mut dry_run_args, "--error-format=json");

    let wavec = wavec_path();
    let validation_output = run_wavec_dry_run(&wavec, &dry_run_args)?;
    plan::validate_dry_run_json_output(&validation_output.stdout, &validation_output.stderr)
        .map_err(|error| {
            format!(
                "installed wavec is incompatible with Vex: {error}\nhelp: update wavec or set VEX_WAVEC=/path/to/wavec"
            )
        })?;

    let status = Command::new(&wavec).args(args).status().map_err(|error| {
        let action = if user_requested_dry_run {
            "build --dry-run"
        } else {
            "build"
        };
        format!("failed to execute `{}` {action}: {error}", wavec.display())
    })?;
    if !status.success() {
        let action = if user_requested_dry_run {
            "build dry-run"
        } else {
            "build"
        };
        return Err(format!("wavec {action} failed [{}]", classify_exit(status)));
    }
    Ok(())
}

pub fn contains_dry_run_flag(args: &[String]) -> bool {
    args.iter()
        .take_while(|argument| argument.as_str() != "--")
        .any(|argument| argument == "--dry-run")
}

fn insert_build_flag(args: &mut Vec<String>, flag: &str) {
    if let Some(separator_index) = args.iter().position(|argument| argument == "--") {
        args.insert(separator_index, flag.to_string());
    } else {
        args.push(flag.to_string());
    }
}

fn classify_exit(status: ExitStatus) -> &'static str {
    match status.code() {
        Some(0) => "success",
        Some(1) => "compile/link/run failure",
        Some(2) => "usage error",
        Some(3) => "environment/toolchain/io failure",
        Some(_) => "unknown failure code",
        None => "terminated by signal",
    }
}

struct DryRunOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_wavec_dry_run(wavec: &Path, dry_run_args: &[String]) -> Result<DryRunOutput, String> {
    let output = Command::new(wavec)
        .args(dry_run_args)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute `{}`. Install wavec or set VEX_WAVEC=/path/to/wavec: {error}",
                wavec.display()
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "wavec dry-run failed using `{}` [{}]: {}",
            wavec.display(),
            classify_exit(output.status),
            combined_output(&output.stdout, &output.stderr)
        ));
    }
    Ok(DryRunOutput {
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn combined_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    match (stdout.trim().is_empty(), stderr.trim().is_empty()) {
        (false, false) => format!("{}\n{}", stdout.trim(), stderr.trim()),
        (false, true) => stdout.trim().to_string(),
        (true, false) => stderr.trim().to_string(),
        (true, true) => "<no output>".to_string(),
    }
}

fn wavec_path() -> PathBuf {
    env::var_os("VEX_WAVEC")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("wavec"))
}
