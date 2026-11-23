use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 为当前进程的所有顶层无边框窗口启用 Windows 11 原生圆角与阴影（DWMWA_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND）
pub fn apply_win11_rounded_corners() {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        type Hwnd = *mut c_void;
        type Bool = i32;
        type EnumProc = unsafe extern "system" fn(Hwnd, isize) -> Bool;

        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcessId() -> u32;
        }
        #[link(name = "user32")]
        extern "system" {
            fn EnumWindows(cb: EnumProc, lparam: isize) -> Bool;
            fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
        }
        #[link(name = "dwmapi")]
        extern "system" {
            fn DwmSetWindowAttribute(
                hwnd: Hwnd,
                dw_attribute: u32,
                pv_attribute: *const c_void,
                cb_attribute: u32,
            ) -> i32;
        }

        unsafe extern "system" fn enum_cb(hwnd: Hwnd, target_pid: isize) -> Bool {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid as isize == target_pid {
                const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
                const DWMWCP_ROUND: u32 = 2;
                let pref = DWMWCP_ROUND;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&pref as *const u32).cast::<c_void>(),
                    std::mem::size_of::<u32>() as u32,
                );
            }
            1
        }

        unsafe {
            let my_pid = GetCurrentProcessId() as isize;
            let _ = EnumWindows(enum_cb, my_pid);
        }
    }
}

/// Windows 单实例进程锁（防止重复打开多个 Moka 窗口与托盘图标）
pub struct SingleInstanceGuard {
    #[cfg(target_os = "windows")]
    handle: *mut std::ffi::c_void,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        {
            #[link(name = "kernel32")]
            extern "system" {
                fn CloseHandle(h: *mut std::ffi::c_void) -> i32;
            }
            if !self.handle.is_null() {
                unsafe {
                    let _ = CloseHandle(self.handle);
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
static SAVED_WINDOW_RECT: std::sync::Mutex<Option<(i32, i32, i32, i32)>> =
    std::sync::Mutex::new(None);

/// 将主窗口隐藏至系统托盘（从任务栏与 Alt+Tab 移除并移至屏外，但保持 WS_VISIBLE，
/// 避免 eframe 0.31 在 SW_HIDE 下跳过 update() 导致托盘右键悬浮面板弹不出来）
pub fn hide_window_to_tray_by_title(window_title_prefix: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        type Hwnd = *mut c_void;
        type EnumProc = unsafe extern "system" fn(Hwnd, isize) -> i32;

        #[repr(C)]
        struct WinRect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }

        #[link(name = "user32")]
        extern "system" {
            fn EnumWindows(cb: EnumProc, lparam: isize) -> i32;
            fn GetWindowTextW(hwnd: Hwnd, buf: *mut u16, max_count: i32) -> i32;
            fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
            fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
            fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, new_long: isize) -> isize;
            fn SetWindowPos(
                hwnd: Hwnd,
                insert_after: Hwnd,
                x: i32,
                y: i32,
                cx: i32,
                cy: i32,
                flags: u32,
            ) -> i32;
            fn ShowWindow(hwnd: Hwnd, cmd_show: i32) -> i32;
        }

        let prefix_owned = window_title_prefix.to_string();
        unsafe extern "system" fn hide_cb(hwnd: Hwnd, lparam: isize) -> i32 {
            let prefix = &*(lparam as *const String);
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len as usize]);
                if title.starts_with(prefix.as_str()) {
                    let mut rc = WinRect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    if GetWindowRect(hwnd, &mut rc) != 0 && rc.left > -10000 && rc.top > -10000 {
                        let w = (rc.right - rc.left).max(940);
                        let h = (rc.bottom - rc.top).max(620);
                        if let Ok(mut guard) = SAVED_WINDOW_RECT.lock() {
                            *guard = Some((rc.left, rc.top, w, h));
                        }
                    }
                    const GWL_EXSTYLE: i32 = -20;
                    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
                    const WS_EX_APPWINDOW: isize = 0x0004_0000;
                    const SW_HIDE: i32 = 0;
                    const SW_SHOWNOACTIVATE: i32 = 4;
                    const SWP_NOSIZE: u32 = 0x0001;
                    const SWP_NOZORDER: u32 = 0x0004;
                    const SWP_NOACTIVATE: u32 = 0x0010;

                    let _ = ShowWindow(hwnd, SW_HIDE);
                    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                    let new_ex = (ex | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW;
                    let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
                    let _ = SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        -32000,
                        -32000,
                        0,
                        0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                    return 0;
                }
            }
            1
        }
        unsafe {
            let ptr = (&prefix_owned as *const String) as isize;
            let _ = EnumWindows(hide_cb, ptr);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window_title_prefix;
    }
}

/// 通过 Win32 API 直接唤醒并置顶指定标题前缀的窗口（恢复任务栏图标、还原原坐标并置顶）
pub fn show_and_focus_window_by_title(window_title_prefix: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;
        type Hwnd = *mut c_void;
        type EnumProc = unsafe extern "system" fn(Hwnd, isize) -> i32;

        #[repr(C)]
        struct WinRect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }

        #[link(name = "user32")]
        extern "system" {
            fn EnumWindows(cb: EnumProc, lparam: isize) -> i32;
            fn GetWindowTextW(hwnd: Hwnd, buf: *mut u16, max_count: i32) -> i32;
            fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
            fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
            fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, new_long: isize) -> isize;
            fn SetWindowPos(
                hwnd: Hwnd,
                insert_after: Hwnd,
                x: i32,
                y: i32,
                cx: i32,
                cy: i32,
                flags: u32,
            ) -> i32;
            fn ShowWindow(hwnd: Hwnd, cmd_show: i32) -> i32;
            fn SetForegroundWindow(hwnd: Hwnd) -> i32;
        }

        let prefix_owned = window_title_prefix.to_string();
        unsafe extern "system" fn focus_cb(hwnd: Hwnd, lparam: isize) -> i32 {
            let prefix = &*(lparam as *const String);
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            if len > 0 {
                let title = String::from_utf16_lossy(&buf[..len as usize]);
                if title.starts_with(prefix.as_str()) {
                    const GWL_EXSTYLE: i32 = -20;
                    const WS_EX_TOOLWINDOW: isize = 0x0000_0080;
                    const WS_EX_APPWINDOW: isize = 0x0004_0000;
                    const SW_HIDE: i32 = 0;
                    const SW_SHOW: i32 = 5;
                    const SW_RESTORE: i32 = 9;
                    const SWP_NOZORDER: u32 = 0x0004;

                    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                    let mut rc = WinRect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    };
                    let _ = GetWindowRect(hwnd, &mut rc);
                    let was_tray_hidden =
                        (ex & WS_EX_TOOLWINDOW) != 0 || rc.left < -10000 || rc.top < -10000;

                    if was_tray_hidden {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                        let new_ex = (ex & !WS_EX_TOOLWINDOW) | WS_EX_APPWINDOW;
                        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex);
                        let (x, y, w, h) = SAVED_WINDOW_RECT
                            .lock()
                            .ok()
                            .and_then(|g| *g)
                            .unwrap_or((160, 90, 1160, 740));
                        let _ = SetWindowPos(
                            hwnd,
                            std::ptr::null_mut(),
                            x,
                            y,
                            w,
                            h,
                            SWP_NOZORDER,
                        );
                    }
                    let _ = ShowWindow(hwnd, SW_SHOW);
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                    return 0;
                }
            }
            1
        }
        unsafe {
            let ptr = (&prefix_owned as *const String) as isize;
            let _ = EnumWindows(focus_cb, ptr);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window_title_prefix;
    }
}

/// 尝试获取单实例锁；若已有 Moka 实例在运行，则尝试唤醒已有窗口并返回 None
pub fn acquire_single_instance(mutex_name: &str, window_title_prefix: &str) -> Option<SingleInstanceGuard> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::c_void;

        #[link(name = "kernel32")]
        extern "system" {
            fn CreateMutexW(attrs: *const c_void, initial_owner: i32, name: *const u16) -> *mut c_void;
            fn GetLastError() -> u32;
            fn CloseHandle(h: *mut c_void) -> i32;
        }

        let wide: Vec<u16> = mutex_name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe { CreateMutexW(std::ptr::null(), 1, wide.as_ptr()) };
        if handle.is_null() {
            return Some(SingleInstanceGuard { handle });
        }
        const ERROR_ALREADY_EXISTS: u32 = 183;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            show_and_focus_window_by_title(window_title_prefix);
            return None;
        }
        Some(SingleInstanceGuard { handle })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (mutex_name, window_title_prefix);
        Some(SingleInstanceGuard {})
    }
}

/// 批量扫描 `apps/` 根目录下所有仍在运行的外部/残留服务进程，返回 `service_id -> pid`
/// （单次 CIM 查询覆盖全部服务，耗时约 100ms，用于重启 Moka 后自动接管后台服务状态）
pub fn list_running_pids_under_apps(apps_dir: &Path) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    #[cfg(target_os = "windows")]
    {
        for (pid, exe, cmd) in query_running_processes() {
            if let Some(svc_id) = service_id_running_in(apps_dir, &exe, &cmd) {
                map.entry(svc_id).or_insert(pid);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = apps_dir;
    }
    map
}

#[cfg(target_os = "windows")]
fn query_running_processes() -> Vec<(u32, String, String)> {
    // PowerShell 重定向输出默认使用 OEM 代码页，显式切到 UTF-8 以免中文路径被解码成乱码
    let ps = "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; \
        Get-CimInstance Win32_Process | Where-Object { \
            $_.Name -ne 'moka.exe' -and $_.Name -ne 'powershell.exe' \
        } | ForEach-Object { \"$($_.ProcessId)|$($_.ExecutablePath)|$($_.CommandLine)\" }";
    let Ok(out) = silent_cmd("powershell")
        .args(["-NoProfile", "-Command", ps])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '|');
            let pid = parts.next()?.trim().parse::<u32>().ok()?;
            let exe = parts.next().unwrap_or("").to_string();
            let cmd = parts.next().unwrap_or("").to_string();
            Some((pid, exe, cmd))
        })
        .collect()
}

/// 进程的可执行文件或它实际启动的程序是否位于 `dir` 之下；命中时返回 `dir` 下第一级子目录名
fn service_id_running_in(dir: &Path, exe: &str, cmdline: &str) -> Option<String> {
    let prefix = normalize_path(&dir_prefix_with_sep(dir));
    let exe = normalize_path(exe);
    if let Some(id) = subdirectory_under(&exe, &prefix) {
        return Some(id.to_string());
    }
    let program = launched_program(cmdline)?;
    subdirectory_under(&program, &prefix).map(str::to_string)
}

fn normalize_path(p: &str) -> String {
    let lower = p.trim().to_lowercase().replace('/', "\\");
    lower.strip_prefix(r"\\?\").unwrap_or(&lower).to_string()
}

fn subdirectory_under<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let id = path.strip_prefix(prefix)?.split('\\').next()?;
    (!id.is_empty()).then_some(id)
}

fn split_command_line_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

/// 解析命令行实际启动的程序路径（小写、`/` 归一为 `\`），兼容 `cmd.exe /C <程序>` 包装。
/// 只有程序本身算数，参数里出现某个路径不代表进程从那里启动。
fn launched_program(cmdline: &str) -> Option<String> {
    let tokens = split_command_line_tokens(cmdline);
    let mut idx = 0;
    let is_cmd_wrapper = tokens
        .first()
        .map(|t| {
            t.rsplit(['\\', '/'])
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("cmd.exe"))
        })
        .unwrap_or(false);
    if is_cmd_wrapper {
        idx = 1;
        while tokens.get(idx).is_some_and(|t| t.starts_with('/')) {
            idx += 1;
        }
    }
    tokens.get(idx).map(|t| normalize_path(t))
}

/// 设置或取消 Windows 当前用户开机自启（写入 HKCU\...\Run，无需管理员权限）
pub fn set_auto_start(enabled: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        const VALUE_NAME: &str = "MokaDevEnv";
        if enabled {
            let exe = std::env::current_exe()?;
            let cmd_val = format!("\"{}\"", exe.to_string_lossy());
            let out = silent_cmd("reg")
                .args([
                    "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &cmd_val, "/f",
                ])
                .output()?;
            if !out.status.success() {
                return Err(anyhow!(
                    "设置开机自启失败: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
        } else {
            let _ = silent_cmd("reg")
                .args(["delete", RUN_KEY, "/v", VALUE_NAME, "/f"])
                .output();
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// 检查是否已开启 Windows 当前用户开机自启
pub fn is_auto_start_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        const VALUE_NAME: &str = "MokaDevEnv";
        silent_cmd("reg")
            .args(["query", RUN_KEY, "/v", VALUE_NAME])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub const MOKA_ROOT_CA_CN: &str = "Moka Local Dev CA";

/// 检查 Moka 本地根证书是否已安装到当前用户受信任的根证书颁发机构（无需管理员权限）
pub fn is_user_root_ca_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        silent_cmd("certutil")
            .args(["-user", "-verifystore", "Root", MOKA_ROOT_CA_CN])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// 将 Moka 本地根证书安装到当前用户受信任根存储（Windows 会弹出原生确认对话框，无需管理员权限）
pub fn install_user_root_ca(ca_cert_path: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        if !ca_cert_path.exists() {
            return Err(anyhow!("根证书文件不存在: {}", ca_cert_path.display()));
        }
        let out = silent_cmd("certutil")
            .args([
                "-user",
                "-addstore",
                "Root",
                &ca_cert_path.to_string_lossy(),
            ])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(&out.stderr);
            let msg = if msg.trim().is_empty() {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            } else {
                msg.trim().to_string()
            };
            Err(anyhow!("安装本地根证书未完成: {msg}"))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = ca_cert_path;
        Ok(())
    }
}

/// 从当前用户受信任根存储中移除 Moka 本地根证书
pub fn uninstall_user_root_ca() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        let out = silent_cmd("certutil")
            .args(["-user", "-delstore", "Root", MOKA_ROOT_CA_CN])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(&out.stderr);
            let msg = if msg.trim().is_empty() {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            } else {
                msg.trim().to_string()
            };
            Err(anyhow!("移除本地根证书失败: {msg}"))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub memory_bytes: u64,
    pub uptime_secs: u64,
}

/// 批量查询指定 PID 列表的内存占用（WorkingSet64）与已运行时长（秒）
pub fn query_process_metrics(pids: &[u32]) -> HashMap<u32, ProcessMetrics> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }
    #[cfg(target_os = "windows")]
    {
        let id_list = pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let ps = format!(
            "$now = Get-Date; Get-Process -Id {id_list} -ErrorAction SilentlyContinue | ForEach-Object {{ \
                $up = 0; try {{ $up = [int64]($now - $_.StartTime).TotalSeconds }} catch {{}}; \
                if ($up -lt 0) {{ $up = 0 }}; \
                \"$($_.Id)|$($_.WorkingSet64)|$up\" \
            }}"
        );
        if let Ok(out) = silent_cmd("powershell")
            .args(["-NoProfile", "-Command", &ps])
            .output()
        {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<&str> = line.trim().split('|').collect();
                    if parts.len() == 3 {
                        if let (Ok(pid), Ok(mem), Ok(up)) = (
                            parts[0].parse::<u32>(),
                            parts[1].parse::<u64>(),
                            parts[2].parse::<u64>(),
                        ) {
                            map.insert(
                                pid,
                                ProcessMetrics {
                                    pid,
                                    memory_bytes: mem,
                                    uptime_secs: up,
                                },
                            );
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = pids;
    }
    map
}

#[derive(Debug, Clone, Serialize)]
pub struct PortOccupancy {
    pub protocol: String,
    pub local_addr: String,
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
}

fn silent_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// 扫描本机正在监听的 TCP 端口（Windows netstat + tasklist）
pub fn list_listening_ports() -> Result<Vec<PortOccupancy>> {
    let pid_map = load_pid_name_map();
    let output = silent_cmd("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        if !parts[0].eq_ignore_ascii_case("TCP") || !parts[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let local_addr = parts[1];
        let Some(port_str) = local_addr.rsplit(':').next() else {
            continue;
        };
        let Ok(port) = port_str.parse::<u16>() else {
            continue;
        };
        let Ok(pid) = parts[4].parse::<u32>() else {
            continue;
        };
        if !seen.insert((port, pid)) {
            continue;
        }
        let process_name = pid_map
            .get(&pid)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        results.push(PortOccupancy {
            protocol: "TCP".into(),
            local_addr: local_addr.to_string(),
            port,
            pid,
            process_name,
        });
    }

    results.sort_by_key(|p| p.port);
    Ok(results)
}

fn load_pid_name_map() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let Ok(output) = silent_cmd("tasklist").args(["/FO", "CSV", "/NH"]).output() else {
        return map;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let cols: Vec<&str> = line.split("\",\"").collect();
        if cols.len() >= 2 {
            let name = cols[0].trim_matches('"').to_string();
            let pid_str = cols[1].trim_matches('"');
            if let Ok(pid) = pid_str.parse::<u32>() {
                map.insert(pid, name);
            }
        }
    }
    map
}

/// 强制结束指定 PID 进程
pub fn kill_pid(pid: u32) -> Result<()> {
    let status = silent_cmd("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("无法结束进程 PID={pid}，可能需要管理员权限"))
    }
}

/// 使用 Windows NTFS Junction 原子切换 current/jdk 或 current/maven，无需 UAC 提权
pub fn switch_junction(link_path: &Path, target_dir: &Path) -> Result<()> {
    if !target_dir.exists() {
        return Err(anyhow!("目标目录不存在: {}", target_dir.display()));
    }
    if let Some(parent) = link_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if link_path.exists() {
        // Windows 下删除 Junction 使用 rmdir（不会删除目标目录内容）
        let _ = silent_cmd("cmd")
            .args(["/C", "rmdir", &link_path.to_string_lossy()])
            .status();
    }
    let output = silent_cmd("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link_path.to_string_lossy(),
            &target_dir.to_string_lossy(),
        ])
        .output()?;

    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "切换版本软链接失败: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// 读取当前 Junction 指向的目标路径
pub fn read_junction_target(link_path: &Path) -> Option<PathBuf> {
    std::fs::read_link(link_path).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    pub ip: String,
    pub domain: String,
    pub comment: String,
    pub enabled: bool,
}

fn looks_like_ip(token: &str) -> bool {
    token.parse::<std::net::IpAddr>().is_ok()
}

/// 解析 Windows Hosts 文本中的域名映射条目（支持识别被 `#` 注释禁用的条目）
pub fn parse_hosts_entries(text: &str) -> Vec<HostEntry> {
    let mut entries = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (enabled, body) = if let Some(rest) = trimmed.strip_prefix('#') {
            (false, rest.trim())
        } else {
            (true, trimmed)
        };
        let (mapping_part, comment) = match body.split_once('#') {
            Some((m, c)) => (m.trim(), c.trim().to_string()),
            None => (body, String::new()),
        };
        let mut parts = mapping_part.split_whitespace();
        let Some(ip) = parts.next() else {
            continue;
        };
        if !looks_like_ip(ip) {
            continue;
        }
        let domains: Vec<&str> = parts.collect();
        if domains.is_empty() {
            continue;
        }
        entries.push(HostEntry {
            ip: ip.to_string(),
            domain: domains.join(" "),
            comment,
            enabled,
        });
    }
    entries
}

/// 将可视化编辑后的条目安全回写为完整 Hosts 文本（保留原文件顶部说明注释）
pub fn render_hosts_file(original: &str, entries: &[HostEntry]) -> String {
    let mut header_lines: Vec<String> = Vec::new();
    for line in original.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            header_lines.push(String::new());
            continue;
        }
        // 只保留纯注释说明行（非 IP 映射行）
        let check_body = trimmed.strip_prefix('#').map(|s| s.trim()).unwrap_or(trimmed);
        let first_tok = check_body.split_whitespace().next().unwrap_or("");
        if !looks_like_ip(first_tok) {
            header_lines.push(line.to_string());
        }
    }
    // 去掉尾部多余空行
    while header_lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        header_lines.pop();
    }

    let mut out = header_lines;
    if !out.is_empty() {
        out.push(String::new());
    }
    for e in entries {
        let ip = e.ip.trim();
        let domain = e.domain.trim();
        if ip.is_empty() || domain.is_empty() {
            continue;
        }
        let prefix = if e.enabled { "" } else { "# " };
        let line = if e.comment.trim().is_empty() {
            format!("{prefix}{ip:<15} {domain}")
        } else {
            format!("{prefix}{ip:<15} {domain} # {}", e.comment.trim())
        };
        out.push(line);
    }
    let mut result = out.join("\r\n");
    result.push_str("\r\n");
    result
}

/// 读取 Windows Hosts 文件内容
pub fn read_hosts_file() -> Result<String> {
    let path = Path::new(r"C:\Windows\System32\drivers\etc\hosts");
    Ok(std::fs::read_to_string(path)?)
}

/// 写入 Windows Hosts 文件：优先直接写入，无权限时自动唤起 UAC 提权写入
pub fn write_hosts_file(content: &str) -> Result<()> {
    let hosts_path = Path::new(r"C:\Windows\System32\drivers\etc\hosts");
    match std::fs::write(hosts_path, content) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            #[cfg(target_os = "windows")]
            {
                let tmp = std::env::temp_dir().join("moka_hosts_pending.txt");
                std::fs::write(&tmp, content)?;
                let tmp_escaped = tmp.to_string_lossy().replace('\'', "''");
                let ps = format!(
                    "Start-Process -FilePath 'cmd.exe' -ArgumentList '/C copy /Y \"{tmp_escaped}\" \"C:\\Windows\\System32\\drivers\\etc\\hosts\"' -Verb RunAs -WindowStyle Hidden -Wait"
                );
                let status = silent_cmd("powershell")
                    .args(["-NoProfile", "-Command", &ps])
                    .status()?;
                let _ = std::fs::remove_file(&tmp);
                if status.success() {
                    Ok(())
                } else {
                    Err(anyhow!("已取消管理员授权或写入 Hosts 失败"))
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                Err(e.into())
            }
        }
        Err(e) => Err(e.into()),
    }
}

fn dir_prefix_with_sep(dir: &Path) -> String {
    let mut s = dir.to_string_lossy().replace('/', "\\");
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

/// 结束从指定软件目录运行的残留孤儿进程（强制带末尾 `\` 路径分隔符匹配，防止短目录名误杀长目录名）
pub fn stop_processes_in_dir(dir: &Path) {
    #[cfg(target_os = "windows")]
    {
        for (pid, exe, cmd) in query_running_processes() {
            if service_id_running_in(dir, &exe, &cmd).is_some() {
                // 连同子进程树一起结束，保证 cmd.exe 拉起的 java（Nacos / Tomcat）不会成为孤儿
                let _ = silent_cmd("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
    }
}

/// 安全卸载软件目录：
/// 1. 结束从该目录启动的残留进程（防止上一次会话遗留的 mysqld/redis 占用句柄导致 os error 5）
/// 2. 递归清除 Windows 只读属性（防止只读文件导致 remove_dir_all 报 os error 5）
/// 3. 带重试执行删除
pub fn safe_remove_app_dir(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    // 1. 清理从目标目录运行的残留子进程
    stop_processes_in_dir(dir);

    // 2. 递归清除只读标志
    clear_readonly_recursive(dir);

    // 3. 删除并短暂重试（等待 Windows 释放刚结束进程的文件锁）
    let mut last_err = None;
    for _ in 0..4 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                clear_readonly_recursive(dir);
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        }
    }

    Err(anyhow!(
        "{}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "目录被占用，请关闭占用该目录的程序后重试".into())
    ))
}

#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly_recursive(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                clear_readonly_recursive(&p);
            } else if let Ok(meta) = entry.metadata() {
                let mut perms = meta.permissions();
                if perms.readonly() {
                    perms.set_readonly(false);
                    let _ = std::fs::set_permissions(&p, perms);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_parse_and_render_preserves_comments() {
        let sample = "# Copyright Microsoft\r\n# Sample hosts\r\n\r\n127.0.0.1 api.local # dev api\r\n# 127.0.0.1 disabled.local\r\n";
        let mut entries = parse_hosts_entries(sample);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].enabled);
        assert_eq!(entries[0].domain, "api.local");
        assert_eq!(entries[0].comment, "dev api");
        assert!(!entries[1].enabled);
        assert_eq!(entries[1].domain, "disabled.local");

        entries[1].enabled = true;
        let out = render_hosts_file(sample, &entries);
        assert!(out.contains("# Copyright Microsoft"));
        assert!(out.contains("127.0.0.1       api.local # dev api"));
        assert!(out.contains("127.0.0.1       disabled.local"));
    }

    #[test]
    fn dir_prefix_boundary_prevents_collision() {
        let short = dir_prefix_with_sep(Path::new(r"C:\Moka\apps\node-2"));
        assert_eq!(short, r"C:\Moka\apps\node-2\");
        assert!(r"C:\Moka\apps\node-2\node.exe".starts_with(&short));
        assert!(!r"C:\Moka\apps\node-20\node.exe".starts_with(&short));
    }

    #[test]
    fn launched_program_handles_quotes_and_cmd_wrapper() {
        assert_eq!(
            launched_program(r#""C:\Moka Runtime\apps\redis-7.2\redis-server.exe" "a b.conf""#).as_deref(),
            Some(r"c:\moka runtime\apps\redis-7.2\redis-server.exe")
        );
        assert_eq!(
            launched_program(
                r#"cmd.exe /C "C:\Moka/apps/nacos-2.4/bin/startup.cmd" -m standalone"#
            )
            .as_deref(),
            Some(r"c:\moka\apps\nacos-2.4\bin\startup.cmd")
        );
        assert_eq!(launched_program(r"cmd.exe /K cd /d C:\Moka\apps\mysql-8.0\bin").as_deref(), Some("cd"));
        assert_eq!(launched_program(""), None);
    }

    #[test]
    fn process_is_attributed_only_by_what_it_launches() {
        let apps = Path::new(r"C:\Moka\apps");
        // 服务本体：按可执行文件路径命中，且目录名前缀相同也不会串号
        assert_eq!(
            service_id_running_in(apps, r"C:\Moka\apps\mysql-8.0\bin\mysqld.exe", "").as_deref(),
            Some("mysql-8.0")
        );
        assert_eq!(
            service_id_running_in(
                apps,
                r"C:\Windows\System32\cmd.exe",
                r#"cmd.exe /C C:\Moka\apps/nacos-2.4/bin/startup.cmd -m standalone"#
            )
            .as_deref(),
            Some("nacos-2.4")
        );
        // 仅在参数里提到 apps 下的路径：编辑器、终端、带 -D 参数的 java 都不算
        assert_eq!(
            service_id_running_in(
                apps,
                r"C:\VSCode\Code.exe",
                r#""C:\VSCode\Code.exe" C:\Moka\apps\mysql-8.0\conf\my.ini"#
            ),
            None
        );
        assert_eq!(
            service_id_running_in(
                apps,
                r"C:\Java\bin\java.exe",
                r"java.exe -Dnacos.home=C:\Moka\apps\nacos-2.4 -m standalone"
            ),
            None
        );
        // 目录边界：清理 apps\node-2 时不能波及 apps\node-20
        assert_eq!(
            service_id_running_in(Path::new(r"C:\Moka\apps\node-2"), r"C:\Moka\apps\node-20\node.exe", "").as_deref(),
            None
        );
    }
}
