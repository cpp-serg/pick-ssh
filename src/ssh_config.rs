use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use glob::glob;

use crate::tree::{ConfigNode, HostEntry};

pub fn load_ssh_config() -> Result<ConfigNode> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let config_path = home.join(".ssh/config");
    if !config_path.exists() {
        return Ok(ConfigNode {
            path: config_path,
            hosts: Vec::new(),
            children: Vec::new(),
        });
    }
    parse_config(&config_path)
}

pub fn parse_config(path: &Path) -> Result<ConfigNode> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let config_dir = path.parent().unwrap_or(Path::new("."));
    let mut hosts = Vec::new();
    let mut children = Vec::new();
    let mut current_host: Option<HostEntry> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (key, value) = match trimmed.split_once(|c: char| c.is_whitespace() || c == '=') {
            Some((k, v)) => (
                k.to_lowercase(),
                v.trim_start_matches('=').trim().to_string(),
            ),
            None => continue,
        };

        match key.as_str() {
            "host" => {
                if let Some(h) = current_host.take() {
                    hosts.push(h);
                }
                // Skip wildcard-only patterns
                if value.contains('*') || value.contains('?') || value.contains('!') {
                    continue;
                }
                // Skip internal entries starting with _
                if value.starts_with('_') {
                    continue;
                }
                current_host = Some(HostEntry {
                    name: value.clone(),
                    hostname: None,
                    user: None,
                    port: None,
                });
            }
            "hostname" => {
                if let Some(ref mut h) = current_host {
                    h.hostname = Some(value);
                }
            }
            "user" => {
                if let Some(ref mut h) = current_host {
                    h.user = Some(value);
                }
            }
            "port" => {
                if let Some(ref mut h) = current_host {
                    h.port = value.parse().ok();
                }
            }
            "include" => {
                if let Some(h) = current_host.take() {
                    hosts.push(h);
                }
                let include_paths = resolve_include(config_dir, &value)?;
                for inc_path in include_paths {
                    match parse_config(&inc_path) {
                        Ok(child) => children.push(child),
                        Err(e) => eprintln!("warning: {e:#}"),
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(h) = current_host.take() {
        hosts.push(h);
    }

    Ok(ConfigNode {
        path: path.to_path_buf(),
        hosts,
        children,
    })
}

fn resolve_include(config_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut all_paths = Vec::new();

    for sub_pattern in pattern.split_whitespace() {
        let full_pattern = if sub_pattern.starts_with('/') || sub_pattern.starts_with('~') {
            if let Some(rest) = sub_pattern.strip_prefix("~/") {
                let home = dirs::home_dir().context("cannot determine home directory")?;
                home.join(rest).display().to_string()
            } else {
                sub_pattern.to_string()
            }
        } else {
            config_dir.join(sub_pattern).display().to_string()
        };

        let is_glob =
            sub_pattern.contains('*') || sub_pattern.contains('?') || sub_pattern.contains('[');

        let mut paths: Vec<PathBuf> = glob(&full_pattern)
            .with_context(|| format!("invalid include pattern: {full_pattern}"))?
            .filter_map(|entry| entry.ok())
            .filter(|p| p.is_file())
            .collect();

        if is_glob {
            paths.sort();
        }

        all_paths.extend(paths);
    }

    Ok(all_paths)
}
