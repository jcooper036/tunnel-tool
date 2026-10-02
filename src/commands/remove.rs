use anyhow::{Result, bail};

use crate::cli::RemoveArgs;
use crate::commands::{Ctx, stop};
use crate::db;
use crate::spec;

pub fn run(ctx: &mut Ctx, args: RemoveArgs) -> Result<()> {
    let dir = ctx.home.tunnels_dir();
    spec::find(&dir, &args.name)?;
    let active: Vec<_> = db::active_sessions(&ctx.conn)?
        .into_iter()
        .filter(|s| s.tunnel_name == args.name)
        .collect();
    if !active.is_empty() && !args.force {
        bail!(
            "{} has {} active session(s); stop it first or pass --force",
            args.name,
            active.len()
        );
    }
    for session in &active {
        stop::stop_session(ctx, session)?;
        println!("stopped {} (session {})", args.name, session.id);
    }
    let path = spec::spec_path(&dir, &args.name);
    std::fs::remove_file(&path)?;
    println!("removed {} ({})", args.name, path.display());
    Ok(())
}
