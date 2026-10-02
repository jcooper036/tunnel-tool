use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const PORT_PLACEHOLDER: &str = "{port}";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TunnelSpec {
    pub name: String,
    pub access_command: String,
    pub preferred_port_range: (u16, u16),
    #[serde(default)]
    pub auto_run_on_start: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check: Option<String>,
}

impl TunnelSpec {
    pub fn validate(&self) -> Result<()> {
        validate_name(&self.name)?;
        validate_command(&self.access_command)?;
        validate_range(self.preferred_port_range)?;
        self.health_check
            .as_deref()
            .map_or(Ok(()), validate_health_check)
    }

    pub fn health_url_for(&self, port: u16) -> Option<String> {
        self.health_check
            .as_ref()
            .map(|url| url.replace(PORT_PLACEHOLDER, &port.to_string()))
    }

    pub fn command_for(&self, port: u16) -> String {
        self.access_command
            .replace(PORT_PLACEHOLDER, &port.to_string())
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        bail!("name '{name}' must be non-empty [A-Za-z0-9_-]");
    }
    Ok(())
}

pub fn validate_command(command: &str) -> Result<()> {
    if !command.contains(PORT_PLACEHOLDER) {
        bail!("access_command must contain {PORT_PLACEHOLDER}");
    }
    Ok(())
}

pub fn validate_health_check(url: &str) -> Result<()> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("health_check must be an http(s) URL");
    }
    if !url.contains(PORT_PLACEHOLDER) {
        bail!("health_check must contain {PORT_PLACEHOLDER}");
    }
    Ok(())
}

pub fn validate_range((start, end): (u16, u16)) -> Result<()> {
    if start == 0 || start > end {
        bail!("preferred_port_range {start}-{end} is invalid");
    }
    Ok(())
}

pub fn spec_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.toml"))
}

pub fn load(path: &Path) -> Result<TunnelSpec> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let spec: TunnelSpec =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    spec.validate()
        .with_context(|| format!("validating {}", path.display()))?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if stem != spec.name {
        bail!(
            "{}: name '{}' must match file name",
            path.display(),
            spec.name
        );
    }
    Ok(spec)
}

pub fn load_all(dir: &Path) -> Result<Vec<TunnelSpec>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    paths.iter().map(|p| load(p)).collect()
}

pub fn find(dir: &Path, name: &str) -> Result<TunnelSpec> {
    let path = spec_path(dir, name);
    if !path.exists() {
        bail!("no tunnel spec '{name}' in {}", dir.display());
    }
    load(&path)
}

pub fn save_new(dir: &Path, spec: &TunnelSpec) -> Result<PathBuf> {
    spec.validate()?;
    let path = spec_path(dir, &spec.name);
    if path.exists() {
        bail!(
            "tunnel '{}' already exists at {}",
            spec.name,
            path.display()
        );
    }
    std::fs::write(&path, toml::to_string_pretty(spec)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}
