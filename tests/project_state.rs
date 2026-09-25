use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Fixture {
    root: PathBuf,
    compiler: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "vex-state-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("vex.ws"), "{name=\"app\",version=0.1.0}").unwrap();
        fs::write(root.join("src/main.wave"), "fun main() {}\n").unwrap();
        fs::write(root.join("vex.lock"), "{version=2,package=[]}\n").unwrap();
        fs::write(root.join("fake.rs"), include_str!("fixtures/fake_wavec.rs")).unwrap();
        let compiler = root.join(if cfg!(windows) { "wavec.exe" } else { "wavec" });
        assert!(Command::new("rustc")
            .arg(root.join("fake.rs"))
            .arg("-o")
            .arg(&compiler)
            .status()
            .unwrap()
            .success());
        Self { root, compiler }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_vex"));
        cmd.args(args)
            .current_dir(&self.root)
            .env("VEX_WAVEC", &self.compiler)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Barrier(TcpListener);
impl Barrier {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        Self(listener)
    }
    fn address(&self) -> String {
        self.0.local_addr().unwrap().to_string()
    }
    fn reached(&self) -> TcpStream {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            match self.0.accept() {
                Ok((mut socket, _)) => {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(15)))
                        .unwrap();
                    socket.read_exact(&mut [0]).unwrap();
                    return socket;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "child did not reach barrier");
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => panic!("{e}"),
            }
        }
    }
}
fn finish(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "{status}");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("child did not finish");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn wait_for_lock(child: &mut Child) {
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            if line.unwrap().contains("Waiting") {
                let _ = tx.send(());
            }
        }
    });
    rx.recv_timeout(Duration::from_secs(15))
        .expect("waiter must report lock contention");
    assert!(child.try_wait().unwrap().is_none());
}

#[test]
fn compiler_source_protection_survives_parent_death() {
    let f = Fixture::new();
    let barrier = Barrier::new();
    let mut holder = f
        .command(&["build", "--locked", "--offline"])
        .env("VEX_TEST_COMPILE", barrier.address())
        .spawn()
        .unwrap();
    let mut compiler = barrier.reached();
    holder.kill().unwrap();
    holder.wait().unwrap();
    let mut writer = f
        .command(&["fetch", "--locked", "--offline"])
        .spawn()
        .unwrap();
    wait_for_lock(&mut writer);
    compiler.write_all(&[1]).unwrap();
    finish(&mut writer);
    assert_eq!(
        fs::read_to_string(f.root.join("vex.lock")).unwrap(),
        "{version=2,package=[]}\n"
    );
}

#[test]
fn running_program_releases_state_and_retains_its_generation() {
    let f = Fixture::new();
    let barrier = Barrier::new();
    let mut running = f
        .command(&[
            "run",
            "--locked",
            "--offline",
            "--",
            "--flag",
            "with spaces",
        ])
        .env("VEX_TEST_RUN", barrier.address())
        .env("VEX_TEST_NESTED", env!("CARGO_BIN_EXE_vex"))
        .spawn()
        .unwrap();
    // The application performs a nested fetch before this barrier. Reaching it
    // proves the lock was released before spawn, not merely after first output.
    let mut application = barrier.reached();
    let generation = fs::read_dir(f.root.join("target/.vex-run"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let artifact = generation.join(if cfg!(windows) {
        "program.exe"
    } else {
        "program"
    });
    let original = fs::read(&artifact).unwrap();
    let mut update = f.command(&["update"]).spawn().unwrap();
    finish(&mut update);
    let mut build = f.command(&["build"]).spawn().unwrap();
    finish(&mut build);
    assert_eq!(fs::read(&artifact).unwrap(), original);
    application.write_all(&[1]).unwrap();
    finish(&mut running);
    assert!(
        artifact.is_file(),
        "initial policy has no automatic generation GC"
    );
}

#[test]
fn readers_share_a_lock_and_block_a_writer() {
    let f = Fixture::new();
    let barrier = Barrier::new();
    let mut first = f
        .command(&["check", "--dry-run", "--locked"])
        .env("VEX_TEST_PLAN", barrier.address())
        .spawn()
        .unwrap();
    let mut one = barrier.reached();
    let mut second = f
        .command(&["check", "--dry-run", "--locked"])
        .env("VEX_TEST_PLAN", barrier.address())
        .spawn()
        .unwrap();
    let mut two = barrier.reached();
    let mut writer = f.command(&["fetch", "--locked"]).spawn().unwrap();
    wait_for_lock(&mut writer);
    one.write_all(&[1]).unwrap();
    finish(&mut first);
    assert!(writer.try_wait().unwrap().is_none());
    two.write_all(&[1]).unwrap();
    finish(&mut second);
    finish(&mut writer);
    assert!(!f.root.join("target").exists());
}

#[test]
fn rejected_dependency_preflight_does_not_create_target() {
    let f = Fixture::new();
    fs::remove_file(f.root.join("vex.lock")).unwrap();
    for mode in ["build", "check", "run"] {
        assert!(!f
            .command(&[mode, "--locked"])
            .output()
            .unwrap()
            .status
            .success());
        assert!(!f.root.join("target").exists());
    }
}
