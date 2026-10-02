use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::cli::LogArgs;
use crate::commands::Ctx;
use crate::db::{self, Session};

const FOLLOW_INTERVAL: Duration = Duration::from_millis(200);

pub fn latest_session(ctx: &Ctx, target: &str) -> Result<Session> {
    match db::find_sessions(&ctx.conn, target, 1)?.into_iter().next() {
        Some(session) => Ok(session),
        None => bail!("no tunnel or session '{target}'"),
    }
}

pub fn read_log(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading log {}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

pub fn tail_lines(text: &str, count: usize) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].iter().map(|line| line.to_string()).collect()
}

fn follow(path: &Path, mut offset: u64) -> Result<()> {
    let mut stdout = std::io::stdout();
    loop {
        sleep(FOLLOW_INTERVAL);
        let mut file =
            File::open(path).with_context(|| format!("reading log {}", path.display()))?;
        let len = file.metadata()?.len();
        if len < offset {
            offset = 0;
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut chunk = Vec::new();
        file.read_to_end(&mut chunk)?;
        offset += chunk.len() as u64;
        stdout.write_all(&chunk)?;
        stdout.flush()?;
    }
}

pub fn run(ctx: &Ctx, args: LogArgs) -> Result<()> {
    let session = latest_session(ctx, &args.target)?;
    let path = Path::new(&session.log_path);
    let text = read_log(path)?;
    for line in tail_lines(&text, args.lines) {
        println!("{line}");
    }
    if args.follow {
        follow(path, text.len() as u64)?;
    }
    Ok(())
}
