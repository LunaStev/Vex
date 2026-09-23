use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

mod support;
use support::git_url;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vex git lock test-{}-{id}#fixture",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("test directory must be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn git_lock_keeps_transitive_graph_until_explicit_update() {
    let fixture = TestDir::new();
    let leaf = fixture.path().join("leaf");
    let middle = fixture.path().join("middle");
    let app = fixture.path().join("app");

    create_package(&leaf, "leaf", &[]);
    init_git(&leaf);
    let leaf_initial = commit_all(&leaf, "initial leaf");

    create_package(
        &middle,
        "middle",
        &[("leaf", git_url(&leaf), Some("master"))],
    );
    init_git(&middle);
    let middle_commit = commit_all(&middle, "initial middle");

    create_package(&app, "app", &[("middle", git_url(&middle), Some("master"))]);

    let missing_lock = vex(&app, &["fetch", "--locked"]);
    assert_failure(&missing_lock, "locked fetch without a lockfile");
    let missing_lock_stderr = String::from_utf8_lossy(&missing_lock.stderr);
    assert!(missing_lock_stderr.contains("required by `--locked`"));
    assert!(!app.join(".vex/deps/middle").exists());

    let first_fetch = vex(&app, &["fetch"]);
    assert_success(&first_fetch, "initial vex fetch");
    assert!(app.join(".vex/deps/middle").is_dir());
    assert!(app.join(".vex/deps/leaf").is_dir());
    let first_stderr = String::from_utf8_lossy(&first_fetch.stderr);
    assert!(first_stderr.contains("Resolving"), "{first_stderr}");
    assert!(first_stderr.contains("Fetching"), "{first_stderr}");
    assert!(first_stderr.contains("Locking"), "{first_stderr}");

    let first_lock = read_lock(&app);
    assert!(first_lock.contains(&format!("commit = \"{leaf_initial}\"")));
    assert!(first_lock.contains(&format!("commit = \"{middle_commit}\"")));
    assert!(first_lock.contains("dependencies = [\"leaf\"]"));

    fs::write(leaf.join("REVISION.txt"), "new leaf revision\n")
        .expect("leaf update must be written");
    let leaf_updated = commit_all(&leaf, "update leaf");
    assert_ne!(leaf_initial, leaf_updated);

    let locked_fetch = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_success(&locked_fetch, "locked offline vex fetch");
    let locked_stderr = String::from_utf8_lossy(&locked_fetch.stderr);
    assert!(locked_stderr.contains("Resolving"), "{locked_stderr}");
    assert!(
        !locked_stderr.contains("Fetching"),
        "locked fetch unexpectedly contacted Git: {locked_stderr}"
    );
    assert_eq!(read_lock(&app), first_lock);
    assert_eq!(
        git_stdout(&app.join(".vex/deps/leaf"), &["rev-parse", "HEAD"]),
        leaf_initial
    );

    fs::remove_dir_all(app.join(".vex/deps/leaf"))
        .expect("managed leaf checkout must be removed for the offline test");
    let missing_offline = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_failure(&missing_offline, "offline fetch with a missing checkout");
    let missing_offline_stderr = String::from_utf8_lossy(&missing_offline.stderr);
    assert!(missing_offline_stderr.contains("not available locally in offline mode"));
    assert!(missing_offline_stderr.contains("run `vex fetch` while online"));
    assert_eq!(read_lock(&app), first_lock);

    let restored = vex(&app, &["fetch", "--locked"]);
    assert_success(&restored, "online locked fetch restoring a checkout");
    assert_eq!(read_lock(&app), first_lock);
    assert_eq!(
        git_stdout(&app.join(".vex/deps/leaf"), &["rev-parse", "HEAD"]),
        leaf_initial
    );

    let update = vex(&app, &["update"]);
    assert_success(&update, "vex update");
    let update_stderr = String::from_utf8_lossy(&update.stderr);
    assert!(update_stderr.contains("Fetching"), "{update_stderr}");
    assert!(update_stderr.contains("Locking"), "{update_stderr}");

    let updated_lock = read_lock(&app);
    assert!(updated_lock.contains(&format!("commit = \"{leaf_updated}\"")));
    assert!(!updated_lock.contains(&format!("commit = \"{leaf_initial}\"")));
    assert!(updated_lock.contains("dependencies = [\"leaf\"]"));
    assert_eq!(
        git_stdout(&app.join(".vex/deps/leaf"), &["rev-parse", "HEAD"]),
        leaf_updated
    );

    create_package(&app, "app", &[("middle", git_url(&middle), Some("other"))]);
    let mismatched = vex(&app, &["fetch", "--locked"]);
    assert_failure(&mismatched, "locked fetch with a changed manifest");
    let mismatched_stderr = String::from_utf8_lossy(&mismatched.stderr);
    assert!(mismatched_stderr.contains("does not match Git dependency `middle`"));
    assert_eq!(read_lock(&app), updated_lock);
}

#[test]
fn dirty_managed_checkouts_are_rejected_without_discarding_changes() {
    let fixture = TestDir::new();
    let dependency = fixture.path().join("dep");
    let app = fixture.path().join("app");
    create_package(&dependency, "dep", &[]);
    init_git(&dependency);
    let commit = commit_all(&dependency, "initial dependency");
    create_package(
        &app,
        "app",
        &[("dep", git_url(&dependency), Some("master"))],
    );

    assert_success(&vex(&app, &["fetch"]), "initial dependency fetch");
    let locked = read_lock(&app);
    let checkout = app.join(".vex/deps/dep");
    let source = checkout.join("src/lib.wave");
    let untracked = checkout.join("UNTRACKED.wave");
    let original = fs::read_to_string(&source).unwrap();

    for (changed_path, content) in [
        (source.as_path(), "pub fun tampered() {}\n"),
        (untracked.as_path(), "untracked source\n"),
    ] {
        fs::write(changed_path, content).unwrap();
        for args in [
            &["fetch"][..],
            &["fetch", "--locked", "--offline"][..],
            &["check", "--dry-run", "--locked", "--offline"][..],
            &["check", "--locked", "--offline"][..],
            &["tree", "--locked", "--offline"][..],
        ] {
            let output = vex(&app, args);
            assert_failure(&output, &format!("reject dirty checkout for {args:?}"));
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains("managed Git dependency `dep`"), "{stderr}");
            assert_reported_dirty_checkout_path(&stderr, "dep", &checkout);
            assert!(stderr.contains("preserve those changes"), "{stderr}");
            assert_eq!(fs::read_to_string(changed_path).unwrap(), content);
            assert_eq!(read_lock(&app), locked);
            assert_eq!(git_stdout(&checkout, &["rev-parse", "HEAD"]), commit);
        }
        if changed_path == source {
            fs::write(&source, &original).unwrap();
        } else {
            fs::remove_file(changed_path).unwrap();
        }
    }

    assert_success(
        &vex(&app, &["fetch", "--locked", "--offline"]),
        "locked dependency after changes are restored",
    );
}

#[test]
fn locked_offline_resolution_rejects_a_transitive_root_name_conflict() {
    let fixture = TestDir::new();
    let conflicting = fixture.path().join("conflicting");
    let middle = fixture.path().join("middle");
    let app = fixture.path().join("app");

    create_package(&conflicting, "app", &[]);
    init_git(&conflicting);
    let conflicting_commit = commit_all(&conflicting, "initial conflicting package");

    create_package(
        &middle,
        "middle",
        &[("app", git_url(&conflicting), Some("master"))],
    );
    init_git(&middle);
    let middle_commit = commit_all(&middle, "initial middle package");

    create_package(
        &app,
        "root",
        &[("middle", git_url(&middle), Some("master"))],
    );
    assert_success(&vex(&app, &["fetch"]), "initial dependency fetch");
    let locked = read_lock(&app);
    assert!(locked.contains(&format!("commit = \"{conflicting_commit}\"")));
    assert!(locked.contains(&format!("commit = \"{middle_commit}\"")));

    let conflicting_checkout = app.join(".vex/deps/app");
    fs::remove_dir_all(&conflicting_checkout).expect("conflicting checkout must be removed");
    create_package(&app, "app", &[("middle", git_url(&middle), Some("master"))]);

    let output = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_failure(&output, "locked offline root-name conflict");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("reuses root package name `app`"),
        "{stderr}"
    );
    let root_manifest = fs::canonicalize(app.join("vex.ws")).unwrap();
    assert!(
        stderr.contains(&root_manifest.display().to_string()),
        "{stderr}"
    );
    assert!(stderr.contains(&git_url(&conflicting)), "{stderr}");
    assert!(!stderr.contains("Fetching"), "{stderr}");
    assert_eq!(read_lock(&app), locked);
    assert!(!conflicting_checkout.exists());
    assert_eq!(
        git_stdout(&app.join(".vex/deps/middle"), &["rev-parse", "HEAD"]),
        middle_commit
    );
}

#[test]
fn git_lock_accepts_uppercase_and_mixed_case_commit_ids_without_recheckout() {
    let fixture = TestDir::new();
    let leaf = fixture.path().join("leaf");
    let app = fixture.path().join("app");

    create_package(&leaf, "leaf", &[]);
    init_git(&leaf);
    let leaf_commit = commit_all(&leaf, "initial leaf");

    create_package(&app, "app", &[("leaf", git_url(&leaf), Some("master"))]);

    let first_fetch = vex(&app, &["fetch"]);
    assert_success(&first_fetch, "initial vex fetch");

    let initial_lock = read_lock(&app);
    assert!(initial_lock.contains(&format!("commit = \"{leaf_commit}\"")));

    // Uppercase commit ID in vex.lock
    let uppercase_commit = leaf_commit.to_ascii_uppercase();
    assert_ne!(leaf_commit, uppercase_commit);
    let uppercase_lock = initial_lock.replace(&leaf_commit, &uppercase_commit);
    fs::write(app.join("vex.lock"), &uppercase_lock).expect("uppercase lock must be written");

    // Locked offline fetch must succeed without re-fetching or rewriting the lockfile
    let locked_fetch = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_success(
        &locked_fetch,
        "locked offline fetch with uppercase commit ID",
    );
    let locked_stderr = String::from_utf8_lossy(&locked_fetch.stderr);
    assert!(
        !locked_stderr.contains("Fetching"),
        "locked fetch unexpectedly contacted Git: {locked_stderr}"
    );
    assert_eq!(read_lock(&app), uppercase_lock);
    assert_eq!(
        git_stdout(&app.join(".vex/deps/leaf"), &["rev-parse", "HEAD"]),
        leaf_commit
    );

    // Locked offline tree command must also succeed without modifying the lockfile
    let tree_output = vex(&app, &["tree", "--locked", "--offline"]);
    assert_success(&tree_output, "locked offline tree with uppercase commit ID");
    assert_eq!(read_lock(&app), uppercase_lock);

    // Mixed-case commit ID in vex.lock
    let mixed_case_commit: String = leaf_commit
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    let mixed_lock = initial_lock.replace(&leaf_commit, &mixed_case_commit);
    fs::write(app.join("vex.lock"), &mixed_lock).expect("mixed-case lock must be written");

    let mixed_locked_fetch = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_success(
        &mixed_locked_fetch,
        "locked offline fetch with mixed-case commit ID",
    );
    assert_eq!(read_lock(&app), mixed_lock);
    assert_eq!(
        git_stdout(&app.join(".vex/deps/leaf"), &["rev-parse", "HEAD"]),
        leaf_commit
    );

    // Genuinely invalid commit ID fails
    let invalid_lock =
        initial_lock.replace(&leaf_commit, "0123456789invalidcommithexbad012345678901");
    fs::write(app.join("vex.lock"), &invalid_lock).expect("invalid lock must be written");
    let invalid_fetch = vex(&app, &["fetch", "--locked", "--offline"]);
    assert_failure(&invalid_fetch, "locked fetch with invalid commit hash");
}

#[test]
fn managed_checkouts_detach_even_when_head_already_matches() {
    let fixture = TestDir::new();
    let dep = fixture.path().join("dep");
    let app = fixture.path().join("app");
    create_package(&dep, "dep", &[]);
    init_git(&dep);
    let commit = commit_all(&dep, "initial");
    create_package(&app, "app", &[("dep", git_url(&dep), None)]);
    assert_success(&vex(&app, &["fetch"]), "initial fetch");
    let checkout = app.join(".vex/deps/dep");
    assert_eq!(
        git_stdout(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "HEAD"
    );
    let locked = read_lock(&app);
    for args in [
        &["fetch"][..],
        &["fetch", "--locked", "--offline"],
        &["update", "dep"],
    ] {
        git_stdout(&checkout, &["checkout", "master"]);
        assert_success(&vex(&app, args), "detach checkout");
        assert_eq!(
            git_stdout(&checkout, &["rev-parse", "--abbrev-ref", "HEAD"]),
            "HEAD"
        );
        assert_eq!(git_stdout(&checkout, &["rev-parse", "HEAD"]), commit);
        assert_eq!(read_lock(&app), locked);
    }
}

#[test]
fn selector_free_updates_follow_changed_remote_default_branches() {
    for update in [&["update"][..], &["update", "dep"]] {
        let fixture = TestDir::new();
        let dep = fixture.path().join("dep");
        let app = fixture.path().join("app");
        create_package(&dep, "dep", &[]);
        init_git(&dep);
        let old = commit_all(&dep, "initial");
        create_package(&app, "app", &[("dep", git_url(&dep), None)]);
        assert_success(&vex(&app, &["fetch"]), "initial default branch fetch");
        let locked = read_lock(&app);
        git_stdout(&dep, &["checkout", "-b", "next"]);
        fs::write(dep.join("revision"), "next").unwrap();
        let new = commit_all(&dep, "new default branch");
        for args in [&["fetch"][..], &["fetch", "--locked", "--offline"]] {
            assert_success(&vex(&app, args), "reuse old default branch lock");
            assert_eq!(read_lock(&app), locked);
            assert_eq!(
                git_stdout(&app.join(".vex/deps/dep"), &["rev-parse", "HEAD"]),
                old
            );
        }
        assert_success(&vex(&app, update), "refresh default branch");
        assert!(read_lock(&app).contains(&new));
        assert_eq!(
            git_stdout(
                &app.join(".vex/deps/dep"),
                &["symbolic-ref", "refs/remotes/origin/HEAD"]
            ),
            "refs/remotes/origin/next"
        );
    }
}

#[test]
fn tag_and_exact_revision_selectors_remain_pinned_on_update() {
    let fixture = TestDir::new();
    let dep = fixture.path().join("dep");
    create_package(&dep, "dep", &[]);
    init_git(&dep);
    let pinned = commit_all(&dep, "tagged revision");
    git_stdout(&dep, &["tag", "v1"]);
    git_stdout(
        &dep,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "tag",
            "-a",
            "v1-annotated",
            "-m",
            "annotated",
        ],
    );
    for (i, selector) in [
        "tag = \"v1\"".to_string(),
        "tag = \"v1-annotated\"".to_string(),
        format!("rev = \"{pinned}\""),
    ]
    .iter()
    .enumerate()
    {
        let app = fixture.path().join(format!("app_{i}"));
        create_package(&app, "app", &[("dep", git_url(&dep), Some("master"))]);
        let manifest = fs::read_to_string(app.join("vex.ws"))
            .unwrap()
            .replace("branch = \"master\"", selector);
        fs::write(app.join("vex.ws"), manifest).unwrap();
        assert_success(&vex(&app, &["fetch"]), "fetch explicit selector");
        let locked = read_lock(&app);
        assert!(locked.contains(&pinned));
        fs::write(dep.join("revision"), format!("revision {i}")).unwrap();
        let moved = commit_all(&dep, "advance branch");
        for args in [
            &["fetch", "--locked", "--offline"][..],
            &["update", "dep"],
            &["update"],
        ] {
            assert_success(&vex(&app, args), "reuse explicit selector");
            assert_eq!(read_lock(&app), locked);
            assert_eq!(
                git_stdout(&app.join(".vex/deps/dep"), &["rev-parse", "HEAD"]),
                pinned
            );
            assert!(!read_lock(&app).contains(&moved));
        }
    }
}

#[test]
fn dependency_git_ignores_inherited_repository_context() {
    let fixture = TestDir::new();
    let dep = fixture.path().join("dep");
    let other = fixture.path().join("unrelated");
    create_package(&dep, "dep", &[]);
    init_git(&dep);
    let dep_commit = commit_all(&dep, "dependency");
    create_package(&other, "other", &[]);
    init_git(&other);
    let other_commit = commit_all(&other, "unrelated");
    fs::write(other.join("src/lib.wave"), "user changes\n").unwrap();
    let index = fs::read(other.join(".git/index")).unwrap();
    let overrides = [
        ("GIT_DIR", other.join(".git")),
        ("GIT_WORK_TREE", other.clone()),
        ("GIT_COMMON_DIR", other.join(".git")),
        ("GIT_INDEX_FILE", other.join(".git/index")),
        ("GIT_OBJECT_DIRECTORY", other.join(".git/objects")),
        (
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            other.join(".git/objects"),
        ),
        ("GIT_NAMESPACE", PathBuf::from("unrelated")),
    ];
    // Test each variable independently and then all together. Overrides belong
    // only to child processes; no global environment races with other tests.
    for case in 0..=overrides.len() {
        let app = fixture.path().join(format!("app_{case}"));
        create_package(&app, "app", &[("dep", git_url(&dep), Some("master"))]);
        for args in [
            &["fetch"][..],
            &["update", "dep"],
            &["fetch", "--locked", "--offline"],
            &["check", "--dry-run", "--locked", "--offline"],
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_vex"));
            command.args(args).current_dir(&app).env(
                "VEX_WAVEC",
                fixture.path().join("deliberately-missing-wavec"),
            );
            for (i, (name, value)) in overrides.iter().enumerate() {
                if case == i || case == overrides.len() {
                    command.env(name, value);
                }
            }
            let output = command.output().unwrap();
            if args[0] == "check" {
                // Reaching the compiler proves read-only checkout verification
                // passed; this fixture does not depend on an installed wavec.
                assert_failure(&output, "missing fixture compiler");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr.contains("failed to execute")
                        && stderr.contains("deliberately-missing-wavec"),
                    "{stderr}"
                );
            } else {
                assert_success(&output, &format!("environment case {case}: {args:?}"));
            }
            assert_eq!(
                git_stdout(&app.join(".vex/deps/dep"), &["rev-parse", "HEAD"]),
                dep_commit
            );
            assert_eq!(git_stdout(&other, &["rev-parse", "HEAD"]), other_commit);
            assert_eq!(fs::read(other.join(".git/index")).unwrap(), index);
            assert_eq!(
                fs::read_to_string(other.join("src/lib.wave")).unwrap(),
                "user changes\n"
            );
        }
    }
}

#[test]
fn url_rewrites_preserve_declared_identity_and_reject_invalid_origins() {
    let fixture = TestDir::new();
    let dep = fixture.path().join("dep");
    let app = fixture.path().join("app");
    let home = fixture.path().join("isolated-home");
    fs::create_dir_all(&home).unwrap();
    create_package(&dep, "dep", &[]);
    init_git(&dep);
    commit_all(&dep, "initial");
    let declared = "https://vex-fixture.invalid/dep.git";
    create_package(&app, "app", &[("dep", declared.to_string(), None)]);
    let config = home.join("gitconfig");
    let rewrite = format!("url.{}.insteadOf", git_url(&dep));
    assert_success(
        &Command::new("git")
            .args(["config", "--file"])
            .arg(&config)
            .args([&rewrite, declared])
            .output()
            .unwrap(),
        "isolated URL rewrite",
    );
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vex"))
            .args(args)
            .current_dir(&app)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", &home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", &config)
            .env(
                "VEX_WAVEC",
                fixture.path().join("deliberately-missing-wavec"),
            )
            .output()
            .unwrap()
    };
    for args in [
        &["fetch"][..],
        &["fetch"],
        &["fetch", "--locked", "--offline"],
        &["update", "dep"],
    ] {
        assert_success(&run(args), "URL-rewritten dependency");
    }
    let output = run(&["check", "--dry-run", "--locked", "--offline"]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to execute"));
    let locked = read_lock(&app);
    assert!(locked.contains(declared));
    assert!(!locked.contains(&git_url(&dep)));
    let checkout = app.join(".vex/deps/dep");
    for invalid in ["changed", "multiple", "missing"] {
        git_stdout(
            &checkout,
            &["config", "--replace-all", "remote.origin.url", declared],
        );
        match invalid {
            "changed" => {
                git_stdout(&checkout, &["config", "remote.origin.url", "file:///wrong"]);
            }
            "multiple" => {
                git_stdout(
                    &checkout,
                    &["config", "--add", "remote.origin.url", declared],
                );
            }
            _ => {
                git_stdout(&checkout, &["config", "--unset-all", "remote.origin.url"]);
            }
        }
        let output = run(&["fetch", "--locked", "--offline"]);
        assert_failure(&output, invalid);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("expected exactly one origin") && stderr.contains("help:"),
            "{stderr}"
        );
        assert_eq!(read_lock(&app), locked);
    }
}

fn create_package(path: &Path, name: &str, dependencies: &[(&str, String, Option<&str>)]) {
    fs::create_dir_all(path.join("src")).expect("package source directory must be created");
    fs::write(path.join("src/lib.wave"), "pub fun package_marker() {}\n")
        .expect("library entry must be written");
    let dependency_entries = dependencies
        .iter()
        .map(|(dependency, url, branch)| match branch {
            Some(branch) => format!(
                "        {{ name = \"{dependency}\", git = \"{url}\", branch = \"{branch}\" }}"
            ),
            None => format!("        {{ name = \"{dependency}\", git = \"{url}\" }}"),
        })
        .collect::<Vec<_>>();
    let dependencies = if dependency_entries.is_empty() {
        "[]".to_string()
    } else {
        format!("[\n{}\n    ]", dependency_entries.join(",\n"))
    };
    let manifest = format!(
        "{{\n    name = \"{name}\",\n    version = 0.1.0,\n    lib = true,\n    dependencies = {dependencies}\n}}\n"
    );
    fs::write(path.join("vex.ws"), manifest).expect("manifest must be written");
}

fn init_git(path: &Path) {
    let output = Command::new("git")
        .args(["init", "-q", "-b", "master"])
        .current_dir(path)
        .output()
        .expect("git init must start");
    assert_success(&output, "git init");
}

fn commit_all(path: &Path, message: &str) -> String {
    let add = Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .expect("git add must start");
    assert_success(&add, "git add");

    let commit = Command::new("git")
        .args([
            "-c",
            "user.name=Vex Test",
            "-c",
            "user.email=vex@example.invalid",
            "commit",
            "-q",
            "-m",
            message,
        ])
        .current_dir(path)
        .output()
        .expect("git commit must start");
    assert_success(&commit, "git commit");
    git_stdout(path, &["rev-parse", "HEAD"])
}

fn git_stdout(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("git command must start");
    assert_success(&output, "git command");
    String::from_utf8(output.stdout)
        .expect("git output must be UTF-8")
        .trim()
        .to_string()
}

fn vex(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vex"))
        .args(args)
        .current_dir(path)
        .output()
        .expect("vex command must start")
}

fn read_lock(app: &Path) -> String {
    fs::read_to_string(app.join("vex.lock")).expect("vex.lock must exist")
}

fn assert_reported_dirty_checkout_path(stderr: &str, package: &str, expected: &Path) {
    let prefix = format!("managed Git dependency `{package}` at `");
    let reported = stderr
        .split_once(&prefix)
        .and_then(|(_, rest)| rest.split_once("` has local changes"))
        .map(|(path, _)| path)
        .unwrap_or_else(|| panic!("dirty-checkout path was not reported:\n{stderr}"));
    let reported = fs::canonicalize(reported).unwrap_or_else(|error| {
        panic!("failed to canonicalize reported path `{reported}`: {error}")
    });
    let expected = fs::canonicalize(expected).unwrap_or_else(|error| {
        panic!(
            "failed to canonicalize expected path `{}`: {error}",
            expected.display()
        )
    });

    assert_eq!(reported, expected, "{stderr}");
}

fn assert_success(output: &Output, action: &str) {
    assert!(
        output.status.success(),
        "{action} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_failure(output: &Output, action: &str) {
    assert!(
        !output.status.success(),
        "{action} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
