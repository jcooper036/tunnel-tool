use std::path::PathBuf;

use anyhow::{Context, Result};

pub struct Home {
    pub root: PathBuf,
}

impl Home {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn resolve(override_root: Option<PathBuf>) -> Result<Self> {
        let root = match override_root {
            Some(root) => root,
            None => dirs::home_dir().context("no home directory")?.join(".tun"),
        };
        Ok(Self::new(root))
    }

    pub fn tunnels_dir(&self) -> PathBuf {
        self.root.join("tunnels")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join("tun.db")
    }

    pub fn supervisor_lock_path(&self) -> PathBuf {
        self.root.join("supervisor.lock")
    }

    pub fn supervisor_pid_path(&self) -> PathBuf {
        self.root.join("supervisor.pid")
    }

    pub fn supervisor_log_path(&self) -> PathBuf {
        self.root.join("supervisor.log")
    }

    pub fn ensure(&self) -> Result<()> {
        for dir in [self.tunnels_dir(), self.logs_dir()] {
            std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        Ok(())
    }
}
