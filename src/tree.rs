use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HostEntry {
    pub name: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigNode {
    pub path: PathBuf,
    pub hosts: Vec<HostEntry>,
    pub children: Vec<ConfigNode>,
}

#[derive(Debug, Clone)]
pub enum FlatItemKind {
    File,
    Host,
}

#[derive(Debug, Clone)]
pub struct FlatItem {
    pub kind: FlatItemKind,
    pub label: String,
    pub depth: usize,
    /// For Host items, the hostname to connect to (falls back to name)
    pub connect_host: Option<String>,
    /// For history items, the full original command line
    pub full_cmd: Option<String>,
    /// Informational hint (e.g., user:key:jump_host) for config entries
    pub info: Option<String>,
}

/// Build informational string: [user@]<ip>[:<key_file_name>][:<jump_host>]
fn build_host_info(host: &HostEntry) -> Option<String> {
    let hostname = host.hostname.as_deref();
    // If hostname is same as name or absent, only show if there are other fields
    let has_extra = host.identity_file.is_some() || host.proxy_jump.is_some();
    if hostname.is_none() && host.user.is_none() && !has_extra {
        return None;
    }

    let mut result = String::new();
    // user@ip or just ip (or just user@ if no hostname differs)
    if let Some(ref user) = host.user {
        result.push_str(user);
        result.push('@');
    }
    if let Some(ip) = hostname {
        result.push_str(ip);
    }
    if let Some(ref identity) = host.identity_file {
        let name = std::path::Path::new(identity)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(identity);
        result.push('/');
        result.push_str(name);
    }
    if let Some(ref jump) = host.proxy_jump {
        result.push(':');
        result.push_str(jump);
    }
    if result.is_empty() { None } else { Some(result) }
}

impl ConfigNode {
    pub fn flatten(&self) -> Vec<FlatItem> {
        let mut items = Vec::new();
        self.flatten_inner(0, &mut items);
        items
    }

    fn flatten_inner(&self, depth: usize, items: &mut Vec<FlatItem>) {
        let label = match self.path.file_name().and_then(|f| f.to_str()) {
            Some("config") if depth == 0 => "config".to_string(),
            Some("config") => self
                .path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .unwrap_or("config")
                .to_string(),
            Some(name) => name.to_string(),
            None => self.path.display().to_string(),
        };
        items.push(FlatItem {
            kind: FlatItemKind::File,
            label,
            depth,
            connect_host: None,
            full_cmd: None,
            info: None,
        });

        for host in &self.hosts {
            let info = build_host_info(host);
            items.push(FlatItem {
                kind: FlatItemKind::Host,
                label: host.name.clone(),
                depth: depth + 1,
                connect_host: Some(host.name.clone()),
                full_cmd: None,
                info,
            });
        }

        for child in &self.children {
            child.flatten_inner(depth + 1, items);
        }
    }
}
