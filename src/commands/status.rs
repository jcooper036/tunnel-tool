use std::fmt;

use anyhow::{Result, bail};
use comfy_table::{Table, presets::NOTHING};

use crate::cli::StatusArgs;
use crate::commands::Ctx;
use crate::db::{self, Session};
use crate::output::{self, SessionView};
use crate::process::Liveness;

const HISTORY_LIMIT: usize = 50;

#[derive(Debug)]
pub struct Silent;

impl fmt::Display for Silent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not all tunnels are up")
    }
}

impl std::error::Error for Silent {}

fn view(ctx: &Ctx, session: &Session) -> SessionView {
    match session.stopped_at {
        Some(_) => SessionView::stopped(session),
        None => SessionView::new(session, ctx.session_state(session)),
    }
}

fn quiet(ctx: &Ctx) -> Result<()> {
    let sessions = db::active_sessions(&ctx.conn)?;
    let all_up = !sessions.is_empty()
        && sessions
            .iter()
            .all(|s| ctx.session_state(s) == Liveness::Up);
    if all_up { Ok(()) } else { bail!(Silent) }
}

fn print_table(views: &[SessionView]) {
    let mut table = Table::new();
    table.load_style(NOTHING);
    table.set_header(["ID", "NAME", "ADDRESS", "PORT", "STATE"]);
    for v in views {
        table.add_row([
            v.session_id.clone(),
            v.tunnel_name.clone(),
            v.address.clone(),
            v.port.to_string(),
            v.state.to_string(),
        ]);
    }
    println!("{table}");
}

pub fn run(ctx: &Ctx, args: StatusArgs) -> Result<()> {
    if args.quiet {
        return quiet(ctx);
    }
    let sessions = if args.all {
        db::all_sessions(&ctx.conn, HISTORY_LIMIT)?
    } else {
        db::active_sessions(&ctx.conn)?
    };
    let views: Vec<SessionView> = sessions.iter().map(|s| view(ctx, s)).collect();
    if args.json {
        return output::print_json(&views);
    }
    if views.is_empty() {
        println!("no running tunnels");
        return Ok(());
    }
    print_table(&views);
    Ok(())
}
