use anyhow::Result;
use serde::Serialize;

use crate::db::Session;
use crate::process::Liveness;

#[derive(Debug, Serialize)]
pub struct SessionView {
    pub session_id: String,
    pub tunnel_name: String,
    pub address: String,
    pub port: u16,
    pub state: &'static str,
    pub process_id: u32,
    pub created_at: String,
    pub stopped_at: Option<String>,
    pub log_path: String,
    pub command: String,
}

pub const STOPPED_LABEL: &str = "stopped";

impl SessionView {
    pub fn new(session: &Session, state: Liveness) -> Self {
        Self::with_label(session, state.label())
    }

    pub fn stopped(session: &Session) -> Self {
        Self::with_label(session, STOPPED_LABEL)
    }

    fn with_label(session: &Session, state: &'static str) -> Self {
        Self {
            session_id: session.id.clone(),
            tunnel_name: session.tunnel_name.clone(),
            address: format!("localhost:{}", session.port),
            port: session.port,
            state,
            process_id: session.process_id,
            created_at: session.created_at.clone(),
            stopped_at: session.stopped_at.clone(),
            log_path: session.log_path.clone(),
            command: session.command.clone(),
        }
    }
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
