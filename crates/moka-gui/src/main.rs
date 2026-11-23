#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config_form;
mod modals;
mod pages;
mod storage;
mod theme;
mod tray;
mod widgets;

use config_form::render_visual_config_form;
use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Pos2, Rect, RichText, ScrollArea,
    Shadow, Stroke, Vec2,
};
use moka_config::{
    apply_visual_config, config_format_label, default_service_config_content, ensure_local_root_ca,
    ensure_service_config, format_and_validate_config, generate_aliyun_maven_settings,
    generate_local_ssl_cert, inspect_dev_mirrors, parse_visual_config, read_service_port,
    set_dev_mirror, sync_nginx_vhosts, DevMirrorKind, DevMirrorState, SiteConfig, SiteProxyRule,
    VisualConfig,
};
use moka_core::downloader::{download_and_extract_cancellable, DownloadProgress};
use moka_core::manifest::{
    builtin_catalog_json, check_latest_release, fetch_remote_catalog, load_catalog, ReleaseInfo,
    SoftwareCategory, SoftwareManifest, SoftwareStatusView,
};
use moka_core::RuntimeLayout;
use moka_process::{
    backup_database, change_mysql_root_password, list_db_backups, reload_nginx, restore_database,
    validate_nginx_config_text, DbBackupFile, ProcessManager,
};
use moka_sys::{
    acquire_single_instance, apply_win11_rounded_corners, install_user_root_ca,
    is_auto_start_enabled, is_user_root_ca_installed, kill_pid, list_listening_ports,
    list_running_pids_under_apps, parse_hosts_entries, query_process_metrics, read_hosts_file,
    read_junction_target, render_hosts_file, safe_remove_app_dir, set_auto_start, switch_junction,
    uninstall_user_root_ca, write_hosts_file, HostEntry, PortOccupancy, ProcessMetrics,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use storage::{
    calc_dir_size, format_bytes, format_uptime, load_presets, load_settings, load_sites,
    save_presets, save_settings, save_sites, write_app_log, AppSettings, ServicePreset, TrayStyle,
};
use theme::{apply_theme, build_moka_icon, configure_fonts, ThemeMode, ThemePalette};
use tray::{restore_window, SystemTray, TrayCommand};
use widgets::{
    card_frame, chip_button, draw_nav_icon, handle_frameless_resize, is_error_log, is_warn_log,
    log_line_color, pill_badge, primary_button, secondary_button, segmented_choice, soft_button,
    solid_button, styled_search_input, window_control_btn, NavTab,
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MirrorRegion {
    China,
    Global,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsSubTab {
    General,
    Sources,
    Guide,
    About,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogLevelFilter {
    All,
    Errors,
    Warnings,
}


enum UiAction {
    Install(SoftwareManifest),
    CancelDownload(String),
    Uninstall(SoftwareManifest),
    ActivateEnv(SoftwareManifest),
    ToggleService(SoftwareStatusView),
    StartAllServices,
    StopAllServices,
    StartPreset(String, Vec<String>),
    OpenPresetEditor(Option<usize>),
    OpenLogs(SoftwareStatusView),
    OpenDir(PathBuf),
    OpenConfig(SoftwareStatusView),
    KillPort(PortOccupancy),
    SwitchTab(NavTab),
    OpenMokaTerminal,
    OpenUrl(String),
    CopyConnStr(String),
    OpenSiteEditor(Option<usize>),
    ToggleSiteEnabled(usize),
    DeleteSite(usize),
    ReloadNginx,
    InstallLocalCa,
    UninstallLocalCa,
    OpenDbBackup(SoftwareStatusView),
    SetDevMirror(DevMirrorKind, bool),
    SetAllDevMirrors(bool),
}

struct SharedAsyncState {
    downloads: HashMap<String, DownloadProgress>,
    cancel_flags: HashMap<String, Arc<AtomicBool>>,
    busy_ids: HashSet<String>,
    toast: Option<(String, Instant)>,
    views_dirty: bool,
    latest_views: Option<Vec<SoftwareStatusView>>,
    toolbox_loading: bool,
    latest_ports: Option<Vec<PortOccupancy>>,
    latest_hosts: Option<String>,
    hosts_saved: Option<String>,
    catalog_updating: bool,
    update_checking: bool,
    update_result: Option<Result<ReleaseInfo, String>>,
    pwd_updating: bool,
    latest_db_pwd: Option<(String, String)>,
    ca_busy: bool,
    ca_trusted_update: Option<bool>,
    metrics_polling: bool,
    latest_metrics: Option<HashMap<u32, ProcessMetrics>>,
    backup_busy: bool,
    latest_backups: Option<Vec<DbBackupFile>>,
}

struct MokaApp {
    rt: Arc<tokio::runtime::Runtime>,
    layout: Arc<RuntimeLayout>,
    process_mgr: Arc<ProcessManager>,
    catalog: Arc<Mutex<Vec<SoftwareManifest>>>,
    shared: Arc<Mutex<SharedAsyncState>>,

    theme_mode: ThemeMode,
    mirror_region: MirrorRegion,
    active_tab: NavTab,
    store_filter: Option<SoftwareCategory>,
    store_search: String,
    store_selected_versions: HashMap<String, String>,
    software_views: Vec<SoftwareStatusView>,
    port_overrides: HashMap<String, String>,

    // 软件源在线更新地址
    catalog_url_input: String,

    // 自定义快捷场景组
    presets: Vec<ServicePreset>,
    preset_modal_open: bool,
    preset_editing_idx: Option<usize>,
    preset_name_input: String,
    preset_selected_ids: HashSet<String>,

    // Nginx 多站点虚拟主机管理
    sites: Vec<SiteConfig>,
    site_modal_open: bool,
    site_editing_idx: Option<usize>,
    site_domain_input: String,
    site_port_input: String,
    site_root_input: String,
    site_ssl_enabled: bool,
    site_ssl_port_input: String,
    site_ssl_cert_input: String,
    site_ssl_key_input: String,
    site_proxy_rules: Vec<SiteProxyRule>,
    site_sync_hosts: bool,
    ca_trusted: bool,

    // 日志弹窗
    log_target_id: Option<String>,
    log_target_title: String,
    log_lines: Vec<String>,
    log_filter: String,
    log_level_filter: LogLevelFilter,
    log_auto_scroll: bool,
    last_log_poll: Instant,

    // 内置配置编辑器（默认可视化小白模式 + 高级源码模式）
    config_editor_view: Option<SoftwareStatusView>,
    config_editor_path: Option<PathBuf>,
    config_editor_text: String,
    config_editor_visual: Option<VisualConfig>,
    config_editor_advanced: bool,
    config_editor_dirty: bool,
    config_editor_error: Option<String>,
    db_new_pwd_input: String,

    // 工具箱
    ports: Vec<PortOccupancy>,
    filtered_port_indices: Vec<usize>,
    port_filter: String,
    last_port_filter: String,
    hosts_text: String,
    host_entries: Vec<HostEntry>,
    hosts_dirty: bool,
    host_new_ip: String,
    host_new_domain: String,
    host_new_comment: String,
    toolbox_loaded: bool,
    dev_mirrors: Vec<DevMirrorState>,

    // 系统托盘与设置页持久化偏好
    tray: SystemTray,
    tray_style: TrayStyle,
    tray_popover_open: bool,
    tray_popover_pos: Pos2,
    tray_popover_just_opened: bool,
    tray_popover_ever_focused: bool,
    tray_popover_search: String,
    win_corners_applied: bool,
    window_hidden_to_tray: bool,
    settings_tab: SettingsSubTab,
    stop_on_exit: bool,
    minimize_to_tray: bool,
    auto_start: bool,
    auto_start_preset: String,
    auto_preset_triggered: bool,
    release_repo_input: String,
    db_passwords: HashMap<String, String>,
    proc_metrics: HashMap<u32, ProcessMetrics>,
    last_metrics_poll: Instant,
    backup_modal_svc: Option<SoftwareStatusView>,
    backup_files: Vec<DbBackupFile>,
    cache_size_bytes: u64,
    update_status: Option<Result<ReleaseInfo, String>>,
}

fn default_runtime_root() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if !dir.to_string_lossy().contains("target") {
                return dir.join("runtime");
            }
        }
    }
    let home = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Moka".into());
    PathBuf::from(home).join("Moka-Runtime")
}

fn category_label(cat: &SoftwareCategory) -> &'static str {
    match cat {
        SoftwareCategory::Runtime => "语言运行时",
        SoftwareCategory::BuildTool => "构建工具",
        SoftwareCategory::Database => "数据库",
        SoftwareCategory::Cache => "缓存",
        SoftwareCategory::Web => "Web / 网关",
        SoftwareCategory::Middleware => "微服务中间件",
    }
}

fn compute_software_views(
    catalog: &[SoftwareManifest],
    layout: &RuntimeLayout,
    process_mgr: &ProcessManager,
) -> Vec<SoftwareStatusView> {
    let mut link_cache: HashMap<String, Option<PathBuf>> = HashMap::new();
    // 单次批量扫描 apps/ 下的真实存活进程，自动接管「保留后台服务退出」后仍在运行的服务状态
    let external_pids = list_running_pids_under_apps(&layout.root_path().join("apps"));

    catalog
        .iter()
        .map(|m| {
            let installed = layout.is_installed(&m.id);
            let (mut running, mut pid, running_port) = process_mgr.is_running(&m.id);
            if !running && installed && m.is_service {
                if let Some(&ext_pid) = external_pids.get(&m.id) {
                    running = true;
                    pid = Some(ext_pid);
                }
            }
            let app_dir = layout.app_dir(&m.id);

            let active_env = match m.env_link_name.as_deref() {
                Some(link_name) => {
                    let target = link_cache
                        .entry(link_name.to_string())
                        .or_insert_with(|| read_junction_target(&layout.current_link(link_name)));
                    target.as_ref().map(|t| t == &app_dir).unwrap_or(false)
                }
                None => false,
            };

            SoftwareStatusView {
                manifest: m.clone(),
                installed,
                running,
                active_env,
                port: running_port.or(m.default_port),
                pid,
                install_path: app_dir.to_string_lossy().to_string(),
            }
        })
        .collect()
}

fn open_in_explorer(path: &Path) {
    let mut cmd = Command::new("explorer.exe");
    cmd.arg(path);
    let _ = cmd.spawn();
}

/// 打开一个零污染系统 PATH 的 Moka 专属 PowerShell 终端（仅当前窗口生效）
fn open_moka_terminal(root: &Path) {
    let cur = root.join("current");
    let candidates = [
        cur.join("node"),
        cur.join("jdk").join("bin"),
        cur.join("go").join("bin"),
        cur.join("python"),
        cur.join("maven").join("bin"),
        cur.join("gradle").join("bin"),
    ];
    let mut prepend: Vec<String> = Vec::new();
    for p in &candidates {
        if p.exists() {
            prepend.push(p.to_string_lossy().to_string());
        }
    }

    let mut env_cmds: Vec<String> = Vec::new();
    if !prepend.is_empty() {
        let joined = prepend.join(";").replace('\'', "''");
        env_cmds.push(format!("$env:PATH = '{joined};' + $env:PATH"));
    }
    let jdk = cur.join("jdk");
    if jdk.exists() {
        let s = jdk.to_string_lossy().replace('\'', "''");
        env_cmds.push(format!("$env:JAVA_HOME = '{s}'"));
    }
    let maven = cur.join("maven");
    if maven.exists() {
        let s = maven.to_string_lossy().replace('\'', "''");
        env_cmds.push(format!("$env:MAVEN_HOME = '{s}'"));
    }
    let gradle = cur.join("gradle");
    if gradle.exists() {
        let s = gradle.to_string_lossy().replace('\'', "''");
        env_cmds.push(format!("$env:GRADLE_HOME = '{s}'"));
    }
    let goroot = cur.join("go");
    if goroot.exists() {
        let s = goroot.to_string_lossy().replace('\'', "''");
        env_cmds.push(format!("$env:GOROOT = '{s}'"));
    }

    env_cmds.push(
        "Write-Host 'Moka 专属开发环境终端已就绪（已自动加载当前激活运行时，仅本窗口生效，零污染系统 PATH）' -ForegroundColor DarkYellow"
            .to_string(),
    );
    let ps_script = env_cmds.join("; ");

    let mut cmd = Command::new("cmd.exe");
    cmd.args([
        "/C",
        "start",
        "Moka 开发终端",
        "powershell.exe",
        "-NoExit",
        "-Command",
        &ps_script,
    ]);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd.spawn();
}

/// 返回运行中服务的快捷动作：(按钮文案, 是否为网页打开, URL 或连接串)
fn service_quick_action(
    svc: &SoftwareStatusView,
    saved_pwd: Option<&str>,
) -> Option<(&'static str, bool, String)> {
    let port = svc.port.or(svc.manifest.default_port)?;
    match svc.manifest.config_kind.as_deref() {
        Some("nginx") => {
            let url = if port == 80 {
                "http://127.0.0.1/".to_string()
            } else {
                format!("http://127.0.0.1:{port}/")
            };
            Some(("访问", true, url))
        }
        Some("nacos") => Some(("访问", true, format!("http://127.0.0.1:{port}/nacos"))),
        Some("mailpit") | Some("tomcat") => Some(("访问", true, format!("http://127.0.0.1:{port}/"))),
        Some("mysql") => {
            let auth = match saved_pwd {
                Some(pw) if !pw.is_empty() => format!("root:{pw}"),
                _ => "root".to_string(),
            };
            Some(("连接串", false, format!("mysql://{auth}@127.0.0.1:{port}")))
        }
        Some("postgres") => Some((
            "连接串",
            false,
            format!("postgresql://postgres@127.0.0.1:{port}/postgres"),
        )),
        Some("redis") => Some(("连接串", false, format!("redis://127.0.0.1:{port}"))),
        Some("mongodb") => Some(("连接串", false, format!("mongodb://127.0.0.1:{port}"))),
        _ => None,
    }
}

fn open_file_default(path: &Path) {
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/C", "start", "", &path.to_string_lossy()]);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = cmd.spawn();
}


impl MokaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime"),
        );
        let layout = Arc::new(RuntimeLayout::new(default_runtime_root()));
        let _ = layout.ensure_dirs();
        let app_settings = load_settings(layout.root_path());
        let initial_mode = if app_settings.dark_mode {
            ThemeMode::ClaudeDark
        } else {
            ThemeMode::ClaudeLight
        };
        let initial_region = if app_settings.mirror_china {
            MirrorRegion::China
        } else {
            MirrorRegion::Global
        };
        configure_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, ThemePalette::from_mode(initial_mode));
        let catalog_path = layout.catalog_file();
        let catalog_vec = load_catalog(Some(&catalog_path)).unwrap_or_default();
        write_app_log(
            layout.root_path(),
            "INFO",
            &format!(
                "Moka v{APP_VERSION} 启动 | runtime={} | 组件数={}",
                layout.root_path().display(),
                catalog_vec.len()
            ),
        );
        let process_mgr = Arc::new(ProcessManager::new());
        let initial_views = compute_software_views(&catalog_vec, &layout, &process_mgr);
        let catalog = Arc::new(Mutex::new(catalog_vec));

        let saved_source_url = std::fs::read_to_string(layout.catalog_source_url_file())
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                "https://raw.githubusercontent.com/xuanjinya168/moka/main/manifests/catalog.json"
                    .to_string()
            });

        let initial_presets = load_presets(layout.root_path());
        let initial_sites = load_sites(layout.root_path());
        let cache_size_bytes = calc_dir_size(&layout.root_path().join("downloads"));
        let tray = SystemTray::new(
            cc.egui_ctx.clone(),
            rt.clone(),
            layout.clone(),
            catalog.clone(),
            process_mgr.clone(),
            app_settings.stop_on_exit,
            app_settings.tray_style,
        );

        // 启动时以配置文件里的端口为准，否则自定义端口会在下次启动时被 ensure_service_config 写回默认值
        let mut port_overrides = HashMap::new();
        for v in &initial_views {
            let (_, _, running_port) = process_mgr.is_running(&v.manifest.id);
            let configured_port = read_service_port(
                v.manifest.config_kind.as_deref(),
                &layout.app_dir(&v.manifest.id),
                &layout.conf_dir(&v.manifest.id),
                &layout.data_dir(&v.manifest.id),
                v.manifest.default_port.unwrap_or(0),
            );
            let port = running_port
                .or(configured_port)
                .or(v.manifest.default_port);
            if let Some(pt) = port {
                port_overrides.insert(v.manifest.id.clone(), pt.to_string());
            }
        }

        Self {
            rt,
            layout,
            process_mgr,
            catalog,
            shared: Arc::new(Mutex::new(SharedAsyncState {
                downloads: HashMap::new(),
                cancel_flags: HashMap::new(),
                busy_ids: HashSet::new(),
                toast: None,
                views_dirty: false,
                latest_views: None,
                toolbox_loading: false,
                latest_ports: None,
                latest_hosts: None,
                hosts_saved: None,
                catalog_updating: false,
                update_checking: false,
                update_result: None,
                pwd_updating: false,
                latest_db_pwd: None,
                ca_busy: false,
                ca_trusted_update: None,
                metrics_polling: false,
                latest_metrics: None,
                backup_busy: false,
                latest_backups: None,
            })),
            theme_mode: initial_mode,
            mirror_region: initial_region,
            active_tab: NavTab::Dashboard,
            store_filter: None,
            store_search: String::new(),
            store_selected_versions: HashMap::new(),
            software_views: initial_views,
            port_overrides,
            catalog_url_input: saved_source_url,
            presets: initial_presets,
            preset_modal_open: false,
            preset_editing_idx: None,
            preset_name_input: String::new(),
            preset_selected_ids: HashSet::new(),
            sites: initial_sites,
            site_modal_open: false,
            site_editing_idx: None,
            site_domain_input: String::new(),
            site_port_input: "80".to_string(),
            site_root_input: String::new(),
            site_ssl_enabled: false,
            site_ssl_port_input: "443".to_string(),
            site_ssl_cert_input: String::new(),
            site_ssl_key_input: String::new(),
            site_proxy_rules: Vec::new(),
            site_sync_hosts: true,
            ca_trusted: is_user_root_ca_installed(),
            log_target_id: None,
            log_target_title: String::new(),
            log_lines: Vec::new(),
            log_filter: String::new(),
            log_level_filter: LogLevelFilter::All,
            log_auto_scroll: true,
            last_log_poll: Instant::now(),
            config_editor_view: None,
            config_editor_path: None,
            config_editor_text: String::new(),
            config_editor_visual: None,
            config_editor_advanced: false,
            config_editor_dirty: false,
            config_editor_error: None,
            db_new_pwd_input: String::new(),
            ports: Vec::new(),
            filtered_port_indices: Vec::new(),
            port_filter: String::new(),
            last_port_filter: String::new(),
            hosts_text: String::new(),
            host_entries: Vec::new(),
            hosts_dirty: false,
            host_new_ip: "127.0.0.1".to_string(),
            host_new_domain: String::new(),
            host_new_comment: String::new(),
            toolbox_loaded: false,
            dev_mirrors: inspect_dev_mirrors(),
            tray,
            tray_style: app_settings.tray_style,
            tray_popover_open: false,
            tray_popover_pos: Pos2::new(800.0, 300.0),
            tray_popover_just_opened: false,
            tray_popover_ever_focused: false,
            tray_popover_search: String::new(),
            win_corners_applied: false,
            window_hidden_to_tray: false,
            settings_tab: SettingsSubTab::General,
            stop_on_exit: app_settings.stop_on_exit,
            minimize_to_tray: app_settings.minimize_to_tray,
            auto_start: is_auto_start_enabled() || app_settings.auto_start,
            auto_start_preset: app_settings.auto_start_preset,
            auto_preset_triggered: false,
            release_repo_input: app_settings.release_repo,
            db_passwords: app_settings.db_passwords,
            proc_metrics: HashMap::new(),
            last_metrics_poll: Instant::now() - Duration::from_secs(10),
            backup_modal_svc: None,
            backup_files: Vec::new(),
            cache_size_bytes,
            update_status: None,
        }
    }

    fn persist_settings(&self) {
        self.tray.set_stop_on_exit(self.stop_on_exit);
        let cfg = AppSettings {
            dark_mode: matches!(self.theme_mode, ThemeMode::ClaudeDark),
            mirror_china: matches!(self.mirror_region, MirrorRegion::China),
            stop_on_exit: self.stop_on_exit,
            minimize_to_tray: self.minimize_to_tray,
            tray_style: self.tray_style,
            auto_start: self.auto_start,
            auto_start_preset: self.auto_start_preset.clone(),
            release_repo: self.release_repo_input.trim().to_string(),
            db_passwords: self.db_passwords.clone(),
        };
        save_settings(self.layout.root_path(), &cfg);
    }

    fn spawn_save_hosts(&mut self, ctx: egui::Context) {
        let new_content = render_hosts_file(&self.hosts_text, &self.host_entries);
        let shared = self.shared.clone();
        let layout = self.layout.clone();
        self.rt.spawn_blocking(move || {
            let msg = match write_hosts_file(&new_content) {
                Ok(()) => {
                    write_app_log(layout.root_path(), "INFO", "Windows Hosts 已保存生效");
                    if let Ok(mut s) = shared.lock() {
                        s.hosts_saved = Some(new_content);
                    }
                    "Windows Hosts 已保存并生效".to_string()
                }
                Err(e) => {
                    let m = format!("保存 Hosts 失败: {e:#}");
                    write_app_log(layout.root_path(), "ERROR", &m);
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn spawn_change_mysql_pwd(&self, view: SoftwareStatusView, ctx: egui::Context) {
        let id = view.manifest.id.clone();
        let app_dir = self.layout.app_dir(&id);
        let port = self.effective_port(&view).unwrap_or(3306);
        let old_pwd = self.db_passwords.get(&id).cloned().unwrap_or_default();
        let new_pwd = self.db_new_pwd_input.clone();
        let shared = self.shared.clone();
        let layout = self.layout.clone();

        if let Ok(mut s) = shared.lock() {
            if s.pwd_updating {
                return;
            }
            s.pwd_updating = true;
        }

        self.rt.spawn(async move {
            let msg = match change_mysql_root_password(&app_dir, port, &old_pwd, &new_pwd).await {
                Ok(()) => {
                    if let Ok(mut s) = shared.lock() {
                        s.latest_db_pwd = Some((id.clone(), new_pwd.clone()));
                    }
                    let tip = if new_pwd.is_empty() {
                        "已恢复 root 免密登录（连接串已同步更新）".to_string()
                    } else {
                        "root 密码已修改生效（连接串已同步更新）".to_string()
                    };
                    write_app_log(layout.root_path(), "INFO", &format!("{id}: {tip}"));
                    tip
                }
                Err(e) => {
                    let m = format!("{e:#}");
                    write_app_log(layout.root_path(), "ERROR", &format!("{id} 修改密码失败: {m}"));
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.pwd_updating = false;
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn spawn_check_update(&mut self, ctx: egui::Context) {
        let repo = self.release_repo_input.trim().to_string();
        self.persist_settings();
        let shared = self.shared.clone();
        if let Ok(mut s) = shared.lock() {
            if s.update_checking {
                return;
            }
            s.update_checking = true;
            s.update_result = None;
        }
        self.rt.spawn(async move {
            let res = check_latest_release(&repo)
                .await
                .map_err(|e| format!("{e:#}"));
            if let Ok(mut s) = shared.lock() {
                s.update_checking = false;
                s.update_result = Some(res);
            }
            ctx.request_repaint();
        });
    }

    fn set_toast(&self, msg: impl Into<String>) {
        let text = msg.into();
        let level = if text.contains("失败") || text.contains("错误") || text.contains("无效") || text.contains("无法") {
            "ERROR"
        } else {
            "INFO"
        };
        write_app_log(self.layout.root_path(), level, &text);
        if let Ok(mut s) = self.shared.lock() {
            s.toast = Some((text, Instant::now()));
        }
    }

    fn effective_port(&self, view: &SoftwareStatusView) -> Option<u16> {
        self.port_overrides
            .get(&view.manifest.id)
            .and_then(|s| s.trim().parse::<u16>().ok())
            .filter(|&p| p > 0)
            .or(view.port)
            .or(view.manifest.default_port)
    }

    fn spawn_refresh_views(&self, ctx: egui::Context) {
        self.spawn_refresh_views_inner(ctx, false);
    }

    fn spawn_refresh_views_manual(&self, ctx: egui::Context) {
        self.spawn_refresh_views_inner(ctx, true);
    }

    fn spawn_refresh_views_inner(&self, ctx: egui::Context, notify: bool) {
        let catalog = self.catalog.clone();
        let layout = self.layout.clone();
        let mgr = self.process_mgr.clone();
        let shared = self.shared.clone();
        self.rt.spawn_blocking(move || {
            let list = catalog.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let views = compute_software_views(&list, &layout, &mgr);
            let running = views
                .iter()
                .filter(|v| v.manifest.is_service && v.installed && v.running)
                .count();
            let installed = views
                .iter()
                .filter(|v| v.manifest.is_service && v.installed)
                .count();
            if let Ok(mut s) = shared.lock() {
                s.latest_views = Some(views);
                if notify {
                    s.toast = Some((
                        format!("状态已刷新：{running} 个服务运行中 / 共 {installed} 个已安装"),
                        Instant::now(),
                    ));
                }
            }
            ctx.request_repaint();
        });
    }

    fn reload_local_catalog(&self, ctx: egui::Context) {
        let path = self.layout.catalog_file();
        match load_catalog(Some(&path)) {
            Ok(list) => {
                let count = list.len();
                *self.catalog.lock().unwrap_or_else(|e| e.into_inner()) = list;
                self.set_toast(format!("已重新加载本地软件源（共 {count} 个组件）"));
                self.spawn_refresh_views(ctx);
            }
            Err(e) => self.set_toast(format!("加载本地软件源失败: {e}")),
        }
    }

    fn spawn_update_remote_catalog(&self, ctx: egui::Context) {
        let raw_url = self.catalog_url_input.trim().to_string();
        if raw_url.is_empty() {
            self.set_toast("请输入远程 catalog.json 地址");
            return;
        }
        let _ = std::fs::write(self.layout.catalog_source_url_file(), &raw_url);

        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if s.catalog_updating {
                return;
            }
            s.catalog_updating = true;
        }

        let mut urls = Vec::new();
        if self.mirror_region == MirrorRegion::China && raw_url.contains("githubusercontent.com") {
            urls.push(format!("https://gh-proxy.com/{raw_url}"));
            urls.push(raw_url);
        } else {
            urls.push(raw_url);
        }

        let save_to = self.layout.catalog_file();
        let catalog = self.catalog.clone();
        let shared = self.shared.clone();
        self.rt.spawn(async move {
            let res = fetch_remote_catalog(&urls, &save_to).await;
            let msg = match res {
                Ok(list) => {
                    let count = list.len();
                    *catalog.lock().unwrap_or_else(|e| e.into_inner()) = list;
                    let m = format!("在线软件源更新成功（共 {count} 个组件）");
                    write_app_log(save_to.parent().and_then(|p| p.parent()).unwrap_or(Path::new(".")), "INFO", &m);
                    m
                }
                Err(e) => {
                    let m = format!("在线更新失败: {e:#}");
                    write_app_log(save_to.parent().and_then(|p| p.parent()).unwrap_or(Path::new(".")), "ERROR", &m);
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.catalog_updating = false;
                s.views_dirty = true;
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn spawn_refresh_toolbox(&self, ctx: egui::Context) {
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if s.toolbox_loading {
                return;
            }
            s.toolbox_loading = true;
        }
        let shared = self.shared.clone();
        self.rt.spawn_blocking(move || {
            let ports = list_listening_ports().unwrap_or_default();
            let hosts =
                read_hosts_file().unwrap_or_else(|e| format!("读取 Hosts 文件失败: {e}"));
            if let Ok(mut s) = shared.lock() {
                s.latest_ports = Some(ports);
                s.latest_hosts = Some(hosts);
                s.toolbox_loading = false;
            }
            ctx.request_repaint();
        });
    }

    fn rebuild_port_filter_cache(&mut self) {
        let kw = self.port_filter.trim().to_lowercase();
        self.filtered_port_indices.clear();
        for (i, p) in self.ports.iter().enumerate() {
            if kw.is_empty()
                || p.port.to_string().contains(&kw)
                || p.process_name.to_lowercase().contains(&kw)
                || p.pid.to_string().contains(&kw)
            {
                self.filtered_port_indices.push(i);
            }
        }
        self.last_port_filter = self.port_filter.clone();
    }

    fn trigger_install(&self, manifest: SoftwareManifest, ctx: egui::Context) {
        let id = manifest.id.clone();
        let name = format!("{} {}", manifest.name, manifest.version);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if !s.busy_ids.insert(id.clone()) {
                return;
            }
            s.cancel_flags.insert(id.clone(), cancel_flag.clone());
        }

        let layout = self.layout.clone();
        let shared = self.shared.clone();
        let region = self.mirror_region;
        self.rt.spawn(async move {
            let zip_dest = layout.download_file(&id);
            let extract_dest = layout.app_dir(&id);
            // 按所选区域优先排序镜像源，并自动保留另一套源作为失败兜底回退
            let mut ordered_urls = Vec::new();
            match region {
                MirrorRegion::China => {
                    ordered_urls.extend(manifest.mirrors_cn.iter().cloned());
                    ordered_urls.extend(manifest.mirrors.iter().cloned());
                }
                MirrorRegion::Global => {
                    ordered_urls.extend(manifest.mirrors.iter().cloned());
                    ordered_urls.extend(manifest.mirrors_cn.iter().cloned());
                }
            }
            ordered_urls.dedup();
            let res = download_and_extract_cancellable(
                &id,
                &ordered_urls,
                &zip_dest,
                &extract_dest,
                cancel_flag.clone(),
                |p| {
                    if let Ok(mut s) = shared.lock() {
                        s.downloads.insert(id.clone(), p);
                    }
                    ctx.request_repaint();
                },
            )
            .await;

            if res.is_ok() {
                if let Some(link_name) = &manifest.env_link_name {
                    let link_path = layout.current_link(link_name);
                    if !link_path.exists() {
                        let _ = switch_junction(&link_path, &extract_dest);
                    }
                    if link_name == "maven" {
                        let _ = generate_aliyun_maven_settings(
                            &extract_dest.join("conf"),
                            &layout.data_dir("maven-repo"),
                        );
                    }
                }
            }

            let toast_text = match res {
                Ok(()) => {
                    let m = format!("{name} 安装完成");
                    write_app_log(layout.root_path(), "INFO", &m);
                    m
                }
                Err(_) if cancel_flag.load(Ordering::Relaxed) => {
                    let m = format!("已取消下载 {name}");
                    write_app_log(layout.root_path(), "INFO", &m);
                    m
                }
                Err(e) => {
                    let m = format!("{name} 安装失败: {e:#}");
                    write_app_log(layout.root_path(), "ERROR", &m);
                    m
                }
            };

            if let Ok(mut s) = shared.lock() {
                s.busy_ids.remove(&id);
                s.downloads.remove(&id);
                s.cancel_flags.remove(&id);
                s.views_dirty = true;
                s.toast = Some((toast_text, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn trigger_uninstall(&self, manifest: SoftwareManifest, ctx: egui::Context) {
        let (tracked_running, _, _) = self.process_mgr.is_running(&manifest.id);
        let view_running = self
            .software_views
            .iter()
            .any(|v| v.manifest.id == manifest.id && v.running);
        if tracked_running || view_running {
            self.set_toast("请先停止该服务再卸载");
            return;
        }
        let id = manifest.id.clone();
        let name = format!("{} {}", manifest.name, manifest.version);
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if !s.busy_ids.insert(id.clone()) {
                return;
            }
        }
        let dir = self.layout.app_dir(&id);
        let root = self.layout.root_path().to_path_buf();
        let shared = self.shared.clone();
        self.rt.spawn_blocking(move || {
            write_app_log(&root, "INFO", &format!("开始卸载 {name} ({})", dir.display()));
            let res = safe_remove_app_dir(&dir);
            let toast_text = match res {
                Ok(()) => {
                    let m = format!("已卸载 {name}");
                    write_app_log(&root, "INFO", &m);
                    m
                }
                Err(e) => {
                    let m = format!("卸载失败: {e:#}");
                    write_app_log(&root, "ERROR", &m);
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.busy_ids.remove(&id);
                s.views_dirty = true;
                s.toast = Some((toast_text, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn trigger_activate_env(&self, manifest: SoftwareManifest, ctx: egui::Context) {
        let Some(link_name) = manifest.env_link_name.clone() else {
            return;
        };
        let target = self.layout.app_dir(&manifest.id);
        let link = self.layout.current_link(&link_name);
        let name = format!("{} {}", manifest.name, manifest.version);
        let shared = self.shared.clone();
        self.rt.spawn_blocking(move || {
            let msg = match switch_junction(&link, &target) {
                Ok(()) => format!("已切换 {name} → {}", link.display()),
                Err(e) => format!("切换版本失败: {e}"),
            };
            if let Ok(mut s) = shared.lock() {
                s.views_dirty = true;
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn trigger_toggle_service(&self, view: SoftwareStatusView, ctx: egui::Context) {
        let id = view.manifest.id.clone();
        let name = format!("{} {}", view.manifest.name, view.manifest.version);
        if !view.running {
            let typed = self
                .port_overrides
                .get(&id)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !typed.is_empty() && typed.parse::<u16>().ok().filter(|p| *p > 0).is_none() {
                self.set_toast(format!("{name} 的端口无效，请输入 1 ~ 65535 之间的数字"));
                return;
            }
        }
        let port = self.effective_port(&view);
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if !s.busy_ids.insert(id.clone()) {
                return;
            }
        }

        let mgr = self.process_mgr.clone();
        let layout = self.layout.clone();
        let shared = self.shared.clone();
        self.rt.spawn(async move {
            let msg = if view.running {
                match mgr.stop_service(&view.manifest, &layout).await {
                    Ok(()) => {
                        let m = format!("已停止 {name}");
                        write_app_log(layout.root_path(), "INFO", &m);
                        m
                    }
                    Err(e) => {
                        let m = format!("停止 {name} 失败: {e:#}");
                        write_app_log(layout.root_path(), "ERROR", &m);
                        m
                    }
                }
            } else {
                match mgr.start_service(&view.manifest, &layout, port).await {
                    Ok(pid) => {
                        let m = format!("已启动 {name} (port={:?}, pid={:?})", port, pid);
                        write_app_log(layout.root_path(), "INFO", &m);
                        format!("已启动 {name}")
                    }
                    Err(e) => {
                        let m = format!("启动 {name} 失败: {e:#}");
                        write_app_log(layout.root_path(), "ERROR", &m);
                        m
                    }
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.busy_ids.remove(&id);
                s.views_dirty = true;
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn trigger_open_config(&mut self, view: &SoftwareStatusView) {
        let app_dir = self.layout.app_dir(&view.manifest.id);
        let conf_dir = self.layout.conf_dir(&view.manifest.id);
        let data_dir = self.layout.data_dir(&view.manifest.id);
        let port = self.effective_port(view).unwrap_or(0);
        match ensure_service_config(
            view.manifest.config_kind.as_deref(),
            &app_dir,
            &conf_dir,
            &data_dir,
            port,
        ) {
            Ok(Some(path)) => {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let visual = parse_visual_config(view.manifest.config_kind.as_deref(), &text, port);
                self.config_editor_advanced = visual.is_none();
                self.config_editor_visual = visual;
                self.config_editor_view = Some(view.clone());
                self.config_editor_path = Some(path);
                self.config_editor_text = text;
                self.config_editor_dirty = false;
                self.db_new_pwd_input = self
                    .db_passwords
                    .get(&view.manifest.id)
                    .cloned()
                    .unwrap_or_default();
            }
            Ok(None) => {
                if view.manifest.config_kind.as_deref() == Some("postgres") {
                    self.set_toast(
                        "PostgreSQL 尚未初始化数据目录，请先点击「启动」完成首次初始化后再编辑配置",
                    );
                } else {
                    open_in_explorer(&app_dir);
                }
            }
            Err(e) => self.set_toast(format!("无法打开配置: {e}")),
        }
    }

    fn trigger_restart_service(&self, view: SoftwareStatusView, ctx: egui::Context) {
        let id = view.manifest.id.clone();
        let name = format!("{} {}", view.manifest.name, view.manifest.version);
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if s.busy_ids.contains(&id) {
                return;
            }
            s.busy_ids.insert(id.clone());
            s.toast = Some((format!("正在重启 {name}..."), Instant::now()));
        }

        let port = self.effective_port(&view);
        let mgr = self.process_mgr.clone();
        let layout = self.layout.clone();
        let shared = self.shared.clone();

        self.rt.spawn(async move {
            let _ = mgr.stop_service(&view.manifest, &layout).await;
            tokio::time::sleep(Duration::from_millis(350)).await;
            let msg = match mgr.start_service(&view.manifest, &layout, port).await {
                Ok(pid) => {
                    let m = format!("已重启 {name} (port={:?}, pid={:?})", port, pid);
                    write_app_log(layout.root_path(), "INFO", &m);
                    format!("配置已生效，{name} 已重启完成")
                }
                Err(e) => {
                    let m = format!("重启 {name} 失败: {e:#}");
                    write_app_log(layout.root_path(), "ERROR", &m);
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.busy_ids.remove(&id);
                s.views_dirty = true;
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn save_config_editor(&mut self) -> bool {
        let Some(ref path) = self.config_editor_path else {
            return false;
        };

        // 若处于可视化模式，先校验端口再合并回配置文本
        if !self.config_editor_advanced {
            if let Some(ref vcfg) = self.config_editor_visual {
                let port_str = match vcfg {
                    VisualConfig::Mysql { port, .. }
                    | VisualConfig::Redis { port, .. }
                    | VisualConfig::Nginx { port, .. }
                    | VisualConfig::Nacos { port, .. }
                    | VisualConfig::Postgres { port, .. }
                    | VisualConfig::Tomcat { port, .. }
                    | VisualConfig::MongoDb { port, .. }
                    | VisualConfig::Mailpit { port, .. } => port.trim(),
                };
                match port_str.parse::<u16>() {
                    Ok(pt) if pt > 0 => {
                        if let Some(ref view) = self.config_editor_view {
                            self.port_overrides
                                .insert(view.manifest.id.clone(), pt.to_string());
                        }
                    }
                    _ => {
                        self.set_toast("端口号无效，请输入 1 ~ 65535 之间的数字");
                        return false;
                    }
                }
                self.config_editor_text = apply_visual_config(&self.config_editor_text, vcfg);
            }
        }

        // Nginx 保存前自动使用 `nginx -t` 语法预检，拦截手写语法错误
        if let Some(ref view) = self.config_editor_view {
            if view.manifest.config_kind.as_deref() == Some("nginx") {
                let app_dir = self.layout.app_dir(&view.manifest.id);
                if let Err(e) =
                    validate_nginx_config_text(&app_dir, path, &self.config_editor_text)
                {
                    self.set_toast(format!("{e:#}"));
                    return false;
                }
            }
        }

        // 保存前自动备份 .bak，防止误操作改坏配置
        if path.exists() {
            let bak_path = PathBuf::from(format!("{}.bak", path.to_string_lossy()));
            let _ = std::fs::copy(path, bak_path);
        }

        match std::fs::write(path, &self.config_editor_text) {
            Ok(()) => {
                self.config_editor_dirty = false;
                let running = self
                    .config_editor_view
                    .as_ref()
                    .map(|v| v.running)
                    .unwrap_or(false);
                if running {
                    self.set_toast("配置已保存（已自动备份 .bak），请重启服务生效");
                } else {
                    self.set_toast("配置已保存（已自动备份 .bak）");
                }
                true
            }
            Err(e) => {
                self.set_toast(format!("保存配置失败: {e}"));
                false
            }
        }
    }

    fn reset_config_editor_default(&mut self) {
        let Some(ref view) = self.config_editor_view else {
            return;
        };
        let app_dir = self.layout.app_dir(&view.manifest.id);
        let data_dir = self.layout.data_dir(&view.manifest.id);
        let port = view.manifest.default_port.unwrap_or(0);
        if let Some(default_txt) = default_service_config_content(
            view.manifest.config_kind.as_deref(),
            &app_dir,
            &data_dir,
            port,
        ) {
            self.config_editor_visual =
                parse_visual_config(view.manifest.config_kind.as_deref(), &default_txt, port);
            self.config_editor_text = default_txt;
            self.config_editor_dirty = true;
            self.set_toast("已恢复官方安全默认模板，点击「保存配置」生效");
        }
    }

    fn trigger_kill_port(&self, p: PortOccupancy, ctx: egui::Context) {
        let shared = self.shared.clone();
        self.rt.spawn_blocking(move || {
            let msg = match kill_pid(p.pid) {
                Ok(()) => format!("已结束进程 {} (PID {})", p.process_name, p.pid),
                Err(e) => format!("{e}"),
            };
            let ports = list_listening_ports().unwrap_or_default();
            if let Ok(mut s) = shared.lock() {
                s.latest_ports = Some(ports);
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn open_logs(&mut self, view: &SoftwareStatusView) {
        self.log_target_id = Some(view.manifest.id.clone());
        self.log_target_title = format!("{} {} 实时日志", view.manifest.name, view.manifest.version);
        let log_file = self.layout.log_file(&view.manifest.id);
        self.log_lines = self.process_mgr.load_persisted_logs(&view.manifest.id, &log_file);
        self.log_filter.clear();
        self.log_level_filter = LogLevelFilter::All;
        self.last_log_poll = Instant::now();
    }

    fn sync_sites_and_reload(
        &self,
        sync_domain_to_hosts: Option<String>,
        ctx: egui::Context,
    ) {
        let _ = save_sites(self.layout.root_path(), &self.sites);
        let nginx_view = self
            .software_views
            .iter()
            .find(|v| v.manifest.config_kind.as_deref() == Some("nginx") && v.installed)
            .cloned();
        let Some(nginx) = nginx_view else {
            self.set_toast("站点配置已保存（安装 Nginx 后自动生效）");
            return;
        };

        let sites = self.sites.clone();
        let layout = self.layout.clone();
        let shared = self.shared.clone();
        let nginx_id = nginx.manifest.id.clone();
        let nginx_port = self.effective_port(&nginx).unwrap_or(80);
        let nginx_running = nginx.running;

        self.rt.spawn_blocking(move || {
            let app_dir = layout.app_dir(&nginx_id);
            let conf_dir = layout.conf_dir(&nginx_id);
            let data_dir = layout.data_dir(&nginx_id);
            let vhosts_dir = conf_dir.join("vhosts");
            let sites_data_root = layout.root_path().join("data").join("sites");

            let sync_res = ensure_service_config(
                Some("nginx"),
                &app_dir,
                &conf_dir,
                &data_dir,
                nginx_port,
            )
            .and_then(|opt| {
                let conf_path = opt.unwrap_or_else(|| conf_dir.join("nginx.conf"));
                sync_nginx_vhosts(&conf_path, &vhosts_dir, &sites_data_root, &sites)?;
                if nginx_running {
                    reload_nginx(&app_dir, &conf_path)?;
                }
                Ok(())
            });

            // 按需将站点域名写入 Windows Hosts（127.0.0.1 <domain>）
            let mut hosts_note = String::new();
            if let Some(dom) = sync_domain_to_hosts {
                let clean = dom.trim();
                if !clean.is_empty() && !clean.eq_ignore_ascii_case("localhost") {
                    if let Ok(raw) = read_hosts_file() {
                        let mut entries = parse_hosts_entries(&raw);
                        let exists = entries
                            .iter()
                            .any(|e| e.enabled && e.domain.eq_ignore_ascii_case(clean));
                        if !exists {
                            entries.push(HostEntry {
                                ip: "127.0.0.1".to_string(),
                                domain: clean.to_string(),
                                comment: "Moka Site".to_string(),
                                enabled: true,
                            });
                            let new_hosts = render_hosts_file(&raw, &entries);
                            match write_hosts_file(&new_hosts) {
                                Ok(()) => {
                                    if let Ok(mut s) = shared.lock() {
                                        s.latest_hosts = Some(new_hosts);
                                    }
                                    hosts_note = format!("，已写入 Hosts ({clean})");
                                }
                                Err(_) => {
                                    hosts_note = "（Hosts 写入未授权，可稍后在工具箱手动添加）".to_string();
                                }
                            }
                        }
                    }
                }
            }

            let msg = match sync_res {
                Ok(()) => {
                    let base = if nginx_running {
                        "站点已同步并热重载 Nginx 生效"
                    } else {
                        "站点配置已同步（启动 Nginx 后生效）"
                    };
                    let full = format!("{base}{hosts_note}");
                    write_app_log(layout.root_path(), "INFO", &full);
                    full
                }
                Err(e) => {
                    let m = format!("{e:#}");
                    write_app_log(layout.root_path(), "ERROR", &m);
                    m
                }
            };
            if let Ok(mut s) = shared.lock() {
                s.toast = Some((msg, Instant::now()));
            }
            ctx.request_repaint();
        });
    }

    fn execute_actions(&mut self, actions: Vec<UiAction>, ctx: &egui::Context) {
        for act in actions {
            match act {
                UiAction::Install(m) => self.trigger_install(m, ctx.clone()),
                UiAction::CancelDownload(id) => {
                    if let Ok(s) = self.shared.lock() {
                        if let Some(flag) = s.cancel_flags.get(&id) {
                            flag.store(true, Ordering::Relaxed);
                        }
                    }
                }
                UiAction::Uninstall(m) => self.trigger_uninstall(m, ctx.clone()),
                UiAction::ActivateEnv(m) => self.trigger_activate_env(m, ctx.clone()),
                UiAction::ToggleService(v) => self.trigger_toggle_service(v, ctx.clone()),
                UiAction::StartAllServices => {
                    let targets: Vec<SoftwareStatusView> = self
                        .software_views
                        .iter()
                        .filter(|v| v.manifest.is_service && v.installed && !v.running)
                        .cloned()
                        .collect();
                    if targets.is_empty() {
                        self.set_toast("所有已安装服务已在运行中");
                    } else {
                        for v in targets {
                            self.trigger_toggle_service(v, ctx.clone());
                        }
                    }
                }
                UiAction::StopAllServices => {
                    let targets: Vec<SoftwareStatusView> = self
                        .software_views
                        .iter()
                        .filter(|v| v.manifest.is_service && v.running)
                        .cloned()
                        .collect();
                    if targets.is_empty() {
                        self.set_toast("当前没有运行中的服务");
                    } else {
                        for v in targets {
                            self.trigger_toggle_service(v, ctx.clone());
                        }
                    }
                }
                UiAction::StartPreset(preset_name, ids) => {
                    let mut started = 0usize;
                    let mut missing: Vec<String> = Vec::new();
                    for id in &ids {
                        if let Some(v) = self
                            .software_views
                            .iter()
                            .find(|v| &v.manifest.id == id)
                            .cloned()
                        {
                            if !v.installed {
                                missing.push(format!("{} {}", v.manifest.name, v.manifest.version));
                            } else if !v.running {
                                self.trigger_toggle_service(v, ctx.clone());
                                started += 1;
                            }
                        }
                    }
                    if !missing.is_empty() {
                        self.set_toast(format!(
                            "{preset_name}：缺少 {}，请先在软件商店安装",
                            missing.join("、")
                        ));
                    } else if started > 0 {
                        self.set_toast(format!("正在一键启动 {preset_name} ({started} 个服务)"));
                    } else {
                        self.set_toast(format!("{preset_name} 服务已全部在运行"));
                    }
                }
                UiAction::OpenPresetEditor(idx_opt) => {
                    self.preset_editing_idx = idx_opt;
                    if let Some(idx) = idx_opt {
                        if let Some(existing) = self.presets.get(idx) {
                            self.preset_name_input = existing.name.clone();
                            self.preset_selected_ids =
                                existing.service_ids.iter().cloned().collect();
                        }
                    } else {
                        self.preset_name_input.clear();
                        self.preset_selected_ids.clear();
                    }
                    self.preset_modal_open = true;
                }
                UiAction::OpenLogs(v) => self.open_logs(&v),
                UiAction::OpenDir(path) => open_in_explorer(&path),
                UiAction::OpenConfig(v) => self.trigger_open_config(&v),
                UiAction::KillPort(p) => self.trigger_kill_port(p, ctx.clone()),
                UiAction::SwitchTab(tab) => {
                    self.active_tab = tab;
                    if tab == NavTab::Toolbox {
                        self.dev_mirrors = inspect_dev_mirrors();
                        if !self.toolbox_loaded {
                            self.spawn_refresh_toolbox(ctx.clone());
                        }
                    }
                    if tab == NavTab::Settings {
                        self.cache_size_bytes =
                            calc_dir_size(&self.layout.root_path().join("downloads"));
                    }
                }
                UiAction::OpenMokaTerminal => {
                    open_moka_terminal(self.layout.root_path());
                    self.set_toast("已打开 Moka 专属开发终端（零污染系统 PATH）");
                }
                UiAction::OpenUrl(url) => {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }
                UiAction::CopyConnStr(uri) => {
                    ctx.copy_text(uri.clone());
                    self.set_toast(format!("已复制连接串: {uri}"));
                }
                UiAction::OpenSiteEditor(idx_opt) => {
                    self.site_editing_idx = idx_opt;
                    if let Some(idx) = idx_opt {
                        if let Some(s) = self.sites.get(idx) {
                            self.site_domain_input = s.domain.clone();
                            self.site_port_input = s.port.to_string();
                            self.site_root_input = s.root_dir.clone();
                            self.site_ssl_enabled = s.ssl_enabled;
                            self.site_ssl_port_input = s.ssl_port.to_string();
                            self.site_ssl_cert_input = s.ssl_cert.clone();
                            self.site_ssl_key_input = s.ssl_key.clone();
                            self.site_proxy_rules = s.proxy_rules.clone();
                            self.site_sync_hosts = false;
                        }
                    } else {
                        self.site_domain_input.clear();
                        self.site_port_input = "80".to_string();
                        self.site_root_input.clear();
                        self.site_ssl_enabled = false;
                        self.site_ssl_port_input = "443".to_string();
                        self.site_ssl_cert_input.clear();
                        self.site_ssl_key_input.clear();
                        self.site_proxy_rules.clear();
                        self.site_sync_hosts = true;
                    }
                    self.site_modal_open = true;
                }
                UiAction::ToggleSiteEnabled(idx) => {
                    if let Some(s) = self.sites.get_mut(idx) {
                        s.enabled = !s.enabled;
                    }
                    self.sync_sites_and_reload(None, ctx.clone());
                }
                UiAction::DeleteSite(idx) => {
                    if idx < self.sites.len() {
                        self.sites.remove(idx);
                        self.sync_sites_and_reload(None, ctx.clone());
                    }
                }
                UiAction::ReloadNginx => {
                    self.sync_sites_and_reload(None, ctx.clone());
                }
                UiAction::InstallLocalCa => {
                    let shared = self.shared.clone();
                    let layout = self.layout.clone();
                    let c = ctx.clone();
                    {
                        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                        if s.ca_busy {
                            continue;
                        }
                        s.ca_busy = true;
                    }
                    self.set_toast("正在安装 Moka Local Dev CA，请在系统弹窗中点击「是」确认信任...");
                    self.rt.spawn_blocking(move || {
                        let ssl_dir = layout.root_path().join("ssl");
                        let res = ensure_local_root_ca(&ssl_dir)
                            .and_then(|(ca_crt, _)| install_user_root_ca(&ca_crt));
                        let trusted = is_user_root_ca_installed();
                        let msg = match res {
                            Ok(()) => {
                                write_app_log(
                                    layout.root_path(),
                                    "INFO",
                                    "已安装并信任 Moka Local Dev CA 到当前用户受信任根存储",
                                );
                                "已信任 Moka Local Dev CA（重开浏览器后本地 HTTPS 自动绿锁）"
                                    .to_string()
                            }
                            Err(e) => {
                                write_app_log(
                                    layout.root_path(),
                                    "WARN",
                                    &format!("安装本地根证书未完成: {e}"),
                                );
                                format!("安装本地 CA 未完成: {e}")
                            }
                        };
                        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                        s.ca_busy = false;
                        s.ca_trusted_update = Some(trusted);
                        s.toast = Some((msg, Instant::now()));
                        c.request_repaint();
                    });
                }
                UiAction::UninstallLocalCa => {
                    let shared = self.shared.clone();
                    let layout = self.layout.clone();
                    let c = ctx.clone();
                    {
                        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                        if s.ca_busy {
                            continue;
                        }
                        s.ca_busy = true;
                    }
                    self.rt.spawn_blocking(move || {
                        let res = uninstall_user_root_ca();
                        let trusted = is_user_root_ca_installed();
                        let msg = match res {
                            Ok(()) => {
                                write_app_log(
                                    layout.root_path(),
                                    "INFO",
                                    "已从当前用户受信任根存储移除 Moka Local Dev CA",
                                );
                                "已移除 Moka Local Dev CA 信任".to_string()
                            }
                            Err(e) => {
                                write_app_log(
                                    layout.root_path(),
                                    "WARN",
                                    &format!("移除本地根证书失败: {e}"),
                                );
                                format!("移除本地 CA 失败: {e}")
                            }
                        };
                        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                        s.ca_busy = false;
                        s.ca_trusted_update = Some(trusted);
                        s.toast = Some((msg, Instant::now()));
                        c.request_repaint();
                    });
                }
                UiAction::OpenDbBackup(svc) => {
                    let bdir = self.layout.root_path().join("backups").join(&svc.manifest.id);
                    self.backup_files = list_db_backups(&bdir);
                    self.backup_modal_svc = Some(svc);
                }
                UiAction::SetDevMirror(kind, use_china) => match set_dev_mirror(kind, use_china) {
                    Ok(path) => {
                        self.dev_mirrors = inspect_dev_mirrors();
                        let mode_lbl = if use_china { "国内高速镜像" } else { "官方默认源" };
                        write_app_log(
                            self.layout.root_path(),
                            "INFO",
                            &format!(
                                "已切换 {} 为 {} ({})",
                                kind.label(),
                                mode_lbl,
                                path.display()
                            ),
                        );
                        self.set_toast(format!("已将 {} 切换为{}", kind.label(), mode_lbl));
                    }
                    Err(e) => {
                        write_app_log(
                            self.layout.root_path(),
                            "ERROR",
                            &format!("切换 {} 镜像失败: {e}", kind.label()),
                        );
                        self.set_toast(format!("切换 {} 失败: {e}", kind.label()));
                    }
                },
                UiAction::SetAllDevMirrors(use_china) => {
                    let mut errs = Vec::new();
                    for kind in DevMirrorKind::ALL {
                        if let Err(e) = set_dev_mirror(kind, use_china) {
                            errs.push(format!("{}: {e}", kind.label()));
                        }
                    }
                    self.dev_mirrors = inspect_dev_mirrors();
                    let mode_lbl = if use_china { "国内高速镜像" } else { "官方默认源" };
                    if errs.is_empty() {
                        write_app_log(
                            self.layout.root_path(),
                            "INFO",
                            &format!("已将全部 4 项包管理器切换为{mode_lbl}"),
                        );
                        self.set_toast(format!("已将 npm / pip / Go / Maven 全部切换为{mode_lbl}"));
                    } else {
                        self.set_toast(format!("部分镜像切换失败: {}", errs.join("；")));
                    }
                }
            }
        }
    }
}

impl eframe::App for MokaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let p = ThemePalette::from_mode(self.theme_mode);
        let is_minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if ctx.input(|i| i.focused) && !is_minimized {
            self.window_hidden_to_tray = false;
        }
        // 仅在窗口最小化或隐藏至托盘（且未展开托盘悬浮面板）时限速，绝不拖慢前台 120Hz/144Hz 高刷可见窗口
        let is_background = self.window_hidden_to_tray || is_minimized;
        if is_background && !self.tray_popover_open {
            std::thread::sleep(Duration::from_millis(24));
        }

        if !self.win_corners_applied {
            apply_win11_rounded_corners();
            self.win_corners_applied = true;
        }
        if !self.auto_preset_triggered {
            self.auto_preset_triggered = true;
            if !self.auto_start_preset.is_empty() {
                if let Some(preset) = self
                    .presets
                    .iter()
                    .find(|pr| pr.name == self.auto_start_preset)
                    .cloned()
                {
                    self.execute_actions(
                        vec![UiAction::StartPreset(preset.name, preset.service_ids)],
                        ctx,
                    );
                }
            }
        }

        // 若启用「最小化到托盘」，拦截 Alt+F4 / 任务栏关闭信号改为无死锁托盘隐藏
        if self.minimize_to_tray && ctx.input(|i| i.viewport().close_requested()) {
            self.window_hidden_to_tray = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            moka_sys::hide_window_to_tray_by_title("Moka v");
        }

        let mut need_spawn_views = false;
        let ppp = ctx.pixels_per_point().max(1.0);
        for cmd in self.tray.drain_commands() {
            match cmd {
                TrayCommand::ShowMain => {
                    self.window_hidden_to_tray = false;
                }
                TrayCommand::StoppedAll => {
                    need_spawn_views = true;
                    self.set_toast("已通过系统托盘停止全部托管服务");
                }
                TrayCommand::TogglePopover(px, py) => {
                    if self.tray_popover_open {
                        self.tray_popover_open = false;
                    } else {
                        let lx = (px as f32 / ppp - 170.0).max(16.0);
                        let ly = (py as f32 / ppp - 452.0).max(16.0);
                        self.tray_popover_pos = Pos2::new(lx, ly);
                        self.tray_popover_just_opened = true;
                        self.tray_popover_ever_focused = false;
                        self.tray_popover_search.clear();
                        self.tray_popover_open = true;
                    }
                }
            }
        }
        let mut need_rebuild_ports = false;
        let mut hosts_draft_kept = false;
        let (downloads_snapshot, busy_snapshot, toast_snapshot, toolbox_loading, update_checking) = {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(views) = s.latest_views.take() {
                self.software_views = views;
            }
            if let Some(ports) = s.latest_ports.take() {
                self.ports = ports;
                self.toolbox_loaded = true;
                need_rebuild_ports = true;
            }
            if let Some(hosts) = s.hosts_saved.take() {
                self.host_entries = parse_hosts_entries(&hosts);
                self.hosts_text = hosts;
                self.hosts_dirty = false;
            }
            if let Some(hosts) = s.latest_hosts.take() {
                // 有未保存的编辑时不能拿磁盘内容覆盖草稿
                if self.hosts_dirty {
                    hosts_draft_kept = true;
                } else {
                    self.host_entries = parse_hosts_entries(&hosts);
                    self.hosts_text = hosts;
                }
            }
            if let Some(upd) = s.update_result.take() {
                self.update_status = Some(upd);
            }
            if let Some((db_id, new_pw)) = s.latest_db_pwd.take() {
                if new_pw.is_empty() {
                    self.db_passwords.remove(&db_id);
                } else {
                    self.db_passwords.insert(db_id, new_pw.clone());
                }
                self.db_new_pwd_input = new_pw;
                self.persist_settings();
            }
            if let Some(trusted) = s.ca_trusted_update.take() {
                self.ca_trusted = trusted;
            }
            if let Some(metrics) = s.latest_metrics.take() {
                self.proc_metrics = metrics;
            }
            if let Some(backups) = s.latest_backups.take() {
                self.backup_files = backups;
            }
            if s.views_dirty {
                s.views_dirty = false;
                need_spawn_views = true;
            }
            if let Some((_, ts)) = &s.toast {
                if ts.elapsed() > Duration::from_secs(4) {
                    s.toast = None;
                }
            }
            (
                s.downloads.clone(),
                s.busy_ids.clone(),
                s.toast.clone(),
                s.toolbox_loading,
                s.update_checking,
            )
        };

        if hosts_draft_kept {
            self.set_toast("Hosts 有未保存的修改，已保留当前编辑内容");
        }
        if need_rebuild_ports {
            self.rebuild_port_filter_cache();
        }
        if need_spawn_views {
            self.spawn_refresh_views(ctx.clone());
        }

        if let Some(ref log_id) = self.log_target_id {
            if !is_background && self.last_log_poll.elapsed() >= Duration::from_secs(1) {
                let log_file = self.layout.log_file(log_id);
                self.log_lines = self.process_mgr.load_persisted_logs(log_id, &log_file);
                self.last_log_poll = Instant::now();
            }
            if !is_background {
                ctx.request_repaint_after(Duration::from_secs(1));
            }
        }
        if !is_background
            && (!downloads_snapshot.is_empty() || !busy_snapshot.is_empty() || toolbox_loading)
        {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        if self.port_filter != self.last_port_filter {
            self.rebuild_port_filter_cache();
        }

        let running_count = self.software_views.iter().filter(|v| v.running).count();
        if !is_background
            && running_count > 0
            && self.last_metrics_poll.elapsed() >= Duration::from_secs(4)
        {
            let pids: Vec<u32> = self
                .software_views
                .iter()
                .filter(|v| v.running)
                .filter_map(|v| v.pid)
                .collect();
            let can_poll = {
                let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
                if s.metrics_polling || pids.is_empty() {
                    false
                } else {
                    s.metrics_polling = true;
                    true
                }
            };
            if can_poll {
                self.last_metrics_poll = Instant::now();
                let shared = self.shared.clone();
                let c = ctx.clone();
                self.rt.spawn_blocking(move || {
                    let metrics = query_process_metrics(&pids);
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.metrics_polling = false;
                    s.latest_metrics = Some(metrics);
                    c.request_repaint();
                });
            }
        }
        if !is_background && running_count > 0 {
            ctx.request_repaint_after(Duration::from_secs(4));
        }
        let mut actions: Vec<UiAction> = Vec::new();
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

        // 无边框窗口四边与四角缩放支持
        if !is_maximized {
            handle_frameless_resize(ctx);
        }

        // 左侧暖沙导航栏（通顶设计，无横向割裂线，与 Claude 桌面端完全一致）
        egui::SidePanel::left("sidebar")
            .exact_width(214.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(p.bg_sidebar)
                    .stroke(Stroke::NONE)
                    .inner_margin(Margin {
                        left: 16,
                        right: 16,
                        top: 18,
                        bottom: 18,
                    }),
            )
            .show(ctx, |ui| {
                // 侧栏顶部区域支持拖拽移动窗口
                let side_rect = ui.max_rect();
                let side_drag = Rect::from_min_size(
                    Pos2::new(side_rect.min.x - 16.0, side_rect.min.y - 18.0),
                    Vec2::new(214.0, 56.0),
                );
                let side_drag_resp = ui.interact(
                    side_drag,
                    ui.id().with("side_drag"),
                    egui::Sense::click_and_drag(),
                );
                if side_drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if side_drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // 侧栏右侧唯一一条通顶柔和细分割线
                let full_side = ctx.screen_rect();
                ui.painter().line_segment(
                    [
                        Pos2::new(full_side.min.x + 214.0, full_side.min.y),
                        Pos2::new(full_side.min.x + 214.0, full_side.max.y),
                    ],
                    Stroke::new(1.0_f32, p.border_subtle),
                );

                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), 38.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        // 精确 36x36 品牌图标（几何 + 视觉双居中，与右侧两行文字上下齐平）
                        let (logo_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(logo_rect, CornerRadius::same(9), p.accent);
                        ui.painter().text(
                            Pos2::new(logo_rect.center().x, logo_rect.center().y - 0.5),
                            egui::Align2::CENTER_CENTER,
                            "M",
                            egui::FontId::proportional(16.5),
                            Color32::WHITE,
                        );

                        ui.add_space(6.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.label(
                                    RichText::new("Moka")
                                        .strong()
                                        .size(16.5)
                                        .color(p.text_primary),
                                );
                                Frame::new()
                                    .fill(p.accent_soft)
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin {
                                        left: 7,
                                        right: 7,
                                        top: 1,
                                        bottom: 2,
                                    })
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new("BETA")
                                                .strong()
                                                .size(10.0)
                                                .color(p.accent_text),
                                        );
                                    });
                            });
                            ui.label(
                                RichText::new(format!("v{APP_VERSION}"))
                                    .size(11.0)
                                    .color(p.text_muted),
                            );
                        });
                    },
                );

                ui.add_space(24.0);

                let nav_items = [
                    (NavTab::Dashboard, "仪表盘", None),
                    (
                        NavTab::Services,
                        "服务管理",
                        if running_count > 0 {
                            Some(running_count.to_string())
                        } else {
                            None
                        },
                    ),
                    (
                        NavTab::Sites,
                        "站点管理",
                        if !self.sites.is_empty() {
                            Some(self.sites.len().to_string())
                        } else {
                            None
                        },
                    ),
                    (NavTab::Store, "软件商店", None),
                    (NavTab::Toolbox, "实用工具箱", None),
                ];

                for (tab, label, badge) in nav_items {
                    let selected = self.active_tab == tab;
                    let id = ui.make_persistent_id(("nav_tab", tab as u8));
                    let is_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
                    let fill = if selected {
                        p.bg_nav_active
                    } else if is_hovered {
                        p.bg_nav_hover
                    } else {
                        Color32::TRANSPARENT
                    };
                    let text_color = if selected {
                        p.text_primary
                    } else {
                        p.text_secondary
                    };
                    let icon_color = if selected { p.accent } else { p.text_muted };

                    let resp = Frame::new()
                        .fill(fill)
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                draw_nav_icon(ui, tab, icon_color);
                                ui.add_space(2.0);
                                let text = if selected {
                                    RichText::new(label).strong().size(13.5).color(text_color)
                                } else {
                                    RichText::new(label).size(13.5).color(text_color)
                                };
                                ui.label(text);
                                if let Some(b) = badge {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        pill_badge(ui, &b, p.success_soft, p.success_text);
                                    });
                                }
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                    ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
                    if resp.clicked() {
                        actions.push(UiAction::SwitchTab(tab));
                    }
                    ui.add_space(3.0);
                }

                // 左下角固定「设置」入口（与主导航自然分离）
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(2.0);
                    let selected = self.active_tab == NavTab::Settings;
                    let id = ui.make_persistent_id(("nav_tab", NavTab::Settings as u8));
                    let is_hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
                    let fill = if selected {
                        p.bg_nav_active
                    } else if is_hovered {
                        p.bg_nav_hover
                    } else {
                        Color32::TRANSPARENT
                    };
                    let text_color = if selected {
                        p.text_primary
                    } else {
                        p.text_secondary
                    };
                    let icon_color = if selected { p.accent } else { p.text_muted };

                    let resp = Frame::new()
                        .fill(fill)
                        .corner_radius(CornerRadius::same(9))
                        .inner_margin(Margin::symmetric(11, 9))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                draw_nav_icon(ui, NavTab::Settings, icon_color);
                                ui.add_space(2.0);
                                let text = if selected {
                                    RichText::new("设置").strong().size(13.5).color(text_color)
                                } else {
                                    RichText::new("设置").size(13.5).color(text_color)
                                };
                                ui.label(text);
                            });
                        })
                        .response
                        .interact(egui::Sense::click())
                        .on_hover_cursor(egui::CursorIcon::PointingHand);

                    ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
                    if resp.clicked() {
                        actions.push(UiAction::SwitchTab(NavTab::Settings));
                    }
                    ui.add_space(12.0);
                    let sep_rect = ui.available_rect_before_wrap();
                    let sep_y = sep_rect.max.y - 1.0;
                    ui.painter().line_segment(
                        [
                            Pos2::new(sep_rect.min.x + 6.0, sep_y),
                            Pos2::new(sep_rect.max.x - 6.0, sep_y),
                        ],
                        Stroke::new(1.0_f32, p.border_subtle),
                    );
                });
            });

        // 主内容区（通顶一体化，右上角内嵌无割裂窗口控制按钮）
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.bg_root).inner_margin(Margin::ZERO))
            .show(ctx, |ui| {
                let panel_rect = ui.max_rect();
                let bar_h = 36.0_f32;
                let btn_w = 46.0_f32;
                let controls_w = btn_w * 3.0;

                // 顶部隐形拖拽条（无任何分割线）
                let drag_rect = Rect::from_min_max(
                    panel_rect.min,
                    Pos2::new(panel_rect.max.x - controls_w, panel_rect.min.y + bar_h),
                );
                let drag_resp = ui.interact(
                    drag_rect,
                    ui.id().with("main_title_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // 右上角极简窗口控制按钮（最小化 / 最大化 / 关闭）
                let x_close = panel_rect.max.x - btn_w;
                let x_max = x_close - btn_w;
                let x_min = x_max - btn_w;

                if window_control_btn(
                    ui,
                    Rect::from_min_size(Pos2::new(x_min, panel_rect.min.y), Vec2::new(btn_w, bar_h)),
                    "win_min",
                    p.bg_nav_hover,
                    p.text_secondary,
                    false,
                    |painter, c, color| {
                        painter.line_segment(
                            [Pos2::new(c.x - 5.0, c.y), Pos2::new(c.x + 5.0, c.y)],
                            Stroke::new(1.3_f32, color),
                        );
                    },
                ) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }

                if window_control_btn(
                    ui,
                    Rect::from_min_size(Pos2::new(x_max, panel_rect.min.y), Vec2::new(btn_w, bar_h)),
                    "win_max",
                    p.bg_nav_hover,
                    p.text_secondary,
                    false,
                    |painter, c, color| {
                        let s = Stroke::new(1.25_f32, color);
                        if is_maximized {
                            let back = Rect::from_center_size(
                                Pos2::new(c.x + 1.5, c.y - 1.5),
                                Vec2::splat(8.0),
                            );
                            let front = Rect::from_center_size(
                                Pos2::new(c.x - 1.5, c.y + 1.5),
                                Vec2::splat(8.0),
                            );
                            painter.rect_stroke(back, 1.0, s, egui::StrokeKind::Middle);
                            painter.rect_filled(front, 1.0, p.bg_root);
                            painter.rect_stroke(front, 1.0, s, egui::StrokeKind::Middle);
                        } else {
                            let r = Rect::from_center_size(c, Vec2::splat(9.5));
                            painter.rect_stroke(r, 1.2, s, egui::StrokeKind::Middle);
                        }
                    },
                ) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                if window_control_btn(
                    ui,
                    Rect::from_min_size(Pos2::new(x_close, panel_rect.min.y), Vec2::new(btn_w, bar_h)),
                    "win_close",
                    p.danger,
                    p.text_secondary,
                    true,
                    |painter, c, color| {
                        let s = Stroke::new(1.35_f32, color);
                        painter.line_segment(
                            [Pos2::new(c.x - 4.8, c.y - 4.8), Pos2::new(c.x + 4.8, c.y + 4.8)],
                            s,
                        );
                        painter.line_segment(
                            [Pos2::new(c.x + 4.8, c.y - 4.8), Pos2::new(c.x - 4.8, c.y + 4.8)],
                            s,
                        );
                    },
                ) {
                    if self.minimize_to_tray {
                        self.window_hidden_to_tray = true;
                        moka_sys::hide_window_to_tray_by_title("Moka v");
                    } else {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }

                // 顶栏（左右保留 28px 留白，与卡片严格对齐）
                let header_h = 48.0_f32;
                let header_rect = Rect::from_min_max(
                    Pos2::new(panel_rect.min.x + 28.0, panel_rect.min.y + bar_h + 2.0),
                    Pos2::new(
                        panel_rect.max.x - 28.0,
                        panel_rect.min.y + bar_h + 2.0 + header_h,
                    ),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(header_rect)
                        .layout(Layout::top_down(Align::LEFT)),
                    |ui| {
                        ui.horizontal(|ui| {
                            let (title, subtitle) = match self.active_tab {
                                NavTab::Dashboard => ("环境总览", "全栈本地运行时、数据库与服务一站式管理"),
                                NavTab::Services => ("服务管理", "自定义端口、可视化配置、无黑框静默启停"),
                                NavTab::Sites => ("站点管理", "Nginx 多域名虚拟主机、独立反代、本地 HTTPS 与 Hosts 联动"),
                                NavTab::Store => ("软件商店", "多语言运行时与主流数据库一键下载安装"),
                                NavTab::Toolbox => ("实用工具箱", "本机 TCP 端口查杀与 Hosts 可视化管理"),
                                NavTab::Settings => ("设置", "外观偏好、软件源策略、使用教程与版本更新"),
                            };
                            ui.vertical(|ui| {
                                ui.label(RichText::new(title).strong().size(21.0).color(p.text_primary));
                                ui.add_space(2.0);
                                ui.label(RichText::new(subtitle).size(12.0).color(p.text_muted));
                            });

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                match self.active_tab {
                                    NavTab::Dashboard | NavTab::Services => {
                                        if secondary_button(ui, p, "刷新状态", 84.0, true).clicked()
                                        {
                                            self.spawn_refresh_views_manual(ctx.clone());
                                        }
                                    }
                                    NavTab::Sites => {
                                        if primary_button(ui, p, "+ 新建站点", 96.0, true).clicked()
                                        {
                                            actions.push(UiAction::OpenSiteEditor(None));
                                        }
                                        let nginx_running = self.software_views.iter().any(|v| {
                                            v.manifest.config_kind.as_deref() == Some("nginx")
                                                && v.running
                                        });
                                        if nginx_running
                                            && secondary_button(ui, p, "热重载 Nginx", 104.0, true)
                                                .clicked()
                                        {
                                            actions.push(UiAction::ReloadNginx);
                                        }
                                    }
                                    NavTab::Store => {
                                        if secondary_button(ui, p, "管理软件源", 92.0, true)
                                            .clicked()
                                        {
                                            self.active_tab = NavTab::Settings;
                                            self.settings_tab = SettingsSubTab::Sources;
                                        }
                                        let (region_lbl, region_bg, region_fg) =
                                            match self.mirror_region {
                                                MirrorRegion::China => {
                                                    ("源: 中国大陆", p.accent_soft, p.accent_text)
                                                }
                                                MirrorRegion::Global => {
                                                    ("源: 国际官方", p.tag_bg, p.tag_text)
                                                }
                                            };
                                        if soft_button(ui, region_lbl, region_bg, region_fg, 96.0, true)
                                            .clicked()
                                        {
                                            self.mirror_region = match self.mirror_region {
                                                MirrorRegion::China => MirrorRegion::Global,
                                                MirrorRegion::Global => MirrorRegion::China,
                                            };
                                            self.persist_settings();
                                            let tip = match self.mirror_region {
                                                MirrorRegion::China => "已切换为「中国大陆镜像优先」（失败自动回退国际官方源）",
                                                MirrorRegion::Global => "已切换为「国际官方源优先」（失败自动回退国内镜像）",
                                            };
                                            self.set_toast(tip);
                                        }
                                    }
                                    NavTab::Toolbox => {
                                        let lbl = if toolbox_loading {
                                            "扫描中..."
                                        } else {
                                            "重新扫描端口"
                                        };
                                        if secondary_button(ui, p, lbl, 108.0, !toolbox_loading)
                                            .clicked()
                                        {
                                            self.spawn_refresh_toolbox(ctx.clone());
                                        }
                                    }
                                    NavTab::Settings => {
                                        // 设置页右上角保持纯净留白，避免与页内主题/配置控件重复
                                    }
                                }
                            });
                        });
                    },
                );

                // 软件商店 / 设置页专属吸顶固定条（不随列表滚走）
                let mut scroll_top = header_rect.max.y + 14.0;
                if self.active_tab == NavTab::Store || self.active_tab == NavTab::Settings {
                    let toolbar_rect = Rect::from_min_max(
                        Pos2::new(panel_rect.min.x + 28.0, header_rect.max.y + 12.0),
                        Pos2::new(panel_rect.max.x - 28.0, header_rect.max.y + 52.0),
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(toolbar_rect)
                            .layout(Layout::top_down(Align::LEFT)),
                        |ui| {
                            if self.active_tab == NavTab::Store {
                                self.render_store_toolbar(ui, p);
                            } else {
                                self.render_settings_toolbar(ui, p);
                            }
                        },
                    );
                    scroll_top = toolbar_rect.max.y + 10.0;
                }

                // 滚动区延伸至窗口最右与最底边缘；内部保持 28px 左右留白
                let scroll_rect = Rect::from_min_max(
                    Pos2::new(panel_rect.min.x, scroll_top),
                    Pos2::new(panel_rect.max.x, panel_rect.max.y),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(scroll_rect)
                        .layout(Layout::top_down(Align::LEFT)),
                    |ui| {
                        let viewport_h = scroll_rect.height();
                        ScrollArea::vertical()
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(ui, |ui| {
                                Frame::new()
                                    .inner_margin(Margin {
                                        left: 28,
                                        right: 28,
                                        top: 2,
                                        bottom: 24,
                                    })
                                    .show(ui, |ui| {
                                        let start_y = ui.cursor().min.y;
                                        match self.active_tab {
                                            NavTab::Dashboard => self.render_dashboard(
                                                ui,
                                                p,
                                                &busy_snapshot,
                                                &mut actions,
                                            ),
                                            NavTab::Services => self.render_services(
                                                ui,
                                                p,
                                                &busy_snapshot,
                                                &mut actions,
                                            ),
                                            NavTab::Sites => {
                                                self.render_sites(ui, p, &mut actions)
                                            }
                                            NavTab::Store => self.render_store(
                                                ui,
                                                p,
                                                &downloads_snapshot,
                                                &busy_snapshot,
                                                &mut actions,
                                            ),
                                            NavTab::Toolbox => self.render_toolbox(
                                                ui,
                                                p,
                                                toolbox_loading,
                                                &mut actions,
                                            ),
                                            NavTab::Settings => self.render_settings(
                                                ui,
                                                ctx,
                                                p,
                                                update_checking,
                                                &mut actions,
                                            ),
                                        }
                                        let content_h = ui.cursor().min.y - start_y;
                                        if content_h > viewport_h - 48.0 {
                                            ui.add_space(12.0);
                                            ui.vertical_centered(|ui| {
                                                ui.label(
                                                    RichText::new("— 已到底部，无更多内容 —")
                                                        .size(11.5)
                                                        .color(p.text_muted),
                                                );
                                            });
                                        }
                                    });
                            });
                    },
                );
            });

        self.execute_actions(actions, ctx);

        self.render_modals_and_overlays(ctx, p, is_maximized, &busy_snapshot, &toast_snapshot);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.stop_on_exit {
            // 退出 Moka 时优雅停止全部运行中服务（含外部接管进程，优先走 stop_cmd 避免 InnoDB 崩溃恢复）
            let mgr = self.process_mgr.clone();
            let lay = self.layout.clone();
            let cat_snap = self
                .catalog
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            self.rt.block_on(async move {
                mgr.stop_all_graceful(&lay, &cat_snap).await;
            });
        }
        // 立即结束进程，避免 tokio Runtime::drop 等待未关闭的子进程管道而假死
        std::process::exit(0);
    }
}



fn main() -> eframe::Result<()> {
    // 单实例保护：若已有 Moka 实例运行，则直接唤醒旧窗口并退出
    let Some(_single_instance_guard) =
        acquire_single_instance("Local\\MokaDevEnvSingleInstance", "Moka v")
    else {
        return Ok(());
    };

    // 注册全局 Panic 钩子，确保 Beta 测试期间任何崩溃堆栈都写入 moka-app.log
    let root_for_panic = default_runtime_root();
    std::panic::set_hook(Box::new(move |info| {
        write_app_log(&root_for_panic, "PANIC", &format!("{info}"));
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(format!("Moka v{APP_VERSION} — 本地开发环境管理器"))
            .with_decorations(false)
            .with_icon(Arc::new(build_moka_icon()))
            .with_inner_size([1160.0, 740.0])
            .with_min_inner_size([940.0, 620.0]),
        centered: true,
        multisampling: 0,
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "Moka",
        options,
        Box::new(|cc| Ok(Box::new(MokaApp::new(cc)))),
    )
}