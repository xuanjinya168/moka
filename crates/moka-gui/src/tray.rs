use crate::storage::TrayStyle;
use crate::theme::build_moka_icon;
use eframe::egui;
use moka_core::manifest::SoftwareManifest;
use moka_core::RuntimeLayout;
use moka_process::ProcessManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Debug, Clone)]
pub enum TrayCommand {
    ShowMain,
    TogglePopover(f64, f64),
    StoppedAll,
}

pub struct SystemTray {
    tray: Option<TrayIcon>,
    stop_on_exit_flag: Arc<AtomicBool>,
    use_panel_flag: Arc<AtomicBool>,
    pending_cmds: Arc<Mutex<Vec<TrayCommand>>>,
}

fn build_native_menu() -> Menu {
    let menu = Menu::new();
    let item_show = MenuItem::with_id("moka_show", "打开 Moka", true, None);
    let item_stop = MenuItem::with_id("moka_stop_all", "停止全部服务", true, None);
    let item_quit = MenuItem::with_id("moka_quit", "彻底退出", true, None);
    let _ = menu.append(&item_show);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&item_stop);
    let _ = menu.append(&item_quit);
    menu
}

impl SystemTray {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: egui::Context,
        rt: Arc<tokio::runtime::Runtime>,
        layout: Arc<RuntimeLayout>,
        catalog: Arc<Mutex<Vec<SoftwareManifest>>>,
        process_mgr: Arc<ProcessManager>,
        stop_on_exit: bool,
        initial_style: TrayStyle,
    ) -> Self {
        let stop_on_exit_flag = Arc::new(AtomicBool::new(stop_on_exit));
        let use_panel_flag = Arc::new(AtomicBool::new(initial_style == TrayStyle::Panel));
        let pending_cmds = Arc::new(Mutex::new(Vec::new()));

        let icon_data = build_moka_icon();
        let tray_icon = Icon::from_rgba(icon_data.rgba, icon_data.width, icon_data.height).ok();

        let tray = tray_icon.and_then(|ic| {
            let mut builder = TrayIconBuilder::new()
                .with_tooltip("Moka — 本地开发环境管理器")
                .with_icon(ic)
                .with_menu_on_left_click(false);
            if initial_style == TrayStyle::Menu {
                builder = builder.with_menu(Box::new(build_native_menu()));
            }
            builder.build().ok()
        });

        if tray.is_some() {
            let stop_flag_clone = stop_on_exit_flag.clone();
            let panel_flag_clone = use_panel_flag.clone();
            let cmds_clone = pending_cmds.clone();
            std::thread::spawn(move || {
                let tray_rx = TrayIconEvent::receiver();
                let menu_rx = MenuEvent::receiver();
                loop {
                    while let Ok(ev) = tray_rx.try_recv() {
                        if let TrayIconEvent::Click {
                            button,
                            button_state: MouseButtonState::Up,
                            position,
                            ..
                        } = ev
                        {
                            match button {
                                MouseButton::Left => {
                                    // Windows 上左键始终直接通过 Win32 + egui 双保险唤醒主界面
                                    if let Ok(mut q) = cmds_clone.lock() {
                                        q.push(TrayCommand::ShowMain);
                                    }
                                    restore_window(&ctx);
                                }
                                MouseButton::Right
                                    if panel_flag_clone.load(Ordering::Relaxed) =>
                                {
                                    if let Ok(mut q) = cmds_clone.lock() {
                                        q.push(TrayCommand::TogglePopover(position.x, position.y));
                                    }
                                    ctx.request_repaint();
                                }
                                _ => {}
                            }
                        }
                    }
                    while let Ok(ev) = menu_rx.try_recv() {
                        match ev.id.0.as_str() {
                            "moka_show" => {
                                if let Ok(mut q) = cmds_clone.lock() {
                                    q.push(TrayCommand::ShowMain);
                                }
                                restore_window(&ctx);
                            }
                            "moka_stop_all" => {
                                let mgr = process_mgr.clone();
                                let lay = layout.clone();
                                let cat = catalog.clone();
                                let cmds = cmds_clone.clone();
                                let c = ctx.clone();
                                rt.spawn(async move {
                                    let cat_snap = cat
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .clone();
                                    mgr.stop_all_graceful(&lay, &cat_snap).await;
                                    if let Ok(mut q) = cmds.lock() {
                                        q.push(TrayCommand::StoppedAll);
                                    }
                                    c.request_repaint();
                                });
                            }
                            "moka_quit" => {
                                if stop_flag_clone.load(Ordering::Relaxed) {
                                    let mgr = process_mgr.clone();
                                    let lay = layout.clone();
                                    let cat_snap = catalog
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner())
                                        .clone();
                                    rt.block_on(async move {
                                        mgr.stop_all_graceful(&lay, &cat_snap).await;
                                    });
                                }
                                std::process::exit(0);
                            }
                            _ => {}
                        }
                    }
                    std::thread::sleep(Duration::from_millis(70));
                }
            });
        }

        Self {
            tray,
            stop_on_exit_flag,
            use_panel_flag,
            pending_cmds,
        }
    }

    pub fn set_stop_on_exit(&self, val: bool) {
        self.stop_on_exit_flag.store(val, Ordering::Relaxed);
    }

    /// 切换托盘右键样式（悬浮面板 ↔ 系统原生菜单），立即生效无需重启
    pub fn set_style(&self, style: TrayStyle) {
        let is_panel = style == TrayStyle::Panel;
        self.use_panel_flag.store(is_panel, Ordering::Relaxed);
        if let Some(ref tray) = self.tray {
            tray.set_show_menu_on_left_click(false);
            if is_panel {
                tray.set_menu(None);
            } else {
                tray.set_menu(Some(Box::new(build_native_menu())));
            }
        }
    }

    pub fn drain_commands(&self) -> Vec<TrayCommand> {
        self.pending_cmds
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

pub fn restore_window(ctx: &egui::Context) {
    // 先直接走 Win32 ShowWindow(SW_SHOW/SW_RESTORE) 解除隐藏，避免 eframe 隐藏窗口时跳过 update 导致指令队列死锁
    moka_sys::show_and_focus_window_by_title("Moka v");
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}
