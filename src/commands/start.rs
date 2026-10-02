use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Result, bail};
use rusqlite::TransactionBehavior;

use crate::cli::StartArgs;
use crate::commands::Ctx;
use crate::db::{self, NewSession, Session};
use crate::ports;
use crate::process::{self, Liveness};
use crate::spec::{self, TunnelSpec};

pub fn launch(ctx: &mut Ctx, spec: &TunnelSpec, port_override: Option<u16>) -> Result<Session> {
    let tx = ctx
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let claimed: HashSet<u16> = db::active_sessions(&tx)?.iter().map(|s| s.port).collect();
    let port = match port_override {
        Some(port) => ports::ensure_free(port, &claimed)?,
        None => ports::pick(spec.preferred_port_range, &claimed)?,
    };
    let id = db::new_session_id();
    let log_path = ctx.home.logs_dir().join(format!("{id}.log"));
    let log_path = log_path.to_string_lossy().into_owned();
    let command = spec.command_for(port);
    let process_id = process::spawn_detached(&command, std::path::Path::new(&log_path))?;
    let new = NewSession {
        tunnel_name: &spec.name,
        port,
        process_id,
        command: &command,
        log_path: &log_path,
    };
    db::insert_session(&tx, &id, &new)?;
    tx.commit()?;
    Ok(db::active_sessions(&ctx.conn)?
        .into_iter()
        .find(|s| s.id == id)
        .expect("session inserted in this call"))
}

pub fn running_session(ctx: &Ctx, name: &str) -> Result<Option<Session>> {
    Ok(db::active_sessions(&ctx.conn)?.into_iter().find(|s| {
        s.tunnel_name == name && process::liveness(s.process_id, s.port) != Liveness::Dead
    }))
}

fn start_one(ctx: &mut Ctx, spec: &TunnelSpec, port: Option<u16>, wait: Duration) -> Result<()> {
    if let Some(existing) = running_session(ctx, &spec.name)? {
        println!(
            "{} already running on localhost:{}",
            spec.name, existing.port
        );
        return Ok(());
    }
    let session = launch(ctx, spec, port)?;
    let ready = process::wait_for_port(session.process_id, session.port, wait);
    let state = if ready {
        "up"
    } else {
        "not yet accepting connections"
    };
    println!(
        "{} -> localhost:{} [{}] session {} log {}",
        spec.name, session.port, state, session.id, session.log_path
    );
    Ok(())
}

pub fn run(ctx: &mut Ctx, args: StartArgs) -> Result<()> {
    let dir = ctx.home.tunnels_dir();
    let wait = Duration::from_secs(args.wait);
    if let Some(name) = args.name {
        return start_one(ctx, &spec::find(&dir, &name)?, args.port, wait);
    }
    let auto: Vec<TunnelSpec> = spec::load_all(&dir)?
        .into_iter()
        .filter(|s| s.auto_run_on_start)
        .collect();
    if auto.is_empty() {
        bail!("no tunnel specs have auto_run_on_start = true");
    }
    let mut failures = 0;
    for spec in &auto {
        if let Err(err) = start_one(ctx, spec, None, wait) {
            eprintln!("{}: {err:#}", spec.name);
            failures += 1;
        }
    }
    if failures > 0 {
        bail!("{failures} tunnel(s) failed to start");
    }
    Ok(())
}
