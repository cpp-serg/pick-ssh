use std::collections::HashSet;
use std::fs;

/// Loads hostnames from `~/.ssh/known_hosts` (and `known_hosts2` if present).
/// Hashed entries (lines starting with `|`) are silently skipped — they cannot
/// be displayed. Entries are deduplicated, preserving first-seen order.
pub fn load_known_hosts() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut hosts = Vec::new();
    for fname in [".ssh/known_hosts", ".ssh/known_hosts2"] {
        if let Ok(content) = fs::read_to_string(home.join(fname)) {
            hosts.extend(parse(&content));
        }
    }
    let mut seen = HashSet::new();
    hosts.retain(|h| seen.insert(h.clone()));
    hosts
}

fn parse(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('|') {
            continue;
        }
        let Some(spec) = line.split_whitespace().next() else {
            continue;
        };
        // @-prefixed markers like `@cert-authority` or `@revoked`: the host spec is
        // the *next* whitespace-separated field, not the first.
        let spec = if spec.starts_with('@') {
            let mut it = line.split_whitespace();
            it.next();
            match it.next() {
                Some(s) => s,
                None => continue,
            }
        } else {
            spec
        };
        for entry in spec.split(',') {
            if let Some(h) = extract_hostname(entry) {
                out.push(h.to_string());
            }
        }
    }
    out
}

/// Extract the hostname from a known_hosts entry. Handles `[host]:port`,
/// bare `host`, and IPv6 in `[fe80::1]:port` form. Skips empty inputs.
fn extract_hostname(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        let host = &rest[..end];
        return if host.is_empty() { None } else { Some(host) };
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain() {
        let entries = parse("server.example.com ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["server.example.com"]);
    }

    #[test]
    fn test_parse_comma_aliases() {
        let entries = parse("server.example.com,1.2.3.4 ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["server.example.com", "1.2.3.4"]);
    }

    #[test]
    fn test_parse_bracketed_port() {
        let entries = parse("[server.example.com]:2222 ssh-ed25519 AAAA...\n");
        assert_eq!(entries, vec!["server.example.com"]);
    }

    #[test]
    fn test_parse_mixed_alias_and_bracketed() {
        let entries = parse("[server.example.com]:2222,[1.2.3.4]:2222 ssh-ed25519 AAAA...\n");
        assert_eq!(entries, vec!["server.example.com", "1.2.3.4"]);
    }

    #[test]
    fn test_parse_skips_hashed() {
        let entries = parse("|1|abc=|def= ssh-rsa AAAA...\n");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_skips_comment() {
        let entries = parse("# this is a comment\nserver.example.com ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["server.example.com"]);
    }

    #[test]
    fn test_parse_at_marker() {
        let entries = parse("@cert-authority *.example.com ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["*.example.com"]);
    }

    #[test]
    fn test_parse_revoked() {
        let entries = parse("@revoked compromised.example.com ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["compromised.example.com"]);
    }

    #[test]
    fn test_parse_ipv6_bracketed() {
        let entries = parse("[fe80::1]:22 ssh-rsa AAAA...\n");
        assert_eq!(entries, vec!["fe80::1"]);
    }

    #[test]
    fn test_parse_mixed_input() {
        let input = "\
# comment line
server1 ssh-rsa AAAA...
|1|hash=|hash= ssh-rsa AAAA...
[server2]:2222,1.2.3.4 ssh-ed25519 AAAA...

server3 ssh-rsa AAAA...
";
        let entries = parse(input);
        assert_eq!(entries, vec!["server1", "server2", "1.2.3.4", "server3"]);
    }
}
