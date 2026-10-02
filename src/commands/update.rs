use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

const SOURCE_DIR: &str = env!("CARGO_MANIFEST_DIR");

fn capture(dir: &Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("running {program}"))?;
    if !output.status.success() {
        bail!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stream(dir: &Path, program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("running {program}"))?;
    if !status.success() {
        bail!("{program} {} exited with {status}", args.join(" "));
    }
    Ok(())
}

pub fn run() -> Result<()> {
    let dir = Path::new(SOURCE_DIR);
    if !dir.join("Cargo.toml").exists() {
        bail!(
            "source directory {} no longer exists; run `cargo install --path <clone>` from a clone",
            dir.display()
        );
    }
    if !capture(dir, "git", &["remote"])?.is_empty() {
        stream(dir, "git", &["pull", "--ff-only"])?;
    }
    stream(dir, "cargo", &["install", "--path", ".", "--locked"])?;
    let branch = capture(dir, "git", &["branch", "--show-current"])?;
    let commit = capture(dir, "git", &["rev-parse", "--short", "HEAD"])?;
    let dirty = if capture(dir, "git", &["status", "--porcelain"])?.is_empty() {
        ""
    } else {
        " (uncommitted changes)"
    };
    println!("installed {branch}@{commit}{dirty} from {}", dir.display());
    Ok(())
}
