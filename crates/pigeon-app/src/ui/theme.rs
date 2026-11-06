//! Colours, the look of widgets, and small display helpers.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, Vec2};

/// Pigeon teal, from the app icon.
const ACCENT: Color32 = Color32::from_rgb(0x12, 0x9a, 0x90);

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub header: Color32,
    pub border: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub send: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub input: Color32,
    pub ok: Color32,
    pub err: Color32,
    pub warn: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: Color32::from_rgb(0x21, 0x21, 0x21),
            panel: Color32::from_rgb(0x21, 0x21, 0x21),
            header: Color32::from_rgb(0x1a, 0x1a, 0x1a),
            border: Color32::from_rgb(0x3a, 0x3a, 0x3a),
            text: Color32::from_rgb(0xe6, 0xe6, 0xe6),
            weak: Color32::from_rgb(0x9a, 0x9a, 0x9a),
            accent: ACCENT,
            send: Color32::from_rgb(0x09, 0x7b, 0xed),
            hover: Color32::from_rgb(0x2e, 0x2e, 0x2e),
            selected: Color32::from_rgb(0x36, 0x36, 0x36),
            input: Color32::from_rgb(0x1b, 0x1b, 0x1b),
            ok: Color32::from_rgb(0x4c, 0xc3, 0x6b),
            err: Color32::from_rgb(0xf0, 0x5a, 0x4f),
            warn: Color32::from_rgb(0xf0, 0xb0, 0x30),
        }
    } else {
        Palette {
            bg: Color32::WHITE,
            panel: Color32::WHITE,
            header: Color32::from_rgb(0xf9, 0xf9, 0xf9),
            border: Color32::from_rgb(0xe6, 0xe6, 0xe6),
            text: Color32::from_rgb(0x21, 0x21, 0x21),
            weak: Color32::from_rgb(0x6b, 0x6b, 0x6b),
            accent: ACCENT,
            send: Color32::from_rgb(0x02, 0x65, 0xd2),
            hover: Color32::from_rgb(0xf2, 0xf2, 0xf2),
            selected: Color32::from_rgb(0xeb, 0xeb, 0xeb),
            input: Color32::WHITE,
            ok: Color32::from_rgb(0x00, 0x7f, 0x31),
            err: Color32::from_rgb(0xd1, 0x2a, 0x1f),
            warn: Color32::from_rgb(0xa1, 0x6a, 0x00),
        }
    }
}

pub fn apply_theme(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);
    let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    v.panel_fill = p.panel;
    v.window_fill = p.bg;
    v.extreme_bg_color = p.input;
    v.faint_bg_color = p.header;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(6);
    v.selection.bg_fill = if dark { Color32::from_rgb(0x1f, 0x4d, 0x80) } else { Color32::from_rgb(0xcc, 0xe0, 0xff) };
    v.selection.stroke = Stroke::new(1.0, p.send);
    v.hyperlink_color = p.send;
    v.override_text_color = Some(p.text);
    let r = CornerRadius::same(4);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.weak_bg_fill = p.header;
    v.widgets.inactive.bg_fill = p.header;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.weak);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.weak_bg_fill = p.selected;
    v.widgets.active.bg_fill = p.selected;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.send);
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open = v.widgets.active;
    ctx.set_visuals_of(if dark { egui::Theme::Dark } else { egui::Theme::Light }, v);
    ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 4.0);
        s.spacing.interact_size.y = 26.0;
        s.text_styles.insert(egui::TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        s.text_styles.insert(egui::TextStyle::Button, FontId::new(13.0, FontFamily::Proportional));
        s.text_styles.insert(egui::TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        s.text_styles.insert(egui::TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
        s.text_styles.insert(egui::TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional));
    });
}

pub fn method_color(m: &str, dark: bool) -> Color32 {
    let (l, d) = match m.to_uppercase().as_str() {
        "GET" => ((0x00, 0x7f, 0x31), (0x6b, 0xdd, 0x9a)),
        "POST" => ((0xad, 0x7a, 0x03), (0xff, 0xe4, 0x7e)),
        "PUT" => ((0x00, 0x53, 0xb8), (0x74, 0xae, 0xf6)),
        "PATCH" => ((0x62, 0x34, 0x97), (0xc0, 0xa8, 0xe1)),
        "DELETE" => ((0x8e, 0x1a, 0x10), (0xf7, 0x9a, 0x8e)),
        "HEAD" => ((0x00, 0x7f, 0x31), (0x6b, 0xdd, 0x9a)),
        "OPTIONS" => ((0xa6, 0x14, 0x68), (0xf1, 0x90, 0xc4)),
        _ => ((0x6b, 0x6b, 0x6b), (0xb0, 0xb0, 0xb0)),
    };
    let c = if dark { d } else { l };
    Color32::from_rgb(c.0, c.1, c.2)
}

pub fn method_short(m: &str) -> String {
    match m.to_uppercase().as_str() {
        "DELETE" => "DEL".into(),
        "OPTIONS" => "OPT".into(),
        other => other.to_string(),
    }
}

pub fn status_color(code: u16, dark: bool) -> Color32 {
    let p = palette(dark);
    match code {
        200..=299 => p.ok,
        300..=399 => p.send,
        400..=599 => p.err,
        _ => p.weak,
    }
}

pub fn human_size(n: usize) -> String {
    if n < 1024 {
        format!("{} B", n)
    } else if n < 1024 * 1024 {
        format!("{:.2} KB", n as f64 / 1024.0)
    } else {
        format!("{:.2} MB", n as f64 / 1024.0 / 1024.0)
    }
}

/// A keyboard shortcut as this computer's keyboard labels it. The keys themselves are
/// Command on a Mac and Ctrl elsewhere.
pub fn shortcut(key: &str, alt: bool) -> String {
    if cfg!(target_os = "macos") {
        format!("{}⌘{}", if alt { "⌥" } else { "" }, if key == "Enter" { "↵" } else { key })
    } else {
        format!("Ctrl+{}{}", if alt { "Alt+" } else { "" }, key)
    }
}
