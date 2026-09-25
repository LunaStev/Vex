use crate::{LockedPackage, LockedSource, Lockfile, LOCKFILE_VERSION};

pub fn render_lockfile(lockfile: Lockfile) -> String {
    let blocks = lockfile
        .packages
        .iter()
        .map(render_package)
        .collect::<Vec<_>>();
    if blocks.is_empty() {
        return format!("{{\n    version = {LOCKFILE_VERSION},\n    package = []\n}}\n");
    }
    format!(
        "{{\n    version = {LOCKFILE_VERSION},\n    package = [\n{}\n    ]\n}}\n",
        blocks.join(",\n")
    )
}

fn render_package(package: &LockedPackage) -> String {
    let mut fields = vec![
        render_field("name", &package.name),
        render_field("version", &package.version),
    ];
    match &package.source {
        LockedSource::Path {
            requested,
            resolved,
        } => {
            fields.push(render_field("source", "path"));
            fields.push(render_field("path", requested));
            fields.push(render_field("resolved", &resolved.to_string_lossy()));
        }
        LockedSource::Git {
            url,
            branch,
            tag,
            rev,
            commit,
            resolved,
        } => {
            fields.push(render_field("source", "git"));
            fields.push(render_field("git", url));
            if let Some(branch) = branch {
                fields.push(render_field("branch", branch));
            }
            if let Some(tag) = tag {
                fields.push(render_field("tag", tag));
            }
            if let Some(rev) = rev {
                fields.push(render_field("rev", rev));
            }
            fields.push(render_field("commit", commit));
            fields.push(render_field("resolved", &resolved.to_string_lossy()));
        }
    }
    let dependencies = package
        .dependencies
        .iter()
        .map(|name| format!("\"{}\"", escape(name)))
        .collect::<Vec<_>>()
        .join(", ");
    fields.push(format!("            dependencies = [{dependencies}]"));
    format!("        {{\n{}\n        }}", fields.join(",\n"))
}

fn render_field(key: &str, value: &str) -> String {
    format!("            {key} = \"{}\"", escape(value))
}

fn escape(value: &str) -> String {
    let quoted = wson::quote(value);
    quoted[1..quoted.len() - 1].to_owned()
}
