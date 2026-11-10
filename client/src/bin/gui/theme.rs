//! Colors, radii, and egui visuals for the dark playful shell.

use eframe::egui::{self, Color32, FontFamily, FontId, Stroke, TextStyle};

pub const BG: Color32 = Color32::from_rgb(18, 18, 20);
pub const PANEL: Color32 = Color32::from_rgb(24, 24, 28);
pub const CARD: Color32 = Color32::from_rgb(32, 32, 38);
pub const CARD_HOVER: Color32 = Color32::from_rgb(40, 38, 48);
pub const SIDEBAR: Color32 = Color32::from_rgb(20, 20, 24);

pub const PURPLE: Color32 = Color32::from_rgb(139, 92, 246);
pub const PURPLE_DEEP: Color32 = Color32::from_rgb(109, 64, 220);
pub const TEXT: Color32 = Color32::from_rgb(245, 243, 255);
pub const MUTED: Color32 = Color32::from_rgb(148, 144, 168);
pub const SOFT: Color32 = Color32::from_rgb(110, 106, 130);
pub const ONLINE: Color32 = Color32::from_rgb(74, 222, 168);
pub const DANGER: Color32 = Color32::from_rgb(248, 113, 113);
pub const STROKE: Color32 = Color32::from_rgb(55, 52, 70);
pub const INK: Color32 = Color32::from_rgb(20, 16, 32);

pub const RADIUS_MD: f32 = 16.0;
pub const RADIUS_SM: f32 = 12.0;
pub const SIDEBAR_W: f32 = 168.0;
pub const SIDEBAR_NARROW: f32 = 72.0;

pub const PALETTE: [Color32; 8] = [
    Color32::from_rgb(167, 139, 250), // lavender
    Color32::from_rgb(110, 231, 183), // mint
    Color32::from_rgb(253, 186, 116), // peach
    Color32::from_rgb(147, 197, 253), // sky
    Color32::from_rgb(249, 168, 212), // pink
    Color32::from_rgb(252, 211, 77),  // butter
    Color32::from_rgb(94, 234, 212),  // teal
    Color32::from_rgb(196, 181, 253), // soft violet
];

pub fn color_at(index: u8) -> Color32 {
    PALETTE[(index as usize) % PALETTE.len()]
}

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(28.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.visuals = dark_visuals();
    ctx.set_style(style);
}

fn dark_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = CARD;
    visuals.faint_bg_color = CARD;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_fill = CARD;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    visuals.widgets.hovered.bg_fill = CARD_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.active.bg_fill = PURPLE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.selection.bg_fill = PURPLE;
    visuals.widgets.open.bg_fill = PURPLE_DEEP;
    visuals.window_corner_radius = RADIUS_MD.into();
    visuals.menu_corner_radius = RADIUS_SM.into();
    visuals.widgets.noninteractive.corner_radius = RADIUS_SM.into();
    visuals.widgets.inactive.corner_radius = RADIUS_SM.into();
    visuals.widgets.hovered.corner_radius = RADIUS_SM.into();
    visuals.widgets.active.corner_radius = RADIUS_SM.into();
    visuals
}
