use std::fs::OpenOptions;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::ports;

pub fn spawn_detached(command: &str, log_path: &Path) -> Result<u32> {
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("opening {}", log_path.display()))?;
    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .with_context(|| format!("spawning: {command}"))?;
    Ok(child.id())
}

pub fn is_alive(pid: u32) -> bool {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub fn terminate(pid: u32, grace: Duration) {
    signal_group(pid, libc::SIGTERM);
    let deadline = Instant::now() + grace;
    while is_alive(pid) && Instant::now() < deadline {
        sleep(Duration::from_millis(50));
    }
    if is_alive(pid) {
        signal_group(pid, libc::SIGKILL);
    }
}

fn signal_group(pid: u32, signal: i32) {
    unsafe {
        libc::kill(-(pid as i32), signal);
        libc::kill(pid as i32, signal);
    }
}

pub fn wait_for_port(pid: u32, port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ports::accepts_connections(port) {
            return true;
        }
        if !is_alive(pid) {
            return false;
        }
        sleep(Duration::from_millis(200));
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liveness {
    Up,
    Unhealthy,
    Unreachable,
    Dead,
}

impl Liveness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Unhealthy => "unhealthy",
            Self::Unreachable => "unreachable",
            Self::Dead => "dead",
        }
    }
}

pub fn liveness(pid: u32, port: u16) -> Liveness {
    if !is_alive(pid) {
        Liveness::Dead
    } else if ports::accepts_connections(port) {
        Liveness::Up
    } else {
        Liveness::Unreachable
    }
}
