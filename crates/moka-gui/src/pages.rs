use super::*;

impl MokaApp {
    pub(super) fn render_dashboard(
        &mut self,
        ui: &mut egui::Ui,
        p: ThemePalette,
        busy: &HashSet<String>,
        actions: &mut Vec<UiAction>,
    ) {
        let installed_runtimes: Vec<SoftwareStatusView> = self
            .software_views
            .iter()
            .filter(|v| v.manifest.env_link_name.is_some() && v.installed)
            .cloned()
            .collect();
        let active_runtimes: Vec<&SoftwareStatusView> = installed_runtimes
            .iter()
            .filter(|v| v.active_env)
            .collect();

        let installed_services: Vec<SoftwareStatusView> = self
            .software_views
            .iter()
            .filter(|v| v.manifest.is_service && v.installed)
            .cloned()
            .collect();
        let running_count = installed_services.iter().filter(|v| v.running).count();

        // 顶部 3 张等高 KPI 卡片
        ui.columns(3, |cols| {
            card_frame(&mut cols[0], p, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(104.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("已激活语言运行时").size(12.0).color(p.text_muted));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if chip_button(
                            ui,
                            "打开终端",
                            11.0,
                            p.accent_soft,
                            p.accent_text,
                            Stroke::NONE,
                            6,
                            62.0,
                            22.0,
                        )
                        .clicked()
                        {
                            actions.push(UiAction::OpenMokaTerminal);
                        }
                    });
                });
                ui.add_space(4.0);
                let summary = if active_runtimes.is_empty() {
                    "尚未安装语言运行时".to_string()
                } else {
                    active_runtimes
                        .iter()
                        .map(|r| r.manifest.id.to_uppercase())
                        .collect::<Vec<_>>()
                        .join(" · ")
                };
                ui.add(
                    egui::Label::new(
                        RichText::new(summary)
                            .strong()
                            .size(15.5)
                            .color(p.text_primary),
                    )
                    .truncate(),
                );
                ui.add_space(10.0);
                ScrollArea::horizontal()
                    .id_salt("dash_rt_pills")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if installed_runtimes.is_empty() {
                                ui.label(
                                    RichText::new("支持 Node / Go / Python / JDK 多版本秒切")
                                        .size(11.5)
                                        .color(p.text_muted),
                                );
                            } else {
                                for rt in &installed_runtimes {
                                    let (fill, fg) = if rt.active_env {
                                        (p.accent, Color32::WHITE)
                                    } else {
                                        (p.tag_bg, p.tag_text)
                                    };
                                    if soft_button(
                                        ui,
                                        &rt.manifest.id.to_uppercase(),
                                        fill,
                                        fg,
                                        56.0,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::ActivateEnv(rt.manifest.clone()));
                                    }
                                }
                            }
                        });
                    });
            });

            let total_mem_bytes: u64 = installed_services
                .iter()
                .filter(|v| v.running)
                .filter_map(|v| v.pid.and_then(|pid| self.proc_metrics.get(&pid)))
                .map(|m| m.memory_bytes)
                .sum();

            card_frame(&mut cols[1], p, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(104.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("运行中服务").size(12.0).color(p.text_muted));
                    if total_mem_bytes > 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            pill_badge(
                                ui,
                                &format!("内存 {}", format_bytes(total_mem_bytes)),
                                p.tag_bg,
                                p.tag_text,
                            );
                        });
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(running_count.to_string())
                            .strong()
                            .size(24.0)
                            .color(p.success),
                    );
                    ui.label(
                        RichText::new(format!("/ {} 已安装", installed_services.len()))
                            .size(13.0)
                            .color(p.text_muted),
                    );
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if soft_button(
                        ui,
                        "全部启动",
                        p.success_soft,
                        p.success_text,
                        72.0,
                        !installed_services.is_empty(),
                    )
                    .clicked()
                    {
                        actions.push(UiAction::StartAllServices);
                    }
                    if soft_button(
                        ui,
                        "全部停止",
                        p.danger_soft,
                        p.danger_text,
                        72.0,
                        running_count > 0,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::StopAllServices);
                    }
                });
            });

            card_frame(&mut cols[2], p, |ui| {
                ui.set_width(ui.available_width());
                ui.set_height(104.0);
                ui.label(RichText::new("绿色隔离目录").size(12.0).color(p.text_muted));
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(self.layout.root_path().to_string_lossy())
                            .strong()
                            .size(12.5)
                            .color(p.text_primary),
                    )
                    .truncate(),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if soft_button(ui, "打开目录", p.tag_bg, p.tag_text, 76.0, true).clicked() {
                        actions.push(UiAction::OpenDir(self.layout.root_path().to_path_buf()));
                    }
                    if soft_button(ui, "复制路径", p.tag_bg, p.tag_text, 76.0, true).clicked() {
                        ui.ctx()
                            .copy_text(self.layout.root_path().to_string_lossy().to_string());
                        self.set_toast("运行时目录路径已复制到剪贴板");
                    }
                });
            });
        });

        // 自定义快捷场景组
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("快捷场景组")
                    .strong()
                    .size(15.0)
                    .color(p.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if soft_button(ui, "+ 新建场景组", p.accent_soft, p.accent_text, 98.0, true)
                    .clicked()
                {
                    actions.push(UiAction::OpenPresetEditor(None));
                }
            });
        });
        ui.add_space(10.0);

        if self.presets.is_empty() {
            card_frame(ui, p, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("还没有自定义场景组，可将常用数据库、缓存与网关打包为一键启动组合。")
                            .size(12.5)
                            .color(p.text_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if soft_button(ui, "立即创建", p.accent_soft, p.accent_text, 80.0, true)
                            .clicked()
                        {
                            actions.push(UiAction::OpenPresetEditor(None));
                        }
                    });
                });
            });
        } else {
            let presets_snapshot = self.presets.clone();
            let views_ref = &self.software_views;
            for chunk in presets_snapshot.chunks(2).enumerate() {
                let (row_idx, pair) = chunk;
                ui.columns(2, |cols| {
                    for (col_idx, preset) in pair.iter().enumerate() {
                        let global_idx = row_idx * 2 + col_idx;
                        let summary = preset
                            .service_ids
                            .iter()
                            .map(|id| {
                                views_ref
                                    .iter()
                                    .find(|v| &v.manifest.id == id)
                                    .map(|v| format!("{} {}", v.manifest.name, v.manifest.version))
                                    .unwrap_or_else(|| id.clone())
                            })
                            .collect::<Vec<_>>()
                            .join(" · ");

                        card_frame(&mut cols[col_idx], p, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&preset.name)
                                        .strong()
                                        .size(14.0)
                                        .color(p.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if soft_button(
                                        ui,
                                        "一键启动",
                                        p.accent_soft,
                                        p.accent_text,
                                        76.0,
                                        !preset.service_ids.is_empty(),
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::StartPreset(
                                            preset.name.clone(),
                                            preset.service_ids.clone(),
                                        ));
                                    }
                                    if soft_button(ui, "编辑", p.tag_bg, p.tag_text, 50.0, true)
                                        .clicked()
                                    {
                                        actions.push(UiAction::OpenPresetEditor(Some(global_idx)));
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if summary.is_empty() {
                                        "未选择任何服务"
                                    } else {
                                        &summary
                                    })
                                    .size(12.0)
                                    .color(p.text_muted),
                                )
                                .truncate(),
                            );
                        });
                    }
                });
                ui.add_space(8.0);
            }
        }

        ui.add_space(20.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("已安装服务")
                    .strong()
                    .size(15.0)
                    .color(p.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if soft_button(ui, "软件商店 →", p.accent_soft, p.accent_text, 92.0, true).clicked()
                {
                    actions.push(UiAction::SwitchTab(NavTab::Store));
                }
            });
        });
        ui.add_space(10.0);

        if installed_services.is_empty() {
            card_frame(ui, p, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(26.0);
                    ui.label(
                        RichText::new("当前尚未安装 MySQL / Redis / Nginx 等服务")
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("点击下方按钮即可从国内镜像源一键下载并免配置启动")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(16.0);
                    if primary_button(ui, p, "前往软件商店一键安装", 184.0, true).clicked() {
                        actions.push(UiAction::SwitchTab(NavTab::Store));
                    }
                    ui.add_space(26.0);
                });
            });
        } else {
            for svc in &installed_services {
                let port_buf = self
                    .port_overrides
                    .entry(svc.manifest.id.clone())
                    .or_insert_with(|| svc.port.map(|pt| pt.to_string()).unwrap_or_default());
                let saved_pwd = self.db_passwords.get(&svc.manifest.id).map(|s| s.as_str());
                let metrics = svc.pid.and_then(|pid| self.proc_metrics.get(&pid).copied());
                render_service_row(
                    ui,
                    p,
                    svc,
                    port_buf,
                    saved_pwd,
                    metrics,
                    busy.contains(&svc.manifest.id),
                    actions,
                );
                ui.add_space(8.0);
            }
        }
    }

    pub(super) fn render_services(
        &mut self,
        ui: &mut egui::Ui,
        p: ThemePalette,
        busy: &HashSet<String>,
        actions: &mut Vec<UiAction>,
    ) {
        let services: Vec<SoftwareStatusView> = self
            .software_views
            .iter()
            .filter(|v| v.manifest.is_service && v.installed)
            .cloned()
            .collect();

        if services.is_empty() {
            card_frame(ui, p, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new("暂无已安装服务，请先在「软件商店」下载安装。")
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    if primary_button(ui, p, "前往软件商店", 130.0, true).clicked() {
                        actions.push(UiAction::SwitchTab(NavTab::Store));
                    }
                    ui.add_space(24.0);
                });
            });
            return;
        }

        for svc in &services {
            let port_buf = self
                .port_overrides
                .entry(svc.manifest.id.clone())
                .or_insert_with(|| svc.port.map(|pt| pt.to_string()).unwrap_or_default());
            let saved_pwd = self.db_passwords.get(&svc.manifest.id).map(|s| s.as_str());
            let metrics = svc.pid.and_then(|pid| self.proc_metrics.get(&pid).copied());
            render_service_row(
                ui,
                p,
                svc,
                port_buf,
                saved_pwd,
                metrics,
                busy.contains(&svc.manifest.id),
                actions,
            );
            ui.add_space(8.0);
        }
    }

    pub(super) fn render_sites(
        &mut self,
        ui: &mut egui::Ui,
        p: ThemePalette,
        actions: &mut Vec<UiAction>,
    ) {
        let nginx_view = self
            .software_views
            .iter()
            .find(|v| v.manifest.config_kind.as_deref() == Some("nginx") && v.installed)
            .cloned();

        let Some(nginx) = nginx_view else {
            card_frame(ui, p, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(28.0);
                    ui.label(
                        RichText::new("多站点虚拟主机基于 Nginx 运行，请先在软件商店安装 Nginx。")
                            .size(14.0)
                            .color(p.text_primary),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("安装后可管理任意多个本地域名（如 api.local、admin.local），支持独立反代、本地 HTTPS 与自动同步 Hosts。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(14.0);
                    if primary_button(ui, p, "前往软件商店安装 Nginx", 180.0, true).clicked() {
                        actions.push(UiAction::SwitchTab(NavTab::Store));
                    }
                    ui.add_space(28.0);
                });
            });
            return;
        };

        // 顶部状态提示条（显示 Nginx 当前运行状态与本地 CA 信任状态）
        let ca_busy = self.shared.lock().unwrap_or_else(|e| e.into_inner()).ca_busy;
        card_frame(ui, p, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if nginx.running {
                    pill_badge(ui, "● Nginx 运行中", p.success_soft, p.success_text);
                } else {
                    pill_badge(ui, "● Nginx 已停止", p.tag_bg, p.text_muted);
                }
                if self.ca_trusted {
                    pill_badge(ui, "● 本地 CA 已信任", p.success_soft, p.success_text);
                } else {
                    pill_badge(ui, "● 本地 CA 未信任", p.tag_bg, p.text_muted);
                }
                ui.label(
                    RichText::new(format!(
                        "共 {} 个本地站点（已启用 {} 个）",
                        self.sites.len(),
                        self.sites.iter().filter(|s| s.enabled).count()
                    ))
                    .size(12.5)
                    .color(p.text_secondary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !nginx.running
                        && soft_button(ui, "启动 Nginx", p.success_soft, p.success_text, 88.0, true)
                            .clicked()
                    {
                        actions.push(UiAction::ToggleService(nginx.clone()));
                    }
                    if self.ca_trusted {
                        if soft_button(
                            ui,
                            if ca_busy { "处理中..." } else { "移除 CA 信任" },
                            p.tag_bg,
                            p.tag_text,
                            96.0,
                            !ca_busy,
                        )
                        .clicked()
                        {
                            actions.push(UiAction::UninstallLocalCa);
                        }
                    } else if soft_button(
                        ui,
                        if ca_busy {
                            "安装中..."
                        } else {
                            "信任本地 CA（绿锁）"
                        },
                        p.accent_soft,
                        p.accent_text,
                        136.0,
                        !ca_busy,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::InstallLocalCa);
                    }
                });
            });
        });

        ui.add_space(12.0);

        if self.sites.is_empty() {
            card_frame(ui, p, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(28.0);
                    ui.label(
                        RichText::new("暂未创建本地站点")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("点击右上角「+ 新建站点」，为你的前端、后台或微服务项目分配独立本地域名、反向代理与本地 HTTPS。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(14.0);
                    if primary_button(ui, p, "+ 新建第一个站点", 148.0, true).clicked() {
                        actions.push(UiAction::OpenSiteEditor(None));
                    }
                    ui.add_space(28.0);
                });
            });
            return;
        }

        let mut toggle_idx: Option<usize> = None;
        let mut delete_idx: Option<usize> = None;
        for (idx, site) in self.sites.iter().enumerate() {
            card_frame(ui, p, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if site.enabled {
                                pill_badge(ui, "● 已启用", p.success_soft, p.success_text);
                            } else {
                                pill_badge(ui, "● 已停用", p.tag_bg, p.text_muted);
                            }
                            ui.label(
                                RichText::new(&site.domain)
                                    .strong()
                                    .size(15.0)
                                    .color(p.text_primary),
                            );
                            pill_badge(
                                ui,
                                &format!("HTTP :{}", site.port),
                                p.tag_bg,
                                p.tag_text,
                            );
                            if site.ssl_enabled {
                                pill_badge(
                                    ui,
                                    &format!("HTTPS :{}", site.ssl_port),
                                    p.accent_soft,
                                    p.accent_text,
                                );
                            }
                            if !site.proxy_rules.is_empty() {
                                pill_badge(
                                    ui,
                                    &format!("{} 条反代", site.proxy_rules.len()),
                                    p.tag_bg,
                                    p.tag_text,
                                );
                            }
                        });
                        ui.add_space(5.0);
                        let root_desc = if site.root_dir.trim().is_empty() {
                            format!(
                                "自动目录: {}\\data\\sites\\{}",
                                self.layout.root_path().display(),
                                site.domain
                            )
                        } else {
                            site.root_dir.clone()
                        };
                        ui.label(
                            RichText::new(root_desc)
                                .size(11.5)
                                .color(p.text_muted),
                        );
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if soft_button(ui, "删除", p.danger_soft, p.danger_text, 54.0, true)
                            .clicked()
                        {
                            delete_idx = Some(idx);
                        }
                        if secondary_button(ui, p, "编辑", 58.0, true).clicked() {
                            actions.push(UiAction::OpenSiteEditor(Some(idx)));
                        }
                        let (t_lbl, t_bg, t_fg) = if site.enabled {
                            ("停用", p.tag_bg, p.tag_text)
                        } else {
                            ("启用", p.success_soft, p.success_text)
                        };
                        if soft_button(ui, t_lbl, t_bg, t_fg, 54.0, true).clicked() {
                            toggle_idx = Some(idx);
                        }
                        if site.enabled && nginx.running {
                            let url = if site.ssl_enabled {
                                if site.ssl_port == 443 {
                                    format!("https://{}/", site.domain)
                                } else {
                                    format!("https://{}:{}/", site.domain, site.ssl_port)
                                }
                            } else if site.port == 80 {
                                format!("http://{}/", site.domain)
                            } else {
                                format!("http://{}:{}/", site.domain, site.port)
                            };
                            if soft_button(ui, "访问", p.accent_soft, p.accent_text, 56.0, true)
                                .clicked()
                            {
                                actions.push(UiAction::OpenUrl(url));
                            }
                        }
                    });
                });
            });
            ui.add_space(8.0);
        }

        if let Some(i) = toggle_idx {
            actions.push(UiAction::ToggleSiteEnabled(i));
        }
        if let Some(i) = delete_idx {
            actions.push(UiAction::DeleteSite(i));
        }
    }

    pub(super) fn render_store_toolbar(&mut self, ui: &mut egui::Ui, p: ThemePalette) {
        // 分类分段切换栏（Claude / macOS 风格 Segmented Control）+ 搜索框
        ui.horizontal(|ui| {
            Frame::new()
                .fill(p.bg_sidebar)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let filters: [(Option<SoftwareCategory>, &str); 7] = [
                            (None, "全部"),
                            (Some(SoftwareCategory::Runtime), "语言运行时"),
                            (Some(SoftwareCategory::Database), "数据库"),
                            (Some(SoftwareCategory::Cache), "Redis"),
                            (Some(SoftwareCategory::Web), "Web / 网关"),
                            (Some(SoftwareCategory::Middleware), "中间件"),
                            (Some(SoftwareCategory::BuildTool), "构建工具"),
                        ];
                        for (idx, (cat, label)) in filters.into_iter().enumerate() {
                            let selected = self.store_filter == cat;
                            let id = ui.make_persistent_id(("store_seg", idx));
                            let hovered =
                                ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
                            let (fill, stroke, shadow, text_color) = if selected {
                                (
                                    p.bg_card,
                                    Stroke::new(1.0_f32, p.border_subtle),
                                    p.card_shadow,
                                    p.accent_text,
                                )
                            } else if hovered {
                                (p.bg_nav_hover, Stroke::NONE, Shadow::NONE, p.text_primary)
                            } else {
                                (Color32::TRANSPARENT, Stroke::NONE, Shadow::NONE, p.text_secondary)
                            };

                            let resp = Frame::new()
                                .fill(fill)
                                .stroke(stroke)
                                .shadow(shadow)
                                .corner_radius(CornerRadius::same(7))
                                .inner_margin(Margin::symmetric(12, 5))
                                .show(ui, |ui| {
                                    let txt = if selected {
                                        RichText::new(label).strong().size(12.5).color(text_color)
                                    } else {
                                        RichText::new(label).size(12.5).color(text_color)
                                    };
                                    ui.label(txt);
                                })
                                .response
                                .interact(egui::Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);

                            ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
                            if resp.clicked() {
                                self.store_filter = cat;
                            }
                        }
                    });
                });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                styled_search_input(ui, p, &mut self.store_search, "搜索软件或版本...", 210.0);
            });
        });
    }

    pub(super) fn render_store(
        &mut self,
        ui: &mut egui::Ui,
        p: ThemePalette,
        downloads: &HashMap<String, DownloadProgress>,
        busy: &HashSet<String>,
        actions: &mut Vec<UiAction>,
    ) {
        let kw = self.store_search.trim().to_lowercase();

        // 按软件名称自动分组（同名多版本合并为一张卡片，卡片内切换版本）
        let mut group_order: Vec<String> = Vec::new();
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, item) in self.software_views.iter().enumerate() {
            if let Some(ref f) = self.store_filter {
                if &item.manifest.category != f {
                    continue;
                }
            }
            if !kw.is_empty()
                && !item.manifest.name.to_lowercase().contains(&kw)
                && !item.manifest.version.to_lowercase().contains(&kw)
                && !item.manifest.description.to_lowercase().contains(&kw)
            {
                continue;
            }
            let key = item.manifest.name.clone();
            if !groups.contains_key(&key) {
                group_order.push(key.clone());
            }
            groups.entry(key).or_default().push(idx);
        }

        for group_name in group_order {
            let indices = &groups[&group_name];
            let installed_count = indices
                .iter()
                .filter(|&&i| self.software_views[i].installed)
                .count();

            // 确定该卡片当前选中的版本（默认选第一个；若上次选的仍在当前筛选结果中则保留）
            let selected_id = self
                .store_selected_versions
                .get(&group_name)
                .filter(|sid| indices.iter().any(|&i| &self.software_views[i].manifest.id == *sid))
                .cloned()
                .unwrap_or_else(|| self.software_views[indices[0]].manifest.id.clone());

            let active_idx = indices
                .iter()
                .copied()
                .find(|&i| self.software_views[i].manifest.id == selected_id)
                .unwrap_or(indices[0]);
            let item = &self.software_views[active_idx];

            let is_busy = busy.contains(&item.manifest.id);
            // 若该软件组内任意版本正在下载，显示其进度条
            let active_prog = indices
                .iter()
                .find_map(|&i| downloads.get(&self.software_views[i].manifest.id));

            card_frame(ui, p, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&item.manifest.name)
                                    .strong()
                                    .size(15.0)
                                    .color(p.text_primary),
                            );
                            pill_badge(
                                ui,
                                category_label(&item.manifest.category),
                                p.tag_bg,
                                p.tag_text,
                            );
                            if installed_count > 0 {
                                let badge_txt = if indices.len() > 1 {
                                    format!("已装 {installed_count} 个版本")
                                } else {
                                    "已安装".to_string()
                                };
                                pill_badge(ui, &badge_txt, p.success_soft, p.success_text);
                            }
                            if item.active_env {
                                pill_badge(ui, "当前激活", p.accent_soft, p.accent_text);
                            }
                        });
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(&item.manifest.description)
                                .size(12.0)
                                .color(p.text_muted),
                        );

                        // 版本切换行（紧凑胶囊直接左对齐）
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            for &v_idx in indices {
                                let v_item = &self.software_views[v_idx];
                                let is_sel = v_idx == active_idx;
                                let prefix = if v_item.installed { "● " } else { "" };
                                let v_lbl = format!("{prefix}v{}", v_item.manifest.version);
                                let (bg, fg) = if is_sel {
                                    (p.accent_soft, p.accent_text)
                                } else {
                                    (p.tag_bg, p.tag_text)
                                };
                                let stroke = if is_sel {
                                    Stroke::new(1.0_f32, p.accent)
                                } else {
                                    Stroke::NONE
                                };
                                if chip_button(
                                    ui, &v_lbl, 11.5, bg, fg, stroke, 7, 62.0, 24.0,
                                )
                                .clicked()
                                {
                                    self.store_selected_versions
                                        .insert(group_name.clone(), v_item.manifest.id.clone());
                                }
                            }
                        });
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !item.installed {
                            if is_busy && active_prog.is_some() {
                                if soft_button(
                                    ui,
                                    "取消下载",
                                    p.danger_soft,
                                    p.danger_text,
                                    88.0,
                                    true,
                                )
                                .clicked()
                                {
                                    actions.push(UiAction::CancelDownload(item.manifest.id.clone()));
                                }
                            } else {
                                let label = if is_busy { "安装中..." } else { "一键安装" };
                                if primary_button(ui, p, label, 88.0, !is_busy).clicked() {
                                    actions.push(UiAction::Install(item.manifest.clone()));
                                }
                            }
                        } else {
                            if soft_button(
                                ui,
                                "卸载",
                                p.danger_soft,
                                p.danger_text,
                                64.0,
                                !is_busy && !item.running,
                            )
                            .clicked()
                            {
                                actions.push(UiAction::Uninstall(item.manifest.clone()));
                            }
                            if item.manifest.env_link_name.is_some()
                                && !item.active_env
                                && soft_button(
                                    ui,
                                    "设为当前版本",
                                    p.accent_soft,
                                    p.accent_text,
                                    98.0,
                                    true,
                                )
                                .clicked()
                            {
                                actions.push(UiAction::ActivateEnv(item.manifest.clone()));
                            }
                        }
                    });
                });

                if let Some(prog) = active_prog {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&prog.message).size(11.5).color(p.text_secondary));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{:.0}%", prog.percent))
                                    .strong()
                                    .size(11.5)
                                    .color(p.accent_text),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.add(
                        egui::ProgressBar::new((prog.percent / 100.0).clamp(0.0, 1.0))
                            .desired_height(6.0)
                            .fill(p.accent),
                    );
                }
            });
            ui.add_space(8.0);
        }
    }

    pub(super) fn render_toolbox(
        &mut self,
        ui: &mut egui::Ui,
        p: ThemePalette,
        loading: bool,
        actions: &mut Vec<UiAction>,
    ) {
        // 国内开发镜像加速中心（npm / pip / Go / Maven）
        card_frame(ui, p, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("国内开发镜像加速中心")
                        .strong()
                        .size(14.0)
                        .color(p.text_primary),
                );
                let china_cnt = self.dev_mirrors.iter().filter(|m| m.is_china).count();
                pill_badge(
                    ui,
                    &format!("已加速 {china_cnt}/{}", self.dev_mirrors.len()),
                    if china_cnt == self.dev_mirrors.len() && china_cnt > 0 {
                        p.success_soft
                    } else {
                        p.tag_bg
                    },
                    if china_cnt == self.dev_mirrors.len() && china_cnt > 0 {
                        p.success_text
                    } else {
                        p.tag_text
                    },
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if soft_button(ui, "全部切换国内源", p.accent_soft, p.accent_text, 108.0, true)
                        .clicked()
                    {
                        actions.push(UiAction::SetAllDevMirrors(true));
                    }
                    if soft_button(ui, "全部恢复官方源", p.tag_bg, p.tag_text, 108.0, true)
                        .clicked()
                    {
                        actions.push(UiAction::SetAllDevMirrors(false));
                    }
                });
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("一键配置当前用户的 npm、pip、Go 与 Maven 镜像源（无需管理员权限，自动保留原有鉴权配置与备份）。")
                    .size(11.5)
                    .color(p.text_muted),
            );
            ui.add_space(10.0);

            let mirrors_snapshot = self.dev_mirrors.clone();
            for pair in mirrors_snapshot.chunks(2) {
                ui.columns(2, |cols| {
                    for (ci, m) in pair.iter().enumerate() {
                        Frame::new()
                            .fill(p.bg_card_alt)
                            .stroke(Stroke::new(1.0_f32, p.border_subtle))
                            .corner_radius(CornerRadius::same(10))
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(&mut cols[ci], |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(m.kind.label())
                                            .strong()
                                            .size(13.0)
                                            .color(p.text_primary),
                                    );
                                    if m.is_china {
                                        pill_badge(ui, "● 国内加速", p.success_soft, p.success_text);
                                    } else {
                                        pill_badge(ui, "● 官方默认", p.tag_bg, p.text_muted);
                                    }
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if m.is_china {
                                            if soft_button(
                                                ui,
                                                "恢复官方",
                                                p.tag_bg,
                                                p.tag_text,
                                                72.0,
                                                true,
                                            )
                                            .clicked()
                                            {
                                                actions.push(UiAction::SetDevMirror(m.kind, false));
                                            }
                                        } else if soft_button(
                                            ui,
                                            "切国内源",
                                            p.accent_soft,
                                            p.accent_text,
                                            72.0,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            actions.push(UiAction::SetDevMirror(m.kind, true));
                                        }
                                    });
                                });
                                ui.add_space(5.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!(
                                            "{}  ·  {}",
                                            m.current_url,
                                            m.kind.china_desc()
                                        ))
                                        .size(11.0)
                                        .color(p.text_muted),
                                    )
                                    .truncate(),
                                );
                            });
                    }
                });
                ui.add_space(8.0);
            }
        });

        ui.add_space(14.0);

        card_frame(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "本机 TCP 监听端口（共 {} 个）",
                        self.filtered_port_indices.len()
                    ))
                    .strong()
                    .size(14.0)
                    .color(p.text_primary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    styled_search_input(
                        ui,
                        p,
                        &mut self.port_filter,
                        "筛选端口 / 进程名 / PID...",
                        210.0,
                    );
                });
            });
            ui.add_space(10.0);

            if loading && self.ports.is_empty() {
                ui.label(RichText::new("正在后台扫描监听端口...").color(p.text_muted));
            } else {
                // 按可用宽度精确切分 5 列（表头与数据行共享同一套坐标，像素级严格对齐）
                let total_w = (ui.available_width() - 10.0).max(540.0);
                let w_port = (total_w * 0.11).clamp(72.0, 100.0);
                let w_pid = (total_w * 0.11).clamp(72.0, 100.0);
                let w_act = 92.0_f32;
                let flex = (total_w - w_port - w_pid - w_act).max(240.0);
                let w_addr = flex * 0.44;
                let w_proc = flex * 0.56;

                let col_spans = |base_x: f32| -> [(f32, f32); 5] {
                    let x0 = base_x + 8.0;
                    let x1 = x0 + w_port;
                    let x2 = x1 + w_addr;
                    let x3 = x2 + w_proc;
                    let x4 = x3 + w_pid;
                    let x5 = base_x + total_w - 8.0;
                    [(x0, x1), (x1, x2), (x2, x3), (x3, x4), (x4, x5)]
                };

                // 表头行
                let (hdr_rect, _) =
                    ui.allocate_exact_size(Vec2::new(total_w, 26.0), egui::Sense::hover());
                let spans = col_spans(hdr_rect.min.x);
                let headers = [
                    ("端口", Align::LEFT),
                    ("监听地址", Align::LEFT),
                    ("进程名", Align::LEFT),
                    ("PID", Align::LEFT),
                    ("操作", Align::RIGHT),
                ];
                for (i, (title, align)) in headers.iter().enumerate() {
                    let cell = Rect::from_min_max(
                        Pos2::new(spans[i].0, hdr_rect.min.y),
                        Pos2::new(spans[i].1 - 6.0, hdr_rect.max.y),
                    );
                    let layout = if *align == Align::RIGHT {
                        Layout::right_to_left(Align::Center)
                    } else {
                        Layout::left_to_right(Align::Center)
                    };
                    ui.scope_builder(egui::UiBuilder::new().max_rect(cell).layout(layout), |ui| {
                        ui.label(RichText::new(*title).strong().size(12.0).color(p.text_muted));
                    });
                }

                ui.separator();

                let row_h = 36.0;
                let total_rows = self.filtered_port_indices.len();
                ScrollArea::vertical()
                    .id_salt("port_virtual_scroll")
                    .max_height(270.0)
                    .show_rows(ui, row_h, total_rows, |ui, range| {
                        for row_idx in range {
                            let port_idx = self.filtered_port_indices[row_idx];
                            let port = &self.ports[port_idx];
                            let (row_rect, _) = ui.allocate_exact_size(
                                Vec2::new(total_w, row_h),
                                egui::Sense::hover(),
                            );

                            if row_idx % 2 == 1 {
                                ui.painter().rect_filled(
                                    row_rect,
                                    CornerRadius::same(6),
                                    p.bg_row_stripe,
                                );
                            }

                            let spans = col_spans(row_rect.min.x);
                            let make_cell = |i: usize| {
                                Rect::from_min_max(
                                    Pos2::new(spans[i].0, row_rect.min.y),
                                    Pos2::new(spans[i].1 - 6.0, row_rect.max.y),
                                )
                            };

                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(make_cell(0))
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    ui.label(
                                        RichText::new(port.port.to_string())
                                            .monospace()
                                            .strong()
                                            .size(12.5)
                                            .color(p.accent_text),
                                    );
                                },
                            );

                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(make_cell(1))
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&port.local_addr)
                                                .monospace()
                                                .size(12.0)
                                                .color(p.text_muted),
                                        )
                                        .truncate(),
                                    );
                                },
                            );

                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(make_cell(2))
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&port.process_name)
                                                .size(12.5)
                                                .color(p.text_primary),
                                        )
                                        .truncate(),
                                    );
                                },
                            );

                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(make_cell(3))
                                    .layout(Layout::left_to_right(Align::Center)),
                                |ui| {
                                    ui.label(
                                        RichText::new(port.pid.to_string())
                                            .monospace()
                                            .size(12.0)
                                            .color(p.text_muted),
                                    );
                                },
                            );

                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .max_rect(make_cell(4))
                                    .layout(Layout::right_to_left(Align::Center)),
                                |ui| {
                                    if soft_button(
                                        ui,
                                        "结束进程",
                                        p.danger_soft,
                                        p.danger_text,
                                        76.0,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::KillPort(port.clone()));
                                    }
                                },
                            );
                        }
                    });
            }
        });

        ui.add_space(14.0);

        let mut do_save_hosts = false;
        card_frame(ui, p, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Windows Hosts 可视化管理")
                        .strong()
                        .size(14.0)
                        .color(p.text_primary),
                );
                if self.hosts_dirty {
                    pill_badge(ui, "未保存", p.accent_soft, p.accent_text);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if soft_button(ui, "打开目录", p.tag_bg, p.tag_text, 76.0, true).clicked() {
                        actions.push(UiAction::OpenDir(PathBuf::from(
                            r"C:\Windows\System32\drivers\etc",
                        )));
                    }
                    if self.hosts_dirty
                        && primary_button(ui, p, "保存 Hosts（需授权）", 142.0, true).clicked()
                    {
                        do_save_hosts = true;
                    }
                });
            });
            ui.add_space(8.0);

            // 快捷新增本地域名行
            ui.horizontal(|ui| {
                styled_search_input(ui, p, &mut self.host_new_ip, "127.0.0.1", 110.0);
                styled_search_input(ui, p, &mut self.host_new_domain, "本地域名，如 api.local", 180.0);
                styled_search_input(ui, p, &mut self.host_new_comment, "备注（可选）", 130.0);
                if soft_button(ui, "+ 添加映射", p.accent_soft, p.accent_text, 86.0, true).clicked()
                {
                    let ip = self.host_new_ip.trim().to_string();
                    let dom = self.host_new_domain.trim().to_string();
                    if ip.parse::<std::net::IpAddr>().is_err() {
                        self.set_toast("请输入合法的 IP 地址（例如 127.0.0.1）");
                    } else if dom.is_empty() {
                        self.set_toast("请输入要绑定的域名（例如 api.local）");
                    } else {
                        self.host_entries.push(HostEntry {
                            ip,
                            domain: dom,
                            comment: self.host_new_comment.trim().to_string(),
                            enabled: true,
                        });
                        self.host_new_domain.clear();
                        self.host_new_comment.clear();
                        self.hosts_dirty = true;
                    }
                }
            });

            ui.add_space(8.0);
            Frame::new()
                .fill(p.bg_card_alt)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.host_entries.is_empty() {
                        ui.label(
                            RichText::new("当前 Hosts 暂无有效域名映射，可在上方直接添加（如 127.0.0.1 api.local）。")
                                .size(12.0)
                                .color(p.text_muted),
                        );
                    } else {
                        let mut remove_idx: Option<usize> = None;
                        ScrollArea::vertical()
                            .id_salt("hosts_scroll")
                            .max_height(190.0)
                            .show(ui, |ui| {
                                for (idx, entry) in self.host_entries.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        let (st_lbl, st_bg, st_fg) = if entry.enabled {
                                            ("已启用", p.success_soft, p.success_text)
                                        } else {
                                            ("已禁用", p.tag_bg, p.text_muted)
                                        };
                                        if soft_button(ui, st_lbl, st_bg, st_fg, 58.0, true)
                                            .clicked()
                                        {
                                            entry.enabled = !entry.enabled;
                                            self.hosts_dirty = true;
                                        }
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(&entry.ip)
                                                .monospace()
                                                .size(12.0)
                                                .color(if entry.enabled {
                                                    p.text_primary
                                                } else {
                                                    p.text_muted
                                                }),
                                        );
                                        ui.add_space(8.0);
                                        ui.label(
                                            RichText::new(&entry.domain)
                                                .strong()
                                                .size(12.5)
                                                .color(if entry.enabled {
                                                    p.text_primary
                                                } else {
                                                    p.text_muted
                                                }),
                                        );
                                        if !entry.comment.is_empty() {
                                            ui.label(
                                                RichText::new(format!("# {}", entry.comment))
                                                    .size(11.5)
                                                    .color(p.text_muted),
                                            );
                                        }
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if soft_button(
                                                    ui,
                                                    "删除",
                                                    p.danger_soft,
                                                    p.danger_text,
                                                    50.0,
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    remove_idx = Some(idx);
                                                }
                                            },
                                        );
                                    });
                                    ui.add_space(4.0);
                                }
                            });
                        if let Some(rm) = remove_idx {
                            self.host_entries.remove(rm);
                            self.hosts_dirty = true;
                        }
                    }
                });
        });
        if do_save_hosts {
            self.spawn_save_hosts(ui.ctx().clone());
        }
    }

    pub(super) fn render_settings_toolbar(&mut self, ui: &mut egui::Ui, p: ThemePalette) {
        ui.horizontal(|ui| {
            Frame::new()
                .fill(p.bg_sidebar)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(3))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let tabs = [
                            (SettingsSubTab::General, "常规与外观"),
                            (SettingsSubTab::Sources, "软件源与环境"),
                            (SettingsSubTab::Guide, "使用教程"),
                            (SettingsSubTab::About, "关于与更新"),
                        ];
                        for (idx, (sub, label)) in tabs.into_iter().enumerate() {
                            let selected = self.settings_tab == sub;
                            let id = ui.make_persistent_id(("settings_seg", idx));
                            let hovered =
                                ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
                            let (fill, stroke, shadow, text_color) = if selected {
                                (
                                    p.bg_card,
                                    Stroke::new(1.0_f32, p.border_subtle),
                                    p.card_shadow,
                                    p.accent_text,
                                )
                            } else if hovered {
                                (p.bg_nav_hover, Stroke::NONE, Shadow::NONE, p.text_primary)
                            } else {
                                (Color32::TRANSPARENT, Stroke::NONE, Shadow::NONE, p.text_secondary)
                            };
                            let resp = Frame::new()
                                .fill(fill)
                                .stroke(stroke)
                                .shadow(shadow)
                                .corner_radius(CornerRadius::same(7))
                                .inner_margin(Margin::symmetric(14, 5))
                                .show(ui, |ui| {
                                    let txt = if selected {
                                        RichText::new(label).strong().size(12.5).color(text_color)
                                    } else {
                                        RichText::new(label).size(12.5).color(text_color)
                                    };
                                    ui.label(txt);
                                })
                                .response
                                .interact(egui::Sense::click())
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
                            if resp.clicked() {
                                self.settings_tab = sub;
                                if sub == SettingsSubTab::General {
                                    self.cache_size_bytes =
                                        calc_dir_size(&self.layout.root_path().join("downloads"));
                                }
                            }
                        }
                    });
                });
        });
    }

    pub(super) fn render_settings(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        p: ThemePalette,
        update_checking: bool,
        actions: &mut Vec<UiAction>,
    ) {
        match self.settings_tab {
            SettingsSubTab::General => {
                // 外观主题卡片
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("外观主题")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("切换 Claude 暖米白浅色或暖炭黑深色主题，自动保存下次启动生效。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    let cur_theme = if matches!(self.theme_mode, ThemeMode::ClaudeLight) {
                        0
                    } else {
                        1
                    };
                    if let Some(pick) = segmented_choice(
                        ui,
                        p,
                        "settings_theme",
                        &["暖米白（浅色）", "暖炭黑（深色）"],
                        cur_theme,
                    ) {
                        self.theme_mode = if pick == 0 {
                            ThemeMode::ClaudeLight
                        } else {
                            ThemeMode::ClaudeDark
                        };
                        apply_theme(ctx, ThemePalette::from_mode(self.theme_mode));
                        self.persist_settings();
                        self.set_toast(if pick == 0 {
                            "已切换为暖米白浅色主题"
                        } else {
                            "已切换为暖炭黑深色主题"
                        });
                    }
                });

                ui.add_space(12.0);

                // 关闭窗口行为卡片
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("关闭窗口行为")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("决定点击右上角关闭按钮时，是否自动停止 Moka 托管的数据库与中间件进程。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    let cur_exit = if self.minimize_to_tray {
                        0
                    } else if self.stop_on_exit {
                        1
                    } else {
                        2
                    };
                    if let Some(pick) = segmented_choice(
                        ui,
                        p,
                        "settings_exit",
                        &["最小化到托盘（推荐）", "停止服务并退出", "保留后台并退出"],
                        cur_exit,
                    ) {
                        match pick {
                            0 => {
                                self.minimize_to_tray = true;
                                self.stop_on_exit = true;
                                self.set_toast("点击关闭时将最小化到右下角系统托盘（右键托盘可彻底退出）");
                            }
                            1 => {
                                self.minimize_to_tray = false;
                                self.stop_on_exit = true;
                                self.set_toast("已设为退出时自动停止所有托管服务");
                            }
                            _ => {
                                self.minimize_to_tray = false;
                                self.stop_on_exit = false;
                                self.set_toast("已设为退出时保留后台服务运行");
                            }
                        }
                        self.persist_settings();
                    }
                });

                ui.add_space(12.0);

                // 托盘样式卡片（悬浮面板 ↔ 系统原生菜单，切换立即生效）
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("托盘样式")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("点托盘图标时弹出什么（Windows 上左键始终打开主界面，右键弹出所选样式）。面板：可搜索并一键启停服务、切换运行时；菜单：系统原生菜单。切换后立即生效。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    let cur_tray = if self.tray_style == TrayStyle::Panel {
                        0
                    } else {
                        1
                    };
                    if let Some(pick) = segmented_choice(
                        ui,
                        p,
                        "settings_tray_style",
                        &["悬浮面板（推荐）", "系统原生菜单"],
                        cur_tray,
                    ) {
                        self.tray_style = if pick == 0 {
                            TrayStyle::Panel
                        } else {
                            TrayStyle::Menu
                        };
                        self.tray.set_style(self.tray_style);
                        if self.tray_style == TrayStyle::Menu {
                            self.tray_popover_open = false;
                        }
                        self.persist_settings();
                        self.set_toast(if self.tray_style == TrayStyle::Panel {
                            "已切换为「悬浮面板」，右键托盘图标即刻生效"
                        } else {
                            "已切换为「系统原生菜单」，右键托盘图标即刻生效"
                        });
                    }
                });

                ui.add_space(12.0);

                // 开机自启与启动自动拉起场景组
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("开机自启与自动拉起")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("免管理员权限写入当前用户开机启动项，并可在启动 Moka 时自动拉起一个常用场景组。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    let cur_auto = if self.auto_start { 1 } else { 0 };
                    if let Some(pick) = segmented_choice(
                        ui,
                        p,
                        "settings_auto_start",
                        &["关闭开机自启", "开机自动启动 Moka"],
                        cur_auto,
                    ) {
                        let enable = pick == 1;
                        match set_auto_start(enable) {
                            Ok(()) => {
                                self.auto_start = enable;
                                self.persist_settings();
                                self.set_toast(if enable {
                                    "已开启 Windows 开机自启（当前用户，免管理员权限）"
                                } else {
                                    "已关闭 Windows 开机自启"
                                });
                            }
                            Err(e) => self.set_toast(format!("{e:#}")),
                        }
                    }

                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("启动 Moka 时自动拉起场景组")
                            .strong()
                            .size(12.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        let none_sel = self.auto_start_preset.is_empty();
                        let (nbg, nfg) = if none_sel {
                            (p.accent, Color32::WHITE)
                        } else {
                            (p.tag_bg, p.tag_text)
                        };
                        if soft_button(ui, "不自动拉起", nbg, nfg, 88.0, true).clicked()
                            && !none_sel
                        {
                            self.auto_start_preset.clear();
                            self.persist_settings();
                            self.set_toast("已取消启动时自动拉起场景组");
                        }
                        let preset_names: Vec<String> =
                            self.presets.iter().map(|pr| pr.name.clone()).collect();
                        for pname in preset_names {
                            let sel = self.auto_start_preset == pname;
                            let (bg, fg) = if sel {
                                (p.accent, Color32::WHITE)
                            } else {
                                (p.tag_bg, p.tag_text)
                            };
                            let w = (pname.chars().count() as f32 * 12.0 + 26.0).clamp(72.0, 180.0);
                            if soft_button(ui, &pname, bg, fg, w, true).clicked() && !sel {
                                self.auto_start_preset = pname.clone();
                                self.persist_settings();
                                self.set_toast(format!("下次启动 Moka 时将自动拉起「{pname}」"));
                            }
                        }
                    });
                });

                ui.add_space(12.0);

                // 绿色目录与存储维护卡片
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("绿色隔离目录与维护")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "运行根目录：{}",
                            self.layout.root_path().to_string_lossy()
                        ))
                        .monospace()
                        .size(12.0)
                        .color(p.text_secondary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "安装包下载缓存（downloads/）：{}",
                            format_bytes(self.cache_size_bytes)
                        ))
                        .size(12.0)
                        .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, p, "打开运行目录", 104.0, true).clicked() {
                            actions.push(UiAction::OpenDir(self.layout.root_path().to_path_buf()));
                        }
                        if secondary_button(ui, p, "清理下载缓存", 104.0, self.cache_size_bytes > 0)
                            .clicked()
                        {
                            let dl_dir = self.layout.root_path().join("downloads");
                            if let Ok(entries) = std::fs::read_dir(&dl_dir) {
                                for e in entries.flatten() {
                                    let _ = std::fs::remove_file(e.path());
                                }
                            }
                            self.cache_size_bytes = calc_dir_size(&dl_dir);
                            self.set_toast("已清理全部下载缓存");
                        }
                        if secondary_button(ui, p, "查看诊断日志 (moka-app.log)", 176.0, true)
                            .clicked()
                        {
                            let log_path = self.layout.root_path().join("logs").join("moka-app.log");
                            if log_path.exists() {
                                open_file_default(&log_path);
                            } else {
                                self.set_toast("暂无诊断日志文件");
                            }
                        }
                    });
                });
            }

            SettingsSubTab::Sources => {
                // 默认镜像源策略
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("默认下载镜像策略")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("主源超时或损坏时，下载器会自动回退到备用镜像源。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(12.0);
                    let cur_mirror = if matches!(self.mirror_region, MirrorRegion::Global) {
                        0
                    } else {
                        1
                    };
                    if let Some(pick) = segmented_choice(
                        ui,
                        p,
                        "settings_mirror",
                        &["国际官方源优先", "中国大陆镜像优先"],
                        cur_mirror,
                    ) {
                        self.mirror_region = if pick == 0 {
                            MirrorRegion::Global
                        } else {
                            MirrorRegion::China
                        };
                        self.persist_settings();
                        self.set_toast(if pick == 0 {
                            "已保存默认下载策略：国际官方源优先"
                        } else {
                            "已保存默认下载策略：中国大陆镜像优先"
                        });
                    }
                });

                ui.add_space(12.0);

                // 远程清单热更新
                let is_updating = self
                    .shared
                    .lock()
                    .map(|s| s.catalog_updating)
                    .unwrap_or(false);
                let local_file = self.layout.catalog_file();
                let has_local_override = local_file.exists();
                let mut do_fetch = false;
                let mut do_edit_local = false;
                let mut do_reload_local = false;
                let mut do_reset_builtin = false;

                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("软件清单源（在线热更新 & 本地自定义）")
                                .strong()
                                .size(14.5)
                                .color(p.text_primary),
                        );
                        ui.add_space(6.0);
                        pill_badge(
                            ui,
                            if has_local_override {
                                "已启用本地/在线覆盖"
                            } else {
                                "内置默认清单"
                            },
                            if has_local_override {
                                p.success_soft
                            } else {
                                p.tag_bg
                            },
                            if has_local_override {
                                p.success_text
                            } else {
                                p.tag_text
                            },
                        );
                    });
                    ui.add_space(8.0);
                    let w = (ui.available_width() - 24.0).max(320.0);
                    styled_search_input(
                        ui,
                        p,
                        &mut self.catalog_url_input,
                        "https://raw.githubusercontent.com/.../catalog.json",
                        w,
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let lbl = if is_updating { "拉取中..." } else { "立即在线更新" };
                        if primary_button(ui, p, lbl, 114.0, !is_updating).clicked() {
                            do_fetch = true;
                        }
                        if secondary_button(ui, p, "编辑本地清单", 104.0, true).clicked() {
                            do_edit_local = true;
                        }
                        if secondary_button(ui, p, "重新加载本地", 104.0, true).clicked() {
                            do_reload_local = true;
                        }
                        if has_local_override
                            && soft_button(ui, "恢复内置", p.danger_soft, p.danger_text, 76.0, true)
                                .clicked()
                        {
                            do_reset_builtin = true;
                        }
                    });
                });

                if do_fetch {
                    self.spawn_update_remote_catalog(ctx.clone());
                }
                if do_edit_local {
                    if !local_file.exists() {
                        if let Some(parent) = local_file.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let _ = std::fs::write(&local_file, builtin_catalog_json());
                    }
                    open_file_default(&local_file);
                    self.set_toast("已打开本地 catalog.json，保存后点击「重新加载本地」生效");
                }
                if do_reload_local {
                    self.reload_local_catalog(ctx.clone());
                }
                if do_reset_builtin {
                    let _ = std::fs::remove_file(&local_file);
                    self.reload_local_catalog(ctx.clone());
                    self.set_toast("已恢复内置默认软件源");
                }

                ui.add_space(12.0);

                // 运行时 PATH 助手（零入侵）
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("语言运行时 PATH 配置助手（零入侵）")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Moka 不会擅自修改你的系统环境变量。如需在终端中跟随 Moka 秒切版本，可将下方路径添加到 Windows 用户 PATH 最前面：")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(10.0);

                    let cur_dir = self.layout.root_path().join("current");
                    let path_entries = [
                        ("Node.js", cur_dir.join("node")),
                        ("OpenJDK", cur_dir.join("jdk").join("bin")),
                        ("Go", cur_dir.join("go").join("bin")),
                        ("Python", cur_dir.join("python")),
                        ("Maven", cur_dir.join("maven").join("bin")),
                        ("Gradle", cur_dir.join("gradle").join("bin")),
                    ];
                    let mut all_paths = Vec::new();
                    for (name, path) in &path_entries {
                        let ps = path.to_string_lossy().to_string();
                        all_paths.push(ps.clone());
                        Frame::new()
                            .fill(p.bg_card_alt)
                            .stroke(Stroke::new(1.0_f32, p.border_subtle))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(12, 7))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(*name)
                                            .strong()
                                            .size(12.0)
                                            .color(p.text_primary),
                                    );
                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new(&ps)
                                            .monospace()
                                            .size(11.5)
                                            .color(p.text_secondary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if soft_button(ui, "复制", p.tag_bg, p.tag_text, 54.0, true)
                                            .clicked()
                                        {
                                            ctx.copy_text(ps.clone());
                                            self.set_toast(format!("已复制 {name} 路径"));
                                        }
                                    });
                                });
                            });
                        ui.add_space(5.0);
                    }
                    ui.add_space(4.0);
                    if secondary_button(ui, p, "一键复制全部 PATH（分号拼接）", 200.0, true)
                        .clicked()
                    {
                        ctx.copy_text(all_paths.join(";"));
                        self.set_toast("已复制全部运行时 PATH 到剪贴板");
                    }
                });
            }

            SettingsSubTab::Guide => {
                let guides: [(&str, &str, &[&str]); 4] = [
                    (
                        "1. 30 秒快速上手",
                        "从零搭建本地开发环境",
                        &[
                            "• 在「软件商店」选择需要的数据库、缓存、Web 服务器或语言运行时，点击「一键安装」。",
                            "• 安装完成后回到「仪表盘」或「服务管理」，直接点击「启动」即可无黑框静默运行。",
                            "• 在仪表盘点击「+ 新建场景组」，可以把常用组合（如 MySQL + Redis + Nginx）一键批量拉起。",
                        ],
                    ),
                    (
                        "2. 多版本并存与语言运行时秒切",
                        "项目切换零冲突",
                        &[
                            "• 同类软件在商店内已自动折叠（如 MySQL 8.4 / 8.0 / 5.7），各版本拥有独立数据目录和默认端口，可同时运行。",
                            "• 语言运行时（Node / Go / Python / JDK / Maven）点击「设为当前版本」通过免管理员 NTFS 软链瞬间切换。",
                            "• Tomcat 10.1 与 Nacos 启动时会自动识别 Moka 管理的兼容 JDK，无需手动改系统环境变量。",
                        ],
                    ),
                    (
                        "3. Nginx 多条反向代理与本地 HTTPS",
                        "前后端联调与跨域一步到位",
                        &[
                            "• 打开 Nginx 的「配置」→「可视化配置」，可添加多条反向代理规则（如 /api/、/auth/、/ws/）。",
                            "• 每条规则均支持独立开启 CORS 跨域头与 WebSocket 协议升级，彻底告别本地联调跨域报错。",
                            "• 开启「本地 HTTPS (SSL)」并点击「一键生成本地证书」，无需 OpenSSL 即可自动签发含 localhost/127.0.0.1 的证书。",
                        ],
                    ),
                    (
                        "4. 排错技巧与安全回滚",
                        "改坏配置也不怕",
                        &[
                            "• 每次保存配置文件前，Moka 都会自动在同目录生成 `.bak` 备份；也可点击「恢复默认」回到安全模板。",
                            "• 服务启动失败时，点「日志」切到「仅错误」即可快速定位原因；应用级异常也会记录在 `logs/moka-app.log`。",
                            "• 遇到端口被其他程序占用，在「实用工具箱」搜索端口号即可一键查杀对应进程。",
                        ],
                    ),
                ];

                for (title, sub, bullets) in guides {
                    card_frame(ui, p, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(title)
                                    .strong()
                                    .size(14.5)
                                    .color(p.text_primary),
                            );
                            ui.add_space(6.0);
                            pill_badge(ui, sub, p.accent_soft, p.accent_text);
                        });
                        ui.add_space(8.0);
                        for line in bullets {
                            ui.label(RichText::new(*line).size(12.5).color(p.text_secondary));
                            ui.add_space(3.0);
                        }
                    });
                    ui.add_space(10.0);
                }
            }

            SettingsSubTab::About => {
                // 品牌与版本信息卡
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        Frame::new()
                            .fill(p.accent)
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(Margin::symmetric(14, 9))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("M")
                                        .strong()
                                        .size(22.0)
                                        .color(Color32::WHITE),
                                );
                            });
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Moka")
                                        .strong()
                                        .size(19.0)
                                        .color(p.text_primary),
                                );
                                pill_badge(ui, format!("v{APP_VERSION}").as_str(), p.accent_soft, p.accent_text);
                                pill_badge(ui, "MIT 开源免费", p.success_soft, p.success_text);
                            });
                            ui.add_space(3.0);
                            ui.label(
                                RichText::new("全栈 Windows 本地开发环境管理器 · 100% Pure Rust 原生单文件 · 零 Electron / WebView 依赖")
                                    .size(12.0)
                                    .color(p.text_muted),
                            );
                        });
                    });
                });

                ui.add_space(12.0);

                // 检查更新卡片
                let mut trigger_check = false;
                let mut open_release_url: Option<String> = None;
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("版本更新检查")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("从 GitHub Releases 检查最新版本，也可以填入你自己 Fork 的仓库地址（格式 owner/repo）。")
                            .size(12.0)
                            .color(p.text_muted),
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        styled_search_input(
                            ui,
                            p,
                            &mut self.release_repo_input,
                            "owner/repo",
                            240.0,
                        );
                        let lbl = if update_checking {
                            "检查中..."
                        } else {
                            "检查更新"
                        };
                        if primary_button(ui, p, lbl, 96.0, !update_checking).clicked() {
                            trigger_check = true;
                        }
                    });

                    if let Some(ref res) = self.update_status {
                        ui.add_space(10.0);
                        match res {
                            Ok(info) => {
                                let remote = info.tag_name.trim_start_matches('v');
                                let is_same = remote == APP_VERSION;
                                Frame::new()
                                    .fill(if is_same { p.success_soft } else { p.accent_soft })
                                    .corner_radius(CornerRadius::same(8))
                                    .inner_margin(Margin::symmetric(12, 10))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            let msg = if is_same {
                                                format!("当前已是最新版本 (v{APP_VERSION})")
                                            } else {
                                                format!("发现新版本 {}（当前 v{APP_VERSION}）", info.tag_name)
                                            };
                                            ui.label(
                                                RichText::new(msg)
                                                    .strong()
                                                    .size(12.5)
                                                    .color(if is_same {
                                                        p.success_text
                                                    } else {
                                                        p.accent_text
                                                    }),
                                            );
                                            if !is_same && !info.html_url.is_empty() {
                                                ui.with_layout(
                                                    Layout::right_to_left(Align::Center),
                                                    |ui| {
                                                        if soft_button(
                                                            ui,
                                                            "前往下载",
                                                            p.accent,
                                                            Color32::WHITE,
                                                            82.0,
                                                            true,
                                                        )
                                                        .clicked()
                                                        {
                                                            open_release_url =
                                                                Some(info.html_url.clone());
                                                        }
                                                    },
                                                );
                                            }
                                        });
                                        if !is_same && !info.body.trim().is_empty() {
                                            ui.add_space(6.0);
                                            let preview: String = info
                                                .body
                                                .lines()
                                                .take(6)
                                                .collect::<Vec<_>>()
                                                .join("\n");
                                            ui.label(
                                                RichText::new(preview)
                                                    .size(11.5)
                                                    .color(p.text_secondary),
                                            );
                                        }
                                    });
                            }
                            Err(err) => {
                                ui.label(
                                    RichText::new(err).size(12.0).color(p.text_muted),
                                );
                            }
                        }
                    }
                });
                if trigger_check {
                    self.spawn_check_update(ctx.clone());
                }
                if let Some(url) = open_release_url {
                    ctx.open_url(egui::OpenUrl::new_tab(url));
                }

                ui.add_space(12.0);

                // 作者与开源信息卡片
                card_frame(ui, p, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new("作者与开源信息")
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    ui.add_space(8.0);
                    let repo = self.release_repo_input.trim();
                    let repo_url = format!("https://github.com/{repo}");
                    let issues_url = format!("https://github.com/{repo}/issues");
                    ui.label(
                        RichText::new("• 作者 / 维护者：Moka Open Source Contributors")
                            .size(12.5)
                            .color(p.text_secondary),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("• 技术栈：Rust + eframe / egui + Tokio + reqwest + rcgen")
                            .size(12.5)
                            .color(p.text_secondary),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new("• 开源协议：MIT License — 永久免费、可自由商用与二次开发")
                            .size(12.5)
                            .color(p.text_secondary),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, p, "GitHub 项目主页", 124.0, true).clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(repo_url));
                        }
                        if secondary_button(ui, p, "反馈问题 / 提建议", 130.0, true).clicked() {
                            ctx.open_url(egui::OpenUrl::new_tab(issues_url));
                        }
                    });
                });
            }
        }
    }
}


#[allow(clippy::too_many_arguments)]
fn render_service_row(
    ui: &mut egui::Ui,
    p: ThemePalette,
    svc: &SoftwareStatusView,
    port_buf: &mut String,
    saved_pwd: Option<&str>,
    metrics: Option<ProcessMetrics>,
    is_busy: bool,
    actions: &mut Vec<UiAction>,
) {
    card_frame(ui, p, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    if svc.running {
                        pill_badge(ui, "● 运行中", p.success_soft, p.success_text);
                    } else {
                        pill_badge(ui, "● 已停止", p.tag_bg, p.text_muted);
                    }
                    ui.label(
                        RichText::new(format!("{} {}", svc.manifest.name, svc.manifest.version))
                            .strong()
                            .size(14.5)
                            .color(p.text_primary),
                    );
                    pill_badge(
                        ui,
                        category_label(&svc.manifest.category),
                        p.tag_bg,
                        p.tag_text,
                    );
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("端口").size(11.5).color(p.text_muted));
                    Frame::new()
                        .fill(if svc.running { p.tag_bg } else { p.bg_input })
                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::symmetric(6, 2))
                        .show(ui, |ui| {
                            ui.add_enabled(
                                !svc.running,
                                egui::TextEdit::singleline(port_buf)
                                    .frame(false)
                                    .desired_width(44.0)
                                    .font(egui::TextStyle::Monospace),
                            );
                        });
                    if let Some(pid) = svc.pid {
                        ui.label(
                            RichText::new(format!("·  PID {pid}"))
                                .size(11.5)
                                .color(p.text_muted),
                        );
                    }
                    if svc.running {
                        if let Some(m) = metrics {
                            ui.label(
                                RichText::new(format!(
                                    "·  内存 {}  ·  运行 {}",
                                    format_bytes(m.memory_bytes),
                                    format_uptime(m.uptime_secs)
                                ))
                                .size(11.5)
                                .color(p.text_secondary),
                            );
                        }
                    } else {
                        ui.label(
                            RichText::new(format!("·  {}", svc.install_path))
                                .size(11.5)
                                .color(p.text_muted),
                        );
                    }
                });
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let (btn_label, btn_fill) = if svc.running {
                    ("停止", p.danger)
                } else {
                    ("启动", p.success)
                };
                if solid_button(ui, btn_label, btn_fill, 68.0, !is_busy).clicked() {
                    actions.push(UiAction::ToggleService(svc.clone()));
                }
                if secondary_button(ui, p, "日志", 54.0, true).clicked() {
                    actions.push(UiAction::OpenLogs(svc.clone()));
                }
                if secondary_button(ui, p, "配置", 54.0, true).clicked() {
                    actions.push(UiAction::OpenConfig(svc.clone()));
                }
                if secondary_button(ui, p, "目录", 54.0, true).clicked() {
                    actions.push(UiAction::OpenDir(PathBuf::from(&svc.install_path)));
                }
                if svc.running {
                    let is_backup_db = matches!(
                        svc.manifest.config_kind.as_deref(),
                        Some("mysql") | Some("postgres")
                    );
                    if is_backup_db && secondary_button(ui, p, "备份", 54.0, true).clicked() {
                        actions.push(UiAction::OpenDbBackup(svc.clone()));
                    }
                    if let Some((lbl, is_web, payload)) = service_quick_action(svc, saved_pwd) {
                        let w = if is_web { 54.0 } else { 64.0 };
                        if soft_button(ui, lbl, p.accent_soft, p.accent_text, w, true).clicked() {
                            if is_web {
                                actions.push(UiAction::OpenUrl(payload));
                            } else {
                                actions.push(UiAction::CopyConnStr(payload));
                            }
                        }
                    }
                }
            });
        });
    });
}


