use diagnostic::Error;
use lockfile::LockedSource;
use manifest::{Manifest, PackageTarget, PlatformSettings, TargetKind};
use serde_json::{json, Value};
use std::path::Path;

pub fn metadata(args: &[String], selection: &crate::project::Selection) -> Result<(), Error> {
    if matches!(args, [help] if help == "-h" || help == "--help") {
        diagnostic::outln!(
            "usage: vex metadata [--format=json] [--format-version <1|2>] [--locked] [--offline]"
        );
        return Ok(());
    }
    let mut locked = false;
    let mut format_version = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--locked" => locked = true,
            "--offline" | "--format=json" => {}
            "--format" if args.next().is_some_and(|s| s == "json") => {}
            "--format-version" => {
                parse_version(args.next().map(String::as_str), &mut format_version)?
            }
            value if value.starts_with("--format-version=") => {
                parse_version(value.strip_prefix("--format-version="), &mut format_version)?;
            }
            _ => {
                return Err(Error::usage(format!(
                    "unsupported metadata option `{arg}`; only JSON format is supported"
                )))
            }
        }
    }
    let format_version = format_version.unwrap_or(1);
    let project = selection.enter()?;
    let root = project.manifest_path.parent().unwrap();
    json_path(root)?;
    let manifest = Manifest::load()?;
    if format_version == 1 {
        require_single_target(&manifest)?;
    }
    let resolution = resolver::inspect(&manifest, locked, crate::ui::status)?;
    let mut packages = Vec::new();
    for package in resolution.packages() {
        let (resolved, source) = match &package.source {
            LockedSource::Path {
                requested,
                resolved,
            } => (resolved, json!({"kind":"path", "requested":requested})),
            LockedSource::Git {
                url,
                branch,
                tag,
                rev,
                commit,
                resolved,
            } => (
                resolved,
                json!({"kind":"git", "url":source::identity(url), "branch":branch,"tag":tag,"rev":rev,"commit":commit}),
            ),
        };
        let directory = resolved.canonicalize().map_err(Error::environment)?;
        let package_manifest_path = directory.join(manifest::MANIFEST_FILE);
        let package_manifest = Manifest::load_from(&package_manifest_path)?;
        let mut value = package_record(
            &package_manifest,
            &directory,
            &package_manifest_path,
            format_version,
        )?;
        value["source"] = source;
        let mut dependencies = package.dependencies.clone();
        dependencies.sort();
        value["dependencies"] = json!(dependencies);
        packages.push(value);
    }
    packages.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let value: Value = json!({"schema_version":format_version,
        "root":package_record(&manifest, root, &project.manifest_path, format_version)?,
        "target_directory":json_path(&root.join("target"))?, "packages":packages});
    diagnostic::outln!(
        "{}",
        serde_json::to_string_pretty(&value).map_err(Error::internal)?
    );
    Ok(())
}

fn parse_version(value: Option<&str>, selected: &mut Option<u32>) -> Result<(), Error> {
    if selected.is_some() {
        return Err(Error::usage("--format-version may only be specified once"));
    }
    *selected = Some(match value {
        Some("1") => 1,
        Some("2") => 2,
        _ => return Err(Error::usage("--format-version requires 1 or 2")),
    });
    Ok(())
}

fn require_single_target(manifest: &Manifest) -> Result<&PackageTarget, Error> {
    if manifest.targets.len() != 1 {
        return Err(Error::resolution(format!(
            "metadata schema 1 cannot represent the targets of package `{}`\nhelp: use `vex metadata --format-version 2`",
            manifest.name
        )));
    }
    Ok(&manifest.targets[0])
}

fn package_record(
    manifest: &Manifest,
    root: &Path,
    manifest_path: &Path,
    version: u32,
) -> Result<Value, Error> {
    let mut dependencies: Vec<_> = manifest
        .dependencies
        .iter()
        .map(|d| d.name.clone())
        .collect();
    dependencies.sort();
    let mut value = json!({"name":manifest.name, "version":manifest.version,
        "manifest_format":manifest.format, "compiler":manifest.compiler,
        "root":json_path(root)?, "manifest_path":json_path(manifest_path)?,
        "native":crate::native::configuration(manifest), "dependencies":dependencies});
    if version == 1 {
        let target = require_single_target(manifest)?;
        value["kind"] = json!(kind_name(target.kind));
        value["entry_path"] = json!(json_path(&root.join(&target.path))?);
    } else {
        value["targets"] = Value::Array(
            manifest
                .targets
                .iter()
                .map(|target| {
                    Ok(json!({"name":target.name, "kind":kind_name(target.kind),
                "entry_path":json_path(&root.join(&target.path))?,
                "output":target.output.map(|output| output.as_str()), "exports":target.exports,
                "platform":platform_record(&target.platform)?}))
                })
                .collect::<Result<_, Error>>()?,
        );
        value["default_run"] = json!(manifest.default_run);
        value["profiles"] = json!(manifest
            .profiles
            .iter()
            .map(|(name, profile)| {
                (
                    name.clone(),
                    json!({"opt_level":profile.opt_level,"inherits":profile.inherits}),
                )
            })
            .collect::<serde_json::Map<String, Value>>());
        value["build_target"] = json!(manifest.build_target);
        value["target_config"] = Value::Array(manifest.target_config.iter().map(|config| {
            Ok(json!({"triple":config.triple,
                "sysroot":config.sysroot.as_deref().map(json_path).transpose()?, "linker":config.linker,
                "bins":config.bins.iter().map(|bin| Ok(json!({"name":bin.name,"platform":platform_record(&bin.platform)?})))
                    .collect::<Result<Vec<_>, Error>>()?}))
        }).collect::<Result<_, Error>>()?);
    }
    Ok(value)
}

fn platform_record(platform: &PlatformSettings) -> Result<Value, Error> {
    Ok(
        json!({"mode":platform.mode.map(|mode| mode.as_str()), "entry":platform.entry,
        "startup":platform.startup.map(|startup| startup.as_str()),
        "linker_script":platform.linker_script.as_deref().map(json_path).transpose()?}),
    )
}

fn json_path(path: &Path) -> Result<&str, Error> {
    path.to_str()
        .ok_or_else(|| Error::resolution("metadata JSON cannot represent a non-UTF-8 path"))
}

fn kind_name(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Library => "library",
        TargetKind::Binary => "binary",
    }
}
