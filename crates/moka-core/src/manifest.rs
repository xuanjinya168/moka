use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

const BUILTIN_CATALOG: &str = include_str!("../../../manifests/catalog.json");

pub fn builtin_catalog_json() -> &'static str {
    BUILTIN_CATALOG
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareCategory {
    Runtime,
    BuildTool,
    Database,
    Cache,
    Web,
    Middleware,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareManifest {
    pub id: String,
    pub category: SoftwareCategory,
    pub name: String,
    pub version: String,
    pub description: String,
    pub mirrors: Vec<String>,
    #[serde(default)]
    pub mirrors_cn: Vec<String>,
    pub default_port: Option<u16>,
    pub is_service: bool,
    #[serde(default)]
    pub env_link_name: Option<String>,
    #[serde(default)]
    pub config_kind: Option<String>,
    #[serde(default)]
    pub init_cmd: Option<CommandSpec>,
    #[serde(default)]
    pub start_cmd: Option<CommandSpec>,
    #[serde(default)]
    pub stop_cmd: Option<CommandSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareStatusView {
    #[serde(flatten)]
    pub manifest: SoftwareManifest,
    pub installed: bool,
    pub running: bool,
    pub active_env: bool,
    pub port: Option<u16>,
    pub pid: Option<u32>,
    pub install_path: String,
}

pub fn load_catalog(override_path: Option<&Path>) -> Result<Vec<SoftwareManifest>> {
    if let Some(path) = override_path {
        if path.exists() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(list) = serde_json::from_str::<Vec<SoftwareManifest>>(&text) {
                    if !list.is_empty() {
                        return Ok(list);
                    }
                }
            }
        }
    }
    let list: Vec<SoftwareManifest> = serde_json::from_str(BUILTIN_CATALOG)?;
    Ok(list)
}

/// 安全校验远程拉取的软件清单，防止恶意源注入非法命令或路径穿越
pub fn validate_catalog_manifests(list: &[SoftwareManifest]) -> Result<()> {
    if list.is_empty() {
        return Err(anyhow!("软件清单不能为空"));
    }
    let mut seen_ids = std::collections::HashSet::new();
    for item in list {
        let id = item.id.trim();
        if id.is_empty()
            || id.contains("..")
            || id.contains('/')
            || id.contains('\\')
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(anyhow!("远程清单包含非法组件 ID: {}", item.id));
        }
        if !seen_ids.insert(id.to_string()) {
            return Err(anyhow!("远程清单包含重复组件 ID: {id}"));
        }
        for url in item.mirrors.iter().chain(item.mirrors_cn.iter()) {
            if !url.starts_with("https://") {
                return Err(anyhow!("组件 {id} 包含非 HTTPS 下载地址: {url}"));
            }
        }
        for cmd in [&item.init_cmd, &item.start_cmd, &item.stop_cmd]
            .into_iter()
            .flatten()
        {
            let first = cmd.program.trim();
            if first.contains("..") || first.starts_with('/') || first.starts_with('\\') {
                return Err(anyhow!("组件 {id} 包含不安全的命令路径: {first}"));
            }
            if first.len() >= 2 && first.as_bytes()[1] == b':' {
                return Err(anyhow!("组件 {id} 命令不允许使用绝对盘符路径: {first}"));
            }
        }
    }
    Ok(())
}

/// 从远程 JSON URL 列表拉取最新软件清单（仅允许 HTTPS，校验安全通过后覆盖本地 catalog.json）
pub async fn fetch_remote_catalog(urls: &[String], save_to: &Path) -> Result<Vec<SoftwareManifest>> {
    let client = reqwest::Client::builder()
        .user_agent("Moka-Catalog/0.1")
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    let mut last_err = None;
    for url in urls {
        let u = url.trim();
        if u.is_empty() {
            continue;
        }
        if !u.starts_with("https://") {
            last_err = Some(anyhow!("安全限制：远程软件源必须使用 HTTPS 协议 ({u})"));
            continue;
        }
        match client.get(u).send().await {
            Ok(resp) if resp.status().is_success() => match resp.text().await {
                Ok(text) => match serde_json::from_str::<Vec<SoftwareManifest>>(&text) {
                    Ok(list) => match validate_catalog_manifests(&list) {
                        Ok(()) => {
                            if let Some(parent) = save_to.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            std::fs::write(save_to, &text)?;
                            return Ok(list);
                        }
                        Err(e) => last_err = Some(anyhow!("远程清单安全校验未通过: {e}")),
                    },
                    Err(e) => last_err = Some(anyhow!("远程清单 JSON 解析失败: {e}")),
                },
                Err(e) => last_err = Some(e.into()),
            },
            Ok(resp) => last_err = Some(anyhow!("HTTP {} from {u}", resp.status())),
            Err(e) => last_err = Some(e.into()),
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("所有远程软件源均拉取失败")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub body: String,
}

/// 异步查询 GitHub Releases 最新版本
pub async fn check_latest_release(repo: &str) -> Result<ReleaseInfo> {
    let clean = repo.trim().trim_matches('/');
    if clean.is_empty() || !clean.contains('/') {
        return Err(anyhow!("仓库地址格式无效，应为 owner/repo"));
    }
    let url = format!("https://api.github.com/repos/{clean}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("Moka-Updater/0.1")
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(12))
        .build()?;
    let resp = client.get(&url).send().await?;
    if resp.status().as_u16() == 404 {
        return Err(anyhow!("仓库 {clean} 暂未发布正式 Release（当前为 Beta 版本）"));
    }
    if !resp.status().is_success() {
        return Err(anyhow!("检查更新失败: HTTP {}", resp.status()));
    }
    let text = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let body = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if tag_name.is_empty() {
        return Err(anyhow!("未获取到有效版本号"));
    }
    Ok(ReleaseInfo {
        tag_name,
        html_url,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn builtin_catalog_is_valid_and_unique() {
        let items = load_catalog(None).expect("内置 catalog.json 应能正确解析");
        assert!(items.len() >= 20, "内置软件目录应包含至少 20 个组件");

        let mut ids = HashSet::new();
        for item in &items {
            assert!(!item.id.trim().is_empty(), "组件 id 不能为空");
            assert!(ids.insert(item.id.clone()), "发现重复组件 id: {}", item.id);
            assert!(!item.name.trim().is_empty(), "组件 name 不能为空");
            assert!(!item.mirrors.is_empty(), "组件 {} 缺少国际官方源", item.id);
            if item.is_service {
                assert!(
                    item.start_cmd.is_some(),
                    "可启动服务 {} 必须配置 start_cmd",
                    item.id
                );
            }
        }
        validate_catalog_manifests(&items).expect("内置目录应全部通过安全校验");
    }

    #[test]
    fn validate_catalog_rejects_unsafe_entries() {
        let mut items = load_catalog(None).unwrap();
        items[0].start_cmd = Some(CommandSpec {
            program: "C:\\Windows\\System32\\calc.exe".into(),
            args: Vec::new(),
        });
        assert!(validate_catalog_manifests(&items).is_err());

        let mut items2 = load_catalog(None).unwrap();
        items2[0].id = "../escape".into();
        assert!(validate_catalog_manifests(&items2).is_err());
    }
}
