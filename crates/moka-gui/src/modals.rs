use super::*;

impl MokaApp {
    pub(super) fn render_modals_and_overlays(
        &mut self,
        ctx: &egui::Context,
        p: ThemePalette,
        is_maximized: bool,
        busy_snapshot: &HashSet<String>,
        toast_snapshot: &Option<(String, Instant)>,
    ) {
        // 内置配置文件编辑器（Claude 风格居中悬浮编辑面板）
        if self.config_editor_view.is_some() {
            let screen = ctx.screen_rect();
            // 柔和半透明遮罩
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("cfg_backdrop"),
            ))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(95));

            let modal_w = (screen.width() - 120.0).clamp(560.0, 820.0);
            let modal_h = (screen.height() - 120.0).clamp(400.0, 580.0);

            let mut do_save = false;
            let mut do_save_and_restart = false;
            let mut do_reset = false;
            let mut do_external = false;
            let mut do_close = false;
            let mut do_change_pwd = false;
            let mut do_format = false;
            let cfg_kind_owned = self
                .config_editor_view
                .as_ref()
                .and_then(|v| v.manifest.config_kind.clone());
            let fmt_label = config_format_label(cfg_kind_owned.as_deref());
            let pwd_updating = self.shared.lock().map(|s| s.pwd_updating).unwrap_or(false);
            let is_mysql_cfg = self
                .config_editor_view
                .as_ref()
                .map(|v| v.manifest.config_kind.as_deref() == Some("mysql"))
                .unwrap_or(false);
            let cfg_svc_running = self
                .config_editor_view
                .as_ref()
                .and_then(|v| {
                    self.software_views
                        .iter()
                        .find(|sv| sv.manifest.id == v.manifest.id)
                        .map(|sv| sv.running)
                })
                .unwrap_or(false);
            let has_saved_pwd = self
                .config_editor_view
                .as_ref()
                .and_then(|v| self.db_passwords.get(&v.manifest.id))
                .map(|pw| !pw.is_empty())
                .unwrap_or(false);

            egui::Area::new(egui::Id::new("config_editor_modal"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .shadow(Shadow {
                            offset: [0, 8],
                            blur: 28,
                            spread: 0,
                            color: Color32::from_black_alpha(55),
                        })
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(20))
                        .show(ui, |ui| {
                            ui.set_width(modal_w);
                            ui.set_height(modal_h);

                            let title = self
                                .config_editor_view
                                .as_ref()
                                .map(|v| format!("{} {} 配置文件", v.manifest.name, v.manifest.version))
                                .unwrap_or_default();
                            let path_str = self
                                .config_editor_path
                                .as_ref()
                                .map(|pt| pt.to_string_lossy().to_string())
                                .unwrap_or_default();

                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(title)
                                                .strong()
                                                .size(16.0)
                                                .color(p.text_primary),
                                        );
                                        if self.config_editor_dirty {
                                            pill_badge(
                                                ui,
                                                "未保存",
                                                p.accent_soft,
                                                p.accent_text,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(path_str)
                                            .monospace()
                                            .size(11.5)
                                            .color(p.text_muted),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, p, "关闭", 64.0, true).clicked() {
                                        do_close = true;
                                    }
                                    if secondary_button(ui, p, "外部打开", 80.0, true).clicked() {
                                        do_external = true;
                                    }
                                    if secondary_button(ui, p, "恢复默认", 80.0, true).clicked() {
                                        do_reset = true;
                                    }
                                    if cfg_svc_running {
                                        if primary_button(ui, p, "保存并重启", 96.0, true).clicked()
                                        {
                                            do_save_and_restart = true;
                                        }
                                        if secondary_button(ui, p, "仅保存", 68.0, true).clicked() {
                                            do_save = true;
                                        }
                                    } else if primary_button(ui, p, "保存配置", 86.0, true)
                                        .clicked()
                                    {
                                        do_save = true;
                                    }
                                });
                            });

                            ui.add_space(12.0);

                            // 可视化模式 ↔ 高级源码模式 分段切换
                            if self.config_editor_visual.is_some() {
                                ui.horizontal(|ui| {
                                    Frame::new()
                                        .fill(p.bg_sidebar)
                                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                        .corner_radius(CornerRadius::same(9))
                                        .inner_margin(Margin::same(3))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 2.0;
                                                for (is_adv, lbl) in
                                                    [(false, "可视化配置（推荐）"), (true, "高级源码")]
                                                {
                                                    let sel = self.config_editor_advanced == is_adv;
                                                    let (fill, stroke, shadow, fg) = if sel {
                                                        (
                                                            p.bg_card,
                                                            Stroke::new(1.0_f32, p.border_subtle),
                                                            p.card_shadow,
                                                            p.accent_text,
                                                        )
                                                    } else {
                                                        (
                                                            Color32::TRANSPARENT,
                                                            Stroke::NONE,
                                                            Shadow::NONE,
                                                            p.text_secondary,
                                                        )
                                                    };
                                                    let r = Frame::new()
                                                        .fill(fill)
                                                        .stroke(stroke)
                                                        .shadow(shadow)
                                                        .corner_radius(CornerRadius::same(6))
                                                        .inner_margin(Margin::symmetric(12, 4))
                                                        .show(ui, |ui| {
                                                            ui.label(
                                                                RichText::new(lbl)
                                                                    .size(12.0)
                                                                    .color(fg),
                                                            );
                                                        })
                                                        .response
                                                        .interact(egui::Sense::click())
                                                        .on_hover_cursor(
                                                            egui::CursorIcon::PointingHand,
                                                        );
                                                    if r.clicked() && self.config_editor_advanced != is_adv {
                                                        if is_adv {
                                                            // 可视化 -> 源码：先把表单改动同步到文本
                                                            if let Some(ref vcfg) =
                                                                self.config_editor_visual
                                                            {
                                                                self.config_editor_text =
                                                                    apply_visual_config(
                                                                        &self.config_editor_text,
                                                                        vcfg,
                                                                    );
                                                            }
                                                        } else if let Some(ref view) =
                                                            self.config_editor_view
                                                        {
                                                            // 源码 -> 可视化：重新解析
                                                            let pt = view
                                                                .manifest
                                                                .default_port
                                                                .unwrap_or(0);
                                                            self.config_editor_visual =
                                                                parse_visual_config(
                                                                    view.manifest
                                                                        .config_kind
                                                                        .as_deref(),
                                                                    &self.config_editor_text,
                                                                    pt,
                                                                );
                                                        }
                                                        self.config_editor_advanced = is_adv;
                                                        self.config_editor_error = None;
                                                    }
                                                }
                                            });
                                        });

                                    ui.add_space(8.0);
                                    let hint = if self.config_editor_advanced {
                                        "高级模式：保存前自动生成 .bak 备份"
                                    } else {
                                        "安全模式：常用参数一键调整，无需手写配置文件"
                                    };
                                    ui.label(RichText::new(hint).size(11.5).color(p.text_muted));

                                    if self.config_editor_advanced {
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if soft_button(
                                                ui,
                                                "格式化 & 校验",
                                                p.accent_soft,
                                                p.accent_text,
                                                104.0,
                                                true,
                                            )
                                            .clicked()
                                            {
                                                do_format = true;
                                            }
                                            pill_badge(ui, fmt_label, p.tag_bg, p.tag_text);
                                        });
                                    }
                                });
                                ui.add_space(12.0);
                            }

                            let mut form_toast: Option<String> = None;
                            if !self.config_editor_advanced {
                                let ssl_dir = self
                                    .config_editor_path
                                    .as_ref()
                                    .and_then(|pt| pt.parent())
                                    .map(|dir| dir.join("ssl"))
                                    .unwrap_or_else(|| self.layout.conf_dir("nginx").join("ssl"));
                                if let Some(ref mut vcfg) = self.config_editor_visual {
                                    ScrollArea::vertical()
                                        .id_salt("visual_config_scroll")
                                        .max_height(modal_h - 132.0)
                                        .show(ui, |ui| {
                                            if is_mysql_cfg {
                                                ui.add_space(4.0);
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new("账号与密码（root）")
                                                            .strong()
                                                            .size(13.0)
                                                            .color(p.accent_text),
                                                    );
                                                    ui.add_space(4.0);
                                                    pill_badge(
                                                        ui,
                                                        if has_saved_pwd {
                                                            "已设置密码"
                                                        } else {
                                                            "默认免密"
                                                        },
                                                        if has_saved_pwd {
                                                            p.success_soft
                                                        } else {
                                                            p.tag_bg
                                                        },
                                                        if has_saved_pwd {
                                                            p.success_text
                                                        } else {
                                                            p.tag_text
                                                        },
                                                    );
                                                });
                                                ui.add_space(4.0);
                                                Frame::new()
                                                    .fill(p.bg_card_alt)
                                                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                                    .corner_radius(CornerRadius::same(10))
                                                    .inner_margin(Margin::symmetric(14, 10))
                                                    .show(ui, |ui| {
                                                        ui.set_width(ui.available_width());
                                                        ui.horizontal(|ui| {
                                                            ui.vertical(|ui| {
                                                                ui.label(
                                                                    RichText::new("root 登录密码")
                                                                        .strong()
                                                                        .size(12.5)
                                                                        .color(p.text_primary),
                                                                );
                                                                let desc = if cfg_svc_running {
                                                                    "留空表示免密登录，输入新密码后点击右侧立即生效"
                                                                } else {
                                                                    "请先启动该数据库服务，再点击右侧更新密码"
                                                                };
                                                                ui.label(
                                                                    RichText::new(desc)
                                                                        .size(11.0)
                                                                        .color(p.text_muted),
                                                                );
                                                            });
                                                            ui.with_layout(
                                                                Layout::right_to_left(Align::Center),
                                                                |ui| {
                                                                    let lbl = if pwd_updating {
                                                                        "更新中..."
                                                                    } else {
                                                                        "更新密码"
                                                                    };
                                                                    if soft_button(
                                                                        ui,
                                                                        lbl,
                                                                        p.accent_soft,
                                                                        p.accent_text,
                                                                        80.0,
                                                                        cfg_svc_running
                                                                            && !pwd_updating,
                                                                    )
                                                                    .clicked()
                                                                    {
                                                                        do_change_pwd = true;
                                                                    }
                                                                    styled_search_input(
                                                                        ui,
                                                                        p,
                                                                        &mut self.db_new_pwd_input,
                                                                        "留空表示免密",
                                                                        160.0,
                                                                    );
                                                                },
                                                            );
                                                        });
                                                    });
                                                ui.add_space(8.0);
                                            }
                                            let changed = render_visual_config_form(
                                                ui,
                                                p,
                                                vcfg,
                                                &ssl_dir,
                                                &mut form_toast,
                                            );
                                            if changed {
                                                self.config_editor_dirty = true;
                                            }
                                        });
                                }
                            } else {
                                if let Some(ref err_msg) = self.config_editor_error {
                                    Frame::new()
                                        .fill(p.danger_soft)
                                        .stroke(Stroke::new(1.0_f32, p.danger))
                                        .corner_radius(CornerRadius::same(8))
                                        .inner_margin(Margin::symmetric(12, 8))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(
                                                RichText::new(format!("语法错误：{err_msg}"))
                                                    .size(12.0)
                                                    .color(p.danger_text),
                                            );
                                        });
                                    ui.add_space(8.0);
                                }
                                let err_extra_h = if self.config_editor_error.is_some() {
                                    42.0
                                } else {
                                    0.0
                                };
                                let code_w = (modal_w - 36.0).max(400.0);
                                Frame::new()
                                    .fill(p.bg_card_alt)
                                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                    .corner_radius(CornerRadius::same(10))
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ScrollArea::vertical()
                                            .id_salt("config_editor_scroll")
                                            .max_height((modal_h - 135.0 - err_extra_h).max(160.0))
                                            .show(ui, |ui| {
                                                let edit = ui.add(
                                                    egui::TextEdit::multiline(
                                                        &mut self.config_editor_text,
                                                    )
                                                    .frame(false)
                                                    .code_editor()
                                                    .desired_width(code_w)
                                                    .desired_rows(20),
                                                );
                                                if edit.changed() {
                                                    self.config_editor_dirty = true;
                                                    self.config_editor_error = None;
                                                }
                                            });
                                    });
                            }
                            if let Some(msg) = form_toast {
                                self.set_toast(msg);
                            }
                        });
                });

            if do_format {
                match format_and_validate_config(
                    cfg_kind_owned.as_deref(),
                    &self.config_editor_text,
                ) {
                    Ok(formatted) => {
                        // Nginx 额外调用真实 nginx -t 做语义校验
                        let mut semantic_err: Option<String> = None;
                        if cfg_kind_owned.as_deref() == Some("nginx") {
                            if let (Some(ref view), Some(ref conf_path)) =
                                (&self.config_editor_view, &self.config_editor_path)
                            {
                                let app_dir = self.layout.app_dir(&view.manifest.id);
                                if let Err(e) =
                                    validate_nginx_config_text(&app_dir, conf_path, &formatted)
                                {
                                    semantic_err = Some(e.to_string());
                                }
                            }
                        }
                        if let Some(err) = semantic_err {
                            self.config_editor_error = Some(err.clone());
                            self.set_toast(format!("Nginx 校验未通过：{err}"));
                        } else {
                            let changed = formatted != self.config_editor_text;
                            self.config_editor_text = formatted;
                            self.config_editor_error = None;
                            if changed {
                                self.config_editor_dirty = true;
                            }
                            self.set_toast(format!("已完成 {fmt_label} 语法校验与格式化"));
                        }
                    }
                    Err(err) => {
                        self.config_editor_error = Some(err.clone());
                        self.set_toast(format!("{fmt_label} 语法错误：{err}"));
                    }
                }
            }
            if do_change_pwd {
                if let Some(view) = self.config_editor_view.clone() {
                    self.spawn_change_mysql_pwd(view, ctx.clone());
                }
            }
            if do_save_and_restart && self.save_config_editor() {
                if let Some(view) = self.config_editor_view.clone() {
                    self.config_editor_view = None;
                    self.config_editor_path = None;
                    self.trigger_restart_service(view, ctx.clone());
                }
            } else if do_save {
                let _ = self.save_config_editor();
            }
            if do_reset {
                self.config_editor_error = None;
                self.reset_config_editor_default();
            }
            if do_external {
                if let Some(ref path) = self.config_editor_path {
                    open_file_default(path);
                }
            }
            if do_close {
                self.config_editor_view = None;
                self.config_editor_path = None;
                self.config_editor_error = None;
            }
        }

        // 暖色融合实时日志弹窗（与配置编辑器视觉完全统一，零割裂、不裁切折行）
        if let Some(log_id_owned) = self.log_target_id.clone() {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("log_backdrop"),
            ))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

            let modal_w = (screen.width() - 120.0).clamp(620.0, 880.0);
            let modal_h = (screen.height() - 120.0).clamp(420.0, 590.0);
            let log_file_path = self.layout.log_file(&log_id_owned);

            let mut do_close = false;
            let mut do_clear = false;
            let mut do_copy = false;
            let mut do_open_file = false;

            egui::Area::new(egui::Id::new("log_viewer_modal"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .shadow(Shadow {
                            offset: [0, 8],
                            blur: 28,
                            spread: 0,
                            color: Color32::from_black_alpha(55),
                        })
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(20))
                        .show(ui, |ui| {
                            ui.set_width(modal_w);
                            ui.set_height(modal_h);

                            // 顶栏：标题 + 日志路径 + 操作按钮
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&self.log_target_title)
                                            .strong()
                                            .size(16.0)
                                            .color(p.text_primary),
                                    );
                                    ui.label(
                                        RichText::new(log_file_path.to_string_lossy())
                                            .monospace()
                                            .size(11.5)
                                            .color(p.text_muted),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, p, "关闭", 60.0, true).clicked() {
                                        do_close = true;
                                    }
                                    if soft_button(
                                        ui,
                                        "清空",
                                        p.danger_soft,
                                        p.danger_text,
                                        60.0,
                                        true,
                                    )
                                    .clicked()
                                    {
                                        do_clear = true;
                                    }
                                    if secondary_button(ui, p, "复制", 60.0, true).clicked() {
                                        do_copy = true;
                                    }
                                    if secondary_button(ui, p, "打开文件", 78.0, true).clicked() {
                                        do_open_file = true;
                                    }
                                });
                            });

                            ui.add_space(12.0);

                            // 筛选工具条：级别分段 + 自动滚底 + 关键词搜索
                            ui.horizontal(|ui| {
                                Frame::new()
                                    .fill(p.bg_sidebar)
                                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                    .corner_radius(CornerRadius::same(9))
                                    .inner_margin(Margin::same(3))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 2.0;
                                            for (lvl, lbl) in [
                                                (LogLevelFilter::All, "全部"),
                                                (LogLevelFilter::Errors, "仅错误"),
                                                (LogLevelFilter::Warnings, "警告"),
                                            ] {
                                                let sel = self.log_level_filter == lvl;
                                                let (fill, stroke, shadow, fg) = if sel {
                                                    (
                                                        p.bg_card,
                                                        Stroke::new(1.0_f32, p.border_subtle),
                                                        p.card_shadow,
                                                        p.accent_text,
                                                    )
                                                } else {
                                                    (
                                                        Color32::TRANSPARENT,
                                                        Stroke::NONE,
                                                        Shadow::NONE,
                                                        p.text_secondary,
                                                    )
                                                };
                                                let r = Frame::new()
                                                    .fill(fill)
                                                    .stroke(stroke)
                                                    .shadow(shadow)
                                                    .corner_radius(CornerRadius::same(6))
                                                    .inner_margin(Margin::symmetric(12, 4))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(lbl)
                                                                .size(12.0)
                                                                .color(fg),
                                                        );
                                                    })
                                                    .response
                                                    .interact(egui::Sense::click())
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                                if r.clicked() {
                                                    self.log_level_filter = lvl;
                                                }
                                            }
                                        });
                                    });

                                ui.add_space(8.0);
                                let (sc_lbl, sc_bg, sc_fg) = if self.log_auto_scroll {
                                    ("自动滚底: 开", p.success_soft, p.success_text)
                                } else {
                                    ("自动滚底: 关", p.tag_bg, p.tag_text)
                                };
                                if soft_button(ui, sc_lbl, sc_bg, sc_fg, 96.0, true).clicked() {
                                    self.log_auto_scroll = !self.log_auto_scroll;
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    styled_search_input(
                                        ui,
                                        p,
                                        &mut self.log_filter,
                                        "过滤日志关键词...",
                                        210.0,
                                    );
                                });
                            });

                            ui.add_space(10.0);

                            // 日志展示区（柔和暖纸色 / 深色暖炭，自适应折行绝不裁切文字）
                            let kw = self.log_filter.trim().to_lowercase();
                            let filtered: Vec<&String> = self
                                .log_lines
                                .iter()
                                .filter(|l| {
                                    let pass_lvl = match self.log_level_filter {
                                        LogLevelFilter::All => true,
                                        LogLevelFilter::Errors => is_error_log(l),
                                        LogLevelFilter::Warnings => is_warn_log(l),
                                    };
                                    pass_lvl
                                        && (kw.is_empty() || l.to_lowercase().contains(&kw))
                                })
                                .collect();
                            let display_start = filtered.len().saturating_sub(300);
                            let visible = &filtered[display_start..];

                            Frame::new()
                                .fill(p.bg_card_alt)
                                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::same(12))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ScrollArea::vertical()
                                        .id_salt("log_modal_scroll")
                                        .max_height(modal_h - 148.0)
                                        .stick_to_bottom(self.log_auto_scroll)
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            if visible.is_empty() {
                                                ui.label(
                                                    RichText::new(if self.log_lines.is_empty() {
                                                        "暂无日志输出，启动服务后将在此实时显示..."
                                                    } else {
                                                        "没有匹配当前筛选条件的日志"
                                                    })
                                                    .color(p.text_muted)
                                                    .size(12.5),
                                                );
                                            } else {
                                                for line in visible {
                                                    let color = log_line_color(line, p);
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(*line)
                                                                .monospace()
                                                                .size(12.0)
                                                                .color(color),
                                                        )
                                                        .wrap(),
                                                    );
                                                    ui.add_space(2.0);
                                                }
                                            }
                                        });
                                });
                        });
                });

            if do_clear {
                self.process_mgr.clear_logs(&log_id_owned);
                let _ = std::fs::write(&log_file_path, "");
                self.log_lines.clear();
            }
            if do_copy {
                ctx.copy_text(self.log_lines.join("\n"));
                self.set_toast("日志已复制到剪贴板");
            }
            if do_open_file {
                if log_file_path.exists() {
                    open_file_default(&log_file_path);
                } else {
                    self.set_toast("日志文件尚未生成");
                }
            }
            if do_close {
                self.log_target_id = None;
            }
        }

        // 新建 / 编辑自定义场景组弹窗
        if self.preset_modal_open {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("preset_backdrop"),
            ))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

            let mut do_save = false;
            let mut do_delete = false;
            let mut do_close = false;
            let is_edit = self.preset_editing_idx.is_some();

            egui::Area::new(egui::Id::new("preset_editor_modal"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .shadow(Shadow {
                            offset: [0, 8],
                            blur: 28,
                            spread: 0,
                            color: Color32::from_black_alpha(55),
                        })
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(22))
                        .show(ui, |ui| {
                            ui.set_width(540.0);

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(if is_edit {
                                        "编辑快捷场景组"
                                    } else {
                                        "新建快捷场景组"
                                    })
                                    .strong()
                                    .size(16.0)
                                    .color(p.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, p, "关闭", 60.0, true).clicked() {
                                        do_close = true;
                                    }
                                });
                            });

                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("场景组名称")
                                    .strong()
                                    .size(12.5)
                                    .color(p.text_primary),
                            );
                            ui.add_space(6.0);
                            styled_search_input(
                                ui,
                                p,
                                &mut self.preset_name_input,
                                "例如：公司后台开发组 / 个人全栈环境",
                                496.0,
                            );

                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("选择包含的服务（点击切换勾选，● 表示已安装）")
                                    .strong()
                                    .size(12.5)
                                    .color(p.text_primary),
                            );
                            ui.add_space(8.0);

                            Frame::new()
                                .fill(p.bg_card_alt)
                                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                .corner_radius(CornerRadius::same(10))
                                .inner_margin(Margin::same(12))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        for svc in self
                                            .software_views
                                            .iter()
                                            .filter(|v| v.manifest.is_service)
                                        {
                                            let selected =
                                                self.preset_selected_ids.contains(&svc.manifest.id);
                                            let prefix = if svc.installed { "● " } else { "" };
                                            let lbl = format!(
                                                "{prefix}{} {}",
                                                svc.manifest.name, svc.manifest.version
                                            );
                                            let (bg, fg, stroke) = if selected {
                                                (
                                                    p.accent_soft,
                                                    p.accent_text,
                                                    Stroke::new(1.0_f32, p.accent),
                                                )
                                            } else {
                                                (p.bg_card, p.text_secondary, Stroke::new(1.0_f32, p.border_subtle))
                                            };
                                            if chip_button(
                                                ui, &lbl, 12.0, bg, fg, stroke, 8, 88.0, 30.0,
                                            )
                                            .clicked()
                                            {
                                                if selected {
                                                    self.preset_selected_ids
                                                        .remove(&svc.manifest.id);
                                                } else {
                                                    self.preset_selected_ids
                                                        .insert(svc.manifest.id.clone());
                                                }
                                            }
                                        }
                                    });
                                });

                            ui.add_space(18.0);
                            ui.horizontal(|ui| {
                                if primary_button(ui, p, "保存场景组", 104.0, true).clicked() {
                                    do_save = true;
                                }
                                if is_edit
                                    && soft_button(
                                        ui,
                                        "删除此场景组",
                                        p.danger_soft,
                                        p.danger_text,
                                        104.0,
                                        true,
                                    )
                                    .clicked()
                                {
                                    do_delete = true;
                                }
                            });
                        });
                });

            if do_save {
                let name = self.preset_name_input.trim().to_string();
                if name.is_empty() {
                    self.set_toast("请输入场景组名称");
                } else if self.preset_selected_ids.is_empty() {
                    self.set_toast("请至少选择一个服务");
                } else {
                    // 按软件目录顺序保存勾选的服务 ID
                    let ordered_ids: Vec<String> = self
                        .software_views
                        .iter()
                        .filter(|v| self.preset_selected_ids.contains(&v.manifest.id))
                        .map(|v| v.manifest.id.clone())
                        .collect();
                    let item = ServicePreset {
                        name: name.clone(),
                        service_ids: ordered_ids,
                    };
                    if let Some(idx) = self.preset_editing_idx {
                        if idx < self.presets.len() {
                            self.presets[idx] = item;
                        }
                    } else {
                        self.presets.push(item);
                    }
                    let _ = save_presets(self.layout.root_path(), &self.presets);
                    self.set_toast(format!("场景组「{name}」已保存"));
                    self.preset_modal_open = false;
                }
            }
            if do_delete {
                if let Some(idx) = self.preset_editing_idx {
                    if idx < self.presets.len() {
                        let removed = self.presets.remove(idx);
                        let _ = save_presets(self.layout.root_path(), &self.presets);
                        self.set_toast(format!("已删除场景组「{}」", removed.name));
                    }
                }
                self.preset_modal_open = false;
            }
            if do_close {
                self.preset_modal_open = false;
            }
        }

        // Nginx 多站点虚拟主机新建/编辑弹窗
        if self.site_modal_open {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("site_backdrop"),
            ))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(95));

            let mut do_save = false;
            let mut do_close = false;
            let mut do_gen_cert = false;
            let is_edit = self.site_editing_idx.is_some();

            egui::Area::new(egui::Id::new("site_editor_modal"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .shadow(Shadow {
                            offset: [0, 8],
                            blur: 28,
                            spread: 0,
                            color: Color32::from_black_alpha(55),
                        })
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(22))
                        .show(ui, |ui| {
                            ui.set_width(600.0);
                            ui.set_max_height((screen.height() - 120.0).clamp(420.0, 580.0));

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(if is_edit {
                                        "编辑本地站点"
                                    } else {
                                        "新建本地站点"
                                    })
                                    .strong()
                                    .size(16.0)
                                    .color(p.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, p, "取消", 60.0, true).clicked() {
                                        do_close = true;
                                    }
                                    if primary_button(ui, p, "保存并生效", 98.0, true).clicked() {
                                        do_save = true;
                                    }
                                });
                            });

                            ui.add_space(12.0);
                            ScrollArea::vertical()
                                .id_salt("site_modal_scroll")
                                .max_height(430.0)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    // 基础信息
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new("本地域名 (server_name)")
                                                    .strong()
                                                    .size(12.0)
                                                    .color(p.text_primary),
                                            );
                                            ui.add_space(4.0);
                                            styled_search_input(
                                                ui,
                                                p,
                                                &mut self.site_domain_input,
                                                "例如 api.local / admin.local",
                                                360.0,
                                            );
                                        });
                                        ui.add_space(12.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new("HTTP 端口")
                                                    .strong()
                                                    .size(12.0)
                                                    .color(p.text_primary),
                                            );
                                            ui.add_space(4.0);
                                            styled_search_input(
                                                ui,
                                                p,
                                                &mut self.site_port_input,
                                                "80",
                                                120.0,
                                            );
                                        });
                                    });

                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new("静态网站根目录（留空自动生成默认演示页目录）")
                                            .strong()
                                            .size(12.0)
                                            .color(p.text_primary),
                                    );
                                    ui.add_space(4.0);
                                    styled_search_input(
                                        ui,
                                        p,
                                        &mut self.site_root_input,
                                        "例如 D:/projects/my-web/dist（可留空）",
                                        556.0,
                                    );

                                    ui.add_space(12.0);
                                    // 本地 HTTPS
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("本地 HTTPS (SSL)")
                                                .strong()
                                                .size(12.5)
                                                .color(p.accent_text),
                                        );
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            let (lbl, bg, fg) = if self.site_ssl_enabled {
                                                ("已启用", p.accent, Color32::WHITE)
                                            } else {
                                                ("未启用", p.tag_bg, p.tag_text)
                                            };
                                            if soft_button(ui, lbl, bg, fg, 66.0, true).clicked() {
                                                self.site_ssl_enabled = !self.site_ssl_enabled;
                                            }
                                        });
                                    });
                                    if self.site_ssl_enabled {
                                        ui.add_space(6.0);
                                        Frame::new()
                                            .fill(p.bg_card_alt)
                                            .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                            .corner_radius(CornerRadius::same(10))
                                            .inner_margin(Margin::same(10))
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        RichText::new("SSL 端口")
                                                            .size(11.5)
                                                            .color(p.text_muted),
                                                    );
                                                    styled_search_input(
                                                        ui,
                                                        p,
                                                        &mut self.site_ssl_port_input,
                                                        "443",
                                                        74.0,
                                                    );
                                                    ui.with_layout(
                                                        Layout::right_to_left(Align::Center),
                                                        |ui| {
                                                            if soft_button(
                                                                ui,
                                                                "一键生成本地证书",
                                                                p.accent_soft,
                                                                p.accent_text,
                                                                124.0,
                                                                true,
                                                            )
                                                            .clicked()
                                                            {
                                                                do_gen_cert = true;
                                                            }
                                                        },
                                                    );
                                                });
                                                ui.add_space(6.0);
                                                styled_search_input(
                                                    ui,
                                                    p,
                                                    &mut self.site_ssl_cert_input,
                                                    "证书路径 (.crt / .pem)",
                                                    530.0,
                                                );
                                                ui.add_space(4.0);
                                                styled_search_input(
                                                    ui,
                                                    p,
                                                    &mut self.site_ssl_key_input,
                                                    "私钥路径 (.key)",
                                                    530.0,
                                                );
                                            });
                                    }

                                    ui.add_space(12.0);
                                    // 多条反向代理规则
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("反向代理规则（支持 CORS & WebSocket）")
                                                .strong()
                                                .size(12.5)
                                                .color(p.accent_text),
                                        );
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            if soft_button(
                                                ui,
                                                "+ 新增规则",
                                                p.accent_soft,
                                                p.accent_text,
                                                86.0,
                                                true,
                                            )
                                            .clicked()
                                            {
                                                let n = self.site_proxy_rules.len() + 1;
                                                self.site_proxy_rules.push(SiteProxyRule {
                                                    prefix: if n == 1 {
                                                        "/api/".into()
                                                    } else {
                                                        format!("/api{n}/")
                                                    },
                                                    target: "http://127.0.0.1:8080".into(),
                                                    cors: true,
                                                    websocket: false,
                                                });
                                            }
                                        });
                                    });
                                    ui.add_space(6.0);
                                    let mut rm_rule: Option<usize> = None;
                                    for (idx, r) in self.site_proxy_rules.iter_mut().enumerate() {
                                        Frame::new()
                                            .fill(p.bg_card_alt)
                                            .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                            .corner_radius(CornerRadius::same(8))
                                            .inner_margin(Margin::same(8))
                                            .show(ui, |ui| {
                                                ui.set_width(ui.available_width());
                                                ui.horizontal(|ui| {
                                                    styled_search_input(
                                                        ui,
                                                        p,
                                                        &mut r.prefix,
                                                        "/api/",
                                                        100.0,
                                                    );
                                                    styled_search_input(
                                                        ui,
                                                        p,
                                                        &mut r.target,
                                                        "http://127.0.0.1:8080",
                                                        200.0,
                                                    );
                                                    let (cbg, cfg) = if r.cors {
                                                        (p.accent, Color32::WHITE)
                                                    } else {
                                                        (p.tag_bg, p.tag_text)
                                                    };
                                                    if soft_button(ui, "CORS", cbg, cfg, 50.0, true)
                                                        .clicked()
                                                    {
                                                        r.cors = !r.cors;
                                                    }
                                                    let (wbg, wfg) = if r.websocket {
                                                        (p.accent, Color32::WHITE)
                                                    } else {
                                                        (p.tag_bg, p.tag_text)
                                                    };
                                                    if soft_button(ui, "WS", wbg, wfg, 40.0, true)
                                                        .clicked()
                                                    {
                                                        r.websocket = !r.websocket;
                                                    }
                                                    if soft_button(
                                                        ui,
                                                        "删",
                                                        p.danger_soft,
                                                        p.danger_text,
                                                        34.0,
                                                        true,
                                                    )
                                                    .clicked()
                                                    {
                                                        rm_rule = Some(idx);
                                                    }
                                                });
                                            });
                                        ui.add_space(4.0);
                                    }
                                    if let Some(i) = rm_rule {
                                        self.site_proxy_rules.remove(i);
                                    }

                                    ui.add_space(10.0);
                                    ui.checkbox(
                                        &mut self.site_sync_hosts,
                                        RichText::new("保存时自动将 127.0.0.1 <域名> 写入 Windows Hosts")
                                            .size(12.0)
                                            .color(p.text_secondary),
                                    );
                                });
                        });
                });

            if do_gen_cert {
                let dom = if self.site_domain_input.trim().is_empty() {
                    "localhost"
                } else {
                    self.site_domain_input.trim()
                };
                let ssl_dir = self.layout.conf_dir("nginx-1.26").join("ssl");
                match generate_local_ssl_cert(&ssl_dir, dom) {
                    Ok((cp, kp)) => {
                        self.site_ssl_cert_input = cp.to_string_lossy().replace('\\', "/");
                        self.site_ssl_key_input = kp.to_string_lossy().replace('\\', "/");
                        self.set_toast(format!("已为 {dom} 生成本地 SSL 证书"));
                    }
                    Err(e) => self.set_toast(format!("生成证书失败: {e}")),
                }
            }
            if do_save {
                let domain = self.site_domain_input.trim().to_string();
                let port = self.site_port_input.trim().parse::<u16>().unwrap_or(0);
                let ssl_port = self.site_ssl_port_input.trim().parse::<u16>().unwrap_or(443);
                if domain.is_empty() {
                    self.set_toast("请输入站点域名（例如 api.local）");
                } else if port == 0 {
                    self.set_toast("HTTP 端口无效，请输入 1 ~ 65535");
                } else {
                    let enabled = self
                        .site_editing_idx
                        .and_then(|i| self.sites.get(i))
                        .map(|s| s.enabled)
                        .unwrap_or(true);
                    let id = self
                        .site_editing_idx
                        .and_then(|i| self.sites.get(i))
                        .map(|s| s.id.clone())
                        .unwrap_or_else(|| format!("site_{}", chrono::Local::now().timestamp_millis()));
                    let cfg = SiteConfig {
                        id,
                        domain: domain.clone(),
                        port,
                        root_dir: self.site_root_input.trim().to_string(),
                        enabled,
                        ssl_enabled: self.site_ssl_enabled,
                        ssl_port,
                        ssl_cert: self.site_ssl_cert_input.trim().to_string(),
                        ssl_key: self.site_ssl_key_input.trim().to_string(),
                        proxy_rules: self.site_proxy_rules.clone(),
                    };
                    if let Some(idx) = self.site_editing_idx {
                        if idx < self.sites.len() {
                            self.sites[idx] = cfg;
                        }
                    } else {
                        self.sites.push(cfg);
                    }
                    let sync_dom = if self.site_sync_hosts {
                        Some(domain)
                    } else {
                        None
                    };
                    self.site_modal_open = false;
                    self.sync_sites_and_reload(sync_dom, ctx.clone());
                }
            }
            if do_close {
                self.site_modal_open = false;
            }
        }

        // 数据库一键备份与恢复弹窗
        if let Some(db_svc) = self.backup_modal_svc.clone() {
            let screen = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("backup_backdrop"),
            ))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(95));

            let modal_w = (screen.width() - 160.0).clamp(520.0, 680.0);
            let modal_h = (screen.height() - 160.0).clamp(340.0, 480.0);
            let backup_busy = self.shared.lock().map(|s| s.backup_busy).unwrap_or(false);
            let backup_dir = self
                .layout
                .root_path()
                .join("backups")
                .join(&db_svc.manifest.id);

            let mut do_create_backup = false;
            let mut do_open_dir = false;
            let mut do_restore_path: Option<PathBuf> = None;
            let mut do_delete_path: Option<PathBuf> = None;
            let mut do_close = false;

            egui::Area::new(egui::Id::new("backup_modal"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .shadow(Shadow {
                            offset: [0, 12],
                            blur: 32,
                            spread: 0,
                            color: Color32::from_black_alpha(55),
                        })
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(Margin::same(20))
                        .show(ui, |ui| {
                            ui.set_min_size(Vec2::new(modal_w, modal_h));
                            ui.set_max_size(Vec2::new(modal_w, modal_h));

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "数据库快照 — {} {}",
                                        db_svc.manifest.name, db_svc.manifest.version
                                    ))
                                    .strong()
                                    .size(16.0)
                                    .color(p.text_primary),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if secondary_button(ui, p, "关闭", 60.0, !backup_busy).clicked()
                                    {
                                        do_close = true;
                                    }
                                    if secondary_button(ui, p, "打开目录", 78.0, true).clicked() {
                                        do_open_dir = true;
                                    }
                                    if primary_button(
                                        ui,
                                        p,
                                        if backup_busy {
                                            "处理中..."
                                        } else {
                                            "+ 创建 SQL 快照"
                                        },
                                        124.0,
                                        !backup_busy,
                                    )
                                    .clicked()
                                    {
                                        do_create_backup = true;
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!(
                                    "快照存储于 {}，支持一键全量导出与回滚恢复。",
                                    backup_dir.display()
                                ))
                                .size(11.5)
                                .color(p.text_muted),
                            );
                            ui.add_space(12.0);

                            ScrollArea::vertical()
                                .id_salt("backup_list_scroll")
                                .max_height(modal_h - 90.0)
                                .show(ui, |ui| {
                                    if self.backup_files.is_empty() {
                                        card_frame(ui, p, |ui| {
                                            ui.vertical_centered(|ui| {
                                                ui.add_space(28.0);
                                                ui.label(
                                                    RichText::new("暂无 SQL 备份快照")
                                                        .size(13.5)
                                                        .color(p.text_secondary),
                                                );
                                                ui.add_space(4.0);
                                                ui.label(
                                                    RichText::new("点击右上角「+ 创建 SQL 快照」即可导出当前实例全量数据。")
                                                        .size(11.5)
                                                        .color(p.text_muted),
                                                );
                                                ui.add_space(28.0);
                                            });
                                        });
                                    } else {
                                        for item in &self.backup_files {
                                            Frame::new()
                                                .fill(p.bg_card_alt)
                                                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                                .corner_radius(CornerRadius::same(8))
                                                .inner_margin(Margin::symmetric(12, 10))
                                                .show(ui, |ui| {
                                                    ui.set_width(ui.available_width());
                                                    ui.horizontal(|ui| {
                                                        ui.vertical(|ui| {
                                                            ui.label(
                                                                RichText::new(&item.file_name)
                                                                    .strong()
                                                                    .size(13.0)
                                                                    .color(p.text_primary),
                                                            );
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "{}  ·  {}",
                                                                    item.modified_label,
                                                                    format_bytes(item.size_bytes)
                                                                ))
                                                                .size(11.5)
                                                                .color(p.text_muted),
                                                            );
                                                        });
                                                        ui.with_layout(
                                                            Layout::right_to_left(Align::Center),
                                                            |ui| {
                                                                if soft_button(
                                                                    ui,
                                                                    "删除",
                                                                    p.danger_soft,
                                                                    p.danger_text,
                                                                    52.0,
                                                                    !backup_busy,
                                                                )
                                                                .clicked()
                                                                {
                                                                    do_delete_path =
                                                                        Some(item.path.clone());
                                                                }
                                                                if soft_button(
                                                                    ui,
                                                                    "恢复",
                                                                    p.accent_soft,
                                                                    p.accent_text,
                                                                    56.0,
                                                                    !backup_busy,
                                                                )
                                                                .clicked()
                                                                {
                                                                    do_restore_path =
                                                                        Some(item.path.clone());
                                                                }
                                                            },
                                                        );
                                                    });
                                                });
                                            ui.add_space(6.0);
                                        }
                                    }
                                });
                        });
                });

            if do_open_dir {
                let _ = std::fs::create_dir_all(&backup_dir);
                open_in_explorer(&backup_dir);
            }
            if let Some(del_path) = do_delete_path {
                let _ = std::fs::remove_file(&del_path);
                self.backup_files = list_db_backups(&backup_dir);
                self.set_toast("已删除指定备份文件");
            }
            if do_create_backup {
                let kind = db_svc
                    .manifest
                    .config_kind
                    .clone()
                    .unwrap_or_default();
                let app_dir = self.layout.app_dir(&db_svc.manifest.id);
                let port = self
                    .effective_port(&db_svc)
                    .or(db_svc.manifest.default_port)
                    .unwrap_or(3306);
                let pwd = self
                    .db_passwords
                    .get(&db_svc.manifest.id)
                    .cloned()
                    .unwrap_or_default();
                let label = db_svc.manifest.id.clone();
                let bdir = backup_dir.clone();
                let shared = self.shared.clone();
                let layout = self.layout.clone();
                let c = ctx.clone();
                {
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.backup_busy = true;
                }
                self.set_toast(format!("正在导出 {} 全量 SQL 快照...", db_svc.manifest.name));
                self.rt.spawn_blocking(move || {
                    let res = backup_database(&kind, &app_dir, port, &pwd, &bdir, &label);
                    let list = list_db_backups(&bdir);
                    let msg = match res {
                        Ok(path) => {
                            write_app_log(
                                layout.root_path(),
                                "INFO",
                                &format!("数据库备份完成: {}", path.display()),
                            );
                            format!(
                                "备份成功: {}",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            )
                        }
                        Err(e) => {
                            write_app_log(
                                layout.root_path(),
                                "ERROR",
                                &format!("数据库备份失败: {e}"),
                            );
                            format!("备份失败: {e}")
                        }
                    };
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.backup_busy = false;
                    s.latest_backups = Some(list);
                    s.toast = Some((msg, Instant::now()));
                    c.request_repaint();
                });
            }
            if let Some(restore_path) = do_restore_path {
                let kind = db_svc
                    .manifest
                    .config_kind
                    .clone()
                    .unwrap_or_default();
                let app_dir = self.layout.app_dir(&db_svc.manifest.id);
                let port = self
                    .effective_port(&db_svc)
                    .or(db_svc.manifest.default_port)
                    .unwrap_or(3306);
                let pwd = self
                    .db_passwords
                    .get(&db_svc.manifest.id)
                    .cloned()
                    .unwrap_or_default();
                let shared = self.shared.clone();
                let layout = self.layout.clone();
                let c = ctx.clone();
                {
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.backup_busy = true;
                }
                self.set_toast("正在恢复数据库快照，请稍候...");
                self.rt.spawn_blocking(move || {
                    let res = restore_database(&kind, &app_dir, port, &pwd, &restore_path);
                    let msg = match res {
                        Ok(()) => {
                            write_app_log(
                                layout.root_path(),
                                "INFO",
                                &format!("数据库恢复完成: {}", restore_path.display()),
                            );
                            "数据库已从 SQL 快照成功恢复".to_string()
                        }
                        Err(e) => {
                            write_app_log(
                                layout.root_path(),
                                "ERROR",
                                &format!("数据库恢复失败: {e}"),
                            );
                            format!("恢复失败: {e}")
                        }
                    };
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.backup_busy = false;
                    s.toast = Some((msg, Instant::now()));
                    c.request_repaint();
                });
            }
            if do_close {
                self.backup_modal_svc = None;
            }
        }

        // 右下角悬浮 Toast（绝不挤压页面标题）
        if let Some((msg, _)) = &toast_snapshot {
            egui::Area::new(egui::Id::new("floating_toast"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, Vec2::new(-24.0, -24.0))
                .show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    Frame::new()
                        .fill(p.bg_card)
                        .stroke(Stroke::new(1.0_f32, p.accent))
                        .shadow(Shadow {
                            offset: [0, 4],
                            blur: 14,
                            spread: 0,
                            color: Color32::from_black_alpha(32),
                        })
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(Margin {
                            left: 16,
                            right: 16,
                            top: 7,
                            bottom: 11,
                        })
                        .show(ui, |ui| {
                            ui.label(RichText::new(msg).size(12.5).color(p.text_primary));
                        });
                });
        }

        // 非最大化时绘制 1px 圆角外框线，与 Windows 11 原生 DWM 圆角严丝合缝
        if !is_maximized {
            let screen_rect = ctx.screen_rect();
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("window_border"),
            ))
            .rect_stroke(
                screen_rect,
                CornerRadius::same(8),
                Stroke::new(1.0_f32, p.border_strong),
                egui::StrokeKind::Inside,
            );
        }

        // 托盘右键悬浮快捷面板（独立无边框置顶视口，支持搜索、一键启停服务与秒切运行时）
        if self.tray_popover_open {
            let mut pop_actions: Vec<UiAction> = Vec::new();
            let mut close_popover = false;
            let mut open_main = false;
            let mut do_quit = false;
            let pop_w = 340.0_f32;
            let pop_h = 440.0_f32;

            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("moka_tray_popover"),
                egui::ViewportBuilder::default()
                    .with_title("Moka 托盘面板")
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_taskbar(false)
                    .with_resizable(false)
                    .with_inner_size([pop_w, pop_h])
                    .with_position([self.tray_popover_pos.x, self.tray_popover_pos.y]),
                |vctx, _class| {
                    // 首帧启用 Windows 11 原生圆角并主动获取焦点，防止任务栏抢占焦点导致闪烁或点不了
                    if self.tray_popover_just_opened {
                        apply_win11_rounded_corners();
                        vctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        self.tray_popover_just_opened = false;
                    }

                    // 只有面板曾经获得过焦点之后再失焦（点击面板外部）才自动收起
                    let focused = vctx.input(|i| i.viewport().focused);
                    if focused == Some(true) {
                        self.tray_popover_ever_focused = true;
                    } else if self.tray_popover_ever_focused && focused == Some(false) {
                        close_popover = true;
                    }
                    if vctx.input(|i| i.key_pressed(egui::Key::Escape) || i.viewport().close_requested()) {
                        close_popover = true;
                    }

                    egui::CentralPanel::default()
                        .frame(
                            Frame::new()
                                .fill(p.bg_card)
                                .stroke(Stroke::new(1.0_f32, p.border_strong))
                                .corner_radius(CornerRadius::same(8))
                                .inner_margin(Margin::same(14)),
                        )
                        .show(vctx, |ui| {
                            // 顶栏（严格固定 28px 行高）
                            ui.allocate_ui_with_layout(
                                Vec2::new(ui.available_width(), 28.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let (lr, _) = ui.allocate_exact_size(
                                        Vec2::splat(22.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(lr, CornerRadius::same(6), p.accent);
                                    ui.painter().text(
                                        Pos2::new(lr.center().x, lr.center().y - 0.5),
                                        egui::Align2::CENTER_CENTER,
                                        "M",
                                        egui::FontId::proportional(11.5),
                                        Color32::WHITE,
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Moka 快捷面板")
                                            .strong()
                                            .size(13.5)
                                            .color(p.text_primary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if soft_button(ui, "收起", p.tag_bg, p.tag_text, 48.0, true)
                                            .clicked()
                                        {
                                            close_popover = true;
                                        }
                                        if soft_button(
                                            ui,
                                            "主界面",
                                            p.accent_soft,
                                            p.accent_text,
                                            62.0,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            open_main = true;
                                            close_popover = true;
                                        }
                                    });
                                },
                            );

                            ui.add_space(8.0);
                            let sw = (ui.available_width() - 22.0).max(200.0);
                            styled_search_input(
                                ui,
                                p,
                                &mut self.tray_popover_search,
                                "搜索已安装服务或运行时...",
                                sw,
                            );
                            ui.add_space(10.0);

                            let kw = self.tray_popover_search.trim().to_lowercase();
                            let installed_svcs: Vec<SoftwareStatusView> = self
                                .software_views
                                .iter()
                                .filter(|v| {
                                    v.manifest.is_service
                                        && v.installed
                                        && (kw.is_empty()
                                            || v.manifest.name.to_lowercase().contains(&kw)
                                            || v.manifest.id.to_lowercase().contains(&kw))
                                })
                                .cloned()
                                .collect();
                            let installed_rts: Vec<SoftwareStatusView> = self
                                .software_views
                                .iter()
                                .filter(|v| {
                                    v.manifest.env_link_name.is_some()
                                        && v.installed
                                        && (kw.is_empty()
                                            || v.manifest.name.to_lowercase().contains(&kw)
                                            || v.manifest.id.to_lowercase().contains(&kw))
                                })
                                .cloned()
                                .collect();

                            let list_h = (ui.available_height() - 42.0).max(120.0);
                            ScrollArea::vertical()
                                .id_salt("tray_pop_scroll")
                                .max_height(list_h)
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("服务启停")
                                            .strong()
                                            .size(11.5)
                                            .color(p.text_muted),
                                    );
                                    ui.add_space(4.0);
                                    if installed_svcs.is_empty() {
                                        ui.label(
                                            RichText::new("暂无匹配服务")
                                                .size(12.0)
                                                .color(p.text_muted),
                                        );
                                    } else {
                                        for svc in &installed_svcs {
                                            let is_busy = busy_snapshot.contains(&svc.manifest.id);
                                            Frame::new()
                                                .fill(p.bg_card_alt)
                                                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                                .corner_radius(CornerRadius::same(8))
                                                .inner_margin(Margin::symmetric(10, 7))
                                                .show(ui, |ui| {
                                                    ui.set_width(ui.available_width());
                                                    ui.horizontal(|ui| {
                                                        let dot_c = if svc.running {
                                                            p.success
                                                        } else {
                                                            p.text_muted
                                                        };
                                                        ui.label(
                                                            RichText::new("●")
                                                                .size(9.0)
                                                                .color(dot_c),
                                                        );
                                                        ui.vertical(|ui| {
                                                            ui.spacing_mut().item_spacing.y = 0.0;
                                                            ui.label(
                                                                RichText::new(format!(
                                                                    "{} {}",
                                                                    svc.manifest.name,
                                                                    svc.manifest.version
                                                                ))
                                                                .strong()
                                                                .size(12.0)
                                                                .color(p.text_primary),
                                                            );
                                                            if let Some(pt) = svc.port {
                                                                ui.label(
                                                                    RichText::new(format!(
                                                                        "端口 {pt}"
                                                                    ))
                                                                    .size(10.5)
                                                                    .color(p.text_muted),
                                                                );
                                                            }
                                                        });
                                                        ui.with_layout(
                                                            Layout::right_to_left(Align::Center),
                                                            |ui| {
                                                                let (lbl, bg, fg) = if svc.running {
                                                                    (
                                                                        "停止",
                                                                        p.danger_soft,
                                                                        p.danger_text,
                                                                    )
                                                                } else {
                                                                    (
                                                                        "启动",
                                                                        p.success_soft,
                                                                        p.success_text,
                                                                    )
                                                                };
                                                                if soft_button(
                                                                    ui, lbl, bg, fg, 50.0, !is_busy,
                                                                )
                                                                .clicked()
                                                                {
                                                                    pop_actions.push(
                                                                        UiAction::ToggleService(
                                                                            svc.clone(),
                                                                        ),
                                                                    );
                                                                }
                                                            },
                                                        );
                                                    });
                                                });
                                            ui.add_space(5.0);
                                        }
                                    }

                                    ui.add_space(8.0);
                                    ui.label(
                                        RichText::new("语言运行时秒切")
                                            .strong()
                                            .size(11.5)
                                            .color(p.text_muted),
                                    );
                                    ui.add_space(5.0);
                                    if installed_rts.is_empty() {
                                        ui.label(
                                            RichText::new("暂无匹配运行时")
                                                .size(12.0)
                                                .color(p.text_muted),
                                        );
                                    } else {
                                        ui.horizontal_wrapped(|ui| {
                                            for rt in &installed_rts {
                                                let (bg, fg) = if rt.active_env {
                                                    (p.accent, Color32::WHITE)
                                                } else {
                                                    (p.tag_bg, p.tag_text)
                                                };
                                                if soft_button(
                                                    ui,
                                                    &rt.manifest.id.to_uppercase(),
                                                    bg,
                                                    fg,
                                                    62.0,
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    pop_actions.push(UiAction::ActivateEnv(
                                                        rt.manifest.clone(),
                                                    ));
                                                }
                                            }
                                        });
                                    }
                                });

                            // 底部操作栏
                            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(ui.available_width(), 30.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        if soft_button(
                                            ui,
                                            "打开终端",
                                            p.tag_bg,
                                            p.tag_text,
                                            74.0,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            pop_actions.push(UiAction::OpenMokaTerminal);
                                            close_popover = true;
                                        }
                                        if soft_button(
                                            ui,
                                            "全部停止",
                                            p.tag_bg,
                                            p.tag_text,
                                            74.0,
                                            true,
                                        )
                                        .clicked()
                                        {
                                            pop_actions.push(UiAction::StopAllServices);
                                        }
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if soft_button(
                                                    ui,
                                                    "彻底退出",
                                                    p.danger_soft,
                                                    p.danger_text,
                                                    74.0,
                                                    true,
                                                )
                                                .clicked()
                                                {
                                                    do_quit = true;
                                                }
                                            },
                                        );
                                    },
                                );
                            });
                        });
                },
            );

            if !pop_actions.is_empty() {
                self.execute_actions(pop_actions, ctx);
            }
            if open_main {
                self.window_hidden_to_tray = false;
                restore_window(ctx);
            }
            if close_popover {
                self.tray_popover_open = false;
            }
            if do_quit {
                if self.stop_on_exit {
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
                std::process::exit(0);
            }
        }
    }
}