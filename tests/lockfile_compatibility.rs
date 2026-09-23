// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const V1: &str = include_str!("fixtures/lockfile/v1.ws");
const V2: &str = include_str!("fixtures/lockfile/v2-path.ws");
const FUTURE: &str = include_str!("fixtures/lockfile/future.ws");
static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "vex-lock-compat-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("app")).unwrap();
        fs::create_dir_all(root.join("dep/src")).unwrap();
        fs::write(root.join("dep/src/lib.wave"), "pub fun dep() {}\n").unwrap();
        fs::write(
            root.join("dep/vex.ws"),
            "{ name = \"dep\", version = 0.1.0, lib = true }",
        )
        .unwrap();
        fs::write(
            root.join("app/vex.ws"),
            "{ name = \"app\", dependencies = [{ name = \"dep\", path = \"../dep\" }] }",
        )
        .unwrap();
        Self(root)
    }
    fn run(&self, input: &str, flags: &[&str]) -> std::process::Output {
        fs::write(self.0.join("app/vex.lock"), input).unwrap();
        Command::new(env!("CARGO_BIN_EXE_vex"))
            .arg("fetch")
            .args(flags)
            .current_dir(self.0.join("app"))
            .output()
            .unwrap()
    }
    fn lock(&self) -> String {
        fs::read_to_string(self.0.join("app/vex.lock")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn historical_lockfiles_have_explicit_migration_and_reuse_behavior() {
    let fixture = Fixture::new();
    let mut migrated = None;
    for flags in [
        &[][..],
        &["--offline"],
        &["--locked"],
        &["--locked", "--offline"],
    ] {
        let output = fixture.run(V1, flags);
        if flags.contains(&"--locked") {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("version 1 cannot be used"));
            assert_eq!(fixture.lock(), V1);
        } else {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if let Some(previous) = &migrated {
                assert_eq!(&fixture.lock(), previous);
            }
            migrated = Some(fixture.lock());
        }
        let output = fixture.run(V2, flags);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fixture.lock(), V2);
        let output = fixture.run(FUTURE, flags);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("unsupported `vex.lock` version `999`"));
        assert_eq!(fixture.lock(), FUTURE);
    }
}

#[test]
fn invalid_v2_locks_fail_before_mutating_dependency_state() {
    let graph = include_str!("fixtures/lockfile/v2-git-graph.ws");
    let cases = [
        (
            V2.replacen("version = 2,", "version = 2, typo = true,", 1),
            "field `typo`",
        ),
        (
            V2.replace("source = \"path\",", "source = \"path\", git = \"wrong\","),
            "field `git`",
        ),
        (
            graph.replace("source = \"git\",", "source = \"git\", path = \"wrong\","),
            "field `path`",
        ),
        (
            graph.replace("branch = \"main\",", "branch = \"main\", rev = \"HEAD\","),
            "at most one",
        ),
        (
            V2.replace("dependencies = []", "dependencies = [\"missing\"]"),
            "dep -> missing",
        ),
        (
            graph.replace(
                "dependencies = [\"beta\"]",
                "dependencies = [\"beta\", \"beta\"]",
            ),
            "duplicate edge: alpha -> beta",
        ),
        (
            V2.replace("dependencies = []", "dependencies = [\"dep\"]"),
            "self-dependency: dep -> dep",
        ),
        (
            graph.replace("dependencies = []", "dependencies = [\"alpha\"]"),
            "cycle:",
        ),
    ];
    for (input, expected) in cases {
        for flags in [
            &[][..],
            &["--offline"],
            &["--locked"],
            &["--locked", "--offline"],
        ] {
            let fixture = Fixture::new();
            let output = fixture.run(&input, flags);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success(), "{input}");
            assert!(stderr.contains(expected), "{stderr}");
            assert!(stderr.contains("help:"), "{stderr}");
            assert_eq!(fixture.lock(), input);
            assert!(!fixture.0.join("app/.vex").exists());
            assert_eq!(fs::read_dir(fixture.0.join("app")).unwrap().count(), 2);
        }
    }
}
