use anyhow::{Result, bail};

use crate::cli::CheckArgs;
use crate::commands::{Ctx, start};
use crate::health;
use crate::spec;

pub fn run(ctx: &Ctx, args: CheckArgs) -> Result<()> {
    let spec = spec::find(&ctx.home.tunnels_dir(), &args.name)?;
    if spec.health_check.is_none() {
        bail!("{} has no health_check in its spec", spec.name);
    }
    let Some(session) = start::running_session(ctx, &spec.name)? else {
        bail!("{} is not running", spec.name);
    };
    let Some(url) = spec.health_url_for(session.port) else {
        bail!("{} has no health_check in its spec", spec.name);
    };
    let report = health::probe(&url);
    if report.ok {
        println!("ok {}", report.detail);
        return Ok(());
    }
    println!("FAILED {}", report.detail);
    std::process::exit(1);
}
