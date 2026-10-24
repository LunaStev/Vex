use crate::{messages::Messages, outcome::Outcome};
use diagnostic::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::ui;
use compiler::{prepare_build, Compiler};
use manifest::Manifest;
use manifest::{PackageTarget, TargetKind};
use resolver::{resolve_validated, ResolveOptions, UpdatePolicy};
use serde_json::json;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildMode {
    Build,
    Run,
    Check,
}

#[derive(Debug, Default)]
struct VexBuildOptions {
    target: Option<String>,
    release: bool,
    profile: Option<String>,
    library: bool,
    binaries: Vec<String>,
    dry_run: bool,
    locked: bool,
    offline: bool,
}

pub fn build(
    mode: BuildMode,
    args: &[String],
    runtime: &[std::ffi::OsString],
    selection: &crate::project::Selection,
    messages: &mut Messages,
) -> Result<Outcome, Error> {
    if matches!(args, [help] if help == "-h" || help == "--help") {
        diagnostic::outln!("{}", build_usage(mode));
        return Ok(Outcome::success());
    }
    let options = parse_vex_build_options(mode, args).map_err(Error::usage)?;
    let compiler = Compiler::select();
    let _project = selection.enter()?;
    let manifest = Manifest::load()?;
    run_build(
        mode,
        &manifest,
        options,
        runtime,
        compiler,
        messages,
        selection.graph.clone(),
    )
}

fn run_build(
    mode: BuildMode,
    manifest: &Manifest,
    options: VexBuildOptions,
    runtime: &[std::ffi::OsString],
    compiler: Result<Compiler, Error>,
    messages: &mut Messages,
    mut graph_selection: source::graph::Selection,
) -> Result<Outcome, Error> {
    let started = Instant::now();
    let root = std::env::current_dir().map_err(Error::environment)?;
    root.to_str().ok_or_else(|| {
        Error::resolution("wavec JSON protocol cannot represent a non-UTF-8 project root")
    })?;
    let selected = select_targets(manifest, mode, &options)?;
    let profile_name =
        options
            .profile
            .as_deref()
            .unwrap_or(if options.release { "release" } else { "dev" });
    let profile = manifest.profile(profile_name).map_err(Error::resolution)?;
    for target in &selected {
        validate_entry(manifest, target)?;
    }
    let mut selected_compiler = compiler?;
    let compiler = &mut selected_compiler;
    let requested_triple = options
        .target
        .as_deref()
        .or(manifest.build_target.as_deref());
    if let Some(target) = requested_triple {
        compiler.validate_target(target)?;
    }
    compiler.validate_version(manifest.compiler.as_deref())?;
    // Recovery has precedence over target-capability queries. Resolution takes
    // its own lease and rechecks recovery after this query, so a concurrent
    // transaction cannot expose sources without coordination.
    let recovery_guard =
        state::Guard::acquire(options.dry_run, ui::status).map_err(Error::environment)?;
    resolver::recover_project(&recovery_guard, options.dry_run)?;
    drop(recovery_guard);
    let spec = compiler.target_spec(requested_triple)?;
    let triple = spec["triple"].as_str().unwrap().to_owned();
    let mut context = source::graph::Context::new(&triple).map_err(Error::resolution)?;
    if let Some(mode) = manifest.platform_for(selected[0], &triple).mode {
        context.mode = mode.as_str().into();
    }
    graph_selection.target = Some(context);
    let mut resolution = resolve_validated(
        manifest,
        ResolveOptions {
            additional_contexts: selected
                .iter()
                .map(|target| {
                    let mut context =
                        source::graph::Context::new(&triple).map_err(Error::resolution)?;
                    if let Some(mode) = manifest.platform_for(target, &triple).mode {
                        context.mode = mode.as_str().into();
                    }
                    Ok(context)
                })
                .collect::<Result<_, Error>>()?,
            selection: graph_selection.clone(),
            dry_run: options.dry_run,
            update: UpdatePolicy::ReuseLocked,
            locked: options.locked,
            offline: options.offline,
        },
        ui::status,
        |candidate| compiler.validate_version(candidate.compiler.as_deref()),
    )?;
    ui::verbose(format!("target: {triple}"));
    ui::verbose(format!("compiler: {}", compiler.path().display()));
    ui::trace(format!(
        "build options: {options:?}; compiler plan is queried for each selected package target"
    ));
    ui::verbose(format!(
        "resolved {} dependencies",
        resolution.package_count()
    ));
    if mode == BuildMode::Run
        && !runtime.is_empty()
        && spec["object_format"] == "wasm"
        && spec["hosted"] == false
    {
        return Err(Error::resolution(
            "bare WebAssembly has no process argument ABI; use a WASI target for program arguments",
        ));
    }
    let mut run_lease = None;
    let generation = if mode == BuildMode::Run {
        Some(if options.dry_run {
            PathBuf::from("target/.vex-run/planned")
        } else {
            let path = compiler::create_run_generation()?;
            run_lease =
                Some(state::artifacts::RunLease::create(&root, &path).map_err(Error::environment)?);
            path
        })
    } else {
        None
    };
    let mut prepared = Vec::new();
    let reporting = messages.enabled();
    messages.status("compiler")?;
    for target in &selected {
        let mut context = source::graph::Context::new(&triple).map_err(Error::resolution)?;
        if let Some(mode) = manifest.platform_for(target, &triple).mode {
            context.mode = mode.as_str().into();
        }
        graph_selection.target = Some(context);
        resolution.activate(&graph_selection)?;
        let configuration = crate::native::prepare_target(
            manifest,
            target,
            &resolution,
            compiler,
            &triple,
            profile_name,
            mode == BuildMode::Check,
            generation.as_deref(),
        )?;
        let mut args = resolution.dependency_args()?;
        if target.kind == TargetKind::Binary && manifest.lib {
            args.push(format!("--dep={}={}", manifest.name, root.display()));
        }
        args.push(format!("-O{}", profile.opt_level));
        args.extend(configuration.global);
        args.push("build".into());
        args.push(input_string(&target.path)?);
        if mode == BuildMode::Run {
            args.push("--run".into());
        }
        if mode == BuildMode::Check {
            args.push("--emit=check".into());
        }
        args.extend(configuration.inputs);
        args.extend(configuration.output);
        let identity = json!({"name":target.name,"kind":target.kind.as_str()});
        let mut report = |mut event: serde_json::Value| {
            event["package_target"] = identity.clone();
            event["profile"] = json!(profile_name);
            messages.emit(event)
        };
        let plan = prepare_build(
            compiler,
            &args,
            runtime,
            options.dry_run,
            generation.as_deref(),
            if reporting { Some(&mut report) } else { None },
        )
        .map_err(|e| target_error(e, target, profile_name))?;
        prepared.push((*target, plan));
    }
    // Even optional linker companions must be disjoint between package targets.
    let paths = prepared
        .iter()
        .flat_map(|(_, p)| p.output_paths().cloned())
        .collect::<Vec<_>>();
    state::artifacts::validate_output_paths(&root, &paths).map_err(Error::compiler)?;
    if options.dry_run {
        ui::verbose("freshness: planning only; compiler will not execute");
        for (target, _) in &prepared {
            ui::status(
                "Planning",
                format!("{} {}", target.kind.as_str(), target.name),
            );
        }
        let plans = prepared
            .iter()
            .map(|(target, plan)| {
                json!({
                    "package_target":{"name":target.name,"kind":target.kind.as_str()},
                    "compiler_plan":plan.plan()
                })
            })
            .collect::<Vec<_>>();
        let envelope = json!({"schema":"vex.build-plan","schema_version":1,"target":triple,
            "profile":{"name":profile_name,"opt_level":profile.opt_level},"plans":plans});
        diagnostic::outln!(
            "{}",
            serde_json::to_string_pretty(&envelope).map_err(Error::internal)?
        );
        return Ok(Outcome::success());
    }
    for (target, plan) in &prepared {
        plan.preflight_ownership()
            .map_err(|e| target_error(e, target, profile_name))?;
    }
    ui::verbose("freshness: no build cache; compiling the current project");
    let mut execution = None;
    for (target, plan) in prepared {
        ui::status(
            if mode == BuildMode::Check {
                "Checking"
            } else {
                "Compiling"
            },
            format!(
                "{} v{} ({} {})",
                manifest.name,
                manifest.version,
                target.kind.as_str(),
                target.name
            ),
        );
        let final_paths = if let Some(path) = plan.plan()["link"]["output"].as_str() {
            vec![path.to_owned()]
        } else {
            plan.plan()["compile"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|job| job["output"].as_str().map(str::to_owned))
                .collect()
        };
        let identity = json!({"name":target.name,"kind":target.kind.as_str()});
        let artifact_kind = target.output.map(|v| v.as_str()).unwrap_or("binary");
        let mut report = |mut event: serde_json::Value| {
            event["package_target"] = identity.clone();
            event["profile"] = json!(profile_name);
            if event["event"] == "artifact" {
                event["artifact_kind"] = json!(artifact_kind);
            }
            messages.emit(event)
        };
        execution = plan
            .execute(compiler, if reporting { Some(&mut report) } else { None })
            .map_err(|e| target_error(e, target, profile_name))?;
        for path in final_paths {
            ui::status("Artifact", path);
        }
    }
    drop(resolution);
    let mut outcome = Outcome::success();
    if let Some(execution) = execution {
        ui::status("Running", &selected[0].name);
        messages.status("running")?;
        outcome = Outcome::program(execution.execute()?);
    }
    drop(run_lease);
    if outcome.code == 0 {
        ui::status(
            "Finished",
            format!(
                "{profile_name} profile in {:.2}s",
                started.elapsed().as_secs_f64()
            ),
        );
    }
    Ok(outcome)
}

fn target_error(error: Error, target: &PackageTarget, profile: &str) -> Error {
    error
        .with_field(
            "package_target",
            format!("{}:{}", target.kind.as_str(), target.name),
        )
        .with_field("profile", profile)
}

fn select_targets<'a>(
    manifest: &'a Manifest,
    mode: BuildMode,
    options: &VexBuildOptions,
) -> Result<Vec<&'a PackageTarget>, Error> {
    let binaries = manifest
        .targets
        .iter()
        .filter(|t| t.kind == TargetKind::Binary)
        .collect::<Vec<_>>();
    if mode == BuildMode::Run {
        if options.library {
            return Err(Error::usage("vex run requires a binary; --lib is invalid"));
        }
        if options.binaries.len() > 1 {
            return Err(Error::usage("vex run selects exactly one binary"));
        }
        let name = options
            .binaries
            .first()
            .map(String::as_str)
            .or(manifest.default_run.as_deref());
        return if let Some(name) = name {
            binaries
                .into_iter()
                .find(|t| t.name == name)
                .map(|t| vec![t])
                .ok_or_else(|| Error::resolution(format!("unknown binary target `{name}`")))
        } else if binaries.len() == 1 {
            Ok(binaries)
        } else if binaries.is_empty() {
            Err(Error::resolution(
                "library packages cannot be run; select a package with a binary target",
            ))
        } else {
            Err(Error::resolution(
                "multiple binary targets; select one with --bin <name> or set default_run",
            ))
        };
    }
    for name in &options.binaries {
        if !binaries.iter().any(|t| t.name == *name) {
            return Err(Error::resolution(format!("unknown binary target `{name}`")));
        }
    }
    if options.library && !manifest.lib {
        return Err(Error::resolution("package has no library target"));
    }
    let all = !options.library && options.binaries.is_empty();
    let mut targets = manifest
        .targets
        .iter()
        .filter(|t| {
            all || match t.kind {
                TargetKind::Library => options.library,
                TargetKind::Binary => options.binaries.contains(&t.name),
            }
        })
        .collect::<Vec<_>>();
    targets.sort_by_key(|t| (t.kind != TargetKind::Library, &t.name));
    Ok(targets)
}

pub(crate) fn validate_entry(manifest: &Manifest, target: &PackageTarget) -> Result<(), Error> {
    let path = &target.path;
    if !path.is_file() {
        return Err(Error::resolution(format!(
            "missing canonical package entry `{}` for {} `{}`",
            path.display(),
            target.kind.as_str(),
            target.name
        )));
    }
    if manifest.format >= 3 {
        let root = manifest
            .source_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(Error::environment)?;
        let actual = path.canonicalize().map_err(Error::environment)?;
        if !actual.starts_with(root) {
            return Err(Error::resolution(
                "package target entry escapes its package directory",
            ));
        }
    }
    input_string(path).map(|_| ())
}

fn parse_vex_build_options(mode: BuildMode, args: &[String]) -> Result<VexBuildOptions, String> {
    let mut options = VexBuildOptions::default();
    let mut i = 0;

    while i < args.len() {
        let token = args[i].as_str();

        if token == "--" {
            if mode != BuildMode::Run {
                return Err(
                    "runtime arguments after `--` are only valid with `vex run`".to_string()
                );
            }
            break;
        }

        match token {
            "--target" => {
                i += 1;
                if i >= args.len() {
                    return Err("missing value for `--target`".to_string());
                }
                set_target(&mut options, &args[i])?;
            }
            "--release" => {
                if options.release {
                    return Err("--release may only be specified once".into());
                }
                options.release = true;
            }
            "--profile" | "--bin" => {
                i += 1;
                let value = args
                    .get(i)
                    .filter(|s| !s.is_empty() && !s.starts_with('-'))
                    .ok_or_else(|| format!("missing value for `{token}`"))?;
                set_selection(&mut options, token, value)?;
            }
            "--lib" => {
                if options.library {
                    return Err("--lib may only be specified once".into());
                }
                options.library = true;
            }
            _ if token.starts_with("--profile=") => {
                set_selection(&mut options, "--profile", &token[10..])?
            }
            _ if token.starts_with("--bin=") => set_selection(&mut options, "--bin", &token[6..])?,
            "--dry-run" => options.dry_run = true,
            "--locked" => options.locked = true,
            "--offline" => options.offline = true,
            "-h" | "--help" => return Err(build_usage(mode).to_string()),
            _ if token.starts_with("--target=") => {
                set_target(&mut options, token.trim_start_matches("--target="))?;
            }
            _ if token.starts_with('-') => {
                return Err(format!(
                    "unknown Vex option `{token}`. Vex does not accept raw wavec flags here"
                ));
            }
            _ => {
                return Err(format!(
                    "unexpected argument `{token}`. Vex builds the package described by vex.ws"
                ));
            }
        }

        i += 1;
    }

    if options.release && options.profile.is_some() {
        return Err("--release and --profile select the same setting; use only one".into());
    }
    if mode == BuildMode::Run && (options.library || options.binaries.len() > 1) {
        return Err(
            "vex run selects one binary with --bin; --lib and multiple binaries are invalid".into(),
        );
    }
    Ok(options)
}

fn set_selection(options: &mut VexBuildOptions, option: &str, value: &str) -> Result<(), String> {
    if value.is_empty() || value.starts_with('-') {
        return Err(format!("missing value for `{option}`"));
    }
    if option == "--profile" {
        if options.profile.is_some() {
            return Err("--profile may only be specified once".into());
        }
        options.profile = Some(value.into());
    } else {
        if options.binaries.iter().any(|name| name == value) {
            return Err(format!("duplicate binary selection `{value}`"));
        }
        options.binaries.push(value.into());
    }
    Ok(())
}

fn set_target(options: &mut VexBuildOptions, target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("missing value for `--target`: expected a target triple".to_string());
    }
    if target.starts_with('-') {
        return Err(format!(
            "missing value for `--target`: expected a target triple, found option `{target}`"
        ));
    }
    if !target
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(format!(
            "invalid value `{target}` for `--target`: expected a target triple containing only ASCII letters, digits, `-`, `_`, or `.`"
        ));
    }
    if options.target.is_some() {
        return Err("`--target` may only be specified once".to_string());
    }
    options.target = Some(target.to_string());
    Ok(())
}

fn build_usage(mode: BuildMode) -> &'static str {
    match mode {
        BuildMode::Build => {
            "usage: vex build [--lib] [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline]"
        }
        BuildMode::Run => {
            "usage: vex run [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline] [-- <args...>]"
        }
        BuildMode::Check => {
            "usage: vex check [--lib] [--bin <name>] [--target <triple>] [--profile <name> | --release] [--dry-run] [--locked] [--offline]"
        }
    }
}

fn input_string(path: &Path) -> Result<String, Error> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        Error::resolution("wavec JSON protocol cannot represent a non-UTF-8 input path")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn parses_small_vex_build_option_surface() {
        let options = parse_vex_build_options(
            BuildMode::Run,
            &strings(&[
                "--target",
                "x86_64-unknown-linux-gnu",
                "--release",
                "--locked",
                "--offline",
                "--",
            ]),
        )
        .expect("options must parse");

        assert_eq!(options.target.as_deref(), Some("x86_64-unknown-linux-gnu"));
        assert!(options.release);
        assert!(options.locked);
        assert!(options.offline);
    }

    #[test]
    fn rejects_raw_wavec_flags_and_source_inputs() {
        let err = parse_vex_build_options(BuildMode::Build, &strings(&["--emit=obj"]))
            .expect_err("raw wavec flags are not Vex build options");
        assert!(err.contains("raw wavec flags"), "{err}");

        let err = parse_vex_build_options(BuildMode::Run, &strings(&["src/main.wave"]))
            .expect_err("Vex run is manifest-based");
        assert!(err.contains("vex.ws"), "{err}");
    }

    #[test]
    fn rejects_empty_and_duplicate_targets() {
        for arguments in [
            strings(&["--target="]),
            strings(&["--target", ""]),
            strings(&["--target", "host", "--target=other"]),
        ] {
            parse_vex_build_options(BuildMode::Build, &arguments)
                .expect_err("invalid target options must be rejected");
        }
    }

    #[test]
    fn rejects_option_tokens_and_invalid_target_syntax() {
        for mode in [BuildMode::Build, BuildMode::Run, BuildMode::Check] {
            for token in ["--release", "--dry-run", "--locked", "--offline", "--"] {
                let err = parse_vex_build_options(mode, &strings(&["--target", token]))
                    .expect_err("another option cannot be a target value");
                assert!(err.contains("missing value for `--target`"), "{err}");
                assert!(err.contains(token), "{err}");
            }
            let err = parse_vex_build_options(mode, &strings(&["--target=bad/target"]))
                .expect_err("target syntax must be checked by Vex");
            assert!(err.contains("invalid value"), "{err}");
        }
    }
}
