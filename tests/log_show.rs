use std::net::TcpListener;
use std::process::{Command, Output};

use tempfile::TempDir;

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn tun(home: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tun"))
        .env("TUN_HOME", home.path())
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn start_talker(home: &TempDir, name: &str, body: &str) -> u16 {
    let port = free_port();
    let range = format!("{port}-{port}");
    let command = format!("{body}; sleep {{port}}");
    let created = tun(
        home,
        &[
            "create",
            name,
            "--command",
            &command,
            "--port-range",
            &range,
        ],
    );
    assert!(created.status.success());
    let started = tun(home, &["start", name, "--wait", "0"]);
    assert!(started.status.success());
    port
}

fn wait_for_log(home: &TempDir, name: &str, needle: &str) {
    for _ in 0..50 {
        if stdout(&tun(home, &["log", name])).contains(needle) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("log never contained {needle}");
}

#[test]
fn log_prints_last_lines_even_after_stop() {
    let home = TempDir::new().unwrap();
    start_talker(&home, "talk", "echo one; echo two; echo three");
    wait_for_log(&home, "talk", "three");

    let tail = stdout(&tun(&home, &["log", "talk", "-n", "2"]));
    assert_eq!(tail, "two\nthree\n");

    tun(&home, &["stop", "talk"]);
    assert_eq!(stdout(&tun(&home, &["log", "talk", "-n", "1"])), "three\n");
}

#[test]
fn log_errors_on_unknown_target() {
    let home = TempDir::new().unwrap();
    assert!(!tun(&home, &["log", "ghost"]).status.success());
}

#[test]
fn log_empty_prints_nothing() {
    let home = TempDir::new().unwrap();
    start_talker(&home, "quiet", "true");
    let out = tun(&home, &["log", "quiet"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "");
}

#[test]
fn log_missing_file_names_path() {
    let home = TempDir::new().unwrap();
    start_talker(&home, "gone", "echo hi");
    wait_for_log(&home, "gone", "hi");
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&tun(&home, &["show", "gone", "--json"]))).unwrap();
    let path = json["session"]["log_path"].as_str().unwrap().to_string();
    std::fs::remove_file(&path).unwrap();
    let out = tun(&home, &["log", "gone"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains(&path));
}

#[test]
fn show_json_reports_spec_session_and_tail() {
    let home = TempDir::new().unwrap();
    let port = start_talker(&home, "diag", "echo a; echo b; echo c");
    wait_for_log(&home, "diag", "c");

    let json: serde_json::Value =
        serde_json::from_str(&stdout(&tun(&home, &["show", "diag", "--json", "-n", "2"]))).unwrap();
    assert_eq!(json["spec"]["name"], "diag");
    assert_eq!(json["session"]["port"], port);
    assert_eq!(json["log_tail"], serde_json::json!(["b", "c"]));
    assert!(json["health"].is_null());

    tun(&home, &["stop", "diag"]);
    let stopped: serde_json::Value =
        serde_json::from_str(&stdout(&tun(&home, &["show", "diag", "--json"]))).unwrap();
    assert_eq!(stopped["session"]["state"], "stopped");
}

#[test]
fn show_handles_missing_spec_or_session() {
    let home = TempDir::new().unwrap();
    assert!(!tun(&home, &["show", "ghost"]).status.success());

    start_talker(&home, "orphan", "echo x");
    wait_for_log(&home, "orphan", "x");
    tun(&home, &["stop", "orphan"]);
    std::fs::remove_file(home.path().join("tunnels").join("orphan.toml")).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&tun(&home, &["show", "orphan", "--json"]))).unwrap();
    assert!(json["spec"].is_null());
    assert_eq!(json["log_tail"], serde_json::json!(["x"]));

    let home = TempDir::new().unwrap();
    tun(
        &home,
        &[
            "create",
            "fresh",
            "--command",
            "sleep {port}",
            "--port-range",
            "40100-40100",
        ],
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&tun(&home, &["show", "fresh", "--json"]))).unwrap();
    assert!(json["session"].is_null());
    assert_eq!(json["log_tail"], serde_json::json!([]));
}
