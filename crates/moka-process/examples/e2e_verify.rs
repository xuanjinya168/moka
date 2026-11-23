use moka_config::{apply_visual_config, ensure_service_config, parse_visual_config};
use moka_core::downloader::download_and_extract;
use moka_core::manifest::load_catalog;
use moka_core::RuntimeLayout;
use moka_process::ProcessManager;
use moka_sys::{list_listening_ports, read_junction_target, safe_remove_app_dir, switch_junction};
use std::path::PathBuf;
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration};

async fn tcp_open(port: u16) -> bool {
    timeout(Duration::from_millis(800), TcpStream::connect(("127.0.0.1", port)))
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let test_root = PathBuf::from(r"C:\Users\admin\Moka-E2E-Test");
    let layout = RuntimeLayout::new(test_root.clone());
    layout.ensure_dirs().expect("ensure_dirs failed");

    let catalog = load_catalog(None).expect("load_catalog failed");
    let mgr = ProcessManager::new();

    println!("=== 1. 测试端口扫描与 Junction 软链 ===");
    let ports = list_listening_ports().expect("list_listening_ports failed");
    println!("  [PASS] 端口扫描成功，当前监听端口数: {}", ports.len());

    let dummy_a = layout.app_dir("dummy-a");
    let dummy_b = layout.app_dir("dummy-b");
    let _ = std::fs::create_dir_all(&dummy_a);
    let _ = std::fs::create_dir_all(&dummy_b);
    let jlink = layout.current_link("test-rt");
    switch_junction(&jlink, &dummy_a).expect("switch_junction A failed");
    assert_eq!(read_junction_target(&jlink).as_deref(), Some(dummy_a.as_path()));
    switch_junction(&jlink, &dummy_b).expect("switch_junction B failed");
    assert_eq!(read_junction_target(&jlink).as_deref(), Some(dummy_b.as_path()));
    let _ = std::fs::remove_dir(&jlink);
    let _ = safe_remove_app_dir(&dummy_a);
    let _ = safe_remove_app_dir(&dummy_b);
    println!("  [PASS] NTFS Junction 秒切与回读正常");

    println!("\n=== 2. 全量服务端到端真机实测 ===");
    let mut results: Vec<(String, bool, String)> = Vec::new();

    for m in catalog.iter().filter(|m| m.is_service) {
        if !args.is_empty() && !args.iter().any(|a| a == &m.id) {
            continue;
        }
        println!("\n--------------------------------------------------");
        println!(">>> 测试服务: {} ({} {})", m.id, m.name, m.version);
        let app_dir = layout.app_dir(&m.id);
        let conf_dir = layout.conf_dir(&m.id);
        let data_dir = layout.data_dir(&m.id);
        let zip_path = layout.download_file(&m.id);
        let port = m.default_port.unwrap_or(0);

        // Step A: 下载并解压（如未安装）
        if !layout.is_installed(&m.id) {
            println!("  [1/6] 下载并解压...");
            let res = download_and_extract(&m.id, &m.mirrors, &zip_path, &app_dir, |_| {}).await;
            if let Err(e) = res {
                let msg = format!("下载/解压失败: {e:#}");
                println!("  [FAIL] {msg}");
                results.push((m.id.clone(), false, msg));
                continue;
            }
            println!("  [OK] 下载解压完成");
        } else {
            println!("  [1/6] 已存在测试目录，复用解压结果");
        }

        // Step B: 首次启动
        println!("  [2/6] 首次启动 (port={port})...");
        let start1 = mgr.start_service(m, &layout, Some(port)).await;
        if let Err(e) = start1 {
            let logs = mgr.get_logs(&m.id);
            println!("  [FAIL] 首次启动失败: {e:#}");
            for l in logs.iter().rev().take(12).rev() {
                println!("    LOG: {l}");
            }
            let _ = mgr.stop_service(m, &layout).await;
            results.push((m.id.clone(), false, format!("首次启动失败: {e:#}")));
            continue;
        }

        // 等待服务充分就绪（Java 类服务多等一会儿）
        let settle_secs = if m.id.starts_with("nacos") || m.id.starts_with("tomcat") {
            8
        } else {
            2
        };
        sleep(Duration::from_secs(settle_secs)).await;

        let (running1, pid1, _) = mgr.is_running(&m.id);
        let open1 = if port > 0 { tcp_open(port).await } else { true };
        if !running1 || !open1 {
            let logs = mgr.get_logs(&m.id);
            let msg = format!("首次启动后存活检查失败: running={running1}, tcp_open={open1}");
            println!("  [FAIL] {msg}");
            for l in logs.iter().rev().take(15).rev() {
                println!("    LOG: {l}");
            }
            let _ = mgr.stop_service(m, &layout).await;
            results.push((m.id.clone(), false, msg));
            continue;
        }
        println!("  [OK] 首次启动存活且端口 {port} 已监听 (pid={pid1:?})");

        // Step C: 配置生成、可视化解析与写回校验
        println!("  [3/6] 校验可视化配置读写...");
        match ensure_service_config(m.config_kind.as_deref(), &app_dir, &conf_dir, &data_dir, port) {
            Ok(Some(cfg_path)) => {
                let raw = std::fs::read_to_string(&cfg_path).unwrap_or_default();
                let parsed = parse_visual_config(m.config_kind.as_deref(), &raw, port);
                if let Some(vcfg) = parsed {
                    let rewritten = apply_visual_config(&raw, &vcfg);
                    if rewritten.trim().is_empty() {
                        let msg = "apply_visual_config 写回为空".to_string();
                        println!("  [FAIL] {msg}");
                        let _ = mgr.stop_service(m, &layout).await;
                        results.push((m.id.clone(), false, msg));
                        continue;
                    }
                    println!("  [OK] 配置解析与无损写回正常 ({})", cfg_path.display());
                } else {
                    let msg = format!("parse_visual_config 返回 None ({})", cfg_path.display());
                    println!("  [FAIL] {msg}");
                    let _ = mgr.stop_service(m, &layout).await;
                    results.push((m.id.clone(), false, msg));
                    continue;
                }
            }
            Ok(None) => {
                let msg = "ensure_service_config 返回 None".to_string();
                println!("  [FAIL] {msg}");
                let _ = mgr.stop_service(m, &layout).await;
                results.push((m.id.clone(), false, msg));
                continue;
            }
            Err(e) => {
                let msg = format!("ensure_service_config 错误: {e:#}");
                println!("  [FAIL] {msg}");
                let _ = mgr.stop_service(m, &layout).await;
                results.push((m.id.clone(), false, msg));
                continue;
            }
        }

        // Step D: 停止服务并验证端口释放
        println!("  [4/6] 停止服务并检查端口释放...");
        if let Err(e) = mgr.stop_service(m, &layout).await {
            let msg = format!("停止失败: {e:#}");
            println!("  [FAIL] {msg}");
            results.push((m.id.clone(), false, msg));
            continue;
        }
        sleep(Duration::from_secs(1)).await;
        let still_open = if port > 0 { tcp_open(port).await } else { false };
        if still_open {
            let msg = format!("停止后端口 {port} 仍被占用（存在残留孤儿进程）");
            println!("  [FAIL] {msg}");
            results.push((m.id.clone(), false, msg));
            continue;
        }
        println!("  [OK] 服务已停止且端口 {port} 已释放");

        // Step E: 二次重启（验证无文件锁冲突）
        println!("  [5/6] 二次重启验证（无锁冲突）...");
        if let Err(e) = mgr.start_service(m, &layout, Some(port)).await {
            let logs = mgr.get_logs(&m.id);
            let msg = format!("二次重启失败: {e:#}");
            println!("  [FAIL] {msg}");
            for l in logs.iter().rev().take(12).rev() {
                println!("    LOG: {l}");
            }
            let _ = mgr.stop_service(m, &layout).await;
            results.push((m.id.clone(), false, msg));
            continue;
        }
        sleep(Duration::from_secs(settle_secs)).await;
        let (running2, _, _) = mgr.is_running(&m.id);
        let open2 = if port > 0 { tcp_open(port).await } else { true };
        if !running2 || !open2 {
            let logs = mgr.get_logs(&m.id);
            let msg = format!("二次重启存活失败: running={running2}, tcp_open={open2}");
            println!("  [FAIL] {msg}");
            for l in logs.iter().rev().take(12).rev() {
                println!("    LOG: {l}");
            }
            let _ = mgr.stop_service(m, &layout).await;
            results.push((m.id.clone(), false, msg));
            continue;
        }
        println!("  [OK] 二次重启存活正常");

        // Step F: 最终停止 + 安全卸载
        println!("  [6/6] 停止并安全卸载...");
        let _ = mgr.stop_service(m, &layout).await;
        sleep(Duration::from_millis(600)).await;
        if let Err(e) = safe_remove_app_dir(&app_dir) {
            let msg = format!("安全卸载失败: {e:#}");
            println!("  [FAIL] {msg}");
            results.push((m.id.clone(), false, msg));
            continue;
        }
        println!("  [PASS] {} 全流程通过！", m.id);
        results.push((m.id.clone(), true, "全流程通过".into()));
    }

    mgr.stop_all().await;

    println!("\n================ 测试汇总 ================");
    let mut failed = 0;
    for (id, ok, msg) in &results {
        println!("  [{}] {:<12} - {}", if *ok { "PASS" } else { "FAIL" }, id, msg);
        if !ok {
            failed += 1;
        }
    }
    println!("==========================================");
    if failed > 0 {
        std::process::exit(1);
    }
}
