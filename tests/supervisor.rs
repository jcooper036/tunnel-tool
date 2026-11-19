use std::net::TcpListener;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

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

fn create(home: &TempDir, name: &str, command: &str, port: u16) {
    let range = format!("{port}-{port}");
    let out = tun(
        home,
        &["create", name, "--command", command, "--port-range", &range],
    );
    assert!(out.status.success());
}

fn status(home: &TempDir) -> serde_json::Value {
    serde_json::from_str(&stdout(&tun(home, &["status", "--json"]))).unwrap()
}

fn wait_for(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("timed out waiting for {what}");
}

fn count_procs(pattern: &str) -> usize {
    let out = Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .unwrap();
    stdout(&out).lines().count()
}

struct Cleanup<'a>(&'a TempDir);
impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        tun(self.0, &["stop", "--all"]);
        tun(self.0, &["supervisor", "stop"]);
    }
}

#[test]
fn killed_tunnel_comes_back_on_same_port_without_duplicates() {
    let home = TempDir::new().unwrap();
    let _cleanup = Cleanup(&home);
    let port = free_port();
    let marker = format!("http.server {port}");
    create(
        &home,
        "svc",
        "exec python3 -m http.server {port} --bind 127.0.0.1",
        port,
    );
    assert!(
        tun(&home, &["start", "svc", "--wait", "0"])
            .status
            .success()
    );
    let id = status(&home)[0]["session_id"].as_str().unwrap().to_string();

    Command::new("pkill")
        .args(["-f", &marker])
        .status()
        .unwrap();
    wait_for("restart", 10, || status(&home)[0]["restarts"] == 1);

    let after = status(&home);
    assert_eq!(after.as_array().unwrap().len(), 1);
    assert_eq!(after[0]["port"], port);
    assert_eq!(after[0]["session_id"], id.as_str());
    wait_for("tunnel up", 10, || status(&home)[0]["state"] == "up");
    assert_eq!(count_procs(&marker), 1);
}

#[test]
fn stopped_tunnel_is_never_revived() {
    let home = TempDir::new().unwrap();
    let _cleanup = Cleanup(&home);
    let port = free_port();
    create(&home, "svc", "exec sleep {port}", port);
    assert!(
        tun(&home, &["start", "svc", "--wait", "0"])
            .status
            .success()
    );
    assert!(tun(&home, &["stop", "svc"]).status.success());

    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(status(&home).as_array().unwrap().len(), 0);
    assert_eq!(count_procs(&format!("sleep {port}")), 0);
}

#[test]
fn failing_tunnel_backs_off_then_recovers() {
    let home = TempDir::new().unwrap();
    let _cleanup = Cleanup(&home);
    let port = free_port();
    let flag = home.path().join("network-up");
    let command = format!(
        "test -f {} && exec python3 -m http.server {{port}} --bind 127.0.0.1",
        flag.display()
    );
    create(&home, "flaky", &command, port);
    assert!(
        tun(&home, &["start", "flaky", "--wait", "0"])
            .status
            .success()
    );

    // 1s + 2s + 4s of backoff: at most ~4 restarts in 6s, never a busy loop.
    std::thread::sleep(Duration::from_secs(6));
    let restarts = status(&home)[0]["restarts"].as_u64().unwrap();
    assert!((2..=4).contains(&restarts), "restarts = {restarts}");

    std::fs::write(&flag, "").unwrap();
    wait_for("recovery", 20, || status(&home)[0]["state"] == "up");
    assert_eq!(status(&home)[0]["port"], port);
}
