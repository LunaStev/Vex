// Standalone process fixture: rustc compiles this without Cargo dependencies.
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

fn checkpoint(name: &str) {
    if let Ok(address) = env::var(format!("VEX_TEST_{name}")) {
        let mut socket = std::net::TcpStream::connect(address).unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(20))).unwrap();
        socket.write_all(&[1]).unwrap();
        socket.read_exact(&mut [0]).unwrap();
    }
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if env::current_exe().unwrap().file_stem().unwrap() == "program" {
        if let Ok(vex) = env::var("VEX_TEST_NESTED") {
            assert!(std::process::Command::new(vex).args(["fetch", "--locked", "--offline"]).status().unwrap().success());
        }
        checkpoint("RUN");
        println!("FAKE_WAVEC_EXECUTED {:?}", args);
        return;
    }
    if let Ok(path) = env::var("FAKE_WAVEC_LOG") {
        let mut log = OpenOptions::new().create(true).append(true).open(path).unwrap();
        writeln!(log, "{}", args.join(" ")).unwrap();
    }
    let generation = args.iter().find_map(|a| a.strip_prefix("--target-dir=")).map(PathBuf::from);
    if args.iter().any(|a| a == "--dry-run") {
        checkpoint("PLAN");
        let mode = if args.iter().any(|a| a == "--run") { "build+run" } else { "build" };
        let schema = env::var("FAKE_SCHEMA").unwrap_or_else(|_| "1".into());
        let output = generation.map(|p| p.join(if cfg!(windows) { "program.exe" } else { "program" }));
        let (link, execute) = if let Some(output) = output {
            let output = env::var("FAKE_OUTPUT").unwrap_or_else(|_| output.to_string_lossy().into_owned());
            let link = format!("{{\"output\":{output:?},\"inputs\":[],\"program\":\"linker\",\"args\":[]}}");
            let runtime = args.iter().position(|a| a == "--").map(|i| &args[i+1..]).unwrap_or(&[]);
            let execute = format!("{{\"program\":{output:?},\"args\":{runtime:?}}}");
            (link, execute)
        } else { ("null".into(), "null".into()) };
        println!("{{\"schema_version\":{schema},\"mode\":\"{mode}\",\"target\":\"test-target\",\"emit\":\"bin\",\"emit_kinds\":[],\"control_mode\":null,\"forced_input_type\":null,\"inputs\":[],\"emit_jobs\":[],\"compile\":[],\"link\":{link},\"execute\":{execute}}}");
    } else {
        assert!(!args.iter().any(|a| a == "--run" || a == "--"));
        checkpoint("COMPILE");
        if let Some(generation) = generation {
            fs::create_dir_all(&generation).unwrap();
            fs::copy(env::current_exe().unwrap(), generation.join(if cfg!(windows) { "program.exe" } else { "program" })).unwrap();
        } else { println!("FAKE_WAVEC_EXECUTED"); }
    }
}
