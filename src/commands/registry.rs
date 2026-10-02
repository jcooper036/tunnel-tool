use anyhow::Result;
use comfy_table::{Table, presets::NOTHING};
use serde::Serialize;

use crate::cli::JsonArgs;
use crate::commands::{Ctx, start};
use crate::output::{self, SessionView};
use crate::spec::{self, TunnelSpec};

#[derive(Debug, Serialize)]
pub struct RegistryEntry {
    pub name: String,
    pub preferred_port_range: (u16, u16),
    pub auto_run_on_start: bool,
    pub health_check: Option<String>,
    pub access_command: String,
    pub running: Option<SessionView>,
}

fn entry(ctx: &Ctx, tunnel: &TunnelSpec) -> Result<RegistryEntry> {
    let running = start::running_session(ctx, &tunnel.name)?
        .map(|session| SessionView::new(&session, ctx.session_state(&session)));
    Ok(RegistryEntry {
        name: tunnel.name.clone(),
        preferred_port_range: tunnel.preferred_port_range,
        auto_run_on_start: tunnel.auto_run_on_start,
        health_check: tunnel.health_check.clone(),
        access_command: tunnel.access_command.clone(),
        running,
    })
}

fn print_table(entries: &[RegistryEntry]) {
    let mut table = Table::new();
    table.load_style(NOTHING);
    table.set_header(["NAME", "PORT RANGE", "AUTO-RUN", "RUNNING", "COMMAND"]);
    for e in entries {
        let (start_port, end_port) = e.preferred_port_range;
        let running = e
            .running
            .as_ref()
            .map_or("-".to_string(), |v| v.address.clone());
        table.add_row([
            e.name.clone(),
            format!("{start_port}-{end_port}"),
            e.auto_run_on_start.to_string(),
            running,
            e.access_command.clone(),
        ]);
    }
    println!("{table}");
}

pub fn run(ctx: &Ctx, args: JsonArgs) -> Result<()> {
    let dir = ctx.home.tunnels_dir();
    let specs = spec::load_all(&dir)?;
    let entries = specs
        .iter()
        .map(|t| entry(ctx, t))
        .collect::<Result<Vec<_>>>()?;
    if args.json {
        return output::print_json(&entries);
    }
    if entries.is_empty() {
        println!("no tunnel specs in {}", dir.display());
        return Ok(());
    }
    print_table(&entries);
    Ok(())
}
