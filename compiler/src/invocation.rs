use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::{env, fs};

use crate::plan;

/// Compilation ends before this value is returned. No project guard is owned
/// here: the caller must drop its resolution/guard before calling execute().
pub struct Execution {
    program: String,
    args: Vec<String>,
}
impl Execution {
    pub fn execute(self) -> Result<(), String> {
        let status = Command::new(&self.program)
            .args(&self.args)
            .status()
            .map_err(|e| format!("failed to run `{}`: {e}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("program failed [{status}]"))
        }
    }
}

pub fn run_build_with_dry_run(
    args: &[String],
    user_requested_dry_run: bool,
    generation: Option<&Path>,
) -> Result<Option<Execution>, String> {
    let mut dry_run_args = args.to_vec();
    if !contains_dry_run_flag(&dry_run_args) {
        insert_build_flag(&mut dry_run_args, "--dry-run");
    }
    insert_build_flag(&mut dry_run_args, "--error-format=json");
    let wavec = wavec_path();
    let validation_output = run_wavec_dry_run(&wavec, &dry_run_args)?;
    let plan = plan::validate_dry_run_json_output(&validation_output.stdout, &validation_output.stderr)
        .map_err(|error| format!("installed wavec is incompatible with Vex: {error}\nhelp: update wavec or set VEX_WAVEC=/path/to/wavec"))?;
    if !validation_output.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&validation_output.stderr));
    }
    if user_requested_dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
        );
        return Ok(None);
    }
    let separator = args.iter().position(|a| a == "--").unwrap_or(args.len());
    let is_run = args[..separator].iter().any(|a| a == "--run");
    let execution = if is_run {
        let generation = generation.ok_or("missing per-run output generation")?;
        if plan["mode"] != "build+run" || plan["emit"] != "bin" {
            return Err("run requires a build plan that emits a binary".into());
        }
        for job in plan["compile"].as_array().unwrap() {
            validate_output_path(Path::new(job["output"].as_str().unwrap()), generation)?;
        }
        let output = plan["link"]["output"]
            .as_str()
            .ok_or("run plan is missing link.output")?;
        validate_output_path(Path::new(output), generation)?;
        let program = plan["execute"]["program"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("run plan is missing execute.program")?
            .to_owned();
        let run_args = plan::string_array(&plan["execute"]["args"], "execute.args")?;
        let expected = if separator < args.len() {
            &args[separator + 1..]
        } else {
            &[]
        };
        if program == output {
            if run_args != expected {
                return Err("native execution plan changed runtime arguments".into());
            }
        } else if !run_args.ends_with(expected)
            || !run_args[..run_args.len() - expected.len()]
                .iter()
                .any(|a| a == output)
        {
            return Err("runner plan does not reference the generated artifact and original runtime arguments".into());
        }
        Some((
            Execution {
                program,
                args: run_args,
            },
            PathBuf::from(output),
        ))
    } else {
        None
    };
    state::ensure_dir(Path::new("target"))?;
    let compile_args: Vec<_> = args[..separator]
        .iter()
        .filter(|a| a.as_str() != "--run")
        .collect();
    let status = Command::new(&wavec)
        .args(compile_args)
        .status()
        .map_err(|e| format!("failed to execute `{}` build: {e}", wavec.display()))?;
    if !status.success() {
        return Err(format!("wavec build failed [{}]", classify_exit(status)));
    }
    if let Some((execution, output)) = execution {
        state::reject_link(&output)?;
        let actual = output
            .canonicalize()
            .map_err(|e| format!("compiler did not produce {}: {e}", output.display()))?;
        let generation = generation
            .unwrap()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !actual.starts_with(&generation) || !actual.is_file() {
            return Err("compiler output escaped the run generation".into());
        }
        return Ok(Some(execution));
    }
    Ok(None)
}

fn validate_output_path(output: &Path, generation: &Path) -> Result<(), String> {
    if !output.starts_with(generation)
        || output
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("link.output must be inside the per-run output generation".into());
    }
    Ok(())
}

pub fn create_run_generation() -> Result<PathBuf, String> {
    state::ensure_dir(Path::new("target"))?;
    let parent = Path::new("target/.vex-run");
    state::ensure_dir(parent)?;
    loop {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = parent.join(format!("{}-{time}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
    }
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
