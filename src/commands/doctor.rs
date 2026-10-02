use std::time::Duration;

use anyhow::{Result, bail};
use serde::Serialize;

use crate::cli::DoctorArgs;
use crate::commands::{Ctx, start, stop};
use crate::health::{self, HealthReport};
use crate::output;
use crate::process;
use crate::spec::{self, TunnelSpec};

#[derive(Debug, Serialize)]
pub struct DoctorResult {
    pub name: String,
    pub ok: bool,
    pub port: Option<u16>,
    pub log_path: Option<String>,
    pub detail: String,
    pub health: Option<HealthReport>,
    pub already_running: bool,
}

impl DoctorResult {
    fn failed(name: &str, detail: String) -> Self {
        Self {
            name: name.to_string(),
            ok: false,
            port: None,
            log_path: None,
            detail,
            health: None,
            already_running: false,
        }
    }

    fn describe(&self) -> String {
        if self.already_running {
            return format!(
                "{}: already running on localhost:{}",
                self.name,
                self.port.unwrap_or_default()
            );
        }
        let verdict = if self.ok { "ok" } else { "FAILED" };
        let health = self.health.as_ref().map_or(String::new(), |h| {
            format!(
                ", health {} ({})",
                if h.ok { "ok" } else { "FAILED" },
                h.detail
            )
        });
        format!(
            "{}: {verdict} (port {}, log {}{health})",
            self.name,
            self.port.unwrap_or_default(),
            self.log_path.as_deref().unwrap_or("-")
        )
    }
}

fn check_one(ctx: &mut Ctx, tunnel: &TunnelSpec, timeout: Duration) -> Result<DoctorResult> {
    if let Some(existing) = start::running_session(ctx, &tunnel.name)? {
        return Ok(DoctorResult {
            name: tunnel.name.clone(),
            ok: true,
            port: Some(existing.port),
            log_path: Some(existing.log_path),
            detail: "already running".to_string(),
            health: None,
            already_running: true,
        });
    }
    let session = start::launch(ctx, tunnel, None)?;
    let port_ok = process::wait_for_port(session.process_id, session.port, timeout);
    let report = tunnel
        .health_url_for(session.port)
        .filter(|_| port_ok)
        .map(|url| health::probe(&url));
    stop::stop_session(ctx, &session)?;
    let health_ok = report.as_ref().is_none_or(|h| h.ok);
    let detail = match (port_ok, health_ok) {
        (false, _) => "port did not accept connections",
        (true, false) => "health check failed",
        (true, true) => "port accepts connections",
    };
    Ok(DoctorResult {
        name: tunnel.name.clone(),
        ok: port_ok && health_ok,
        port: Some(session.port),
        log_path: Some(session.log_path),
        detail: detail.to_string(),
        health: report,
        already_running: false,
    })
}

fn collect(ctx: &mut Ctx, specs: &[TunnelSpec], timeout: Duration) -> Vec<DoctorResult> {
    specs
        .iter()
        .map(|tunnel| {
            check_one(ctx, tunnel, timeout)
                .unwrap_or_else(|err| DoctorResult::failed(&tunnel.name, format!("{err:#}")))
        })
        .collect()
}

pub fn run(ctx: &mut Ctx, args: DoctorArgs) -> Result<()> {
    let dir = ctx.home.tunnels_dir();
    let specs = spec::load_all(&dir)?;
    if specs.is_empty() && !args.json {
        println!("no tunnel specs in {}", dir.display());
        return Ok(());
    }
    let results = collect(ctx, &specs, Duration::from_secs(args.timeout));
    if args.json {
        output::print_json(&results)?;
    } else {
        for result in &results {
            match result.log_path {
                None => eprintln!("{}: {}", result.name, result.detail),
                Some(_) => println!("{}", result.describe()),
            }
        }
    }
    let failures = results.iter().filter(|r| !r.ok).count();
    if failures > 0 {
        bail!("{failures} tunnel(s) failed");
    }
    Ok(())
}
