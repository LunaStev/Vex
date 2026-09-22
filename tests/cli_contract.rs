// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("vex-cli-contract-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).expect("test directory must be created");
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn help_is_read_only_and_succeeds_without_a_manifest() {
    let fixture = TestDir::new();
    for arguments in [
        &["init", "--help"][..],
        &["build", "--help"],
        &["run", "--help"],
        &["check", "--help"],
        &["fetch", "--help"],
        &["update", "--help"],
        &["info", "--help"],
        &["tree", "--help"],
        &["setup", "--help"],
        &["setup", "wavec", "--help"],
    ] {
        let output = vex(&fixture.0, arguments);
        assert_success(&output, &format!("vex {}", arguments.join(" ")));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("usage:"),
            "help output did not contain usage: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    assert!(!fixture.0.join("vex.ws").exists());
    assert!(!fixture.0.join("src").exists());
}

#[test]
fn invalid_init_and_info_fail_without_mutating_the_directory() {
    let fixture = TestDir::new();
    let invalid_init = vex(&fixture.0, &["init", "--unknown"]);
    assert_eq!(invalid_init.status.code(), Some(2));
    assert!(!fixture.0.join("vex.ws").exists());
    assert!(!fixture.0.join("src").exists());

    let missing_info = vex(&fixture.0, &["info"]);
    assert_eq!(missing_info.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing_info.stderr).contains("could not find `vex.ws`"));

    let invalid_info = vex(&fixture.0, &["info", "extra"]);
    assert_eq!(invalid_info.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid_info.stderr).contains("unexpected argument"));

    let invalid_setup = vex(&fixture.0, &["setup", "wavec", "--version", "--unknown"]);
    assert_eq!(invalid_setup.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid_setup.stderr).contains("version value"));
}

#[test]
fn init_scaffolds_public_library_and_ignores_generated_state() {
    let fixture = TestDir::new();
    let library = fixture.0.join("library");
    let app = fixture.0.join("app");
    fs::create_dir_all(&library).unwrap();
    fs::create_dir_all(&app).unwrap();

    assert_success(&vex(&library, &["init", "--lib"]), "initialize library");
    assert_eq!(
        fs::read_to_string(library.join("src/lib.wave")).unwrap(),
        "pub fun greet() {\n    println(\"Hello from library\");\n}\n"
    );
    assert_eq!(
        fs::read_to_string(library.join(".gitignore")).unwrap(),
        "/target/\n/.vex/\n"
    );
    assert!(!library.join(".vex/deps").exists());

    assert_success(&vex(&app, &["init"]), "initialize binary");
    assert_eq!(
        fs::read_to_string(app.join("src/main.wave")).unwrap(),
        "fun main() {\n    println(\"Hello World\");\n}\n"
    );
    assert_eq!(
        fs::read_to_string(app.join(".gitignore")).unwrap(),
        "/target/\n/.vex/\n"
    );
    assert!(!app.join(".vex/deps").exists());

    fs::write(
        app.join("vex.ws"),
        "{ name = \"app\", version = 0.1.0, dependencies = [{ name = \"library\", path = \"../library\" }] }\n",
    )
    .unwrap();
    assert_success(&vex(&app, &["fetch"]), "resolve generated path library");
    assert!(!app.join(".vex/deps").exists());
    assert_success(
        &vex(&app, &["fetch", "--locked", "--offline"]),
        "reuse path dependency without managed Git state",
    );
    assert!(!app.join(".vex/deps").exists());
}

#[test]
fn init_preserves_an_existing_gitignore() {
    let fixture = TestDir::new();
    for (name, args) in [("binary", &[][..]), ("library", &["--lib"][..])] {
        let project = fixture.0.join(name);
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join(".gitignore"), "custom-rule\n").unwrap();
        assert_success(
            &vex(&project, &[&["init"][..], args].concat()),
            "initialize project",
        );
        assert_eq!(
            fs::read_to_string(project.join(".gitignore")).unwrap(),
            "custom-rule\n"
        );
    }
}

#[test]
fn manifest_optional_metadata_errors_name_the_field_and_file() {
    let fixture = TestDir::new();
    for field in ["description", "author", "license"] {
        fs::write(
            fixture.0.join("vex.ws"),
            format!("{{ name = \"app\", version = 0.1.0, {field} = 123, dependencies = [] }}\n"),
        )
        .unwrap();
        let output = vex(&fixture.0, &["info"]);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("manifest `vex.ws`"), "{stderr}");
        assert!(
            stderr.contains(&format!("manifest field `{field}` must be a string")),
            "{stderr}"
        );
    }
}

#[test]
fn missing_or_malformed_target_fails_before_project_work() {
    let fixture = TestDir::new();
    for mode in ["build", "run", "check"] {
        for args in [
            &[mode, "--target", "--release", "--dry-run"][..],
            &[mode, "--target", "--"][..],
        ] {
            let output = vex(&fixture.0, args);
            assert_eq!(output.status.code(), Some(1));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("missing value for `--target`"), "{stderr}");
            assert!(!stderr.contains("could not find `vex.ws`"), "{stderr}");
        }
        let malformed = vex(&fixture.0, &[mode, "--target=bad/target"]);
        assert_eq!(malformed.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&malformed.stderr).contains("invalid value"));
    }
    assert!(!fixture.0.join("target").exists());
    assert!(!fixture.0.join(".vex").exists());
}

#[test]
fn dependencies_must_be_library_packages_with_src_lib_wave() {
    let fixture = TestDir::new();
    let app = fixture.0.join("app");
    let dependency = fixture.0.join("add");
    fs::create_dir_all(&app).unwrap();
    fs::create_dir_all(&dependency).unwrap();
    fs::write(
        app.join("vex.ws"),
        "{ name = \"app\", version = 0.1.0, dependencies = [{ name = \"add\", path = \"../add\" }] }\n",
    )
    .unwrap();
    fs::write(
        dependency.join("vex.ws"),
        "{ name = \"add\", version = 0.1.0, lib = false, dependencies = [] }\n",
    )
    .unwrap();

    let non_library = vex(&app, &["fetch"]);
    assert_eq!(non_library.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&non_library.stderr)
        .contains("dependency `add` is not a library package"));

    fs::write(
        dependency.join("vex.ws"),
        "{ name = \"add\", version = 0.1.0, lib = true, dependencies = [] }\n",
    )
    .unwrap();
    let missing_entry = vex(&app, &["fetch"]);
    assert_eq!(missing_entry.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&missing_entry.stderr).contains("has no canonical library entry")
    );

    fs::create_dir_all(dependency.join("src")).unwrap();
    fs::write(
        dependency.join("src/lib.wave"),
        "pub fun sum(a: i32, b: i32) -> i32 { return a + b; }\n",
    )
    .unwrap();
    assert_success(&vex(&app, &["fetch"]), "fetch canonical library");
}

#[test]
fn tree_prints_locked_direct_and_transitive_dependencies() {
    let fixture = TestDir::new();
    let app = fixture.0.join("app");
    let alpha = fixture.0.join("alpha");
    let shared = fixture.0.join("shared");
    let leaf = fixture.0.join("leaf");
    for package in [&app, &alpha, &shared, &leaf] {
        fs::create_dir_all(package.join("src")).unwrap();
    }
    fs::write(
        app.join("vex.ws"),
        "{ name = \"app\", version = 0.1.0, dependencies = [{ name = \"shared\", path = \"../shared\" }, { name = \"alpha\", path = \"../alpha\" }] }\n",
    )
    .unwrap();
    fs::write(
        alpha.join("vex.ws"),
        "{ name = \"alpha\", version = 1.0.0, lib = true, dependencies = [{ name = \"shared\", path = \"../shared\" }] }\n",
    )
    .unwrap();
    fs::write(
        shared.join("vex.ws"),
        "{ name = \"shared\", version = 2.0.0, lib = true, dependencies = [{ name = \"leaf\", path = \"../leaf\" }] }\n",
    )
    .unwrap();
    fs::write(
        leaf.join("vex.ws"),
        "{ name = \"leaf\", version = 3.0.0, lib = true, dependencies = [] }\n",
    )
    .unwrap();
    for package in [&alpha, &shared, &leaf] {
        fs::write(package.join("src/lib.wave"), "pub fun marker() {}\n").unwrap();
    }

    let first = vex(&app, &["tree"]);
    assert_success(&first, "resolve dependency tree");
    let stdout = String::from_utf8_lossy(&first.stdout);
    assert!(stdout.contains("app v0.1.0"), "{stdout}");
    assert!(
        stdout.contains("├── alpha v1.0.0 (path ../alpha)"),
        "{stdout}"
    );
    assert!(
        stdout.contains("│   └── shared v2.0.0 (path ../shared)"),
        "{stdout}"
    );
    assert!(stdout.contains("leaf v3.0.0 (path ../leaf)"), "{stdout}");
    assert!(
        stdout.contains("└── shared v2.0.0 (path ../shared) (*)"),
        "{stdout}"
    );
    assert!(app.join("vex.lock").is_file());

    let locked = vex(&app, &["tree", "--locked", "--offline"]);
    assert_success(&locked, "print locked offline dependency tree");
    assert_eq!(first.stdout, locked.stdout);
}

fn vex(path: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vex"))
        .args(args)
        .current_dir(path)
        .output()
        .expect("vex command must start")
}

fn assert_success(output: &Output, action: &str) {
    assert!(
        output.status.success(),
        "{action} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
