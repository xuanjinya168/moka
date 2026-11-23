use crate::theme::ThemePalette;
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, Pos2, Rect, RichText, Shadow, Stroke, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Dashboard,
    Services,
    Sites,
    Store,
    Toolbox,
    Settings,
}

/// 自绘 14x14 矢量导航图标（不依赖系统字体，高分屏绝对锐利）
pub fn draw_nav_icon(ui: &mut egui::Ui, tab: NavTab, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(15.0), egui::Sense::hover());
    let painter = ui.painter();
    let stroke = Stroke::new(1.4_f32, color);
    let c = rect.center();

    match tab {
        NavTab::Dashboard => {
            // 2x2 精致圆角方格
            let s = 4.6_f32;
            let gap = 1.6_f32;
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let r = Rect::from_center_size(
                    Pos2::new(c.x + dx * (s * 0.5 + gap * 0.5), c.y + dy * (s * 0.5 + gap * 0.5)),
                    Vec2::splat(s),
                );
                painter.rect_stroke(r, 1.5, stroke, egui::StrokeKind::Middle);
            }
        }
        NavTab::Services => {
            // 双层服务器堆叠
            let top = Rect::from_center_size(Pos2::new(c.x, c.y - 3.2), Vec2::new(11.5, 4.4));
            let bot = Rect::from_center_size(Pos2::new(c.x, c.y + 3.2), Vec2::new(11.5, 4.4));
            painter.rect_stroke(top, 1.5, stroke, egui::StrokeKind::Middle);
            painter.rect_stroke(bot, 1.5, stroke, egui::StrokeKind::Middle);
            painter.circle_filled(Pos2::new(c.x - 3.2, c.y - 3.2), 1.0, color);
            painter.circle_filled(Pos2::new(c.x - 3.2, c.y + 3.2), 1.0, color);
        }
        NavTab::Sites => {
            // 极简地球/站点矢量图标
            let r = 5.8_f32;
            painter.circle_stroke(c, r, stroke);
            painter.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], stroke);
            painter.line_segment(
                [Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)],
                stroke,
            );
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(5.2, r * 2.0)),
                2.6,
                stroke,
                egui::StrokeKind::Middle,
            );
        }
        NavTab::Store => {
            // 简洁购物袋/安装包图标
            let body = Rect::from_center_size(Pos2::new(c.x, c.y + 1.5), Vec2::new(10.5, 8.5));
            painter.rect_stroke(body, 2.0, stroke, egui::StrokeKind::Middle);
            painter.line_segment(
                [Pos2::new(c.x - 2.5, c.y - 2.8), Pos2::new(c.x - 2.5, c.y - 5.2)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + 2.5, c.y - 2.8), Pos2::new(c.x + 2.5, c.y - 5.2)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x - 2.5, c.y - 5.2), Pos2::new(c.x + 2.5, c.y - 5.2)],
                stroke,
            );
        }
        NavTab::Toolbox => {
            // 终端/工具箱符号 >_
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(12.0, 10.0)),
                2.0,
                stroke,
                egui::StrokeKind::Middle,
            );
            painter.line_segment(
                [Pos2::new(c.x - 3.2, c.y - 2.0), Pos2::new(c.x - 1.0, c.y)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x - 1.0, c.y), Pos2::new(c.x - 3.2, c.y + 2.0)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(c.x + 0.5, c.y + 2.0), Pos2::new(c.x + 3.5, c.y + 2.0)],
                stroke,
            );
        }
        NavTab::Settings => {
            // 极简矢量齿轮图标
            painter.circle_stroke(c, 2.3, stroke);
            painter.circle_stroke(c, 4.9, stroke);
            for i in 0..8 {
                let angle = (i as f32) * std::f32::consts::FRAC_PI_4;
                let (sin_a, cos_a) = angle.sin_cos();
                let p1 = Pos2::new(c.x + cos_a * 4.9, c.y + sin_a * 4.9);
                let p2 = Pos2::new(c.x + cos_a * 6.5, c.y + sin_a * 6.5);
                painter.line_segment([p1, p2], stroke);
            }
        }
    }
}

pub fn styled_search_input(
    ui: &mut egui::Ui,
    p: ThemePalette,
    buf: &mut String,
    hint: &str,
    width: f32,
) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(p.bg_input)
        .stroke(Stroke::new(1.0_f32, p.border_strong))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 5))
        .show(ui, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(buf)
                    .frame(false)
                    .hint_text(hint)
                    .desired_width(width),
            );
            changed = r.changed();
        });
    changed
}

fn adjust_color_brightness(c: Color32, delta: i16) -> Color32 {
    let clamp = |v: u8| (v as i16 + delta).clamp(0, 255) as u8;
    Color32::from_rgba_unmultiplied(clamp(c.r()), clamp(c.g()), clamp(c.b()), c.a())
}

/// 统一自绘圆角按钮：在任何布局（vertical / horizontal / right_to_left / columns）中
/// 保证文字 100% 水平+垂直几何居中、永不折行截断、自动按文本宽度撑开、悬停/按下/禁用状态统一。
#[allow(clippy::too_many_arguments)]
fn render_styled_button(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    bold: bool,
    bg: Color32,
    fg: Color32,
    stroke: Stroke,
    corner_radius: u8,
    min_w: f32,
    height: f32,
    pad_x: f32,
    enabled: bool,
) -> egui::Response {
    let draw_fg = if enabled {
        fg
    } else {
        fg.gamma_multiply(0.48)
    };
    let rt = if bold {
        RichText::new(text).strong().size(font_size).color(draw_fg)
    } else {
        RichText::new(text).size(font_size).color(draw_fg)
    };
    let galley = egui::WidgetText::from(rt).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let desired_w = (galley.size().x + pad_x * 2.0).max(min_w);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(desired_w, height), sense);

    if ui.is_rect_visible(rect) {
        let is_dark = ui.visuals().dark_mode;
        let hover_delta: i16 = if is_dark { 14 } else { -10 };
        let press_delta: i16 = if is_dark { 22 } else { -18 };

        let (fill, eff_stroke) = if !enabled {
            (
                bg.gamma_multiply(0.55),
                Stroke::new(stroke.width, stroke.color.gamma_multiply(0.55)),
            )
        } else if resp.is_pointer_button_down_on() {
            (adjust_color_brightness(bg, press_delta), stroke)
        } else if resp.hovered() {
            (adjust_color_brightness(bg, hover_delta), stroke)
        } else {
            (bg, stroke)
        };

        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(corner_radius), fill);
        if eff_stroke.width > 0.0 && eff_stroke.color != Color32::TRANSPARENT {
            painter.rect_stroke(
                rect,
                CornerRadius::same(corner_radius),
                eff_stroke,
                egui::StrokeKind::Inside,
            );
        }

        let text_pos = Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5 - 0.5,
        );
        painter.galley(text_pos, galley, draw_fg);
    }

    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn primary_button(
    ui: &mut egui::Ui,
    p: ThemePalette,
    text: &str,
    min_w: f32,
    enabled: bool,
) -> egui::Response {
    render_styled_button(
        ui,
        text,
        12.5,
        true,
        p.accent,
        Color32::WHITE,
        Stroke::NONE,
        8,
        min_w,
        32.0,
        12.0,
        enabled,
    )
}

pub fn secondary_button(
    ui: &mut egui::Ui,
    p: ThemePalette,
    text: &str,
    min_w: f32,
    enabled: bool,
) -> egui::Response {
    render_styled_button(
        ui,
        text,
        12.5,
        false,
        p.bg_card,
        p.text_primary,
        Stroke::new(1.0_f32, p.border_strong),
        8,
        min_w,
        32.0,
        12.0,
        enabled,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn chip_button(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    bg: Color32,
    fg: Color32,
    stroke: Stroke,
    corner_radius: u8,
    min_w: f32,
    height: f32,
) -> egui::Response {
    render_styled_button(
        ui,
        text,
        font_size,
        false,
        bg,
        fg,
        stroke,
        corner_radius,
        min_w,
        height,
        9.0,
        true,
    )
}

pub fn segmented_choice(
    ui: &mut egui::Ui,
    p: ThemePalette,
    id_salt: &str,
    labels: &[&str],
    active_idx: usize,
) -> Option<usize> {
    let mut picked = None;
    ui.horizontal(|ui| {
        Frame::new()
            .fill(p.bg_sidebar)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(3))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for (idx, label) in labels.iter().enumerate() {
                        let selected = idx == active_idx;
                        let id = ui.make_persistent_id((id_salt, idx));
                        let hovered = ui.ctx().data(|d| d.get_temp::<bool>(id).unwrap_or(false));
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
                            (
                                Color32::TRANSPARENT,
                                Stroke::NONE,
                                Shadow::NONE,
                                p.text_secondary,
                            )
                        };
                        let resp = Frame::new()
                            .fill(fill)
                            .stroke(stroke)
                            .shadow(shadow)
                            .corner_radius(CornerRadius::same(7))
                            .inner_margin(Margin::symmetric(14, 6))
                            .show(ui, |ui| {
                                let txt = if selected {
                                    RichText::new(*label).strong().size(12.5).color(text_color)
                                } else {
                                    RichText::new(*label).size(12.5).color(text_color)
                                };
                                ui.label(txt);
                            })
                            .response
                            .interact(egui::Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.ctx().data_mut(|d| d.insert_temp(id, resp.hovered()));
                        if resp.clicked() && !selected {
                            picked = Some(idx);
                        }
                    }
                });
            });
    });
    picked
}

pub fn soft_button(
    ui: &mut egui::Ui,
    text: &str,
    bg: Color32,
    fg: Color32,
    min_w: f32,
    enabled: bool,
) -> egui::Response {
    render_styled_button(
        ui,
        text,
        12.0,
        false,
        bg,
        fg,
        Stroke::NONE,
        8,
        min_w,
        30.0,
        10.0,
        enabled,
    )
}

pub fn solid_button(
    ui: &mut egui::Ui,
    text: &str,
    bg: Color32,
    min_w: f32,
    enabled: bool,
) -> egui::Response {
    render_styled_button(
        ui,
        text,
        12.5,
        true,
        bg,
        Color32::WHITE,
        Stroke::NONE,
        8,
        min_w,
        32.0,
        12.0,
        enabled,
    )
}

pub fn is_error_log(line: &str) -> bool {
    let u = line.to_uppercase();
    // 明确标记为 INFO / WARN 的正常日志不算错误（避免 Spring 类名如 ExceptionTranslationFilter 误报）
    if u.contains(" INFO ") || u.contains("[INFO]") || u.contains(" WARN ") || u.contains("[WARN") {
        return false;
    }
    u.contains("[ERROR]")
        || u.contains(" ERROR ")
        || u.contains("FATAL")
        || u.contains("EXCEPTION:")
        || u.contains("EXCEPTION IN ")
        || u.contains("FAILED")
        || line.contains("失败")
}

pub fn is_warn_log(line: &str) -> bool {
    let u = line.to_uppercase();
    u.contains("[WARNING]") || u.contains(" WARN ") || u.contains("[WARN]")
}

pub fn log_line_color(line: &str, p: ThemePalette) -> Color32 {
    let u = line.to_uppercase();
    if is_error_log(line) {
        p.danger_text
    } else if is_warn_log(line) {
        if p.is_dark {
            Color32::from_rgb(245, 199, 122)
        } else {
            Color32::from_rgb(176, 108, 16)
        }
    } else if u.contains("[OK]")
        || u.contains("READY FOR CONNECTIONS")
        || line.contains("已就绪")
        || u.contains("[SYSTEM]")
    {
        p.success_text
    } else {
        p.text_secondary
    }
}

pub fn pill_badge(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(fg));
        });
}

pub fn card_frame(ui: &mut egui::Ui, p: ThemePalette, add_contents: impl FnOnce(&mut egui::Ui)) {
    Frame::new()
        .fill(p.bg_card)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .shadow(p.card_shadow)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(Margin::same(16))
        .show(ui, add_contents);
}

pub fn window_control_btn(
    ui: &mut egui::Ui,
    rect: Rect,
    id: &str,
    hover_bg: Color32,
    icon_color: Color32,
    white_on_hover: bool,
    draw_icon: impl FnOnce(&egui::Painter, Pos2, Color32),
) -> bool {
    let resp = ui
        .interact(rect, ui.id().with(id), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.hovered() {
        ui.painter().rect_filled(rect, 0.0, hover_bg);
    }
    let fg = if resp.hovered() && white_on_hover {
        Color32::WHITE
    } else {
        icon_color
    };
    draw_icon(ui.painter(), rect.center(), fg);
    resp.clicked()
}

pub fn handle_frameless_resize(ctx: &egui::Context) {
    let rect = ctx.screen_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let margin = 6.0_f32;
    let left = pos.x - rect.min.x <= margin;
    let right = rect.max.x - pos.x <= margin;
    let top = pos.y - rect.min.y <= margin;
    let bottom = rect.max.y - pos.y <= margin;

    let dir = match (left, right, top, bottom) {
        (true, _, true, _) => Some((egui::ResizeDirection::NorthWest, egui::CursorIcon::ResizeNwSe)),
        (_, true, true, _) => Some((egui::ResizeDirection::NorthEast, egui::CursorIcon::ResizeNeSw)),
        (true, _, _, true) => Some((egui::ResizeDirection::SouthWest, egui::CursorIcon::ResizeNeSw)),
        (_, true, _, true) => Some((egui::ResizeDirection::SouthEast, egui::CursorIcon::ResizeNwSe)),
        (true, _, _, _) => Some((egui::ResizeDirection::West, egui::CursorIcon::ResizeHorizontal)),
        (_, true, _, _) => Some((egui::ResizeDirection::East, egui::CursorIcon::ResizeHorizontal)),
        (_, _, true, _) => Some((egui::ResizeDirection::North, egui::CursorIcon::ResizeVertical)),
        (_, _, _, true) => Some((egui::ResizeDirection::South, egui::CursorIcon::ResizeVertical)),
        _ => None,
    };

    if let Some((resize_dir, cursor)) = dir {
        ctx.set_cursor_icon(cursor);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(resize_dir));
        }
    }
}

