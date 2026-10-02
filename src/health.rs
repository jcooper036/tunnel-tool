use std::time::Duration;

use serde::Serialize;

use crate::process::{self, Liveness};

const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize)]
pub struct HealthReport {
    pub url: String,
    pub ok: bool,
    pub detail: String,
}

pub fn probe(url: &str) -> HealthReport {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PROBE_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let (ok, detail) = match agent.get(url).call() {
        Ok(response) => {
            let status = response.status().as_u16();
            ((200..300).contains(&status), format!("HTTP {status}"))
        }
        Err(err) => (false, err.to_string()),
    };
    HealthReport {
        url: url.to_string(),
        ok,
        detail,
    }
}

pub fn state(pid: u32, port: u16, health_url: Option<&str>) -> Liveness {
    match (process::liveness(pid, port), health_url) {
        (Liveness::Up, Some(url)) if !probe(url).ok => Liveness::Unhealthy,
        (state, _) => state,
    }
}
