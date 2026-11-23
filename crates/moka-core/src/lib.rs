pub mod downloader;
pub mod manifest;

use anyhow::Result;
use std::path::{Path, PathBuf};

/// 绿色化运行时根目录布局
#[derive(Debug, Clone)]
pub struct RuntimeLayout {
    pub root: PathBuf,
}

impl RuntimeLayout {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for sub in ["apps", "conf", "data", "logs", "current", "downloads", "manifests"] {
            std::fs::create_dir_all(self.root.join(sub))?;
        }
        Ok(())
    }

    pub fn catalog_file(&self) -> PathBuf {
        self.root.join("manifests").join("catalog.json")
    }

    pub fn catalog_source_url_file(&self) -> PathBuf {
        self.root.join("manifests").join("source_url.txt")
    }

    pub fn app_dir(&self, id: &str) -> PathBuf {
        self.root.join("apps").join(id)
    }

    pub fn conf_dir(&self, id: &str) -> PathBuf {
        self.root.join("conf").join(id)
    }

    pub fn data_dir(&self, id: &str) -> PathBuf {
        self.root.join("data").join(id)
    }

    pub fn log_file(&self, id: &str) -> PathBuf {
        self.root.join("logs").join(format!("{id}.log"))
    }

    pub fn current_link(&self, link_name: &str) -> PathBuf {
        self.root.join("current").join(link_name)
    }

    pub fn download_file(&self, id: &str) -> PathBuf {
        self.root.join("downloads").join(format!("{id}.zip"))
    }

    pub fn is_installed(&self, id: &str) -> bool {
        let dir = self.app_dir(id);
        dir.exists()
            && std::fs::read_dir(&dir)
                .map(|mut it| it.next().is_some())
                .unwrap_or(false)
    }

    pub fn root_path(&self) -> &Path {
        &self.root
    }
}
