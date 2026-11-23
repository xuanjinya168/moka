use anyhow::{anyhow, Result};
use chrono::Local;
use moka_config::ensure_service_config;
use moka_core::manifest::{CommandSpec, SoftwareManifest};
use moka_core::RuntimeLayout;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const MAX_LOG_LINES: usize = 500;

#[derive(Debug, Clone, Serialize)]
pub struct ServiceLogEvent {
    pub id: String,
    pub line: String,
}

struct RunningService {
    child: Child,
    pid: Option<u32>,
    port: Option<u16>,
}

pub struct ProcessManager {
    running: Arc<Mutex<HashMap<String, RunningService>>>,
    logs: Arc<Mutex<HashMap<String, VecDeque<String>>>>,
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(HashMap::new())),
            logs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn is_running(&self, id: &str) -> (bool, Option<u32>, Option<u16>) {
        let mut map = self.running.lock().unwrap();
        if let Some(svc) = map.get_mut(id) {
            match svc.child.try_wait() {
                Ok(None) => return (true, svc.pid, svc.port),
                _ => {
                    map.remove(id);
                }
            }
        }
        (false, None, None)
    }

    pub fn get_logs(&self, id: &str) -> Vec<String> {
        let map = self.logs.lock().unwrap();
        map.get(id)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn clear_logs(&self, id: &str) {
        let mut map = self.logs.lock().unwrap();
        if let Some(q) = map.get_mut(id) {
            q.clear();
        }
    }

    fn append_log(
        logs: &Arc<Mutex<HashMap<String, VecDeque<String>>>>,
        log_file: Option<&Path>,
        id: &str,
        msg: String,
    ) {
        let ts = Local::now().format("%H:%M:%S");
        let formatted = format!("[{ts}] {msg}");
        {
            let mut map = logs.lock().unwrap();
            let queue = map.entry(id.to_string()).or_default();
            if queue.len() >= MAX_LOG_LINES {
                queue.pop_front();
            }
            queue.push_back(formatted.clone());
        }
        if let Some(path) = log_file {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            // 超过 5 MB 自动轮转为 .log.1，防止长期运行日志无限膨胀
            const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
            if let Ok(meta) = std::fs::metadata(path) {
                if meta.len() >= MAX_LOG_FILE_BYTES {
                    let rotated = path.with_extension("log.1");
                    let _ = std::fs::remove_file(&rotated);
                    let _ = std::fs::rename(path, &rotated);
                }
            }
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{formatted}");
                let _ = f.flush();
            }
        }
    }

    /// 读取持久化日志文件最后 N 行（仅 Seek 读取末尾 256 KB，避免大日志每秒全量读入内存）
    pub fn load_persisted_logs(&self, id: &str, log_file: &Path) -> Vec<String> {
        let mem = self.get_logs(id);
        if !mem.is_empty() {
            return mem;
        }
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(log_file) else {
            return Vec::new();
        };
        const MAX_TAIL_BYTES: u64 = 256 * 1024;
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len > MAX_TAIL_BYTES {
            let _ = f.seek(SeekFrom::End(-(MAX_TAIL_BYTES as i64)));
        }
        let mut buf = Vec::new();
        if f.read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        let text = String::from_utf8_lossy(&buf);
        let mut iter = text.lines();
        // 若从文件中间 seek 开始，丢弃第一行可能被截断的残缺行
        if len > MAX_TAIL_BYTES {
            let _ = iter.next();
        }
        let all: Vec<String> = iter.map(|l| l.to_string()).collect();
        let start = all.len().saturating_sub(MAX_LOG_LINES);
        all[start..].to_vec()
    }

    pub async fn start_service(
        &self,
        manifest: &SoftwareManifest,
        layout: &RuntimeLayout,
        port_override: Option<u16>,
    ) -> Result<Option<u32>> {
        let (already_running, pid, _) = self.is_running(&manifest.id);
        if already_running {
            return Ok(pid);
        }

        let app_dir = layout.app_dir(&manifest.id);
        if !app_dir.exists() {
            return Err(anyhow!("软件 {} 尚未安装", manifest.name));
        }

        // 启动前清理可能由上一次 Moka 会话残留的同目录孤儿进程，避免 ibdata1 文件锁与端口冲突
        let app_dir_clone = app_dir.clone();
        let _ = tokio::task::spawn_blocking(move || {
            moka_sys::stop_processes_in_dir(&app_dir_clone);
        })
        .await;
        let conf_dir = layout.conf_dir(&manifest.id);
        let data_dir = layout.data_dir(&manifest.id);
        let log_file: PathBuf = layout.log_file(&manifest.id);
        let port = port_override.or(manifest.default_port).unwrap_or(0);

        // 启动前端口冲突智能预检：若目标端口已被其他程序占用，立即提示具体进程名与 PID
        if port > 0
            && timeout(
                Duration::from_millis(200),
                TcpStream::connect(("127.0.0.1", port)),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
        {
            let owner = tokio::task::spawn_blocking(move || {
                moka_sys::list_listening_ports()
                    .ok()
                    .and_then(|list| list.into_iter().find(|p| p.port == port))
            })
            .await
            .ok()
            .flatten();
            let err_msg = match owner {
                Some(occ) => format!(
                    "端口 {port} 已被 {} (PID {}) 占用，请修改端口或在「实用工具箱」中释放",
                    occ.process_name, occ.pid
                ),
                None => format!("端口 {port} 已被其他程序占用，请修改端口或在「实用工具箱」中释放"),
            };
            Self::append_log(
                &self.logs,
                Some(&log_file),
                &manifest.id,
                format!("[ERROR] {err_msg}"),
            );
            return Err(anyhow!("{err_msg}"));
        }

        let conf_file = ensure_service_config(
            manifest.config_kind.as_deref(),
            &app_dir,
            &conf_dir,
            &data_dir,
            port,
        )?;

        // 如需首次初始化（例如 MySQL --initialize-insecure）
        if let Some(init_spec) = &manifest.init_cmd {
            let needs_init = std::fs::read_dir(&data_dir)
                .map(|mut it| it.next().is_none())
                .unwrap_or(true);
            if needs_init {
                Self::append_log(
                    &self.logs,
                    Some(&log_file),
                    &manifest.id,
                    "检测到首次运行，正在初始化数据目录...".into(),
                );
                let (prog, args) =
                    render_cmd(init_spec, &app_dir, conf_file.as_deref(), &data_dir, port);
                let mut cmd = build_silent_command(&prog, &args, &app_dir);
                let out = cmd.output().await?;
                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    Self::append_log(
                        &self.logs,
                        Some(&log_file),
                        &manifest.id,
                        format!("[ERR] 初始化失败: {err}"),
                    );
                    return Err(anyhow!("初始化失败: {err}"));
                }
                Self::append_log(
                    &self.logs,
                    Some(&log_file),
                    &manifest.id,
                    "[OK] 数据目录初始化完成".into(),
                );
            }
        }

        let start_spec = manifest
            .start_cmd
            .as_ref()
            .ok_or_else(|| anyhow!("该组件不是可启动服务"))?;

        // MSYS2 编译版 Redis 将 Windows 盘符路径视为相对路径，使用相对 POSIX 路径可 100% 兼容任意盘符与中文用户名
        let effective_conf = if manifest.config_kind.as_deref() == Some("redis") {
            Some(PathBuf::from(format!("../../conf/{}/redis.conf", manifest.id)))
        } else {
            conf_file.clone()
        };

        let (prog, args) = render_cmd(
            start_spec,
            &app_dir,
            effective_conf.as_deref(),
            &data_dir,
            port,
        );

        let mut cmd = build_silent_command(&prog, &args, &app_dir);
        if let Some(java_home) =
            apply_service_env(&mut cmd, manifest, layout, conf_file.as_deref(), true)?
        {
            Self::append_log(
                &self.logs,
                Some(&log_file),
                &manifest.id,
                format!("使用 JAVA_HOME: {}", java_home.display()),
            );
        }

        Self::append_log(
            &self.logs,
            Some(&log_file),
            &manifest.id,
            format!("启动命令: {} {}", prog, args.join(" ")),
        );
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let pid = child.id();

        // 异步采集 stdout/stderr 并持久化写入日志文件
        if let Some(stdout) = child.stdout.take() {
            let logs = self.logs.clone();
            let id = manifest.id.clone();
            let lf = log_file.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    Self::append_log(&logs, Some(&lf), &id, line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let logs = self.logs.clone();
            let id = manifest.id.clone();
            let lf = log_file.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    Self::append_log(&logs, Some(&lf), &id, line);
                }
            });
        }

        self.running.lock().unwrap().insert(
            manifest.id.clone(),
            RunningService {
                child,
                pid,
                port: if port > 0 { Some(port) } else { None },
            },
        );

        if port > 0 {
            let healthy = wait_tcp_ready(port, 3500).await;
            // 二次校验子进程是否仍在存活，防止误把其他进程的端口当成就绪
            let still_alive = self.is_running(&manifest.id).0;
            if !still_alive {
                let err_msg = "进程启动后异常退出，请查看上方错误日志".to_string();
                Self::append_log(
                    &self.logs,
                    Some(&log_file),
                    &manifest.id,
                    format!("[ERROR] {err_msg}"),
                );
                return Err(anyhow!(err_msg));
            }
            Self::append_log(
                &self.logs,
                Some(&log_file),
                &manifest.id,
                if healthy {
                    format!("[OK] 服务已就绪，监听端口 {port} (PID: {:?})", pid)
                } else {
                    format!("进程已启动 (PID: {:?})，正在监听端口 {port}...", pid)
                },
            );
        }

        Ok(pid)
    }

    pub async fn stop_service(
        &self,
        manifest: &SoftwareManifest,
        layout: &RuntimeLayout,
    ) -> Result<()> {
        let mut entry = self
            .running
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&manifest.id);

        let app_dir = layout.app_dir(&manifest.id);
        let conf_dir = layout.conf_dir(&manifest.id);
        let log_file = layout.log_file(&manifest.id);
        let conf_file = match manifest.config_kind.as_deref() {
            Some("mysql") => Some(conf_dir.join("my.ini")),
            Some("redis") => Some(conf_dir.join("redis.conf")),
            Some("nginx") => Some(conf_dir.join("nginx.conf")),
            Some("postgres") => Some(layout.data_dir(&manifest.id).join("postgresql.conf")),
            _ => None,
        };
        // 优先取内存记录端口；重启后外部接管时从配置文件读取真实端口，最后兜底默认端口
        let port = entry
            .as_ref()
            .and_then(|s| s.port)
            .or_else(|| read_configured_port(manifest.config_kind.as_deref(), conf_file.as_deref()))
            .or(manifest.default_port)
            .unwrap_or(0);

        // 优先优雅停止（同时兼容本次启动、重启后外部接管、自定义端口以及已设 root 密码的 MySQL）
        if let Some(stop_spec) = &manifest.stop_cmd {
            Self::append_log(
                &self.logs,
                Some(&log_file),
                &manifest.id,
                "正在优雅停止服务...".into(),
            );
            let (prog, args) = render_cmd(
                stop_spec,
                &app_dir,
                conf_file.as_deref(),
                &layout.data_dir(&manifest.id),
                port,
            );
            let mut cmd = build_silent_command(&prog, &args, &app_dir);
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            let _ = apply_service_env(&mut cmd, manifest, layout, conf_file.as_deref(), false);
            if manifest.config_kind.as_deref() == Some("mysql") {
                if let Some(pw) = read_saved_db_password(layout.root_path(), &manifest.id) {
                    if !pw.is_empty() {
                        cmd.env("MYSQL_PWD", pw);
                    }
                }
            }
            let _ = timeout(Duration::from_millis(3500), cmd.status()).await;
        }

        // 兜底强制终止整棵进程树（兼容 cmd.exe 拉起的 Nacos / Tomcat 子进程）
        if let Some(ref mut svc) = entry {
            if svc.child.try_wait()?.is_none() {
                if let Some(pid) = svc.pid {
                    kill_process_tree(pid).await;
                }
                let _ = svc.child.start_kill();
                let _ = timeout(Duration::from_millis(800), svc.child.wait()).await;
            }
        }
        // 清理同目录下所有残留或外部接管的子进程（如重启 Moka 后接管的 mysqld/redis/nginx/nacos）
        let app_dir_clean = app_dir.clone();
        let _ = tokio::task::spawn_blocking(move || {
            moka_sys::stop_processes_in_dir(&app_dir_clean);
        })
        .await;
        Self::append_log(&self.logs, Some(&log_file), &manifest.id, "服务已停止".into());
        Ok(())
    }

    pub async fn stop_all(&self) {
        let items: Vec<(String, RunningService)> = {
            let mut map = self.running.lock().unwrap_or_else(|e| e.into_inner());
            map.drain().collect()
        };
        for (_, mut svc) in items {
            if let Some(pid) = svc.pid {
                kill_process_tree(pid).await;
            }
            let _ = svc.child.start_kill();
        }
    }

    /// 并行优雅停止全部运行中的托管服务：
    /// 1. 有界超时仅包裹「扫描 + 优雅停机」阶段；
    /// 2. 之后无条件执行兜底句柄清理与一次同步 `apps/` 全目录扫尾，绝不会被外层 timeout 截断。
    pub async fn stop_all_graceful(
        self: &Arc<Self>,
        layout: &Arc<RuntimeLayout>,
        catalog: &[SoftwareManifest],
    ) {
        let mut target_ids: std::collections::HashSet<String> = {
            let map = self.running.lock().unwrap_or_else(|e| e.into_inner());
            map.keys().cloned().collect()
        };
        let apps_dir_scan = layout.root_path().join("apps");
        if let Ok(Ok(ext)) = timeout(
            Duration::from_millis(2000),
            tokio::task::spawn_blocking(move || {
                moka_sys::list_running_pids_under_apps(&apps_dir_scan)
            }),
        )
        .await
        {
            target_ids.extend(ext.into_keys());
        }

        let mut join_set = tokio::task::JoinSet::new();
        for id in target_ids {
            if let Some(manifest) = catalog.iter().find(|m| m.id == id).cloned() {
                let mgr = Arc::clone(self);
                let lay = Arc::clone(layout);
                join_set.spawn(async move {
                    let _ = mgr.stop_service(&manifest, &lay).await;
                });
            }
        }
        let _ = timeout(Duration::from_millis(5500), async {
            while join_set.join_next().await.is_some() {}
        })
        .await;

        // 兜底阶段无条件执行，不受外层取消影响
        self.stop_all().await;
        let apps_dir_final = layout.root_path().join("apps");
        let _ = tokio::task::spawn_blocking(move || {
            moka_sys::stop_processes_in_dir(&apps_dir_final);
        })
        .await;
    }
}

/// 从服务配置文件中解析真实端口（供重启接管后优雅停机使用）
fn read_configured_port(config_kind: Option<&str>, conf_file: Option<&Path>) -> Option<u16> {
    let path = conf_file?;
    let content = std::fs::read_to_string(path).ok()?;
    for raw in content.lines() {
        let t = raw.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with(';') {
            continue;
        }
        match config_kind {
            Some("mysql") => {
                if let Some(v) = t.strip_prefix("port=") {
                    if let Ok(p) = v.trim().parse::<u16>() {
                        return Some(p);
                    }
                }
            }
            Some("redis") => {
                if let Some(v) = t.strip_prefix("port ") {
                    if let Ok(p) = v.split_whitespace().next().unwrap_or("").parse::<u16>() {
                        return Some(p);
                    }
                }
            }
            Some("postgres") => {
                if let Some((k, v)) = t.split_once('=') {
                    if k.trim() == "port" {
                        let val = v.split('#').next().unwrap_or("").trim().trim_matches('\'');
                        if let Ok(p) = val.parse::<u16>() {
                            return Some(p);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// 从 runtime/conf/settings.json 读取已保存的数据库 root 密码（供 mysqladmin shutdown 优雅停机认证）
fn read_saved_db_password(root_path: &Path, service_id: &str) -> Option<String> {
    let settings_path = root_path.join("conf").join("settings.json");
    let text = std::fs::read_to_string(settings_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("db_passwords")?
        .get(service_id)?
        .as_str()
        .map(|s| s.to_string())
}

/// 按服务类型统一注入运行环境变量（Mailpit .env 文件 / Tomcat & Nacos JAVA_HOME）
fn apply_service_env(
    cmd: &mut Command,
    manifest: &SoftwareManifest,
    layout: &RuntimeLayout,
    conf_file: Option<&Path>,
    strict_jdk: bool,
) -> Result<Option<PathBuf>> {
    match manifest.config_kind.as_deref() {
        Some("mailpit") => {
            if let Some(env_path) = conf_file {
                if let Ok(content) = std::fs::read_to_string(env_path) {
                    for line in content.lines() {
                        let t = line.trim();
                        if t.starts_with('#') || t.is_empty() {
                            continue;
                        }
                        if let Some((k, v)) = t.split_once('=') {
                            cmd.env(k.trim(), v.trim());
                        }
                    }
                }
            }
            Ok(None)
        }
        Some("tomcat") | Some("nacos") => {
            let require_jdk17 = manifest.config_kind.as_deref() == Some("tomcat");
            if let Some(java_home) = resolve_java_home(layout, require_jdk17) {
                cmd.env("JAVA_HOME", &java_home);
                cmd.env_remove("JRE_HOME");
                Ok(Some(java_home))
            } else if strict_jdk && require_jdk17 {
                Err(anyhow!(
                    "Apache Tomcat 10.1 需要 JDK 17 或 JDK 21，请先在「软件商店」安装 OpenJDK 17 / 21 LTS"
                ))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

async fn kill_process_tree(pid: u32) {
    let mut cmd = Command::new("taskkill");
    cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = timeout(Duration::from_millis(1200), cmd.status()).await;
}

/// 读取 JDK 目录下的 `release` 文件解析主版本号（如 8 / 11 / 17 / 21）
fn detect_jdk_major_version(jdk_dir: &Path) -> Option<u32> {
    if !jdk_dir.join("bin").join("java.exe").exists() {
        return None;
    }
    let release = std::fs::read_to_string(jdk_dir.join("release")).ok()?;
    for line in release.lines() {
        if let Some(rest) = line.trim().strip_prefix("JAVA_VERSION=") {
            let ver = rest.trim().trim_matches('"');
            let first = ver.split('.').next()?;
            if first == "1" {
                return ver.split('.').nth(1)?.parse::<u32>().ok();
            }
            return first.parse::<u32>().ok();
        }
    }
    None
}

/// 自动查找可用的 JAVA_HOME（优先使用 Moka 管理的 JDK，再回退本机已安装 JDK）
fn resolve_java_home(layout: &RuntimeLayout, require_jdk17: bool) -> Option<PathBuf> {
    let min_ver = if require_jdk17 { 17 } else { 8 };
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. Moka 当前激活的 JDK 软链与已安装的 JDK 目录
    candidates.push(layout.current_link("jdk"));
    candidates.push(layout.app_dir("jdk-21"));
    candidates.push(layout.app_dir("jdk-17"));
    if !require_jdk17 {
        candidates.push(layout.app_dir("jdk-8"));
    }

    // 2. 用户主目录默认 Moka-Runtime（方便独立测试目录也能复用）
    if let Ok(home) = std::env::var("USERPROFILE") {
        let hp = PathBuf::from(&home);
        candidates.push(hp.join("Moka-Runtime").join("apps").join("jdk-21"));
        candidates.push(hp.join("Moka-Runtime").join("apps").join("jdk-17"));
        if let Ok(entries) = std::fs::read_dir(hp.join(".jdks")) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    candidates.push(e.path());
                }
            }
        }
    }

    // 3. 系统常见 JDK 安装目录与环境变量
    for root in [
        r"C:\Program Files\Eclipse Adoptium",
        r"C:\Program Files\Java",
        r"C:\Program Files\Microsoft",
    ] {
        if let Ok(entries) = std::fs::read_dir(root) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    candidates.push(e.path());
                }
            }
        }
    }
    if let Ok(env_jh) = std::env::var("JAVA_HOME") {
        candidates.push(PathBuf::from(env_jh));
    }

    for dir in candidates {
        if let Some(major) = detect_jdk_major_version(&dir) {
            if major >= min_ver {
                return Some(dir);
            }
        }
    }
    None
}

fn build_silent_command(program: &str, args: &[String], cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(cwd);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn render_cmd(
    spec: &CommandSpec,
    app_dir: &Path,
    conf_file: Option<&Path>,
    data_dir: &Path,
    port: u16,
) -> (String, Vec<String>) {
    let replace = |s: &str| {
        s.replace("{{app_dir}}", &app_dir.to_string_lossy())
            .replace(
                "{{conf_file}}",
                &conf_file
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
            .replace("{{data_dir}}", &data_dir.to_string_lossy())
            .replace("{{port}}", &port.to_string())
    };
    let program = replace(&spec.program);
    let args = spec.args.iter().map(|a| replace(a)).collect();
    (program, args)
}

async fn wait_tcp_ready(port: u16, max_ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(max_ms);
    while tokio::time::Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    false
}

/// 在 MySQL / MariaDB 运行中时一键修改 root@localhost 密码（兼容 MySQL 5.7/8.0/8.4 与 MariaDB 11.4）
pub async fn change_mysql_root_password(
    app_dir: &Path,
    port: u16,
    old_password: &str,
    new_password: &str,
) -> Result<()> {
    let bin_dir = app_dir.join("bin");
    let client = if bin_dir.join("mysql.exe").exists() {
        bin_dir.join("mysql.exe")
    } else if bin_dir.join("mariadb.exe").exists() {
        bin_dir.join("mariadb.exe")
    } else {
        return Err(anyhow!("未找到 mysql.exe 或 mariadb.exe 客户端"));
    };

    let escaped = new_password.replace('\\', "\\\\").replace('\'', "\\'");
    let sql = format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{escaped}'; FLUSH PRIVILEGES;");

    let mut cmd = Command::new(&client);
    cmd.args([
        "-h",
        "127.0.0.1",
        "-P",
        &port.to_string(),
        "-u",
        "root",
        "-e",
        &sql,
    ])
    .current_dir(app_dir);
    if !old_password.is_empty() {
        cmd.env("MYSQL_PWD", old_password);
    } else {
        cmd.env_remove("MYSQL_PWD");
    }
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let out = timeout(Duration::from_secs(8), cmd.output())
        .await
        .map_err(|_| anyhow!("修改密码超时，请确认数据库服务已正常运行"))??;

    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(anyhow!(if stderr.is_empty() {
            "修改 root 密码失败，请确认当前密码正确".to_string()
        } else {
            format!("修改密码失败: {stderr}")
        }))
    }
}

/// 保存前使用 `nginx.exe -t` 对候选配置文本做语法预检，防止手写语法错误导致运行中 Nginx 崩溃
pub fn validate_nginx_config_text(
    app_dir: &Path,
    conf_path: &Path,
    candidate_text: &str,
) -> Result<()> {
    let nginx_exe = app_dir.join("nginx.exe");
    if !nginx_exe.exists() {
        return Ok(());
    }
    let tmp_conf = conf_path.with_extension("conf.moka_test");
    std::fs::write(&tmp_conf, candidate_text)?;

    let mut cmd = std::process::Command::new(&nginx_exe);
    cmd.args([
        "-t",
        "-p",
        &app_dir.to_string_lossy(),
        "-c",
        &tmp_conf.to_string_lossy(),
    ])
    .current_dir(app_dir);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let res = cmd.output();
    let _ = std::fs::remove_file(&tmp_conf);

    let out = res?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let first_err = stderr
            .lines()
            .find(|l| l.contains("[emerg]") || l.contains("[error]"))
            .unwrap_or(stderr.trim())
            .replace(".conf.moka_test", ".conf");
        Err(anyhow!("Nginx 语法校验未通过: {first_err}"))
    }
}

/// 校验配置语法并热重载正在运行的 Nginx（`nginx -t` + `nginx -s reload`）
pub fn reload_nginx(app_dir: &Path, conf_path: &Path) -> Result<()> {
    let nginx_exe = app_dir.join("nginx.exe");
    if !nginx_exe.exists() {
        return Err(anyhow!("未找到 nginx.exe"));
    }
    let test_out = {
        let mut cmd = std::process::Command::new(&nginx_exe);
        cmd.args([
            "-t",
            "-p",
            &app_dir.to_string_lossy(),
            "-c",
            &conf_path.to_string_lossy(),
        ])
        .current_dir(app_dir);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.output()?
    };
    if !test_out.status.success() {
        let stderr = String::from_utf8_lossy(&test_out.stderr);
        let first_err = stderr
            .lines()
            .find(|l| l.contains("[emerg]") || l.contains("[error]"))
            .unwrap_or(stderr.trim());
        return Err(anyhow!("Nginx 配置校验失败: {first_err}"));
    }

    let reload_out = {
        let mut cmd = std::process::Command::new(&nginx_exe);
        cmd.args([
            "-p",
            &app_dir.to_string_lossy(),
            "-c",
            &conf_path.to_string_lossy(),
            "-s",
            "reload",
        ])
        .current_dir(app_dir);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        cmd.output()?
    };
    if reload_out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&reload_out.stderr).trim().to_string();
        Err(anyhow!("Nginx 热重载失败: {stderr}"))
    }
}

#[derive(Debug, Clone)]
pub struct DbBackupFile {
    pub file_name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_label: String,
}

/// 列出指定备份目录下的所有 `.sql` 快照（按修改时间倒序）
pub fn list_db_backups(backup_dir: &Path) -> Vec<DbBackupFile> {
    let Ok(entries) = std::fs::read_dir(backup_dir) else {
        return Vec::new();
    };
    let mut items: Vec<(std::time::SystemTime, DbBackupFile)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sql") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let modified = meta
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let dt: chrono::DateTime<chrono::Local> = modified.into();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        items.push((
            modified,
            DbBackupFile {
                file_name,
                path,
                size_bytes: meta.len(),
                modified_label: dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            },
        ));
    }
    items.sort_by_key(|a| std::cmp::Reverse(a.0));
    items.into_iter().map(|(_, f)| f).collect()
}

fn find_first_exe(app_dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    for rel in candidates {
        let p = app_dir.join(rel);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 一键导出数据库全量 SQL 快照（支持 MySQL / MariaDB / PostgreSQL）
pub fn backup_database(
    config_kind: &str,
    app_dir: &Path,
    port: u16,
    root_password: &str,
    backup_dir: &Path,
    db_label: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(backup_dir)?;
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let safe_label: String = db_label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let out_path = backup_dir.join(format!("{safe_label}_{ts}.sql"));

    let mut cmd = match config_kind {
        "mysql" => {
            let dump_exe = find_first_exe(
                app_dir,
                &["bin/mysqldump.exe", "bin/mariadb-dump.exe"],
            )
            .ok_or_else(|| anyhow!("未找到 mysqldump.exe / mariadb-dump.exe"))?;
            let mut c = std::process::Command::new(dump_exe);
            c.args([
                "-h",
                "127.0.0.1",
                "-P",
                &port.to_string(),
                "-u",
                "root",
                "--all-databases",
                "--single-transaction",
                "--routines",
                "--triggers",
                "--default-character-set=utf8mb4",
            ]);
            if !root_password.is_empty() {
                c.env("MYSQL_PWD", root_password);
            }
            c
        }
        "postgres" => {
            let dump_exe = find_first_exe(
                app_dir,
                &["pgsql/bin/pg_dumpall.exe", "bin/pg_dumpall.exe"],
            )
            .ok_or_else(|| anyhow!("未找到 pg_dumpall.exe"))?;
            let mut c = std::process::Command::new(dump_exe);
            c.args([
                "-h",
                "127.0.0.1",
                "-p",
                &port.to_string(),
                "-U",
                "postgres",
                "--clean",
                "--if-exists",
            ]);
            if !root_password.is_empty() {
                c.env("PGPASSWORD", root_password);
            }
            c
        }
        _ => return Err(anyhow!("暂不支持该类型数据库备份: {config_kind}")),
    };

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    // 直接流式写入目标文件，避免大库整份 SQL 缓存在内存中撑爆进程
    let out_file = std::fs::File::create(&out_path)?;
    cmd.stdout(Stdio::from(out_file)).stderr(Stdio::piped());

    let out = cmd.output()?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&out_path);
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(anyhow!("备份失败: {stderr}"));
    }
    let written_len = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    if written_len == 0 {
        let _ = std::fs::remove_file(&out_path);
        return Err(anyhow!("备份输出为空，请确认数据库运行正常且密码正确"));
    }
    Ok(out_path)
}

/// 一键从 SQL 快照文件恢复数据库（支持 MySQL / MariaDB / PostgreSQL）
pub fn restore_database(
    config_kind: &str,
    app_dir: &Path,
    port: u16,
    root_password: &str,
    sql_file: &Path,
) -> Result<()> {
    if !sql_file.exists() {
        return Err(anyhow!("备份文件不存在: {}", sql_file.display()));
    }
    match config_kind {
        "mysql" => {
            let mysql_exe = find_first_exe(
                app_dir,
                &["bin/mysql.exe", "bin/mariadb.exe"],
            )
            .ok_or_else(|| anyhow!("未找到 mysql.exe / mariadb.exe"))?;
            let file = std::fs::File::open(sql_file)?;
            let mut cmd = std::process::Command::new(mysql_exe);
            cmd.args([
                "-h",
                "127.0.0.1",
                "-P",
                &port.to_string(),
                "-u",
                "root",
                "--default-character-set=utf8mb4",
            ])
            .stdin(std::process::Stdio::from(file));
            if !root_password.is_empty() {
                cmd.env("MYSQL_PWD", root_password);
            }
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            let out = cmd.output()?;
            if out.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(anyhow!("恢复失败: {stderr}"))
            }
        }
        "postgres" => {
            let psql_exe = find_first_exe(
                app_dir,
                &["pgsql/bin/psql.exe", "bin/psql.exe"],
            )
            .ok_or_else(|| anyhow!("未找到 psql.exe"))?;
            let mut cmd = std::process::Command::new(psql_exe);
            cmd.args([
                "-h",
                "127.0.0.1",
                "-p",
                &port.to_string(),
                "-U",
                "postgres",
                // psql 默认遇到 SQL 错误会跳过继续并以 0 退出，必须让它遇错即停
                "-v",
                "ON_ERROR_STOP=1",
                "-f",
                &sql_file.to_string_lossy(),
            ]);
            if !root_password.is_empty() {
                cmd.env("PGPASSWORD", root_password);
            }
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            let out = cmd.output()?;
            if out.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                Err(anyhow!("恢复失败: {stderr}"))
            }
        }
        _ => Err(anyhow!("暂不支持该类型数据库恢复: {config_kind}")),
    }
}
