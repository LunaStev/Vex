use diagnostic::Error;
use manifest::{DependencySource, Manifest};

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
    diagnostic::outln!("type: {}", if manifest.lib { "library" } else { "binary" });
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
