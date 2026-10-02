use std::time::Duration;

use anyhow::Result;

use crate::cli::RestartArgs;
use crate::commands::{Ctx, start, stop};
use crate::db::{self, Session};
use crate::process;
use crate::spec;

fn resolve_name(active: &[Session], target: &str) -> String {
    active
        .iter()
        .find(|s| s.id == target)
        .map_or_else(|| target.to_string(), |s| s.tunnel_name.clone())
}

pub fn run(ctx: &mut Ctx, args: RestartArgs) -> Result<()> {
    let active = db::active_sessions(&ctx.conn)?;
    let name = resolve_name(&active, &args.target);
    let tunnel = spec::find(&ctx.home.tunnels_dir(), &name)?;
    let previous: Vec<Session> = active
        .into_iter()
        .filter(|s| s.tunnel_name == name)
        .collect();
    let old_port = previous.first().map(|s| s.port);
    for session in &previous {
        stop::stop_session(ctx, session)?;
        println!("stopped {} (session {})", name, session.id);
    }
    if old_port.is_none() {
        println!("{name} was not running; starting");
    }
    let session = match old_port {
        Some(port) => start::launch(ctx, &tunnel, Some(port))
            .or_else(|_| start::launch(ctx, &tunnel, None))?,
        None => start::launch(ctx, &tunnel, None)?,
    };
    let ready = process::wait_for_port(
        session.process_id,
        session.port,
        Duration::from_secs(args.wait),
    );
    let state = if ready {
        "up"
    } else {
        "not yet accepting connections"
    };
    println!(
        "{} -> localhost:{} [{}] session {} log {}",
        name, session.port, state, session.id, session.log_path
    );
    Ok(())
}
