use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Result, bail};
use dialoguer::{Confirm, Input};

use crate::cli::{CreateArgs, parse_port_range};
use crate::commands::Ctx;
use crate::spec::{self, PORT_PLACEHOLDER, TunnelSpec};

fn prompt_name(dir: &Path) -> Result<String> {
    let name = Input::<String>::new()
        .with_prompt("Tunnel name")
        .validate_with(|input: &String| -> Result<(), String> {
            spec::validate_name(input).map_err(|e| e.to_string())?;
            if spec::spec_path(dir, input).exists() {
                return Err(format!("'{input}' already exists"));
            }
            Ok(())
        })
        .interact_text()?;
    Ok(name)
}

fn prompt_command() -> Result<String> {
    let command = Input::<String>::new()
        .with_prompt(format!(
            "Access command (use {PORT_PLACEHOLDER} for the local port)"
        ))
        .validate_with(|input: &String| spec::validate_command(input).map_err(|e| e.to_string()))
        .interact_text()?;
    Ok(command)
}

fn prompt_port_range() -> Result<(u16, u16)> {
    let raw = Input::<String>::new()
        .with_prompt("Preferred port range START-END (avoid standard ports)")
        .validate_with(|input: &String| -> Result<(), String> {
            let range = parse_port_range(input).map_err(|e| e.to_string())?;
            spec::validate_range(range).map_err(|e| e.to_string())
        })
        .interact_text()?;
    parse_port_range(&raw)
}

fn prompt_auto_run() -> Result<bool> {
    Ok(Confirm::new()
        .with_prompt("Start with `tun start -a`?")
        .default(false)
        .interact()?)
}

pub fn run(ctx: &Ctx, args: CreateArgs) -> Result<()> {
    let complete = args.name.is_some() && args.command.is_some() && args.port_range.is_some();
    if !complete && !std::io::stdin().is_terminal() {
        bail!("name, --command and --port-range are required when stdin is not a terminal");
    }
    let dir = ctx.home.tunnels_dir();
    let name = match args.name {
        Some(name) => name,
        None => prompt_name(&dir)?,
    };
    let access_command = match args.command {
        Some(command) => command,
        None => prompt_command()?,
    };
    let preferred_port_range = match args.port_range {
        Some(range) => range,
        None => prompt_port_range()?,
    };
    let auto_run_on_start = if complete {
        args.auto_run
    } else {
        args.auto_run || prompt_auto_run()?
    };
    let tunnel = TunnelSpec {
        name,
        access_command,
        preferred_port_range,
        auto_run_on_start,
        health_check: args.health_check,
    };
    let path = spec::save_new(&dir, &tunnel)?;
    println!("created {}", path.display());
    Ok(())
}
