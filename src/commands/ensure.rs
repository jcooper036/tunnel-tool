use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::cli::EnsureArgs;
use crate::commands::{Ctx, start};
use crate::db::{self, Session};
use crate::health;
use crate::process::{self, Liveness};
use crate::spec::{self, TunnelSpec};

const HEALTH_POLL: Duration = Duration::from_millis(250);

fn relaunch(ctx: &mut Ctx, spec: &TunnelSpec, dead: &Session) -> Result<Session> {
    db::mark_stopped(&ctx.conn, &dead.id)?;
    match start::launch(ctx, spec, Some(dead.port)) {
        Ok(session) => Ok(session),
        Err(_) => start::launch(ctx, spec, None),
    }
}

fn existing_session(ctx: &Ctx, name: &str) -> Result<Option<Session>> {
    Ok(db::active_sessions(&ctx.conn)?
        .into_iter()
        .rfind(|s| s.tunnel_name == name))
}

fn failure_message(ctx: &Ctx, spec: &TunnelSpec, session: &Session, detail: &str) -> String {
    let state = ctx.session_state(session).label();
    format!(
        "{} is {state} on localhost:{} after waiting; log {}{detail}",
        spec.name, session.port, session.log_path
    )
}

fn wait_for_health(url: &str, deadline: Instant) -> Result<(), String> {
    loop {
        let report = health::probe(url);
        if report.ok {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!("; health check {} failed: {}", url, report.detail));
        }
        sleep(HEALTH_POLL);
    }
}

fn wait_until_ready(
    ctx: &Ctx,
    spec: &TunnelSpec,
    session: &Session,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    if !process::wait_for_port(session.process_id, session.port, timeout) {
        bail!("{}", failure_message(ctx, spec, session, ""));
    }
    let Some(url) = spec.health_url_for(session.port) else {
        return Ok(());
    };
    wait_for_health(&url, deadline)
        .map_err(|detail| anyhow::anyhow!("{}", failure_message(ctx, spec, session, &detail)))
}

pub fn ensure_one(ctx: &mut Ctx, spec: &TunnelSpec, timeout: Duration) -> Result<u16> {
    let session = match existing_session(ctx, &spec.name)? {
        None => start::launch(ctx, spec, None)?,
        Some(session) => match ctx.session_state(&session) {
            Liveness::Up => return Ok(session.port),
            Liveness::Dead => relaunch(ctx, spec, &session)?,
            Liveness::Unhealthy | Liveness::Unreachable => session,
        },
    };
    wait_until_ready(ctx, spec, &session, timeout)?;
    Ok(session.port)
}

fn ensure_all(ctx: &mut Ctx, timeout: Duration) -> Result<()> {
    let auto: Vec<TunnelSpec> = spec::load_all(&ctx.home.tunnels_dir())?
        .into_iter()
        .filter(|s| s.auto_run_on_start)
        .collect();
    if auto.is_empty() {
        bail!("no tunnel specs have auto_run_on_start = true");
    }
    let mut failures = 0;
    for spec in &auto {
        match ensure_one(ctx, spec, timeout) {
            Ok(port) => println!("{}\t{port}", spec.name),
            Err(err) => {
                eprintln!("{err:#}");
                failures += 1;
            }
        }
    }
    if failures > 0 {
        bail!("{failures} tunnel(s) failed to become ready");
    }
    Ok(())
}

pub fn run(ctx: &mut Ctx, args: EnsureArgs) -> Result<()> {
    let timeout = Duration::from_secs(args.timeout);
    match args.name {
        Some(name) => {
            let spec = spec::find(&ctx.home.tunnels_dir(), &name)?;
            println!("{}", ensure_one(ctx, &spec, timeout)?);
            Ok(())
        }
        None => ensure_all(ctx, timeout),
    }
}
