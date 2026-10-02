use anyhow::Result;
use serde::Serialize;

use crate::cli::ShowArgs;
use crate::commands::Ctx;
use crate::commands::log::{read_log, tail_lines};
use crate::db::{self, Session};
use crate::health::{self, HealthReport};
use crate::output::print_json;
use crate::spec::{self, TunnelSpec};

#[derive(Serialize)]
struct SessionDetail {
    session_id: String,
    state: &'static str,
    port: u16,
    process_id: u32,
    created_at: String,
    stopped_at: Option<String>,
    log_path: String,
}

#[derive(Serialize)]
struct Report {
    spec: Option<TunnelSpec>,
    session: Option<SessionDetail>,
    health: Option<HealthReport>,
    log_tail: Vec<String>,
}

fn session_detail(ctx: &Ctx, session: &Session) -> SessionDetail {
    let state = match session.stopped_at {
        Some(_) => "stopped",
        None => ctx.session_state(session).label(),
    };
    SessionDetail {
        session_id: session.id.clone(),
        state,
        port: session.port,
        process_id: session.process_id,
        created_at: session.created_at.clone(),
        stopped_at: session.stopped_at.clone(),
        log_path: session.log_path.clone(),
    }
}

fn find_spec(ctx: &Ctx, name: &str) -> Result<Option<TunnelSpec>> {
    let dir = ctx.home.tunnels_dir();
    if !spec::spec_path(&dir, name).exists() {
        return Ok(None);
    }
    Ok(Some(spec::find(&dir, name)?))
}

fn probe_health(spec: Option<&TunnelSpec>, session: Option<&Session>) -> Option<HealthReport> {
    let session = session.filter(|s| s.stopped_at.is_none())?;
    let url = spec?.health_url_for(session.port)?;
    Some(health::probe(&url))
}

fn build_report(ctx: &Ctx, args: &ShowArgs) -> Result<Report> {
    let session = db::find_sessions(&ctx.conn, &args.target, 1)?
        .into_iter()
        .next();
    let spec_name = session
        .as_ref()
        .map_or(args.target.as_str(), |s| s.tunnel_name.as_str());
    let spec = find_spec(ctx, spec_name)?;
    if spec.is_none() && session.is_none() {
        anyhow::bail!("no tunnel spec, tunnel or session '{}'", args.target);
    }
    let log_tail = match &session {
        Some(s) => tail_lines(&read_log(std::path::Path::new(&s.log_path))?, args.lines),
        None => Vec::new(),
    };
    Ok(Report {
        health: probe_health(spec.as_ref(), session.as_ref()),
        session: session.as_ref().map(|s| session_detail(ctx, s)),
        spec,
        log_tail,
    })
}

fn print_text(report: &Report) {
    match &report.spec {
        Some(spec) => {
            println!("spec: {}", spec.name);
            println!("  access_command: {}", spec.access_command);
            let (start, end) = spec.preferred_port_range;
            println!("  port_range: {start}-{end}");
            println!("  auto_run_on_start: {}", spec.auto_run_on_start);
            println!(
                "  health_check: {}",
                spec.health_check.as_deref().unwrap_or("none")
            );
        }
        None => println!("spec: none"),
    }
    match &report.session {
        Some(s) => {
            println!("session: {}", s.session_id);
            println!("  state: {}", s.state);
            println!("  port: {}", s.port);
            println!("  process_id: {}", s.process_id);
            println!("  created_at: {}", s.created_at);
            println!("  stopped_at: {}", s.stopped_at.as_deref().unwrap_or("-"));
            println!("  log_path: {}", s.log_path);
        }
        None => println!("session: none"),
    }
    if let Some(health) = &report.health {
        let verdict = if health.ok { "ok" } else { "failing" };
        println!("health: {verdict} ({}) {}", health.detail, health.url);
    }
    println!("log:");
    for line in &report.log_tail {
        println!("  {line}");
    }
}

pub fn run(ctx: &Ctx, args: ShowArgs) -> Result<()> {
    let report = build_report(ctx, &args)?;
    if args.json {
        return print_json(&report);
    }
    print_text(&report);
    Ok(())
}
