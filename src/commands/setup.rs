pub fn setup(args: &[String]) {
    if matches!(args, [help] if help == "-h" || help == "--help")
        || matches!(args, [wavec, help] if wavec == "wavec" && (help == "-h" || help == "--help"))
    {
        println!("usage: vex setup wavec [--version <version>]");
        return;
    }
    if args.first().map(String::as_str) != Some("wavec") {
        eprintln!("error: usage: vex setup wavec [--version <version>]");
        std::process::exit(2);
    }

    let mut version: Option<&str> = None;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--version" => {
                if version.is_some() {
                    eprintln!("error: `--version` may only be specified once");
                    std::process::exit(2);
                }
                if index + 1 >= args.len() {
                    eprintln!("error: missing value for --version");
                    std::process::exit(2);
                }
                if args[index + 1].is_empty() || args[index + 1].starts_with('-') {
                    eprintln!("error: `--version` must be a version value, not an option");
                    std::process::exit(2);
                }
                version = Some(args[index + 1].as_str());
                index += 2;
            }
            unknown => {
                eprintln!("error: unknown setup option `{unknown}`");
                eprintln!("usage: vex setup wavec [--version <version>]");
                std::process::exit(2);
            }
        }
    }

    println!("installing wavec {}", toolchain::requested_version(version));
    if let Err(error) = toolchain::install_wavec(version) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
    println!("wavec installed successfully");
}
