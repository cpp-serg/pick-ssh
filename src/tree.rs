use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct HostEntry {
    pub name: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
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
        });

        for host in &self.hosts {
            items.push(FlatItem {
                kind: FlatItemKind::Host,
                label: host.name.clone(),
                depth: depth + 1,
                connect_host: Some(host.name.clone()),
                full_cmd: None,
            });
        }

        for child in &self.children {
            child.flatten_inner(depth + 1, items);
        }
    }
}
