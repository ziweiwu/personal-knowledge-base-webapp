//! Tests that run the real binary, for the behaviour that only exists in `main`.

use kbviewer_core::config::Config;
use kbviewer_server::auth::store::AuthStore;
use std::fs::File;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Far longer than a failed bind takes, so only a server that kept running hits it.
const EXIT_DEADLINE: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The launch agent and KBViewer.app both want 4321. Whichever process loses must find out
/// before it indexes anything: launchd restarts the agent every ten seconds, and a loser
/// that indexed the whole vault first did so a quarter of a million times in one month.
/// Claiming the port first is also what lets the app's probe see an agent still indexing.
#[test]
fn a_port_already_taken_is_refused_before_the_vault_is_indexed() {
    let base = std::env::temp_dir().join("kbviewer-startup-port-taken");
    let _ = std::fs::remove_dir_all(&base);
    let holder = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken_port = holder.local_addr().unwrap().port();
    let config_path = write_config(&base, taken_port);

    let status = run_server_until_exit(&config_path, &base.join("server.log"));
    let output = std::fs::read_to_string(base.join("server.log")).unwrap();

    assert!(
        !status.success(),
        "the server started on a taken port:\n{output}"
    );
    assert!(
        output.contains("could not bind"),
        "unexpected failure:\n{output}"
    );
    assert!(
        !output.contains("indexed root"),
        "the vault was indexed before the bind was attempted:\n{output}"
    );
}

fn write_config(base: &Path, port: u16) -> std::path::PathBuf {
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

fn run_server_until_exit(config_path: &Path, log_path: &Path) -> ExitStatus {
    let log = File::create(log_path).unwrap();
    let mut server = Command::new(env!("CARGO_BIN_EXE_kbviewer"))
        .env("KBVIEWER_CONFIG", config_path)
        .env("RUST_LOG", "kbviewer=info")
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();

    let started = Instant::now();
    loop {
        if let Some(status) = server.try_wait().unwrap() {
            return status;
        }
        if started.elapsed() > EXIT_DEADLINE {
            server.kill().unwrap();
            panic!("the server was still running after {EXIT_DEADLINE:?}");
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
