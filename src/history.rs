use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub host: String,
    pub full_cmd: String,
}

/// Loads SSH-related hosts from ~/.zsh_history, sorted by recency (most recent first),
/// deduplicated (keeping most recent occurrence).
pub fn load_history() -> Vec<HistoryEntry> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let path = home.join(".zsh_history");
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let content = String::from_utf8_lossy(&data);

    let mut entries = Vec::new();
    for line in content.lines() {
        let cmd = parse_history_line(line);
        if let Some(host) = extract_host(cmd) {
            entries.push(HistoryEntry {
                host,
                full_cmd: cmd.to_string(),
            });
        }
    }

    // Deduplicate by full command keeping most recent (last occurrence)
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for entry in entries.into_iter().rev() {
        if seen.insert(entry.full_cmd.clone()) {
            deduped.push(entry);
        }
    }
    deduped.reverse();

    // Most recent first
    deduped.reverse();
    deduped
}

/// Parse a zsh history line. Format: `: TIMESTAMP:0;COMMAND` or just `COMMAND`.
fn parse_history_line(line: &str) -> &str {
    if let Some(rest) = line.strip_prefix(": ") {
        // `: 1234567890:0;actual command`
        if let Some(pos) = rest.find(';') {
            return &rest[pos + 1..];
        }
    }
    line
}

// SSH flags that consume the next argument (so we skip that arg as a host candidate)
const FLAGS_WITH_ARG: &[&str] = &[
    "-p", "-l", "-i", "-o", "-F", "-c", "-b", "-D", "-e", "-L", "-m", "-O", "-R", "-S", "-W", "-w",
    "-E", "-J", "-Q",
];

fn extract_host(cmd: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let binary = parts[0].rsplit('/').next().unwrap_or(parts[0]);

    match binary {
        "ssh" | "sftp" => extract_ssh_host(&parts[1..]),
        "scp" => extract_copy_host(&parts[1..]),
        "rsync" => extract_copy_host(&parts[1..]),
        _ => None,
    }
}

/// For ssh/sftp: find the first non-flag positional argument.
fn extract_ssh_host(args: &[&str]) -> Option<String> {
    let mut skip_next = false;
    for &arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if FLAGS_WITH_ARG.contains(&arg) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        // This is the host (possibly user@host)
        let host = strip_user_prefix_if_needed(arg);
        return Some(host);
    }
    None
}

/// For scp/rsync: find an argument containing `:` (remote path spec), extract the host part.
fn extract_copy_host(args: &[&str]) -> Option<String> {
    let mut skip_next = false;
    for &arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if FLAGS_WITH_ARG.contains(&arg) {
            skip_next = true;
            continue;
        }
        if arg.starts_with('-') {
            continue;
        }
        // Look for user@host:path or host:path
        if let Some(colon_pos) = arg.find(':') {
            let host_part = &arg[..colon_pos];
            if !host_part.is_empty() {
                return Some(host_part.to_string());
            }
        }
    }
    None
}

/// Keep user@host as-is so the label shows who you connected as.
fn strip_user_prefix_if_needed(arg: &str) -> String {
    arg.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_history_line_zsh_format() {
        assert_eq!(
            parse_history_line(": 1234567890:0;ssh myhost"),
            "ssh myhost"
        );
    }

    #[test]
    fn test_parse_history_line_plain() {
        assert_eq!(parse_history_line("ssh myhost"), "ssh myhost");
    }

    #[test]
    fn test_extract_ssh_host() {
        assert_eq!(extract_host("ssh myhost"), Some("myhost".to_string()));
    }

    #[test]
    fn test_extract_ssh_user_host() {
        assert_eq!(
            extract_host("ssh user@myhost"),
            Some("user@myhost".to_string())
        );
    }

    #[test]
    fn test_extract_ssh_with_flags() {
        assert_eq!(
            extract_host("ssh -p 22 -i ~/.ssh/key myhost"),
            Some("myhost".to_string())
        );
    }

    #[test]
    fn test_extract_sftp() {
        assert_eq!(extract_host("sftp myhost"), Some("myhost".to_string()));
    }

    #[test]
    fn test_extract_scp() {
        assert_eq!(
            extract_host("scp file.txt myhost:/tmp/"),
            Some("myhost".to_string())
        );
    }

    #[test]
    fn test_extract_scp_user() {
        assert_eq!(
            extract_host("scp file.txt user@myhost:/tmp/"),
            Some("user@myhost".to_string())
        );
    }

    #[test]
    fn test_extract_rsync() {
        assert_eq!(
            extract_host("rsync -avz ./dir user@server:/path"),
            Some("user@server".to_string())
        );
    }

    #[test]
    fn test_extract_unknown_command() {
        assert_eq!(extract_host("ls -la"), None);
    }

    #[test]
    fn test_extract_empty() {
        assert_eq!(extract_host(""), None);
    }
}
