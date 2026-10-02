use std::collections::HashSet;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use anyhow::{Result, bail};

pub fn is_bindable(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

pub fn accepts_connections(port: u16) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

pub fn pick(range: (u16, u16), claimed: &HashSet<u16>) -> Result<u16> {
    let (start, end) = range;
    match (start..=end).find(|p| !claimed.contains(p) && is_bindable(*p)) {
        Some(port) => Ok(port),
        None => bail!("no free port in {start}-{end}"),
    }
}

pub fn ensure_free(port: u16, claimed: &HashSet<u16>) -> Result<u16> {
    if claimed.contains(&port) {
        bail!("port {port} is claimed by another tunnel session");
    }
    if !is_bindable(port) {
        bail!("port {port} is in use");
    }
    Ok(port)
}
