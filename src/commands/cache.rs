use diagnostic::Error;

pub fn cache(args: &[String]) -> Result<(), Error> {
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        diagnostic::outln!("usage: vex cache prune [--apply] [--dry-run]");
        return Ok(());
    }
    if args.first().map(String::as_str) != Some("prune") {
        return Err(Error::usage("usage: vex cache prune [--apply] [--dry-run]"));
    }
    let mut apply = false;
    let mut dry_run = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--apply" if !apply => apply = true,
            "--dry-run" if !dry_run => dry_run = true,
            _ => {
                return Err(Error::usage(format!(
                    "unexpected cache prune argument `{arg}`"
                )))
            }
        }
    }
    for path in resolver::prune_cache(apply && !dry_run)? {
        diagnostic::outln!(
            "{} {}",
            if apply && !dry_run {
                "Removed"
            } else {
                "Would remove"
            },
            path.display()
        );
    }
    Ok(())
}
