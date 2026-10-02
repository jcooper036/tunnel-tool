use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "tun",
    version,
    about = "Manage long-lived tunnels to remote services",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        env = "TUN_HOME",
        help = "State directory (default ~/.tun)"
    )]
    pub home: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Start tunnels")]
    Start(StartArgs),
    #[command(about = "Rebuild and reinstall tun from its source checkout")]
    Update,
    #[command(about = "Stop tunnels")]
    Stop(StopArgs),
    #[command(about = "Show running tunnels")]
    Status(StatusArgs),
    #[command(about = "List registered tunnel specs")]
    Registry(JsonArgs),
    #[command(about = "Create a tunnel spec")]
    Create(CreateArgs),
    #[command(about = "Restart dead tunnels")]
    Heal,
    #[command(about = "Check that every tunnel spec can come up")]
    Doctor(DoctorArgs),
    #[command(about = "Stop and relaunch a tunnel, preferring its previous port")]
    Restart(RestartArgs),
    #[command(about = "Delete a tunnel spec")]
    Remove(RemoveArgs),
    #[command(about = "Print a tunnel's latest session log")]
    Log(LogArgs),
    #[command(about = "Diagnose a tunnel: spec, session, health, log tail")]
    Show(ShowArgs),
    #[command(about = "Start the tunnel if needed and print only its port")]
    Ensure(EnsureArgs),
    #[command(about = "Print the port of a running tunnel")]
    Port(PortArgs),
    #[command(about = "Run a running tunnel's health check")]
    Check(CheckArgs),
}

#[derive(Args)]
pub struct StartArgs {
    #[arg(conflicts_with = "all", required_unless_present = "all")]
    pub name: Option<String>,
    #[arg(short, long, help = "Start all tunnels marked auto_run_on_start")]
    pub all: bool,
    #[arg(short, long, requires = "name")]
    pub port: Option<u16>,
    #[arg(
        long,
        default_value_t = 5,
        help = "Seconds to wait for each tunnel's port"
    )]
    pub wait: u64,
}

#[derive(Args)]
pub struct StopArgs {
    #[arg(
        conflicts_with = "all",
        required_unless_present = "all",
        help = "Tunnel name or session id"
    )]
    pub target: Option<String>,
    #[arg(short, long)]
    pub all: bool,
}

#[derive(Args)]
pub struct CreateArgs {
    #[arg(help = "Omit name, --command or --port-range to be prompted")]
    pub name: Option<String>,
    #[arg(long, help = "Command to run; must contain {port}")]
    pub command: Option<String>,
    #[arg(long, value_parser = parse_port_range, help = "START-END")]
    pub port_range: Option<(u16, u16)>,
    #[arg(long)]
    pub auto_run: bool,
    #[arg(long, help = "http(s) URL containing {port}; 2xx means healthy")]
    pub health_check: Option<String>,
}

#[derive(Args)]
pub struct JsonArgs {
    #[arg(long, help = "Print JSON to stdout")]
    pub json: bool,
}

#[derive(Args)]
pub struct StatusArgs {
    #[arg(long, help = "Print JSON to stdout")]
    pub json: bool,
    #[arg(short, long, help = "Include stopped sessions (newest first, max 50)")]
    pub all: bool,
    #[arg(
        short,
        long,
        conflicts_with = "json",
        help = "No output; exit 0 only if there are active sessions and all are up"
    )]
    pub quiet: bool,
}

#[derive(Args)]
pub struct DoctorArgs {
    #[arg(long, help = "Print JSON to stdout")]
    pub json: bool,
    #[arg(
        long,
        default_value_t = 10,
        help = "Seconds to wait for each tunnel's port"
    )]
    pub timeout: u64,
}

#[derive(Args)]
pub struct RestartArgs {
    #[arg(help = "Tunnel name or session id")]
    pub target: String,
    #[arg(
        long,
        default_value_t = 5,
        help = "Seconds to wait for the tunnel's port"
    )]
    pub wait: u64,
}

#[derive(Args)]
pub struct RemoveArgs {
    pub name: String,
    #[arg(short, long, help = "Stop active sessions first")]
    pub force: bool,
}

#[derive(Args)]
pub struct LogArgs {
    #[arg(help = "Tunnel name or session id")]
    pub target: String,
    #[arg(short = 'n', long = "lines", default_value_t = 100)]
    pub lines: usize,
    #[arg(short, long, help = "Keep printing appended lines")]
    pub follow: bool,
}

#[derive(Args)]
pub struct ShowArgs {
    #[arg(help = "Tunnel name or session id")]
    pub target: String,
    #[arg(short = 'n', long = "lines", default_value_t = 20)]
    pub lines: usize,
    #[arg(long, help = "Print one JSON object")]
    pub json: bool,
}

#[derive(Args)]
pub struct EnsureArgs {
    #[arg(conflicts_with = "all", required_unless_present = "all")]
    pub name: Option<String>,
    #[arg(short, long, help = "Ensure all tunnels marked auto_run_on_start")]
    pub all: bool,
    #[arg(long, default_value_t = 15, help = "Seconds to wait for readiness")]
    pub timeout: u64,
}

#[derive(Args)]
pub struct PortArgs {
    pub name: String,
}

#[derive(Args)]
pub struct CheckArgs {
    pub name: String,
}

pub fn parse_port_range(raw: &str) -> Result<(u16, u16)> {
    let Some((start, end)) = raw.split_once('-') else {
        bail!("expected START-END");
    };
    let start: u16 = start.trim().parse().context("invalid start port")?;
    let end: u16 = end.trim().parse().context("invalid end port")?;
    Ok((start, end))
}
