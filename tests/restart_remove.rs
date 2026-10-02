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

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
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
    assert!(out.status.success(), "{}", stderr(&out));
}

fn session_id(line: &str) -> String {
    let after = line.split("session ").nth(1).unwrap();
    after.split_whitespace().next().unwrap().to_string()
}

fn started_line(out: &Output) -> String {
    stdout(out)
        .lines()
        .find(|l| l.contains("->"))
        .unwrap()
        .to_string()
}

#[test]
fn restart_keeps_port_and_changes_session() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create_sleeper(&home, "svc", port);
    let first = started_line(&tun(&home, &["start", "svc", "--wait", "0"]));

    let out = tun(&home, &["restart", "svc", "--wait", "0"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let second = started_line(&out);
    assert!(second.contains(&format!("localhost:{port}")));
    assert_ne!(session_id(&first), session_id(&second));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn restart_by_session_id() {
    let home = TempDir::new().unwrap();
    create_sleeper(&home, "svc", free_port());
    let first = started_line(&tun(&home, &["start", "svc", "--wait", "0"]));
    let out = tun(&home, &["restart", &session_id(&first), "--wait", "0"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(started_line(&out).starts_with("svc ->"));
    tun(&home, &["stop", "--all"]);
}

#[test]
fn restart_of_stopped_tunnel_starts_it() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create_sleeper(&home, "svc", port);
    let out = tun(&home, &["restart", "svc", "--wait", "0"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("was not running"));
    assert!(stdout(&out).contains(&format!("localhost:{port}")));
    tun(&home, &["stop", "--all"]);
}

#[test]
fn restart_unknown_spec_errors() {
    let home = TempDir::new().unwrap();
    assert!(!tun(&home, &["restart", "nope"]).status.success());
}

#[test]
fn remove_refuses_while_running_then_force_removes() {
    let home = TempDir::new().unwrap();
    create_sleeper(&home, "svc", free_port());
    tun(&home, &["start", "svc", "--wait", "0"]);

    let refused = tun(&home, &["remove", "svc"]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("--force"));
    assert!(stdout(&tun(&home, &["registry"])).contains("svc"));

    let forced = tun(&home, &["remove", "svc", "--force"]);
    assert!(forced.status.success(), "{}", stderr(&forced));
    assert!(stdout(&forced).contains("removed svc"));
    assert!(!stdout(&tun(&home, &["registry"])).contains("svc"));
    assert!(stdout(&tun(&home, &["status"])).contains("no running tunnels"));
}

#[test]
fn remove_stopped_and_unknown() {
    let home = TempDir::new().unwrap();
    create_sleeper(&home, "svc", free_port());
    assert!(tun(&home, &["remove", "svc"]).status.success());
    assert!(!tun(&home, &["remove", "svc"]).status.success());
}
