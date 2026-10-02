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

fn create_sleeper(home: &TempDir, name: &str, port: u16) {
    let range = format!("{port}-{port}");
    let out = tun(
        home,
        &[
            "create",
            name,
            "--command",
            "sleep {port}",
            "--port-range",
            &range,
        ],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn start_status_stop() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create_sleeper(&home, "svc", port);

    let started = tun(&home, &["start", "svc", "--wait", "0"]);
    assert!(started.status.success());
    assert!(stdout(&started).contains(&format!("localhost:{port}")));

    let status = stdout(&tun(&home, &["status"]));
    assert!(status.contains("svc") && status.contains(&port.to_string()));

    let again = stdout(&tun(&home, &["start", "svc", "--wait", "0"]));
    assert!(again.contains("already running"));

    assert!(tun(&home, &["stop", "svc"]).status.success());
    assert!(stdout(&tun(&home, &["status"])).contains("no running tunnels"));
}

#[test]
fn heal_restarts_dead_tunnel_on_same_port() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create_sleeper(&home, "svc", port);
    assert!(
        tun(&home, &["start", "svc", "--wait", "0"])
            .status
            .success()
    );

    Command::new("pkill")
        .args(["-f", &format!("sleep {port}")])
        .status()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(stdout(&tun(&home, &["status"])).contains("dead"));

    let healed = tun(&home, &["heal"]);
    assert!(healed.status.success());
    assert!(stdout(&healed).contains(&format!("localhost:{port}")));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn port_override_must_be_free() {
    let home = TempDir::new().unwrap();
    let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let busy = held.local_addr().unwrap().port().to_string();
    create_sleeper(&home, "svc", free_port());
    let out = tun(&home, &["start", "svc", "-p", &busy, "--wait", "0"]);
    assert!(!out.status.success());
}

#[test]
fn registry_lists_specs_and_running_state() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create_sleeper(&home, "svc", port);
    let before = stdout(&tun(&home, &["registry"]));
    assert!(before.contains("svc") && before.contains(&format!("{port}-{port}")));
    assert!(!before.contains("localhost:"));

    tun(&home, &["start", "svc", "--wait", "0"]);
    assert!(stdout(&tun(&home, &["registry"])).contains(&format!("localhost:{port}")));
    tun(&home, &["stop", "--all"]);
}

#[test]
fn create_without_args_fails_when_not_interactive() {
    let home = TempDir::new().unwrap();
    let out = tun(&home, &["create"]);
    assert!(!out.status.success());
}

#[test]
fn bare_tun_prints_help() {
    let home = TempDir::new().unwrap();
    let out = tun(&home, &[]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("Usage: tun"));
}
