use diagnostic::Error;
use manifest::{DependencySource, Manifest, TargetKind};

pub fn info(args: &[String], selection: &crate::project::Selection) -> Result<(), Error> {
    if matches!(args, [help] if help == "-h" || help == "--help") {
        diagnostic::outln!("usage: vex info");
        return Ok(());
    }
    if let Some(argument) = args.first() {
        return Err(Error::usage(format!(
            "unexpected argument `{argument}`\nusage: vex info"
        )));
    }
    let project = selection.enter()?;
    run_info(&project)
}

fn run_info(project: &crate::project::Project) -> Result<(), Error> {
    let manifest = Manifest::load()?;
    diagnostic::outln!("Vex project info");
    diagnostic::outln!("name: {}", manifest.name);
    diagnostic::outln!("version: {}", manifest.version);
    if manifest.targets.len() == 1 {
        diagnostic::outln!("type: {}", kind_name(manifest.targets[0].kind));
    } else {
        diagnostic::outln!("type: multiple targets");
        for target in &manifest.targets {
            diagnostic::outln!(
                "  {} {} {}",
                kind_name(target.kind),
                target.name,
                target.path.display()
            );
        }
    }
    if let Some(default_run) = &manifest.default_run {
        diagnostic::outln!("default_run: {default_run}");
    }
    diagnostic::outln!("manifest: {}", project.manifest_path.display());
    if let Some(description) = manifest.description.as_ref() {
        diagnostic::outln!("description: {description}");
    }
    if let Some(author) = manifest.author.as_ref() {
        diagnostic::outln!("author: {author}");
    }
    if let Some(license) = manifest.license.as_ref() {
        diagnostic::outln!("license: {license}");
    }
    diagnostic::outln!("dependencies: {}", manifest.dependencies.len());

    for dep in manifest.dependencies {
        match dep.source {
            DependencySource::Path { path } => match dep.version {
                Some(version) => diagnostic::outln!("  {} {} path {}", dep.name, version, path),
                None => diagnostic::outln!("  {} path {}", dep.name, path),
            },
            DependencySource::Git {
                url,
                branch,
                tag,
                rev,
            } => {
                let reference = branch
                    .map(|value| format!(" branch {value}"))
                    .or_else(|| tag.map(|value| format!(" tag {value}")))
                    .or_else(|| rev.map(|value| format!(" rev {value}")))
                    .unwrap_or_default();
                match dep.version {
                    Some(version) => {
                        diagnostic::outln!(
                            "  {} {} git {}{}",
                            dep.name,
                            version,
                            source::identity(&url),
                            reference
                        )
                    }
                    None => diagnostic::outln!(
                        "  {} git {}{}",
                        dep.name,
                        source::identity(&url),
                        reference
                    ),
                }
            }
        }
    }
    Ok(())
}

fn kind_name(kind: TargetKind) -> &'static str {
    match kind {
        TargetKind::Library => "library",
        TargetKind::Binary => "binary",
    }
}
