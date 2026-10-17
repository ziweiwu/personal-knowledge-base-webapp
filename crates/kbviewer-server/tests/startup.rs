//! Tests that run the real binary, for the behaviour that only exists in `main`.

use kbviewer_core::config::Config;
use kbviewer_server::auth::store::AuthStore;
use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Far longer than a failed bind or a one-note index takes, so only a stuck server hits it.
const DEADLINE: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// What `main` logs once per root it has indexed, and once it is serving.
const INDEXED_LINE: &str = "indexed root";
const LISTENING_LINE: &str = "kbviewer listening on http://";
/// Asks the kernel for any free port, so the control run cannot collide with anything.
const ANY_FREE_PORT: u16 = 0;
/// What starts every ANSI colour sequence the terminal formatter writes.
const COLOUR_CODE: char = '\u{1b}';

/// The launch agent and KBViewer.app both want 4321. Whichever process loses must find out
/// before it indexes anything: launchd restarts the agent every ten seconds, and a loser
/// that indexed the whole vault first did so a quarter of a million times in one month.
/// Claiming the port first is also what lets the app's probe see an agent still indexing.
#[test]
fn a_port_already_taken_is_refused_before_the_vault_is_indexed() {
    let base = tempfile::tempdir().unwrap();
    let holder = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken_port = holder.local_addr().unwrap().port();
    let log_path = base.path().join("server.log");

    let mut server = RunningServer::spawn(&write_config(base.path(), taken_port), &log_path);
    let status = server.wait_for_exit();
    let output = std::fs::read_to_string(&log_path).unwrap();

    assert!(
        !status.success(),
        "the server started on a taken port:\n{output}"
    );
    assert!(
        output.contains("could not bind"),
        "unexpected failure:\n{output}"
    );
    assert!(
        !output.contains(INDEXED_LINE),
        "the vault was indexed before the bind was attempted:\n{output}"
    );
}

/// The control for the test above: on a free port the same config is indexed and logged,
/// so the absence of the line there means something rather than a renamed message.
#[test]
fn a_free_port_is_indexed_and_announced() {
    let base = tempfile::tempdir().unwrap();
    let log_path = base.path().join("server.log");

    let mut server = RunningServer::spawn(&write_config(base.path(), ANY_FREE_PORT), &log_path);
    let output = server.wait_for_output(&log_path, LISTENING_LINE);

    assert!(
        output.contains(INDEXED_LINE),
        "the index was not logged:\n{output}"
    );
}

/// KBViewer.app passes `--log-file` and still reads stderr, for the line that says the
/// server is ready; the log itself must go to the file, without a terminal's colour codes.
#[test]
fn a_log_file_takes_the_log_while_stderr_keeps_the_ready_line() {
    let base = tempfile::tempdir().unwrap();
    let stderr_path = base.path().join("stderr.log");
    let log_file = base.path().join("kbviewer.log");
    let config = write_config(base.path(), ANY_FREE_PORT);

    let mut server = RunningServer::spawn_logging_to(&config, &stderr_path, &log_file);
    let stderr = server.wait_for_output(&stderr_path, LISTENING_LINE);
    let logged = std::fs::read_to_string(&log_file).unwrap();

    assert!(
        logged.contains(INDEXED_LINE),
        "the index was not logged:\n{logged}"
    );
    assert!(
        !logged.contains(COLOUR_CODE),
        "colour codes in the log file:\n{logged}"
    );
    assert!(
        !stderr.contains(INDEXED_LINE),
        "the log still went to stderr:\n{stderr}"
    );
}

/// Under launchd stderr goes nowhere, so the file is the only place a failed start can say
/// why — and a port already taken is exactly how the agent fails beside the app.
#[test]
fn a_log_file_records_why_the_server_stopped() {
    let base = tempfile::tempdir().unwrap();
    let holder = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken_port = holder.local_addr().unwrap().port();
    let log_file = base.path().join("kbviewer.log");
    let config = write_config(base.path(), taken_port);

    let stderr_path = base.path().join("stderr.log");
    let mut server = RunningServer::spawn_logging_to(&config, &stderr_path, &log_file);
    server.wait_for_exit();
    let logged = std::fs::read_to_string(&log_file).unwrap();

    assert!(
        logged.contains("could not bind"),
        "the reason is missing:\n{logged}"
    );
}

fn write_config(base: &Path, port: u16) -> PathBuf {
    let root = base.join("vault");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("index.md"), "# Index\n").unwrap();

    let config_path = base.join("kbviewer.config.json");
    let config = serde_json::json!({
        "host": "127.0.0.1",
        "port": port,
        "dataDir": base.join("data"),
        "roots": [{ "id": "kb", "name": "KB", "path": root }],
    });
    std::fs::write(&config_path, config.to_string()).unwrap();

    let loaded = Config::load(&config_path).unwrap();
    let store = AuthStore::open(&loaded.data_dir).unwrap();
    store
        .add_user("tester@example.com", "startup-test-password")
        .unwrap();
    config_path
}

/// The binary under test, killed and reaped when dropped so a failed assertion cannot
/// leave a server holding a port behind it.
struct RunningServer {
    child: Child,
}

impl RunningServer {
    fn spawn(config_path: &Path, log_path: &Path) -> Self {
        Self::start(Self::command(config_path), log_path)
    }

    /// As `spawn`, with `--log-file` set, so `stderr_path` receives only what the server
    /// writes outside its log.
    fn spawn_logging_to(config_path: &Path, stderr_path: &Path, log_file: &Path) -> Self {
        let mut command = Self::command(config_path);
        command.arg("--log-file").arg(log_file);
        Self::start(command, stderr_path)
    }

    fn command(config_path: &Path) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_kbviewer"));
        command
            .env("KBVIEWER_CONFIG", config_path)
            .env("RUST_LOG", "kbviewer=info")
            .stdin(Stdio::null());
        command
    }

    fn start(mut command: Command, output_path: &Path) -> Self {
        let output = File::create(output_path).unwrap();
        let child = command
            .stdout(output.try_clone().unwrap())
            .stderr(output)
            .spawn()
            .unwrap();
        Self { child }
    }

    fn wait_for_exit(&mut self) -> ExitStatus {
        let started = Instant::now();
        while started.elapsed() < DEADLINE {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        panic!("the server was still running after {DEADLINE:?}");
    }

    fn wait_for_output(&mut self, log_path: &Path, expected: &str) -> String {
        let started = Instant::now();
        while started.elapsed() < DEADLINE {
            let output = std::fs::read_to_string(log_path).unwrap_or_default();
            if output.contains(expected) {
                return output;
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!("the server exited with {status} before logging {expected:?}:\n{output}");
            }
            std::thread::sleep(POLL_INTERVAL);
        }
        panic!("the server did not log {expected:?} within {DEADLINE:?}");
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
