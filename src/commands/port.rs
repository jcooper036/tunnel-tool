use anyhow::{Result, bail};

use crate::cli::PortArgs;
use crate::commands::{Ctx, start};

pub fn run(ctx: &Ctx, args: PortArgs) -> Result<()> {
    let Some(session) = start::running_session(ctx, &args.name)? else {
        bail!("{} is not running", args.name);
    };
    println!("{}", session.port);
    Ok(())
}
