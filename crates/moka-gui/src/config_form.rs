use crate::theme::ThemePalette;
use crate::widgets::{soft_button, styled_search_input};
use eframe::egui::{self, Align, Color32, CornerRadius, Frame, Layout, Margin, RichText, Stroke};
use moka_config::{generate_local_ssl_cert, NginxProxyRule, VisualConfig};
use std::path::Path;

pub fn render_visual_config_form(
    ui: &mut egui::Ui,
    p: ThemePalette,
    cfg: &mut VisualConfig,
    ssl_dir: &Path,
    toast_out: &mut Option<String>,
) -> bool {
    let mut changed = false;

    let section_title = |ui: &mut egui::Ui, title: &str| {
        ui.add_space(4.0);
        ui.label(RichText::new(title).strong().size(13.0).color(p.accent_text));
        ui.add_space(4.0);
    };

    let form_row = |ui: &mut egui::Ui,
                    label: &str,
                    desc: &str,
                    add_ctrl: &mut dyn FnMut(&mut egui::Ui)| {
        Frame::new()
            .fill(p.bg_card_alt)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width() - 6.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(label).strong().size(13.0).color(p.text_primary));
                        ui.label(RichText::new(desc).size(11.5).color(p.text_muted));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        add_ctrl(ui);
                    });
                });
            });
        ui.add_space(6.0);
    };

    let toggle_pills = |ui: &mut egui::Ui, val: &mut bool, off_lbl: &str, on_lbl: &str| -> bool {
        let mut hit = false;
        for (target, lbl) in [(false, off_lbl), (true, on_lbl)] {
            let sel = *val == target;
            let (bg, fg) = if sel {
                (p.accent, Color32::WHITE)
            } else {
                (p.tag_bg, p.tag_text)
            };
            if soft_button(ui, lbl, bg, fg, 64.0, true).clicked() {
                *val = target;
                hit = true;
            }
        }
        hit
    };

    let option_pills = |ui: &mut egui::Ui, current: &mut String, opts: &[(&str, &str)]| -> bool {
        let mut hit = false;
        for &(val, lbl) in opts.iter().rev() {
            let sel = current.trim().eq_ignore_ascii_case(val);
            let (bg, fg) = if sel {
                (p.accent, Color32::WHITE)
            } else {
                (p.tag_bg, p.tag_text)
            };
            let w = (lbl.chars().count() as f32 * 11.0 + 22.0).clamp(54.0, 96.0);
            if soft_button(ui, lbl, bg, fg, w, true).clicked() {
                *current = val.to_string();
                hit = true;
            }
        }
        hit
    };

    match cfg {
        VisualConfig::Mysql {
            port,
            bind_address,
            charset,
            time_zone,
            relaxed_sql_mode,
            max_connections,
            innodb_buffer_pool_size,
            max_allowed_packet,
            slow_query_log,
            long_query_time,
        } => {
            section_title(ui, "基础网络与兼容");
            form_row(
                ui,
                "监听端口 (port)",
                "默认 3306；多版本 MySQL 并存时可改为 3307 / 3308",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "3306", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "访问范围 (bind-address)",
                "仅本机连接选 127.0.0.1；允许局域网同事连接选 0.0.0.0",
                &mut |ui| {
                    for (val, lbl) in [("0.0.0.0", "允许局域网"), ("127.0.0.1", "仅本机")] {
                        let sel = bind_address.trim() == val;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, lbl, bg, fg, 84.0, true).clicked() {
                            *bind_address = val.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "默认字符集 (character-set-server)",
                "推荐 utf8mb4，完整支持中文与 Emoji 表情",
                &mut |ui| {
                    for cs in ["utf8", "utf8mb4"] {
                        let sel = charset.trim() == cs;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, cs, bg, fg, 76.0, true).clicked() {
                            *charset = cs.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "默认时区 (default-time-zone)",
                "推荐 +8:00（北京时间），避免 Java LocalDateTime 差 8 小时",
                &mut |ui| {
                    for tz in ["SYSTEM", "+0:00", "+8:00"] {
                        let sel = time_zone.trim() == tz;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, tz, bg, fg, 66.0, true).clicked() {
                            *time_zone = tz.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "兼容老项目 SQL 模式（关闭 ONLY_FULL_GROUP_BY）",
                "开启后可解决老项目 GROUP BY 报错与日期零值报错",
                &mut |ui| {
                    if toggle_pills(ui, relaxed_sql_mode, "严格默认", "宽松兼容") {
                        changed = true;
                    }
                },
            );

            section_title(ui, "性能与大 SQL 导入");
            form_row(
                ui,
                "最大连接数 (max_connections)",
                "本地开发推荐 200，微服务较多时可调至 500 或 1000",
                &mut |ui| {
                    for preset in ["1000", "500", "200", "100"] {
                        let sel = max_connections.trim() == preset;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, preset, bg, fg, 54.0, true).clicked() {
                            *max_connections = preset.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "InnoDB 内存缓冲池 (innodb_buffer_pool_size)",
                "缓存索引与热数据，越大查询越快（本地推荐 256M）",
                &mut |ui| {
                    for preset in ["1G", "512M", "256M", "128M"] {
                        let sel = innodb_buffer_pool_size.trim().eq_ignore_ascii_case(preset);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, preset, bg, fg, 58.0, true).clicked() {
                            *innodb_buffer_pool_size = preset.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "最大数据包上限 (max_allowed_packet)",
                "导入超大 .sql 备份文件报错时，可调大到 64M / 256M",
                &mut |ui| {
                    for preset in ["256M", "64M", "16M"] {
                        let sel = max_allowed_packet.trim().eq_ignore_ascii_case(preset);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, preset, bg, fg, 60.0, true).clicked() {
                            *max_allowed_packet = preset.to_string();
                            changed = true;
                        }
                    }
                },
            );

            section_title(ui, "慢查询排错日志");
            form_row(
                ui,
                "慢查询日志 (slow_query_log)",
                "开启后自动记录耗时超过阈值的 SQL，方便排查接口卡顿",
                &mut |ui| {
                    if toggle_pills(ui, slow_query_log, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "慢查询阈值（秒 / long_query_time）",
                "单条 SQL 执行超过此秒数即被记录",
                &mut |ui| {
                    for sec in ["5", "2", "1"] {
                        let sel = long_query_time.trim() == sec;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, &format!("{sec} 秒"), bg, fg, 58.0, true).clicked() {
                            *long_query_time = sec.to_string();
                            changed = true;
                        }
                    }
                },
            );
        }
        VisualConfig::Redis {
            port,
            bind,
            password,
            protected_mode,
            databases,
            maxmemory,
            maxmemory_policy,
            appendonly,
            appendfsync,
            timeout,
            loglevel,
        } => {
            section_title(ui, "网络与安全");
            form_row(
                ui,
                "监听端口 (port)",
                "默认 6379，修改后重启 Redis 生效",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "6379", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "绑定地址 (bind)",
                "仅本机选 127.0.0.1；局域网共享选 0.0.0.0（建议同时设密码）",
                &mut |ui| {
                    for (val, lbl) in [("0.0.0.0", "允许局域网"), ("127.0.0.1", "仅本机")] {
                        let sel = bind.trim() == val;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, lbl, bg, fg, 84.0, true).clicked() {
                            *bind = val.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "访问密码 (requirepass)",
                "留空表示本地免密访问；设置后客户端需带密码连接",
                &mut |ui| {
                    if styled_search_input(ui, p, password, "留空则无密码", 180.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "保护模式 (protected-mode)",
                "未设密码时禁止外网直连，建议保持开启",
                &mut |ui| {
                    if toggle_pills(ui, protected_mode, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "逻辑库数量 (databases)",
                "对应 Spring Boot 的 spring.data.redis.database 索引数",
                &mut |ui| {
                    for db in ["64", "32", "16"] {
                        let sel = databases.trim() == db;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, db, bg, fg, 54.0, true).clicked() {
                            *databases = db.to_string();
                            changed = true;
                        }
                    }
                },
            );

            section_title(ui, "内存与淘汰策略");
            form_row(
                ui,
                "最大内存上限 (maxmemory)",
                "防止本地缓存无限增长拖慢电脑",
                &mut |ui| {
                    for mem in ["1gb", "512mb", "256mb", "128mb"] {
                        let sel = maxmemory.trim().eq_ignore_ascii_case(mem);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, mem, bg, fg, 60.0, true).clicked() {
                            *maxmemory = mem.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "内存淘汰策略 (maxmemory-policy)",
                "内存满时的处理方式：allkeys-lru 淘汰最久未用键（推荐）",
                &mut |ui| {
                    for pol in ["noeviction", "volatile-lru", "allkeys-lru"] {
                        let sel = maxmemory_policy.trim().eq_ignore_ascii_case(pol);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, pol, bg, fg, 86.0, true).clicked() {
                            *maxmemory_policy = pol.to_string();
                            changed = true;
                        }
                    }
                },
            );

            section_title(ui, "持久化与运维");
            form_row(
                ui,
                "AOF 持久化 (appendonly)",
                "开启后重启 Redis 不会丢失缓存数据",
                &mut |ui| {
                    if toggle_pills(ui, appendonly, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "AOF 刷盘策略 (appendfsync)",
                "everysec 每秒刷盘兼顾性能与安全（推荐）",
                &mut |ui| {
                    for fsync in ["no", "always", "everysec"] {
                        let sel = appendfsync.trim().eq_ignore_ascii_case(fsync);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, fsync, bg, fg, 74.0, true).clicked() {
                            *appendfsync = fsync.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "连接空闲超时 (timeout)",
                "0 表示永不主动断开空闲连接",
                &mut |ui| {
                    for t in ["600", "300", "0"] {
                        let sel = timeout.trim() == t;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        let lbl = if t == "0" { "不超时" } else { t };
                        if soft_button(ui, lbl, bg, fg, 60.0, true).clicked() {
                            *timeout = t.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "日志级别 (loglevel)",
                "排查连接问题可临时切到 verbose",
                &mut |ui| {
                    for lvl in ["warning", "verbose", "notice"] {
                        let sel = loglevel.trim().eq_ignore_ascii_case(lvl);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, lvl, bg, fg, 68.0, true).clicked() {
                            *loglevel = lvl.to_string();
                            changed = true;
                        }
                    }
                },
            );
        }
        VisualConfig::Nginx {
            worker_processes,
            worker_connections,
            client_max_body_size,
            gzip,
            port,
            server_name,
            root_dir,
            proxy_rules,
            ssl_enabled,
            ssl_port,
            ssl_cert,
            ssl_key,
        } => {
            section_title(ui, "全局性能与文件上传");
            form_row(
                ui,
                "工作进程数 (worker_processes)",
                "本地开发一般 1 即可，压测可选 auto",
                &mut |ui| {
                    for wp in ["auto", "4", "2", "1"] {
                        let sel = worker_processes.trim() == wp;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, wp, bg, fg, 54.0, true).clicked() {
                            *worker_processes = wp.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "最大并发连接 (worker_connections)",
                "单个进程可同时处理的连接上限",
                &mut |ui| {
                    for wc in ["4096", "2048", "1024"] {
                        let sel = worker_connections.trim() == wc;
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, wc, bg, fg, 60.0, true).clicked() {
                            *worker_connections = wc.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "最大上传体积 (client_max_body_size)",
                "解决上传图片/附件报 413 Request Entity Too Large",
                &mut |ui| {
                    for sz in ["500m", "100m", "50m", "10m"] {
                        let sel = client_max_body_size.trim().eq_ignore_ascii_case(sz);
                        let (bg, fg) = if sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, sz, bg, fg, 58.0, true).clicked() {
                            *client_max_body_size = sz.to_string();
                            changed = true;
                        }
                    }
                },
            );
            form_row(
                ui,
                "Gzip 静态压缩 (gzip)",
                "开启后自动压缩 JS/CSS/JSON 响应体",
                &mut |ui| {
                    if toggle_pills(ui, gzip, "关闭", "开启") {
                        changed = true;
                    }
                },
            );

            section_title(ui, "站点基础设置");
            form_row(
                ui,
                "监听端口 (listen)",
                "默认 80；若 80 被占用可改为 8080 / 8088",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "80", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "站点域名 (server_name)",
                "默认 localhost，可配合 Hosts 绑定本地域名",
                &mut |ui| {
                    if styled_search_input(ui, p, server_name, "localhost", 190.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "静态网站根目录 (root)",
                "前端 dist 打包产物或静态页面所在目录",
                &mut |ui| {
                    if styled_search_input(ui, p, root_dir, "留空使用默认目录", 240.0) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "本地 HTTPS 证书（SSL）");
            form_row(
                ui,
                "启用 HTTPS (SSL)",
                "开启后同时监听 HTTPS 端口，支持自签名或自定义证书",
                &mut |ui| {
                    if toggle_pills(ui, ssl_enabled, "关闭", "启用") {
                        changed = true;
                    }
                },
            );
            if *ssl_enabled {
                form_row(
                    ui,
                    "HTTPS 监听端口",
                    "默认 443；如被占用可改为 8443",
                    &mut |ui| {
                        if styled_search_input(ui, p, ssl_port, "443", 110.0) {
                            changed = true;
                        }
                    },
                );
                form_row(
                    ui,
                    "快速生成本地自签名证书",
                    "零命令自动生成含 localhost / 127.0.0.1 与当前域名的 PEM 证书与私钥",
                    &mut |ui| {
                        if soft_button(ui, "一键生成本地证书", p.accent_soft, p.accent_text, 128.0, true)
                            .clicked()
                        {
                            match generate_local_ssl_cert(ssl_dir, server_name) {
                                Ok((cpath, kpath)) => {
                                    *ssl_cert = cpath.to_string_lossy().replace('\\', "/");
                                    *ssl_key = kpath.to_string_lossy().replace('\\', "/");
                                    changed = true;
                                    *toast_out = Some(
                                        "已生成本地 SSL 证书与私钥，点击「保存配置」并重启 Nginx 生效"
                                            .into(),
                                    );
                                }
                                Err(e) => {
                                    *toast_out = Some(format!("生成证书失败: {e}"));
                                }
                            }
                        }
                    },
                );
                form_row(
                    ui,
                    "证书文件路径 (.crt / .pem)",
                    "支持一键生成的本地证书或自行准备的测试证书",
                    &mut |ui| {
                        if styled_search_input(ui, p, ssl_cert, "C:/.../localhost.crt", 260.0) {
                            changed = true;
                        }
                    },
                );
                form_row(
                    ui,
                    "私钥文件路径 (.key)",
                    "与上方证书配套的私钥文件",
                    &mut |ui| {
                        if styled_search_input(ui, p, ssl_key, "C:/.../localhost.key", 260.0) {
                            changed = true;
                        }
                    },
                );
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("反向代理与跨域（支持多条规则 & WebSocket）")
                        .strong()
                        .size(13.0)
                        .color(p.accent_text),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if soft_button(ui, "+ 新增代理规则", p.accent_soft, p.accent_text, 108.0, true)
                        .clicked()
                    {
                        let next_idx = proxy_rules.len() + 1;
                        let default_prefix = if next_idx == 1 {
                            "/api/".to_string()
                        } else {
                            format!("/api{next_idx}/")
                        };
                        proxy_rules.push(NginxProxyRule {
                            prefix: default_prefix,
                            target: "http://127.0.0.1:8080".to_string(),
                            cors: true,
                            websocket: false,
                        });
                        changed = true;
                    }
                });
            });
            ui.add_space(4.0);

            if proxy_rules.is_empty() {
                Frame::new()
                    .fill(p.bg_card_alt)
                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(
                                "暂未添加反向代理规则，点击右上角「+ 新增代理规则」可同时代理多个后端服务（如 /api/、/auth/、/ws/）。",
                            )
                            .size(12.0)
                            .color(p.text_muted),
                        );
                    });
            } else {
                let mut remove_idx: Option<usize> = None;
                for (idx, rule) in proxy_rules.iter_mut().enumerate() {
                    Frame::new()
                        .fill(p.bg_card_alt)
                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin::symmetric(14, 10))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("规则 #{}", idx + 1))
                                        .strong()
                                        .size(12.5)
                                        .color(p.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if soft_button(
                                        ui,
                                        "删除",
                                        p.danger_soft,
                                        p.danger_text,
                                        52.0,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        remove_idx = Some(idx);
                                    }
                                    let (ws_bg, ws_fg) = if rule.websocket {
                                        (p.accent, Color32::WHITE)
                                    } else {
                                        (p.tag_bg, p.tag_text)
                                    };
                                    if soft_button(ui, "WebSocket", ws_bg, ws_fg, 84.0, true)
                                        .clicked()
                                    {
                                        rule.websocket = !rule.websocket;
                                        changed = true;
                                    }
                                    let (cors_bg, cors_fg) = if rule.cors {
                                        (p.accent, Color32::WHITE)
                                    } else {
                                        (p.tag_bg, p.tag_text)
                                    };
                                    if soft_button(ui, "CORS 跨域", cors_bg, cors_fg, 82.0, true)
                                        .clicked()
                                    {
                                        rule.cors = !rule.cors;
                                        changed = true;
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("路径前缀").size(11.5).color(p.text_muted),
                                );
                                if styled_search_input(ui, p, &mut rule.prefix, "/api/", 150.0) {
                                    changed = true;
                                }
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("目标地址").size(11.5).color(p.text_muted),
                                );
                                let rem = (ui.available_width() - 4.0).max(180.0);
                                if styled_search_input(
                                    ui,
                                    p,
                                    &mut rule.target,
                                    "http://127.0.0.1:8080",
                                    rem,
                                ) {
                                    changed = true;
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
                if let Some(rm) = remove_idx {
                    proxy_rules.remove(rm);
                    changed = true;
                }
            }
        }
        VisualConfig::Nacos {
            port,
            context_path,
            auth_enabled,
            auth_secret_key,
            use_mysql,
            mysql_url,
            mysql_user,
            mysql_password,
        } => {
            section_title(ui, "基础服务设置");
            form_row(
                ui,
                "服务端口 (server.port)",
                "控制台与客户端接入端口，默认 8848（gRPC 端口自动为 +1000）",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "8848", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "控制台路径 (contextPath)",
                "浏览器访问路径，例如 http://127.0.0.1:8848/nacos",
                &mut |ui| {
                    if styled_search_input(ui, p, context_path, "/nacos", 160.0) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "鉴权与安全");
            form_row(
                ui,
                "开启控制台鉴权 (auth.enabled)",
                "开启后登录控制台与客户端注册均需鉴权，本地调试可按需开关",
                &mut |ui| {
                    if toggle_pills(ui, auth_enabled, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
            if *auth_enabled {
                form_row(
                    ui,
                    "JWT Token Secret Key",
                    "Nacos 2.4 开启鉴权必填项，需为 32 字节以上的 Base64 字符串",
                    &mut |ui| {
                        if styled_search_input(ui, p, auth_secret_key, "Base64 密钥", 250.0) {
                            changed = true;
                        }
                    },
                );
            }

            section_title(ui, "数据存储模式");
            form_row(
                ui,
                "持久化存储引擎",
                "单机开发推荐内置 Derby 零配置；需要持久化或联调可切外置 MySQL",
                &mut |ui| {
                    if toggle_pills(ui, use_mysql, "内置 Derby", "外置 MySQL") {
                        changed = true;
                    }
                },
            );
            if *use_mysql {
                form_row(
                    ui,
                    "MySQL JDBC 地址 (db.url.0)",
                    "需提前在 MySQL 中创建对应库并导入 Nacos 初始化 SQL",
                    &mut |ui| {
                        if styled_search_input(
                            ui,
                            p,
                            mysql_url,
                            "jdbc:mysql://127.0.0.1:3306/nacos?...",
                            250.0,
                        ) {
                            changed = true;
                        }
                    },
                );
                form_row(
                    ui,
                    "数据库用户名 (db.user.0)",
                    "例如 root",
                    &mut |ui| {
                        if styled_search_input(ui, p, mysql_user, "root", 160.0) {
                            changed = true;
                        }
                    },
                );
                form_row(
                    ui,
                    "数据库密码 (db.password.0)",
                    "空密码可直接留空",
                    &mut |ui| {
                        if styled_search_input(ui, p, mysql_password, "留空表示无密码", 180.0) {
                            changed = true;
                        }
                    },
                );
            }
        }
        VisualConfig::Postgres {
            port,
            listen_addresses,
            max_connections,
            shared_buffers,
            work_mem,
            timezone,
            log_statement,
        } => {
            section_title(ui, "连接与网络");
            form_row(
                ui,
                "监听端口 (port)",
                "PostgreSQL 默认端口 5432，保存后自动同步服务列表",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "5432", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "监听地址 (listen_addresses)",
                "localhost 仅本机可连，* 允许局域网其他设备连接",
                &mut |ui| {
                    if option_pills(ui, listen_addresses, &[("localhost", "仅本机"), ("*", "局域网 (*)")]) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "默认时区 (timezone)",
                "推荐 Asia/Shanghai，避免时间字段偏差 8 小时",
                &mut |ui| {
                    if option_pills(
                        ui,
                        timezone,
                        &[("Asia/Shanghai", "北京时间"), ("UTC", "UTC")],
                    ) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "性能与日志");
            form_row(
                ui,
                "最大连接数 (max_connections)",
                "并发连接上限，本地开发推荐 100 ~ 200",
                &mut |ui| {
                    if option_pills(
                        ui,
                        max_connections,
                        &[("100", "100"), ("200", "200"), ("500", "500")],
                    ) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "共享缓冲区 (shared_buffers)",
                "数据库缓存内存大小，推荐 128MB ~ 256MB",
                &mut |ui| {
                    if option_pills(
                        ui,
                        shared_buffers,
                        &[("128MB", "128MB"), ("256MB", "256MB"), ("512MB", "512MB")],
                    ) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "单查询排序内存 (work_mem)",
                "排序与哈希连接可用内存，复杂查询可调大",
                &mut |ui| {
                    if option_pills(
                        ui,
                        work_mem,
                        &[("4MB", "4MB"), ("16MB", "16MB"), ("32MB", "32MB")],
                    ) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "SQL 语句日志 (log_statement)",
                "调试排错时可临时切到 all 记录所有执行语句",
                &mut |ui| {
                    if option_pills(
                        ui,
                        log_statement,
                        &[("none", "关闭"), ("ddl", "仅 DDL"), ("all", "全部 SQL")],
                    ) {
                        changed = true;
                    }
                },
            );
        }
        VisualConfig::Tomcat {
            port,
            shutdown_port,
            uri_encoding,
            max_threads,
            connection_timeout,
        } => {
            section_title(ui, "HTTP 连接器设置");
            form_row(
                ui,
                "HTTP 服务端口 (Connector port)",
                "默认 8080，与 Spring Boot 冲突时可改成 8081 / 8090",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "8080", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "关闭指令端口 (Server port)",
                "用于接收 shutdown.bat 停止指令，默认 8005",
                &mut |ui| {
                    if styled_search_input(ui, p, shutdown_port, "8005", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "URL 字符编码 (URIEncoding)",
                "强制 UTF-8 避免 GET 请求中文参数乱码",
                &mut |ui| {
                    if option_pills(ui, uri_encoding, &[("UTF-8", "UTF-8"), ("GBK", "GBK")]) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "线程池与超时");
            form_row(
                ui,
                "最大工作线程数 (maxThreads)",
                "同时处理 HTTP 请求的最大线程数，默认 200",
                &mut |ui| {
                    if option_pills(
                        ui,
                        max_threads,
                        &[("100", "100"), ("200", "200"), ("500", "500")],
                    ) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "连接超时时间 (connectionTimeout)",
                "单位毫秒，默认 20000（20 秒）",
                &mut |ui| {
                    if styled_search_input(ui, p, connection_timeout, "20000", 120.0) {
                        changed = true;
                    }
                },
            );
        }
        VisualConfig::MongoDb {
            port,
            bind_ip,
            db_path,
            auth_enabled,
        } => {
            section_title(ui, "网络与连接");
            form_row(
                ui,
                "监听端口 (net.port)",
                "MongoDB 默认端口 27017，保存后自动同步服务列表",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "27017", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "绑定地址 (net.bindIp)",
                "仅本机连接选 127.0.0.1；允许局域网设备访问选 0.0.0.0",
                &mut |ui| {
                    if option_pills(
                        ui,
                        bind_ip,
                        &[("127.0.0.1", "仅本机"), ("0.0.0.0", "允许局域网")],
                    ) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "存储与安全");
            form_row(
                ui,
                "数据存储目录 (storage.dbPath)",
                "默认位于 Moka-Runtime/data/mongodb-8.0 绿色目录",
                &mut |ui| {
                    if styled_search_input(ui, p, db_path, "数据目录路径", 260.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "访问鉴权 (security.authorization)",
                "本地开发默认关闭免密直连；需要账号密码验证时可开启",
                &mut |ui| {
                    if toggle_pills(ui, auth_enabled, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
        }
        VisualConfig::Mailpit {
            port,
            smtp_port,
            bind_host,
            max_messages,
            accept_any_auth,
        } => {
            section_title(ui, "端口与网络");
            form_row(
                ui,
                "Web 预览后台端口 (MP_UI_BIND_ADDR)",
                "浏览器打开 http://127.0.0.1:8025 查看拦截到的所有邮件",
                &mut |ui| {
                    if styled_search_input(ui, p, port, "8025", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "SMTP 接收端口 (MP_SMTP_BIND_ADDR)",
                "后端代码发邮件时将 SMTP Host 设为 127.0.0.1、端口设为 1025",
                &mut |ui| {
                    if styled_search_input(ui, p, smtp_port, "1025", 110.0) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "监听地址",
                "本地开发推荐 127.0.0.1；局域网联调选 0.0.0.0",
                &mut |ui| {
                    if option_pills(
                        ui,
                        bind_host,
                        &[("127.0.0.1", "仅本机"), ("0.0.0.0", "允许局域网")],
                    ) {
                        changed = true;
                    }
                },
            );

            section_title(ui, "邮件捕获规则");
            form_row(
                ui,
                "最大保留邮件数 (MP_MAX_MESSAGES)",
                "超过上限后自动滚动清理最早的测试邮件，默认 500",
                &mut |ui| {
                    if option_pills(
                        ui,
                        max_messages,
                        &[("200", "200"), ("500", "500"), ("2000", "2000")],
                    ) {
                        changed = true;
                    }
                },
            );
            form_row(
                ui,
                "免密接受任意 SMTP 认证",
                "开启后后端无论填什么邮箱账号密码都能成功发信并被捕获",
                &mut |ui| {
                    if toggle_pills(ui, accept_any_auth, "关闭", "开启") {
                        changed = true;
                    }
                },
            );
        }
    }

    changed
}

