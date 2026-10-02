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

fn create(home: &TempDir, name: &str, command: &str, port: u16, extra: &[&str]) {
    let range = format!("{port}-{port}");
    let mut args = vec!["create", name, "--command", command, "--port-range", &range];
    args.extend_from_slice(extra);
    let out = tun(home, &args);
    assert!(out.status.success(), "{}", stderr(&out));
}

const SERVER: &str = "python3 -m http.server {port} --bind 127.0.0.1";
const HEALTH: &str = "http://127.0.0.1:{port}/";

#[test]
fn ensure_starts_then_reuses_and_prints_only_port() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "web", SERVER, port, &["--health-check", HEALTH]);

    let first = tun(&home, &["ensure", "web"]);
    assert!(first.status.success(), "{}", stderr(&first));
    assert_eq!(stdout(&first), format!("{port}\n"));

    let second = tun(&home, &["ensure", "web"]);
    assert_eq!(stdout(&second), format!("{port}\n"));

    assert_eq!(stdout(&tun(&home, &["port", "web"])), format!("{port}\n"));
    let check = tun(&home, &["check", "web"]);
    assert!(check.status.success());
    assert!(stdout(&check).starts_with("ok "));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn ensure_relaunches_dead_tunnel_on_same_port() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "web", SERVER, port, &[]);
    assert!(tun(&home, &["ensure", "web"]).status.success());

    Command::new("pkill")
        .args(["-f", &format!("http.server {port}")])
        .status()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!tun(&home, &["port", "web"]).status.success());

    let again = tun(&home, &["ensure", "web"]);
    assert!(again.status.success(), "{}", stderr(&again));
    assert_eq!(stdout(&again), format!("{port}\n"));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn ensure_failure_prints_no_port_and_names_log() {
    let home = TempDir::new().unwrap();
    create(&home, "idle", "sleep {port}", free_port(), &[]);

    let out = tun(&home, &["ensure", "idle", "--timeout", "0"]);
    assert!(!out.status.success());
    assert_eq!(stdout(&out), "");
    assert!(stderr(&out).contains(".log"));
    assert!(stderr(&out).contains("unreachable"));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn ensure_all_prints_tab_separated_and_fails_on_any_failure() {
    let home = TempDir::new().unwrap();
    let port = free_port();
    create(&home, "web", SERVER, port, &["--auto-run"]);
    create(&home, "idle", "sleep {port}", free_port(), &["--auto-run"]);

    let out = tun(&home, &["ensure", "--all", "--timeout", "1"]);
    assert!(!out.status.success());
    assert_eq!(stdout(&out), format!("web\t{port}\n"));
    assert!(stderr(&out).contains("idle"));

    tun(&home, &["stop", "--all"]);
}

#[test]
fn port_and_check_error_when_not_running() {
    let home = TempDir::new().unwrap();
    create(
        &home,
        "web",
        SERVER,
        free_port(),
        &["--health-check", HEALTH],
    );
    for cmd in ["port", "check"] {
        let out = tun(&home, &[cmd, "web"]);
        assert!(!out.status.success());
        assert_eq!(stdout(&out), "");
        assert!(stderr(&out).contains("not running"));
    }
}

#[test]
fn check_errors_without_health_check_and_reports_failure() {
    let home = TempDir::new().unwrap();
    create(&home, "plain", "sleep {port}", free_port(), &[]);
    tun(&home, &["start", "plain", "--wait", "0"]);
    let out = tun(&home, &["check", "plain"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("no health_check"));

    create(
        &home,
        "sick",
        "sleep {port}",
        free_port(),
        &["--health-check", HEALTH],
    );
    tun(&home, &["start", "sick", "--wait", "0"]);
    let out = tun(&home, &["check", "sick"]);
    assert!(!out.status.success());
    assert!(stdout(&out).starts_with("FAILED "));

    tun(&home, &["stop", "--all"]);
}
