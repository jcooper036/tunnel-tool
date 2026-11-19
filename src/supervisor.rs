//! One supervisor per `TUN_HOME`. It polls active sessions and relaunches the
//! ones whose process died or whose health check keeps failing, reusing the
//! session row (same id, port and log) so clients and logs stay continuous.
//! `tun stop` marks the session stopped, which removes it from supervision.

use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::TransactionBehavior;

use crate::cli::SupervisorAction;
use crate::commands::Ctx;
use crate::db::{self, Session};
use crate::health;
use crate::home::Home;
use crate::ports;
use crate::process::{self, Liveness};
use crate::spec::{self, RestartPolicy};

const TICK: Duration = Duration::from_secs(1);
const BACKOFF_BASE: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);
/// Continuous healthy time after which the backoff starts over.
const HEALTHY_RESET: Duration = Duration::from_secs(60);
/// How long a live-but-not-up process (port closed or health failing) is tolerated.
const UNHEALTHY_GRACE: Duration = Duration::from_secs(20);
const HEALTH_INTERVAL: Duration = Duration::from_secs(10);
const STOP_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
pub struct Backoff {
    failures: u32,
    next_attempt: Option<Instant>,
    up_since: Option<Instant>,
}

impl Backoff {
    pub fn on_up(&mut self, now: Instant) {
        let since = *self.up_since.get_or_insert(now);
        if now.duration_since(since) >= HEALTHY_RESET {
            self.failures = 0;
            self.next_attempt = None;
        }
    }

    pub fn on_down(&mut self) {
        self.up_since = None;
    }

    pub fn due(&self, now: Instant) -> bool {
        self.next_attempt.is_none_or(|at| now >= at)
    }

    /// Record a restart attempt and schedule the earliest next one.
    pub fn record_attempt(&mut self, now: Instant) {
        let delay = BACKOFF_BASE
            .saturating_mul(1u32.checked_shl(self.failures).unwrap_or(u32::MAX))
            .min(BACKOFF_CAP);
        self.failures = self.failures.saturating_add(1);
        self.next_attempt = Some(now + delay);
        self.up_since = None;
    }
}

struct Track {
    backoff: Backoff,
    bad_since: Option<Instant>,
    last_probe: Option<Instant>,
    probe_ok: bool,
}

impl Track {
    fn new() -> Self {
        Self {
            backoff: Backoff::default(),
            bad_since: None,
            last_probe: None,
            probe_ok: true,
        }
    }
}

fn is_healthy(
    ctx: &Ctx,
    session: &Session,
    live: Liveness,
    track: &mut Track,
    now: Instant,
) -> bool {
    if live != Liveness::Up {
        return false;
    }
    let url = spec::find(&ctx.home.tunnels_dir(), &session.tunnel_name)
        .ok()
        .and_then(|t| t.health_url_for(session.port));
    let Some(url) = url else { return true };
    if track
        .last_probe
        .is_none_or(|at| now.duration_since(at) >= HEALTH_INTERVAL)
    {
        track.probe_ok = health::probe(&url).ok;
        track.last_probe = Some(now);
    }
    track.probe_ok
}

fn log_line(path: &str, line: &str) {
    if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(path) {
        let _ = writeln!(file, "[tun] {} {line}", Utc::now().to_rfc3339());
    }
}

/// Replace the session's process, preferring its previous port. Runs in one
/// immediate transaction so `stop`, `heal` and other supervisors cannot interleave.
fn restart(ctx: &mut Ctx, session: &Session, live: Liveness) -> Result<()> {
    let spec = spec::find(&ctx.home.tunnels_dir(), &session.tunnel_name)?;
    let tx = ctx
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current = db::get_session(&tx, &session.id)?;
    if !current.is_some_and(|s| s.stopped_at.is_none() && s.process_id == session.process_id) {
        return Ok(());
    }
    if live != Liveness::Dead {
        process::terminate(session.process_id, STOP_GRACE);
    }
    let claimed: HashSet<u16> = db::active_sessions(&tx)?
        .iter()
        .filter(|s| s.id != session.id)
        .map(|s| s.port)
        .collect();
    let port = ports::ensure_free(session.port, &claimed)
        .or_else(|_| ports::pick(spec.preferred_port_range, &claimed))?;
    let command = spec.command_for(port);
    let reason = match live {
        Liveness::Dead => "process exited",
        _ => "health check failing",
    };
    log_line(
        &session.log_path,
        &format!(
            "restart #{}: {reason}; relaunching on port {port}",
            session.restarts + 1
        ),
    );
    let pid = process::spawn_detached(&command, std::path::Path::new(&session.log_path))?;
    db::record_restart(&tx, &session.id, session.process_id, pid, port, &command)?;
    tx.commit()?;
    Ok(())
}

/// Reap exited children (processes this supervisor relaunched) so they read as
/// dead instead of lingering as zombies that still answer `kill(pid, 0)`.
fn reap_children() {
    let mut status = 0;
    while unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) } > 0 {}
}

fn tick(ctx: &mut Ctx, tracks: &mut HashMap<String, Track>, now: Instant) -> Result<()> {
    reap_children();
    let sessions = db::active_sessions(&ctx.conn)?;
    tracks.retain(|id, _| sessions.iter().any(|s| &s.id == id));
    for session in &sessions {
        let policy = spec::find(&ctx.home.tunnels_dir(), &session.tunnel_name).map(|s| s.restart);
        if !matches!(policy, Ok(RestartPolicy::OnFailure)) {
            continue;
        }
        let live = process::liveness(session.process_id, session.port);
        let track = tracks.entry(session.id.clone()).or_insert_with(Track::new);
        if is_healthy(ctx, session, live, track, now) {
            track.backoff.on_up(now);
            track.bad_since = None;
            continue;
        }
        track.backoff.on_down();
        if live != Liveness::Dead {
            let since = *track.bad_since.get_or_insert(now);
            if now.duration_since(since) < UNHEALTHY_GRACE {
                continue;
            }
        }
        if !track.backoff.due(now) {
            continue;
        }
        track.backoff.record_attempt(now);
        track.bad_since = None;
        track.last_probe = None;
        if let Err(err) = restart(ctx, session, live) {
            let msg = format!("restart failed: {err:#}");
            eprintln!("{} (session {}): {msg}", session.tunnel_name, session.id);
            log_line(&session.log_path, &msg);
        }
    }
    Ok(())
}

fn open_lock(home: &Home) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(home.supervisor_lock_path())
        .context("opening supervisor lock")
}

fn try_lock(file: &File) -> bool {
    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

/// pid of the running supervisor, if any. The lock is held for its lifetime.
pub fn running_pid(home: &Home) -> Result<Option<u32>> {
    let lock = open_lock(home)?;
    if try_lock(&lock) {
        return Ok(None);
    }
    let mut text = String::new();
    File::open(home.supervisor_pid_path())
        .and_then(|mut f| f.read_to_string(&mut text))
        .context("reading supervisor pid")?;
    Ok(text.trim().parse().ok())
}

/// Start the supervisor unless one is already running. Failure is reported on
/// stderr only: the command that called this has its own job to finish.
pub fn ensure_running(home: &Home) {
    if let Err(err) = spawn_if_needed(home) {
        eprintln!("warning: could not start supervisor: {err:#}");
    }
}

fn spawn_if_needed(home: &Home) -> Result<()> {
    if running_pid(home)?.is_some() {
        return Ok(());
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.supervisor_log_path())?;
    // A concurrent spawn is harmless: the loser fails to take the lock and exits.
    Command::new(std::env::current_exe()?)
        .arg("--home")
        .arg(&home.root)
        .args(["supervisor", "run"])
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        .process_group(0)
        .spawn()
        .context("spawning supervisor")?;
    Ok(())
}

fn run_loop(ctx: &mut Ctx) -> Result<()> {
    let lock = open_lock(&ctx.home)?;
    if !try_lock(&lock) {
        return Ok(());
    }
    std::fs::write(
        ctx.home.supervisor_pid_path(),
        std::process::id().to_string(),
    )?;
    let mut tracks = HashMap::new();
    // Exit when the state directory is deleted (e.g. a test's temp home).
    while ctx.home.db_path().exists() {
        if let Err(err) = tick(ctx, &mut tracks, Instant::now()) {
            eprintln!("supervisor tick failed: {err:#}");
        }
        sleep(TICK);
    }
    drop(lock);
    Ok(())
}

fn stop(home: &Home) -> Result<()> {
    let Some(pid) = running_pid(home)? else {
        println!("supervisor not running");
        return Ok(());
    };
    unsafe { libc::kill(pid as i32, libc::SIGTERM) };
    let deadline = Instant::now() + STOP_GRACE;
    while process::is_alive(pid) && Instant::now() < deadline {
        sleep(Duration::from_millis(50));
    }
    if process::is_alive(pid) {
        bail!("supervisor (pid {pid}) did not exit");
    }
    println!("stopped supervisor (pid {pid}); dead tunnels will no longer be restarted");
    Ok(())
}

pub fn run(ctx: &mut Ctx, action: SupervisorAction) -> Result<()> {
    match action {
        SupervisorAction::Run => run_loop(ctx),
        SupervisorAction::Stop => stop(&ctx.home),
        SupervisorAction::Status => {
            match running_pid(&ctx.home)? {
                Some(pid) => println!("supervisor running (pid {pid})"),
                None => println!("supervisor not running"),
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_to_cap_and_gates_attempts() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        assert!(b.due(t0));
        let mut now = t0;
        let mut delays = Vec::new();
        for _ in 0..8 {
            b.record_attempt(now);
            let next = b.next_attempt.unwrap();
            assert!(!b.due(now));
            delays.push(next.duration_since(now).as_secs());
            now = next;
            assert!(b.due(now));
        }
        assert_eq!(delays, [1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn backoff_resets_only_after_sustained_health() {
        let t0 = Instant::now();
        let mut b = Backoff::default();
        b.record_attempt(t0);
        b.record_attempt(t0);
        // A blip of health, then down again: no reset.
        b.on_up(t0);
        b.on_up(t0 + HEALTHY_RESET - Duration::from_secs(1));
        b.on_down();
        assert_eq!(b.failures, 2);
        // Sustained health resets.
        let t1 = t0 + Duration::from_secs(100);
        b.on_up(t1);
        b.on_up(t1 + HEALTHY_RESET);
        assert_eq!(b.failures, 0);
        assert!(b.due(t1));
    }
}
