use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Shadow, Stroke, Vec2};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    ClaudeLight,
    ClaudeDark,
}


#[derive(Debug, Clone, Copy)]
pub struct ThemePalette {
    pub is_dark: bool,
    pub bg_root: Color32,
    pub bg_sidebar: Color32,
    pub bg_card: Color32,
    pub bg_card_alt: Color32,
    pub bg_input: Color32,
    pub bg_nav_active: Color32,
    pub bg_nav_hover: Color32,
    pub bg_row_stripe: Color32,
    pub border_subtle: Color32,
    pub border_strong: Color32,
    pub card_shadow: Shadow,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_text: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub success_text: Color32,
    pub danger: Color32,
    pub danger_soft: Color32,
    pub danger_text: Color32,
    pub tag_bg: Color32,
    pub tag_text: Color32,
}

impl ThemePalette {
    pub fn from_mode(mode: ThemeMode) -> Self {
        match mode {
            // Claude Desktop 暖米白（Warm Linen & Terracotta）
            ThemeMode::ClaudeLight => Self {
                is_dark: false,
                bg_root: Color32::from_rgb(250, 249, 245),       // #FAF9F5
                bg_sidebar: Color32::from_rgb(243, 241, 234),    // #F3F1EA
                bg_card: Color32::from_rgb(255, 255, 255),       // #FFFFFF
                bg_card_alt: Color32::from_rgb(246, 244, 238),   // #F6F4EE
                bg_input: Color32::from_rgb(255, 255, 255),      // #FFFFFF
                bg_nav_active: Color32::from_rgb(233, 230, 221), // #E9E6DD
                bg_nav_hover: Color32::from_rgb(239, 236, 228),  // #EFECE4
                bg_row_stripe: Color32::from_rgb(249, 248, 244), // #F9F8F4
                border_subtle: Color32::from_rgb(232, 229, 220), // #E8E5DC
                border_strong: Color32::from_rgb(211, 206, 194), // #D3CEC2
                card_shadow: Shadow {
                    offset: [0, 2],
                    blur: 8,
                    spread: 0,
                    color: Color32::from_black_alpha(10),
                },
                text_primary: Color32::from_rgb(30, 29, 27),     // #1E1D1B
                text_secondary: Color32::from_rgb(77, 75, 70),   // #4D4B46
                text_muted: Color32::from_rgb(128, 124, 115),    // #807C73
                accent: Color32::from_rgb(217, 119, 87),         // #D97757
                accent_soft: Color32::from_rgb(248, 234, 227),   // #F8EAE3
                accent_text: Color32::from_rgb(179, 77, 46),     // #B34D2E
                success: Color32::from_rgb(42, 133, 94),         // #2A855E
                success_soft: Color32::from_rgb(231, 244, 238),  // #E7F4EE
                success_text: Color32::from_rgb(29, 107, 74),    // #1D6B4A
                danger: Color32::from_rgb(198, 63, 59),          // #C63F3B
                danger_soft: Color32::from_rgb(252, 235, 234),   // #FCEBEA
                danger_text: Color32::from_rgb(168, 44, 41),     // #A82C29
                tag_bg: Color32::from_rgb(239, 236, 228),        // #EFECE4
                tag_text: Color32::from_rgb(94, 91, 84),         // #5E5B54
            },
            // Claude Desktop 暖炭黑（Warm Charcoal & Terracotta）
            ThemeMode::ClaudeDark => Self {
                is_dark: true,
                bg_root: Color32::from_rgb(38, 38, 36),          // #262624
                bg_sidebar: Color32::from_rgb(31, 30, 29),       // #1F1E1D
                bg_card: Color32::from_rgb(47, 46, 43),          // #2F2E2B
                bg_card_alt: Color32::from_rgb(55, 54, 50),      // #373632
                bg_input: Color32::from_rgb(33, 32, 30),         // #21201E
                bg_nav_active: Color32::from_rgb(54, 53, 49),    // #363531
                bg_nav_hover: Color32::from_rgb(44, 43, 40),     // #2C2B28
                bg_row_stripe: Color32::from_rgb(42, 41, 39),    // #2A2927
                border_subtle: Color32::from_rgb(65, 63, 58),    // #413F3A
                border_strong: Color32::from_rgb(90, 87, 80),    // #5A5750
                card_shadow: Shadow::NONE,
                text_primary: Color32::from_rgb(242, 240, 233),  // #F2F0E9
                text_secondary: Color32::from_rgb(206, 202, 193),// #CECAC1
                text_muted: Color32::from_rgb(162, 158, 149),    // #A29E95
                accent: Color32::from_rgb(217, 119, 87),         // #D97757
                accent_soft: Color32::from_rgb(74, 48, 39),      // #4A3027
                accent_text: Color32::from_rgb(242, 165, 138),   // #F2A58A
                success: Color32::from_rgb(56, 161, 115),        // #38A173
                success_soft: Color32::from_rgb(32, 62, 49),     // #203E31
                success_text: Color32::from_rgb(134, 224, 184),  // #86E0B8
                danger: Color32::from_rgb(214, 79, 75),          // #D64F4B
                danger_soft: Color32::from_rgb(76, 36, 36),      // #4C2424
                danger_text: Color32::from_rgb(246, 158, 155),   // #F69E9B
                tag_bg: Color32::from_rgb(60, 59, 55),           // #3C3B37
                tag_text: Color32::from_rgb(196, 192, 183),      // #C4C0B7
            },
        }
    }
}

/// 生成内置 32x32 陶土橘圆角 Moka 图标（零外部图片依赖）
pub fn build_moka_icon() -> egui::IconData {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];
    let bg = [217u8, 119u8, 87u8, 255u8]; // #D97757
    let fg = [255u8, 255u8, 255u8, 255u8];

    for y in 0..size {
        for x in 0..size {
            let idx = (y * size + x) * 4;
            // 圆角矩形遮罩 (r=7)
            let rx = (x as f32 - 15.5).abs() - 8.5;
            let ry = (y as f32 - 15.5).abs() - 8.5;
            let dist = rx.max(0.0).hypot(ry.max(0.0));
            if dist <= 7.0 {
                rgba[idx..idx + 4].copy_from_slice(&bg);
            }
            // 居中极简字母 M
            let in_left = (8..=10).contains(&x) && (8..=23).contains(&y);
            let in_right = (21..=23).contains(&x) && (8..=23).contains(&y);
            let in_diag_l =
                (11..=15).contains(&x) && (y as isize - (8 + (x as isize - 10) * 8 / 5)).abs() <= 1;
            let in_diag_r =
                (16..=20).contains(&x) && (y as isize - (8 + (21 - x as isize) * 8 / 5)).abs() <= 1;
            if in_left || in_right || in_diag_l || in_diag_r {
                rgba[idx..idx + 4].copy_from_slice(&fg);
            }
        }
    }

    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let candidates = [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyh.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let data = Arc::new(FontData::from_owned(bytes));
            fonts.font_data.insert("cn_font".to_owned(), data);
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "cn_font".to_owned());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("cn_font".to_owned());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

pub fn apply_theme(ctx: &egui::Context, p: ThemePalette) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(13.0, 6.5);

    // 极简悬浮细滚动条（对标 macOS / Claude Desktop：透明轨道 + 低对比度暖灰 + 静止隐匿）
    let mut scroll = egui::style::ScrollStyle::floating();
    scroll.bar_width = 6.0;
    scroll.floating_width = 2.5;
    scroll.floating_allocated_width = 0.0;
    scroll.bar_outer_margin = 3.0;
    scroll.handle_min_length = 28.0;
    scroll.dormant_background_opacity = 0.0;
    scroll.active_background_opacity = 0.0;
    scroll.interact_background_opacity = 0.0;
    scroll.dormant_handle_opacity = 0.08;
    scroll.active_handle_opacity = 0.24;
    scroll.interact_handle_opacity = 0.44;
    style.spacing.scroll = scroll;

    let mut visuals = if p.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    visuals.panel_fill = p.bg_root;
    visuals.window_fill = p.bg_card;
    visuals.extreme_bg_color = p.bg_input;
    visuals.faint_bg_color = p.bg_row_stripe;
    visuals.hyperlink_color = p.accent_text;
    visuals.selection.bg_fill = p.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.accent);

    visuals.widgets.noninteractive.bg_fill = p.bg_card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);

    visuals.widgets.inactive.bg_fill = p.bg_card_alt;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border_subtle);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);

    visuals.widgets.hovered.bg_fill = p.bg_nav_active;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text_primary);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);

    visuals.widgets.active.bg_fill = p.accent;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.widgets.active.corner_radius = CornerRadius::same(8);

    style.visuals = visuals;
    ctx.set_style(style);
}


