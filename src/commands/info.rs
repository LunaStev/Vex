use diagnostic::Error;
use manifest::{DependencySource, Manifest};

pub fn info(args: &[String]) -> Result<(), Error> {
    if matches!(args, [help] if help == "-h" || help == "--help") {
        println!("usage: vex info");
        return Ok(());
    }
    if let Some(argument) = args.first() {
        return Err(Error::usage(format!(
            "unexpected argument `{argument}`\nusage: vex info"
        )));
    }
    run_info()
}

fn run_info() -> Result<(), Error> {
    let manifest = Manifest::load()?;
    println!("Vex project info");
    println!("name: {}", manifest.name);
    println!("version: {}", manifest.version);
    println!("type: {}", if manifest.lib { "library" } else { "binary" });
    println!("manifest: {}", manifest.source_path.to_string_lossy());
    if let Some(description) = manifest.description.as_ref() {
        println!("description: {description}");
    }
    if let Some(author) = manifest.author.as_ref() {
        println!("author: {author}");
    }
    if let Some(license) = manifest.license.as_ref() {
        println!("license: {license}");
    }
    println!("dependencies: {}", manifest.dependencies.len());

    for dep in manifest.dependencies {
        match dep.source {
            DependencySource::Path { path } => match dep.version {
                Some(version) => println!("  {} {} path {}", dep.name, version, path),
                None => println!("  {} path {}", dep.name, path),
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
                        println!(
                            "  {} {} git {}{}",
                            dep.name,
                            version,
                            source::identity(&url),
                            reference
                        )
                    }
                    None => println!("  {} git {}{}", dep.name, source::identity(&url), reference),
                }
            }
        }
    }
    Ok(())
}
