use anyhow::{Result, bail};

use crate::commands::{Ctx, start};
use crate::db::{self, Session};
use crate::process::{self, Liveness};
use crate::spec;

fn heal_one(ctx: &mut Ctx, session: &Session) -> Result<()> {
    let tunnel = spec::find(&ctx.home.tunnels_dir(), &session.tunnel_name)?;
    db::mark_stopped(&ctx.conn, &session.id)?;
    let revived = start::launch(ctx, &tunnel, Some(session.port))?;
    println!(
        "{} restarted on localhost:{} session {} log {}",
        tunnel.name, revived.port, revived.id, revived.log_path
    );
    Ok(())
}

pub fn run(ctx: &mut Ctx) -> Result<()> {
    let dead: Vec<Session> = db::active_sessions(&ctx.conn)?
        .into_iter()
        .filter(|s| process::liveness(s.process_id, s.port) == Liveness::Dead)
        .collect();
    if dead.is_empty() {
        println!("no dead tunnels");
        return Ok(());
    }
    let mut failures = 0;
    for session in &dead {
        if let Err(err) = heal_one(ctx, session) {
            eprintln!("{} (session {}): {err:#}", session.tunnel_name, session.id);
            failures += 1;
        }
    }
    if failures > 0 {
        bail!("{failures} tunnel(s) failed to heal");
    }
    Ok(())
}
