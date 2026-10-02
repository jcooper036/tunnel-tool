use std::collections::HashSet;
use std::net::TcpListener;

use tunnel_tool::ports;
use tunnel_tool::spec::TunnelSpec;

fn spec(command: &str, range: (u16, u16)) -> TunnelSpec {
    TunnelSpec {
        name: "db".into(),
        access_command: command.into(),
        preferred_port_range: range,
        auto_run_on_start: false,
        health_check: None,
    }
}

#[test]
fn command_substitutes_port() {
    let tunnel = spec("proxy --port {port} --other {port}", (5000, 5001));
    assert_eq!(tunnel.command_for(5001), "proxy --port 5001 --other 5001");
}

#[test]
fn rejects_command_without_placeholder() {
    assert!(spec("proxy --port 5000", (5000, 5001)).validate().is_err());
}

#[test]
fn rejects_inverted_range() {
    assert!(spec("proxy {port}", (5001, 5000)).validate().is_err());
}

#[test]
fn pick_skips_claimed_and_bound_ports() {
    let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let bound = held.local_addr().unwrap().port();
    let claimed: HashSet<u16> = HashSet::from([bound + 1]);
    let picked = ports::pick((bound, bound + 2), &claimed).unwrap();
    assert_eq!(picked, bound + 2);
}

#[test]
fn pick_errors_when_range_exhausted() {
    let held = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let bound = held.local_addr().unwrap().port();
    assert!(ports::pick((bound, bound), &HashSet::new()).is_err());
}
