pub mod check;
pub mod create;
pub mod doctor;
pub mod ensure;
pub mod heal;
pub mod log;
pub mod port;
pub mod registry;
pub mod remove;
pub mod restart;
pub mod show;
pub mod start;
pub mod status;
pub mod stop;
pub mod update;

use anyhow::Result;
use rusqlite::Connection;

use crate::cli::{Cli, Command};
use crate::db::{self, Session};
use crate::health;
use crate::home::Home;
use crate::process::Liveness;
use crate::spec;

pub struct Ctx {
    pub home: Home,
    pub conn: Connection,
}

impl Ctx {
    pub fn open(home: Home) -> Result<Self> {
        home.ensure()?;
        let conn = db::open(&home.db_path())?;
        Ok(Self { home, conn })
    }
}

impl Ctx {
    pub fn session_state(&self, session: &Session) -> Liveness {
        let health_url = spec::find(&self.home.tunnels_dir(), &session.tunnel_name)
            .ok()
            .and_then(|tunnel| tunnel.health_url_for(session.port));
        health::state(session.process_id, session.port, health_url.as_deref())
    }
}

pub fn run(cli: Cli) -> Result<()> {
    if matches!(cli.command, Command::Update) {
        return update::run();
    }
    let mut ctx = Ctx::open(Home::resolve(cli.home)?)?;
    match cli.command {
        Command::Update => unreachable!("handled before state is opened"),
        Command::Start(args) => start::run(&mut ctx, args),
        Command::Stop(args) => stop::run(&mut ctx, args),
        Command::Status(args) => status::run(&ctx, args),
        Command::Registry(args) => registry::run(&ctx, args),
        Command::Create(args) => create::run(&ctx, args),
        Command::Heal => heal::run(&mut ctx),
        Command::Doctor(args) => doctor::run(&mut ctx, args),
        Command::Restart(args) => restart::run(&mut ctx, args),
        Command::Remove(args) => remove::run(&mut ctx, args),
        Command::Log(args) => log::run(&ctx, args),
        Command::Show(args) => show::run(&ctx, args),
        Command::Ensure(args) => ensure::run(&mut ctx, args),
        Command::Port(args) => port::run(&ctx, args),
        Command::Check(args) => check::run(&ctx, args),
    }
}
