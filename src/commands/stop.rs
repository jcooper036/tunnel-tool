use std::time::Duration;

use anyhow::{Result, bail};

use crate::cli::StopArgs;
use crate::commands::Ctx;
use crate::db::{self, Session};
use crate::process;

const GRACE: Duration = Duration::from_secs(3);

pub fn stop_session(ctx: &Ctx, session: &Session) -> Result<()> {
    process::terminate(session.process_id, GRACE);
    db::mark_stopped(&ctx.conn, &session.id)
}

pub fn run(ctx: &mut Ctx, args: StopArgs) -> Result<()> {
    let active = db::active_sessions(&ctx.conn)?;
    let targets: Vec<Session> = match &args.target {
        Some(target) => active
            .into_iter()
            .filter(|s| &s.tunnel_name == target || &s.id == target)
            .collect(),
        None => active,
    };
    if targets.is_empty() {
        match args.target {
            Some(target) => bail!("no running tunnel or session '{target}'"),
            None => println!("no running tunnels"),
        }
        return Ok(());
    }
    for session in &targets {
        stop_session(ctx, session)?;
        println!("stopped {} (session {})", session.tunnel_name, session.id);
    }
    Ok(())
}
