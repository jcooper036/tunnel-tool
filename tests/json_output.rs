use std::net::TcpListener;
use std::process::{Command, Output};

use serde_json::Value;
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

fn json(home: &TempDir, args: &[&str]) -> Value {
    let out = tun(home, args);
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

fn create(home: &TempDir, name: &str, command: &str, port: u16) {
    let range = format!("{port}-{port}");
    let out = tun(
        home,
        &["create", name, "--command", command, "--port-range", &range],
    );
    assert!(out.status.success());
}

fn listener_command() -> String {
    "python3 -m http.server {port} --bind 127.0.0.1".to_string()
}

#[test]
fn status_json_and_all_history() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "svc", "sleep {port}", port);
    assert_eq!(json(&home, &["status", "--json"]), Value::Array(vec![]));

    tun(&home, &["start", "svc", "--wait", "0"]);
    let active = json(&home, &["status", "--json"]);
    assert_eq!(active[0]["tunnel_name"], "svc");
    assert_eq!(active[0]["port"], port);

    tun(&home, &["stop", "--all"]);
    assert_eq!(json(&home, &["status", "--json"]), Value::Array(vec![]));
    let history = json(&home, &["status", "--all", "--json"]);
    assert_eq!(history[0]["state"], "stopped");
    assert!(history[0]["stopped_at"].is_string());
}

#[test]
fn registry_json_reports_running_view() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "svc", "sleep {port}", port);
    let before = json(&home, &["registry", "--json"]);
    assert_eq!(
        before[0]["preferred_port_range"],
        serde_json::json!([port, port])
    );
    assert!(before[0]["running"].is_null());

    tun(&home, &["start", "svc", "--wait", "0"]);
    let after = json(&home, &["registry", "--json"]);
    assert_eq!(after[0]["running"]["port"], port);
    tun(&home, &["stop", "--all"]);
}

#[test]
fn quiet_status_exit_codes() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "svc", &listener_command(), port);
    let none = tun(&home, &["status", "-q"]);
    assert!(!none.status.success() && none.stdout.is_empty() && none.stderr.is_empty());

    tun(&home, &["start", "svc", "--wait", "10"]);
    let up = tun(&home, &["status", "--quiet"]);
    assert!(up.status.success() && up.stdout.is_empty());

    tun(&home, &["stop", "--all"]);
    assert!(!tun(&home, &["status", "-q"]).status.success());
}

#[test]
fn doctor_json_counts_health_failure() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    let range = format!("{port}-{port}");
    let out = tun(
        &home,
        &[
            "create",
            "svc",
            "--command",
            &listener_command(),
            "--port-range",
            &range,
            "--health-check",
            "http://127.0.0.1:{port}/missing-path",
        ],
    );
    assert!(out.status.success());
    let result = tun(&home, &["doctor", "--json", "--timeout", "10"]);
    assert!(!result.status.success());
    let report: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report[0]["ok"], false);
    assert_eq!(report[0]["health"]["ok"], false);
    assert_eq!(report[0]["already_running"], false);
}
