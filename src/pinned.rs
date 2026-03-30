use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pinned: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("pick-ssh.yaml")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    let data = match fs::read_to_string(&path) {
        Ok(d) => d,
        Err(_) => return AppConfig::default(),
    };
    serde_yaml::from_str(&data).unwrap_or_default()
}

fn save_config(cfg: &AppConfig) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(yaml) = serde_yaml::to_string(cfg) {
        let _ = fs::write(&path, yaml);
    }
}

pub fn save_pinned(pinned: &[String]) {
    let mut cfg = load_config();
    cfg.pinned = pinned.to_vec();
    save_config(&cfg);
}

pub fn save_theme(theme_name: &str) {
    let mut cfg = load_config();
    cfg.theme = Some(theme_name.to_string());
    save_config(&cfg);
}
