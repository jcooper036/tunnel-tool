use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

const MIGRATIONS: &[(&str, &str)] = &[(
    "26100212_create_sessions",
    "CREATE TABLE sessions (
        id TEXT PRIMARY KEY,
        tunnel_name TEXT NOT NULL,
        created_at TEXT NOT NULL,
        port INTEGER NOT NULL,
        process_id INTEGER NOT NULL,
        command TEXT NOT NULL,
        log_path TEXT NOT NULL,
        stopped_at TEXT
    );
    CREATE INDEX sessions_active_idx ON sessions (stopped_at);",
)];

const SESSION_COLUMNS: &str =
    "id AS sessions_id, tunnel_name, created_at, port, process_id, command, log_path, stopped_at";

#[derive(Debug, Clone, serde::Serialize)]
pub struct Session {
    pub id: String,
    pub tunnel_name: String,
    pub created_at: String,
    pub port: u16,
    pub process_id: u32,
    pub command: String,
    pub log_path: String,
    pub stopped_at: Option<String>,
}

pub struct NewSession<'a> {
    pub tunnel_name: &'a str,
    pub port: u16,
    pub process_id: u32,
    pub command: &'a str,
    pub log_path: &'a str,
}

impl Session {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            tunnel_name: row.get(1)?,
            created_at: row.get(2)?,
            port: row.get(3)?,
            process_id: row.get(4)?,
            command: row.get(5)?,
            log_path: row.get(6)?,
            stopped_at: row.get(7)?,
        })
    }
}

pub fn new_session_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL)",
    )?;
    for (name, sql) in MIGRATIONS {
        let applied: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM migrations WHERE name = ?1)",
            [name],
            |row| row.get(0),
        )?;
        if applied {
            continue;
        }
        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO migrations (name, applied_at) VALUES (?1, ?2)",
            params![name, Utc::now().to_rfc3339()],
        )?;
    }
    Ok(())
}

pub fn insert_session(conn: &Connection, id: &str, new: &NewSession) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, tunnel_name, created_at, port, process_id, command, log_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            new.tunnel_name,
            Utc::now().to_rfc3339(),
            new.port,
            new.process_id,
            new.command,
            new.log_path
        ],
    )?;
    Ok(())
}

pub fn active_sessions(conn: &Connection) -> Result<Vec<Session>> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions WHERE stopped_at IS NULL ORDER BY created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], Session::from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn mark_stopped(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET stopped_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), session_id],
    )?;
    Ok(())
}

pub fn find_sessions(conn: &Connection, target: &str, limit: usize) -> Result<Vec<Session>> {
    let sql = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions
         WHERE tunnel_name = ?1 OR id = ?1
         ORDER BY created_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![target, limit as i64], Session::from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn all_sessions(conn: &Connection, limit: usize) -> Result<Vec<Session>> {
    let sql = format!("SELECT {SESSION_COLUMNS} FROM sessions ORDER BY created_at DESC LIMIT ?1");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([limit as i64], Session::from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
