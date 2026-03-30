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
    let (mut node, wildcards) = parse_config(&config_path)?;
    apply_defaults(&mut node, &wildcards);
    Ok(node)
}

fn parse_config(path: &Path) -> Result<(ConfigNode, Vec<HostEntry>)> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;

    let config_dir = path.parent().unwrap_or(Path::new("."));
    let mut hosts = Vec::new();
    let mut wildcards = Vec::new();
    let mut children = Vec::new();
    let mut current_host: Option<HostEntry> = None;
    let mut current_is_wildcard = false;

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
                    if current_is_wildcard {
                        wildcards.push(h);
                    } else {
                        hosts.push(h);
                    }
                }
                let is_wildcard = value.contains('*') || value.contains('?') || value.contains('!');
                // Skip internal entries starting with _
                if !is_wildcard && value.starts_with('_') {
                    continue;
                }
                current_is_wildcard = is_wildcard;
                current_host = Some(HostEntry {
                    name: value.clone(),
                    hostname: None,
                    user: None,
                    port: None,
                    identity_file: None,
                    proxy_jump: None,
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
            "identityfile" => {
                if let Some(ref mut h) = current_host {
                    h.identity_file = Some(value);
                }
            }
            "proxyjump" => {
                if let Some(ref mut h) = current_host {
                    h.proxy_jump = Some(value);
                }
            }
            "include" => {
                if let Some(h) = current_host.take() {
                    if current_is_wildcard {
                        wildcards.push(h);
                    } else {
                        hosts.push(h);
                    }
                }
                current_is_wildcard = false;
                let include_paths = resolve_include(config_dir, &value)?;
                for inc_path in include_paths {
                    match parse_config(&inc_path) {
                        Ok((child, child_wildcards)) => {
                            children.push(child);
                            wildcards.extend(child_wildcards);
                        }
                        Err(e) => eprintln!("warning: {e:#}"),
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(h) = current_host.take() {
        if current_is_wildcard {
            wildcards.push(h);
        } else {
            hosts.push(h);
        }
    }

    Ok((
        ConfigNode {
            path: path.to_path_buf(),
            hosts,
            children,
        },
        wildcards,
    ))
}

/// Check if a hostname matches an SSH Host pattern (space-separated, with `*`/`?` globs and `!` negation).
fn host_pattern_matches(pattern: &str, hostname: &str) -> bool {
    let mut matched = false;
    for pat in pattern.split_whitespace() {
        if let Some(negated) = pat.strip_prefix('!') {
            if glob_match(negated, hostname) {
                return false;
            }
        } else if glob_match(pat, hostname) {
            matched = true;
        }
    }
    matched
}

/// Simple glob match supporting `*` (any chars) and `?` (single char).
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0);

    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn apply_defaults(node: &mut ConfigNode, wildcards: &[HostEntry]) {
    for host in &mut node.hosts {
        for wc in wildcards {
            if !host_pattern_matches(&wc.name, &host.name) {
                continue;
            }
            if host.hostname.is_none() {
                host.hostname.clone_from(&wc.hostname);
            }
            if host.user.is_none() {
                host.user.clone_from(&wc.user);
            }
            if host.port.is_none() {
                host.port = wc.port;
            }
            if host.identity_file.is_none() {
                host.identity_file.clone_from(&wc.identity_file);
            }
            if host.proxy_jump.is_none() {
                host.proxy_jump.clone_from(&wc.proxy_jump);
            }
        }
    }
    for child in &mut node.children {
        apply_defaults(child, wildcards);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("JH_GW*", "JH_GW1"));
        assert!(glob_match("JH_GW*", "JH_GW123"));
        assert!(!glob_match("JH_GW*", "labvm"));
        assert!(!glob_match("JH_GW*", "JH_G"));
        assert!(glob_match("lt*", "ltnode"));
        assert!(!glob_match("lt*", "labvm"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("server?", "server1"));
        assert!(!glob_match("server?", "server12"));
        assert!(!glob_match("server?", "server"));
    }

    #[test]
    fn test_host_pattern_matches_multi() {
        assert!(host_pattern_matches("MainA* MainB*", "MainA1"));
        assert!(host_pattern_matches("MainA* MainB*", "MainB2"));
        assert!(!host_pattern_matches("MainA* MainB*", "MainC3"));
    }

    #[test]
    fn test_host_pattern_matches_negation() {
        assert!(host_pattern_matches("* !jumphost", "server1"));
        assert!(!host_pattern_matches("* !jumphost", "jumphost"));
    }

    #[test]
    fn test_apply_defaults_pattern_scoped() {
        let mut node = ConfigNode {
            path: PathBuf::from("test"),
            hosts: vec![
                HostEntry {
                    name: "JH_GW1".into(),
                    hostname: Some("10.15.1.100".into()),
                    user: None,
                    port: None,
                    identity_file: None,
                    proxy_jump: None,
                },
                HostEntry {
                    name: "labvm".into(),
                    hostname: Some("10.0.0.128".into()),
                    user: Some("root".into()),
                    port: None,
                    identity_file: None,
                    proxy_jump: None,
                },
            ],
            children: Vec::new(),
        };
        let wildcards = vec![HostEntry {
            name: "JH_GW*".into(),
            hostname: None,
            user: Some("root".into()),
            port: None,
            identity_file: Some("~/.ssh/JumpHost.pem".into()),
            proxy_jump: None,
        }];

        apply_defaults(&mut node, &wildcards);

        // JH_GW1 matches JH_GW* — should inherit
        assert_eq!(node.hosts[0].user.as_deref(), Some("root"));
        assert_eq!(
            node.hosts[0].identity_file.as_deref(),
            Some("~/.ssh/JumpHost.pem")
        );

        // labvm does NOT match JH_GW* — should NOT inherit
        assert_eq!(node.hosts[1].identity_file, None);
    }
}
