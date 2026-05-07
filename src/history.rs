use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub host: String,
    pub full_cmd: String,
}

/// Loads SSH-related hosts from the user's shell histories (zsh, bash, fish),
/// merged and sorted newest-first by timestamp where available, then deduplicated.
pub fn load_history() -> Vec<HistoryEntry> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };

    let mut all: Vec<(Option<u64>, String)> = Vec::new();

    if let Ok(bytes) = fs::read(home.join(".zsh_history")) {
        let s = String::from_utf8_lossy(&bytes);
        all.extend(parse_zsh(&s));
    }
    if let Ok(bytes) = fs::read(home.join(".bash_history")) {
        let s = String::from_utf8_lossy(&bytes);
        all.extend(parse_bash(&s));
    }
    if let Ok(bytes) = fs::read(home.join(".local/share/fish/fish_history")) {
        let s = String::from_utf8_lossy(&bytes);
        all.extend(parse_fish(&s));
    }

    // Newest-first by timestamp; entries without timestamps go to the end in stable order.
    all.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let mut entries: Vec<HistoryEntry> = Vec::new();
    for (_, cmd) in all {
        if let Some(host) = extract_host(&cmd) {
            entries.push(HistoryEntry {
                host,
                full_cmd: cmd,
            });
        }
    }

    // Dedup by full command, keeping the first (newest) occurrence.
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for entry in entries {
        if seen.insert(entry.full_cmd.clone()) {
            deduped.push(entry);
        }
    }
    deduped
}

/// Parse zsh history. Format: `: TIMESTAMP:0;COMMAND` or just `COMMAND`.
fn parse_zsh(content: &str) -> Vec<(Option<u64>, String)> {
    let mut out = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix(": ")
            && let Some((meta, cmd)) = rest.split_once(';')
        {
            let ts = meta.split_once(':').and_then(|(t, _)| t.parse().ok());
            out.push((ts, cmd.to_string()));
        } else if !line.is_empty() {
            out.push((None, line.to_string()));
        }
    }
    out
}

/// Parse bash history. With `HISTTIMEFORMAT` set, lines are preceded by `#<unix-ts>`.
/// Without it, each line is a plain command.
fn parse_bash(content: &str) -> Vec<(Option<u64>, String)> {
    let mut out = Vec::new();
    let mut pending_ts: Option<u64> = None;
    for line in content.lines() {
        if let Some(ts_str) = line.strip_prefix('#')
            && let Ok(ts) = ts_str.parse::<u64>()
        {
            pending_ts = Some(ts);
            continue;
        }
        if !line.is_empty() {
            out.push((pending_ts.take(), line.to_string()));
        }
    }
    out
}

/// Parse fish history. YAML-ish blocks:
///   - cmd: COMMAND
///     when: TIMESTAMP
///     paths:
///       - some/path
fn parse_fish(content: &str) -> Vec<(Option<u64>, String)> {
    let mut out = Vec::new();
    let mut current_cmd: Option<String> = None;
    let mut current_ts: Option<u64> = None;

    for line in content.lines() {
        if let Some(cmd) = line.strip_prefix("- cmd: ") {
            if let Some(c) = current_cmd.take() {
                out.push((current_ts.take(), c));
            }
            current_cmd = Some(unescape_fish(cmd));
        } else if let Some(rest) = line.strip_prefix("  when: ") {
            current_ts = rest.trim().parse().ok();
        }
    }
    if let Some(c) = current_cmd.take() {
        out.push((current_ts.take(), c));
    }
    out
}

/// Fish escapes backslashes as `\\` and embedded newlines as `\n` in the cmd field.
/// We collapse `\n` to a space (multi-line commands get joined) and unescape `\\` to `\`.
fn unescape_fish(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push(' '),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
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
    fn test_parse_zsh_with_timestamp() {
        let entries = parse_zsh(": 1234567890:0;ssh myhost\n");
        assert_eq!(entries, vec![(Some(1234567890), "ssh myhost".to_string())]);
    }

    #[test]
    fn test_parse_zsh_plain() {
        let entries = parse_zsh("ssh myhost\n");
        assert_eq!(entries, vec![(None, "ssh myhost".to_string())]);
    }

    #[test]
    fn test_parse_zsh_mixed() {
        let entries = parse_zsh(": 100:0;ssh a\n\n: 200:0;ssh b\nbarecmd\n");
        assert_eq!(
            entries,
            vec![
                (Some(100), "ssh a".to_string()),
                (Some(200), "ssh b".to_string()),
                (None, "barecmd".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_bash_no_timestamps() {
        let entries = parse_bash("ssh a\nssh b\n");
        assert_eq!(
            entries,
            vec![(None, "ssh a".to_string()), (None, "ssh b".to_string()),]
        );
    }

    #[test]
    fn test_parse_bash_with_timestamps() {
        let entries = parse_bash("#100\nssh a\n#200\nssh b\n");
        assert_eq!(
            entries,
            vec![
                (Some(100), "ssh a".to_string()),
                (Some(200), "ssh b".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_bash_pound_non_timestamp() {
        // `#` lines that aren't valid timestamps fall through as commands;
        // extract_host filters them out later if they're not ssh-family.
        let entries = parse_bash("#abc\nssh a\n");
        assert_eq!(
            entries,
            vec![(None, "#abc".to_string()), (None, "ssh a".to_string())]
        );
    }

    #[test]
    fn test_parse_fish_basic() {
        let content = "- cmd: ssh a\n  when: 100\n- cmd: ssh b\n  when: 200\n";
        let entries = parse_fish(content);
        assert_eq!(
            entries,
            vec![
                (Some(100), "ssh a".to_string()),
                (Some(200), "ssh b".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_fish_with_paths_block() {
        let content = "- cmd: ssh a\n  when: 100\n  paths:\n    - foo\n- cmd: ssh b\n  when: 200\n";
        let entries = parse_fish(content);
        assert_eq!(
            entries,
            vec![
                (Some(100), "ssh a".to_string()),
                (Some(200), "ssh b".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_fish_no_timestamp() {
        let content = "- cmd: ssh a\n";
        let entries = parse_fish(content);
        assert_eq!(entries, vec![(None, "ssh a".to_string())]);
    }

    #[test]
    fn test_unescape_fish() {
        assert_eq!(unescape_fish("plain"), "plain");
        assert_eq!(unescape_fish(r"with\\backslash"), "with\\backslash");
        assert_eq!(unescape_fish(r"line1\nline2"), "line1 line2");
        assert_eq!(unescape_fish(r"a\\b\nc"), "a\\b c");
        assert_eq!(unescape_fish(r"trailing\\"), "trailing\\");
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
