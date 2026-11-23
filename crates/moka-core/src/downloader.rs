use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::Serialize;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub id: String,
    pub stage: String, // "downloading" | "extracting" | "done" | "error"
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: f32,
    pub message: String,
}

pub async fn download_and_extract<F>(
    id: &str,
    mirrors: &[String],
    zip_dest: &Path,
    extract_dest: &Path,
    on_progress: F,
) -> Result<()>
where
    F: FnMut(DownloadProgress) + Send,
{
    download_and_extract_cancellable(
        id,
        mirrors,
        zip_dest,
        extract_dest,
        Arc::new(AtomicBool::new(false)),
        on_progress,
    )
    .await
}

pub async fn download_and_extract_cancellable<F>(
    id: &str,
    mirrors: &[String],
    zip_dest: &Path,
    extract_dest: &Path,
    cancel: Arc<AtomicBool>,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(DownloadProgress) + Send,
{
    if mirrors.is_empty() {
        return Err(anyhow!("软件 {id} 未配置下载地址"));
    }

    if let Some(parent) = zip_dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let client = reqwest::Client::builder()
        .user_agent("Moka/0.1")
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut last_err = None;
    let mut downloaded_ok = false;

    for url in mirrors {
        if cancel.load(Ordering::Relaxed) {
            let _ = tokio::fs::remove_file(zip_dest).await;
            return Err(anyhow!("已取消下载"));
        }
        on_progress(DownloadProgress {
            id: id.to_string(),
            stage: "downloading".into(),
            downloaded: 0,
            total: None,
            percent: 0.0,
            message: format!("正在连接镜像源: {url}"),
        });

        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let total = resp.content_length();
                let mut file = tokio::fs::File::create(zip_dest).await?;
                let mut stream = resp.bytes_stream();
                let mut downloaded: u64 = 0;
                let mut last_emit: u64 = 0;
                let mut header_buf: Vec<u8> = Vec::with_capacity(4);
                let mut stream_err: Option<anyhow::Error> = None;

                while let Some(chunk_res) = stream.next().await {
                    if cancel.load(Ordering::Relaxed) {
                        drop(file);
                        let _ = tokio::fs::remove_file(zip_dest).await;
                        return Err(anyhow!("已取消下载"));
                    }
                    let chunk = match chunk_res {
                        Ok(c) => c,
                        Err(e) => {
                            stream_err = Some(e.into());
                            break;
                        }
                    };
                    if header_buf.len() < 4 {
                        let need = 4 - header_buf.len();
                        let take = need.min(chunk.len());
                        header_buf.extend_from_slice(&chunk[..take]);
                        if header_buf.len() == 4 && header_buf.as_slice() != b"PK\x03\x04" {
                            stream_err = Some(anyhow!(
                                "镜像源返回的不是有效 ZIP 压缩包（可能为错误页或上游损坏文件）: {url}"
                            ));
                            break;
                        }
                    }
                    file.write_all(&chunk).await?;
                    downloaded += chunk.len() as u64;

                    if downloaded - last_emit >= 256 * 1024 || total == Some(downloaded) {
                        last_emit = downloaded;
                        let percent = total
                            .filter(|&t| t > 0)
                            .map(|t| (downloaded as f32 / t as f32) * 100.0)
                            .unwrap_or(0.0);
                        on_progress(DownloadProgress {
                            id: id.to_string(),
                            stage: "downloading".into(),
                            downloaded,
                            total,
                            percent,
                            message: format!(
                                "已下载 {:.1} MB{}",
                                downloaded as f64 / 1024.0 / 1024.0,
                                total
                                    .map(|t| format!(" / {:.1} MB", t as f64 / 1024.0 / 1024.0))
                                    .unwrap_or_default()
                            ),
                        });
                    }
                }
                file.flush().await?;
                drop(file);

                if let Some(err) = stream_err {
                    let _ = tokio::fs::remove_file(zip_dest).await;
                    last_err = Some(err);
                    continue;
                }
                if header_buf.len() < 4 || header_buf.as_slice() != b"PK\x03\x04" {
                    let _ = tokio::fs::remove_file(zip_dest).await;
                    last_err = Some(anyhow!("下载内容不完整或非有效 ZIP 文件: {url}"));
                    continue;
                }
                downloaded_ok = true;
                break;
            }
            Ok(resp) => {
                last_err = Some(anyhow!("HTTP {} from {}", resp.status(), url));
            }
            Err(e) => {
                last_err = Some(e.into());
            }
        }
    }

    if !downloaded_ok {
        return Err(last_err.unwrap_or_else(|| anyhow!("所有镜像源下载失败")));
    }

    on_progress(DownloadProgress {
        id: id.to_string(),
        stage: "extracting".into(),
        downloaded: 0,
        total: None,
        percent: 99.0,
        message: "下载完成，正在解压到绿色目录...".into(),
    });

    let zip_path = zip_dest.to_path_buf();
    let target_dir = extract_dest.to_path_buf();
    tokio::task::spawn_blocking(move || extract_zip_strip_root(&zip_path, &target_dir))
        .await
        .context("解压线程异常")??;

    // 清理临时压缩包释放磁盘空间
    let _ = fs::remove_file(zip_dest);

    on_progress(DownloadProgress {
        id: id.to_string(),
        stage: "done".into(),
        downloaded: 0,
        total: None,
        percent: 100.0,
        message: "安装完成".into(),
    });

    Ok(())
}

/// 校验 ZIP 条目相对路径，严格阻止 Zip Slip（`..`、绝对路径、Windows 盘符前缀）
fn sanitize_zip_rel_path(rel: &str) -> Result<Option<PathBuf>> {
    let trimmed = rel.trim_start_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }
    let path = Path::new(trimmed);
    let mut clean = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.contains(':') {
                    return Err(anyhow!("检测到非法 ZIP 路径（含盘符）: {rel}"));
                }
                clean.push(seg);
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(anyhow!("检测到不安全的 ZIP 路径穿越条目: {rel}"));
            }
        }
    }
    if clean.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(clean))
    }
}

/// 智能安全解压：
/// 1. 严格校验每个 ZIP 条目路径（防 Zip Slip 路径穿越）；
/// 2. 先完整解压到同级 `.moka_tmp` 暂存目录，全部成功后再原子替换目标目录，
///    避免解压中途失败留下残缺目录并误判为已安装。
pub fn extract_zip_strip_root(zip_path: &Path, dest: &Path) -> Result<()> {
    let file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 检测是否存在统一顶层前缀，同时预检所有条目安全性
    let mut common_root: Option<String> = None;
    let mut has_single_root = true;

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        if entry.enclosed_name().is_none() {
            return Err(anyhow!("检测到不安全的 ZIP 路径条目: {}", entry.name()));
        }
        let name = entry.name().replace('\\', "/");
        if let Some(first) = name.split('/').next() {
            if first.is_empty() {
                continue;
            }
            match &common_root {
                None => common_root = Some(first.to_string()),
                Some(prev) if prev != first => {
                    has_single_root = false;
                }
                _ => {}
            }
        }
    }

    let strip_prefix = if has_single_root {
        common_root.map(|r| format!("{r}/"))
    } else {
        None
    };

    // 在同级目录创建临时暂存目录，保证最终 rename 在同一卷内原子完成
    let file_name = dest
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "extract".to_string());
    let staging = dest.with_file_name(format!("{file_name}.moka_tmp"));
    let backup = dest.with_file_name(format!("{file_name}.moka_old"));

    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    fs::create_dir_all(&staging)?;

    let extract_res: Result<()> = (|| {
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.enclosed_name().is_none() {
                return Err(anyhow!("检测到不安全的 ZIP 路径条目: {}", entry.name()));
            }
            let raw_name = entry.name().replace('\\', "/");
            let rel = match &strip_prefix {
                Some(prefix) => raw_name.strip_prefix(prefix).unwrap_or(&raw_name),
                None => &raw_name,
            };
            let Some(clean_rel) = sanitize_zip_rel_path(rel)? else {
                continue;
            };
            let out_path = staging.join(&clean_rel);
            if entry.is_dir() {
                fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = File::create(&out_path)?;
                io::copy(&mut entry, &mut outfile)?;
            }
        }
        Ok(())
    })();

    if let Err(e) = extract_res {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }

    // 解压全部成功后原子替换目标目录
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    let had_existing = dest.exists();
    if had_existing {
        fs::rename(dest, &backup)?;
    }
    if let Err(e) = fs::rename(&staging, dest) {
        if had_existing && backup.exists() {
            let _ = fs::rename(&backup, dest);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(e.into());
    }
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn zip_slip_entry_is_rejected_and_existing_dir_preserved() {
        let base = std::env::temp_dir().join(format!("moka_zip_slip_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let dest = base.join("app");
        fs::create_dir_all(&dest).unwrap();
        let sentinel = dest.join("keep.txt");
        fs::write(&sentinel, "original").unwrap();

        let zip_path = base.join("malicious.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("../escaped.txt", opts).unwrap();
            zw.write_all(b"evil").unwrap();
            zw.finish().unwrap();
        }

        let res = extract_zip_strip_root(&zip_path, &dest);
        assert!(res.is_err(), "恶意 ../ 路径穿越必须被拒绝");
        assert!(!base.join("escaped.txt").exists(), "不得写出到目标目录之外");
        assert!(sentinel.exists(), "失败时原安装目录必须完好保留");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn valid_zip_strips_single_root_atomically() {
        let base = std::env::temp_dir().join(format!("moka_zip_ok_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        let dest = base.join("app");
        let zip_path = base.join("good.zip");
        {
            let f = File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zw.start_file("pkg-1.0/bin/hello.txt", opts).unwrap();
            zw.write_all(b"world").unwrap();
            zw.finish().unwrap();
        }

        extract_zip_strip_root(&zip_path, &dest).unwrap();
        assert_eq!(
            fs::read_to_string(dest.join("bin/hello.txt")).unwrap(),
            "world"
        );
        assert!(!base.join("app.moka_tmp").exists());

        let _ = fs::remove_dir_all(&base);
    }
}
