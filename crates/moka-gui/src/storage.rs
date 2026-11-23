use moka_config::SiteConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// 全局 Beta 诊断日志：统一写入 Moka-Runtime/logs/moka-app.log（立即 flush + 5 MB 自动轮转）
pub fn write_app_log(root: &Path, level: &str, msg: &str) {
    let log_dir = root.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("moka-app.log");
    const MAX_APP_LOG_BYTES: u64 = 5 * 1024 * 1024;
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() >= MAX_APP_LOG_BYTES {
            let rotated = log_dir.join("moka-app.log.1");
            let _ = std::fs::remove_file(&rotated);
            let _ = std::fs::rename(&log_path, &rotated);
        }
    }
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(f, "[{ts}] [{level}] {msg}");
        let _ = f.flush();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrayStyle {
    #[default]
    Panel,
    Menu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub dark_mode: bool,
    #[serde(default)]
    pub mirror_china: bool,
    #[serde(default = "default_true")]
    pub stop_on_exit: bool,
    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub tray_style: TrayStyle,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub auto_start_preset: String,
    #[serde(default = "default_release_repo")]
    pub release_repo: String,
    #[serde(default)]
    pub db_passwords: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

fn default_release_repo() -> String {
    "xuanjinya168/moka".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: false,
            mirror_china: false,
            stop_on_exit: true,
            minimize_to_tray: true,
            tray_style: TrayStyle::Panel,
            auto_start: false,
            auto_start_preset: String::new(),
            release_repo: default_release_repo(),
            db_passwords: HashMap::new(),
        }
    }
}

pub fn load_settings(root: &Path) -> AppSettings {
    let path = root.join("conf").join("settings.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<AppSettings>(&text) {
            return cfg;
        }
    }
    AppSettings::default()
}

pub fn save_settings(root: &Path, settings: &AppSettings) {
    let conf_dir = root.join("conf");
    let _ = std::fs::create_dir_all(&conf_dir);
    let path = conf_dir.join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = std::fs::write(path, json);
    }
}

pub fn calc_dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            total += calc_dir_size(&p);
        } else if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    total
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

pub fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h {m}m")
        }
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        format!("{d}d {h}h")
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePreset {
    pub name: String,
    pub service_ids: Vec<String>,
}

fn default_presets() -> Vec<ServicePreset> {
    vec![
        ServicePreset {
            name: "经典 Web 全栈组".to_string(),
            service_ids: vec![
                "mysql-8.4".to_string(),
                "redis-7.2".to_string(),
                "nginx-1.26".to_string(),
            ],
        },
        ServicePreset {
            name: "现代 PG 数据组".to_string(),
            service_ids: vec![
                "postgres-17".to_string(),
                "redis-7.2".to_string(),
                "nginx-1.26".to_string(),
            ],
        },
    ]
}

pub fn load_presets(root: &Path) -> Vec<ServicePreset> {
    let path = root.join("conf").join("presets.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<ServicePreset>>(&text) {
            return list;
        }
    }
    default_presets()
}

pub fn save_presets(root: &Path, presets: &[ServicePreset]) -> std::io::Result<()> {
    let conf_dir = root.join("conf");
    std::fs::create_dir_all(&conf_dir)?;
    let path = conf_dir.join("presets.json");
    let json = serde_json::to_string_pretty(presets).unwrap_or_else(|_| "[]".into());
    std::fs::write(path, json)
}

pub fn load_sites(root: &Path) -> Vec<SiteConfig> {
    let path = root.join("conf").join("sites.json");
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(list) = serde_json::from_str::<Vec<SiteConfig>>(&text) {
            return list;
        }
    }
    Vec::new()
}

pub fn save_sites(root: &Path, sites: &[SiteConfig]) -> std::io::Result<()> {
    let conf_dir = root.join("conf");
    std::fs::create_dir_all(&conf_dir)?;
    let path = conf_dir.join("sites.json");
    let json = serde_json::to_string_pretty(sites).unwrap_or_else(|_| "[]".into());
    std::fs::write(path, json)
}

