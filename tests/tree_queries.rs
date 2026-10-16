// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

mod support;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "vex-tree-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn package(&self, name: &str, dependencies: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("vex.ws"), format!("{{ name = \"{name}\", version = 0.1.0, lib = true, dependencies = [{dependencies}] }}\n")).unwrap();
        fs::write(path.join("src/lib.wave"), "pub fun marker() {}\n").unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn vex(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vex"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Vex Test")
        .env("GIT_AUTHOR_EMAIL", "vex@example.invalid")
        .env("GIT_COMMITTER_NAME", "Vex Test")
        .env("GIT_COMMITTER_EMAIL", "vex@example.invalid")
        .output()
        .unwrap();
    success(&output);
}

#[test]
fn inverse_and_depth_queries_reuse_the_locked_graph_without_the_git_remote() {
    let fixture = Fixture::new();
    let leaf = fixture.package("leaf", "");
    git(&leaf, &["init", "--initial-branch=master"]);
    git(&leaf, &["add", "."]);
    git(
        &leaf,
        &["-c", "commit.gpgsign=false", "commit", "-m", "fixture"],
    );
    fixture.package(
        "shared",
        &format!(
            "{{ name = \"leaf\", git = \"{}\" }}",
            support::git_url(&leaf)
        ),
    );
    fixture.package("alpha", "{ name = \"shared\", path = \"../shared\" }");
    fixture.package("beta", "{ name = \"shared\", path = \"../shared\" }");
    let app = fixture.package(
        "app",
        "{ name = \"beta\", path = \"../beta\" }, { name = \"alpha\", path = \"../alpha\" }",
    );
    success(&vex(&app, &["fetch"]));
    let lock_path = app.join("vex.lock");
    let lock = fs::read(&lock_path).unwrap();
    let modified = fs::metadata(&lock_path).unwrap().modified().unwrap();
    fs::rename(&leaf, fixture.0.join("hidden-remote")).unwrap();

    let inverse = vex(
        &app,
        &["tree", "--invert", "shared", "--locked", "--offline"],
    );
    success(&inverse);
    assert_eq!(
        String::from_utf8_lossy(&inverse.stdout),
        concat!(
            "shared v0.1.0 (path ../shared)\n",
            "├── alpha v0.1.0 (path ../alpha)\n",
            "│   └── app v0.1.0\n",
            "└── beta v0.1.0 (path ../beta)\n",
            "    └── app v0.1.0\n",
        )
    );
    let verbose = vex(
        &app,
        &["-vv", "tree", "--invert", "shared", "--locked", "--offline"],
    );
    success(&verbose);
    assert_eq!(verbose.stdout, inverse.stdout);
    let diagnostics = String::from_utf8_lossy(&verbose.stderr);
    for decision in [
        "leaf: reuse locked local checkout (offline); no Git transport",
        "leaf: selected Git commit",
        "network: disabled by offline mode",
    ] {
        assert!(
            diagnostics.contains(decision),
            "missing {decision}: {diagnostics}"
        );
    }
    let limited = vex(&app, &["tree", "--depth", "1", "--offline", "--locked"]);
    success(&limited);
    assert_eq!(
        String::from_utf8_lossy(&limited.stdout),
        concat!(
            "app v0.1.0\n",
            "├── alpha v0.1.0 (path ../alpha)\n",
            "└── beta v0.1.0 (path ../beta)\n",
        )
    );
    let inverse_limited = vex(
        &app,
        &[
            "tree",
            "--invert",
            "leaf",
            "--depth",
            "1",
            "--locked",
            "--offline",
        ],
    );
    success(&inverse_limited);
    let output = String::from_utf8_lossy(&inverse_limited.stdout);
    assert!(output.starts_with("leaf v0.1.0 (git file:"), "{output}");
    assert_eq!(output.lines().count(), 2);
    assert!(
        output.ends_with("└── shared v0.1.0 (path ../shared)\n"),
        "{output}"
    );
    let unknown = vex(&app, &["tree", "--invert", "typo", "--locked", "--offline"]);
    assert_eq!(unknown.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("package `typo` is not present"));
    assert!(unknown.stdout.is_empty());
    assert_eq!(fs::read(&lock_path).unwrap(), lock);
    assert_eq!(
        fs::metadata(&lock_path).unwrap().modified().unwrap(),
        modified
    );
}

#[test]
fn invalid_query_options_do_not_start_resolution() {
    let fixture = Fixture::new();
    let app = fixture.package("app", "");
    for args in [
        vec!["tree", "--depth"],
        vec!["tree", "--depth", "-1"],
        vec!["tree", "--depth", "0", "--depth", "1"],
        vec!["tree", "--invert"],
        vec!["tree", "--invert", "app", "--invert", "app"],
    ] {
        let output = vex(&app, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(!app.join("vex.lock").exists());
        assert!(!app.join(".vex").exists());
    }
}

#[test]
fn locked_queries_require_a_lockfile_even_at_zero_depth() {
    let fixture = Fixture::new();
    let app = fixture.package("app", "");
    let output = vex(
        &app,
        &["tree", "--invert", "app", "--depth", "0", "--locked"],
    );
    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("required by `--locked`"));
    assert!(output.stdout.is_empty());
    assert!(!app.join("vex.lock").exists());
}
