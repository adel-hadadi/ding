//! Shared chrome: sidebar, cards, toast, buttons.

use std::time::{Duration, Instant};

use ding_client::avatar::Avatar;
use eframe::egui::{self, Color32, CornerRadius, RichText, Sense, Stroke, Vec2};

use crate::gui::blob;
use crate::gui::theme::{self, MUTED, ONLINE, PURPLE, PURPLE_DEEP, SOFT, TEXT};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    People,
    Settings,
}

pub struct Toast {
    pub message: String,
    pub at: Instant,
    pub ttl: Duration,
}

impl Toast {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            at: Instant::now(),
            ttl: Duration::from_secs(4),
        }
    }

    pub fn alive(&self) -> bool {
        self.at.elapsed() < self.ttl
    }

    pub fn progress(&self) -> f32 {
        1.0 - (self.at.elapsed().as_secs_f32() / self.ttl.as_secs_f32()).clamp(0.0, 1.0)
    }
}

pub fn sidebar(
    ui: &mut egui::Ui,
    narrow: bool,
    nav: &mut Nav,
    awake: bool,
    online: bool,
) {
    let width = if narrow {
        theme::SIDEBAR_NARROW
    } else {
        theme::SIDEBAR_W
    };
    ui.set_width(width);
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        blob::paint_logo_bell(ui, if narrow { 36.0 } else { 42.0 }, awake);
        if !narrow {
            ui.add_space(4.0);
            ui.label(
                RichText::new("ding")
                    .size(26.0)
                    .strong()
                    .color(TEXT),
            );
        }
    });

    ui.add_space(28.0);
    nav_pill(ui, narrow, nav, Nav::People, "People", "P");
    ui.add_space(6.0);
    nav_pill(ui, narrow, nav, Nav::Settings, "Settings", "S");

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let color = if online { ONLINE } else { SOFT };
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
            if !narrow {
                ui.label(
                    RichText::new(if online { "Online" } else { "Offline" })
                        .size(13.0)
                        .color(MUTED),
                );
            }
        });
    });
}

fn nav_pill(ui: &mut egui::Ui, narrow: bool, nav: &mut Nav, value: Nav, label: &str, short: &str) {
    let selected = *nav == value;
    let fill = if selected { PURPLE } else { Color32::TRANSPARENT };
    let text = if selected { TEXT } else { MUTED };
    let content = if narrow {
        RichText::new(short).size(16.0).strong().color(text)
    } else {
        RichText::new(label).size(15.0).color(text)
    };
    let button = egui::Button::new(content)
        .fill(fill)
        .corner_radius(CornerRadius::same(18))
        .min_size(Vec2::new(ui.available_width(), 38.0));
    if ui.add(button).clicked() {
        *nav = value;
    }
}

pub fn discovery_chip(ui: &mut egui::Ui, discovery_off: bool, searching: bool) {
    let (label, color) = if discovery_off {
        ("Network discovery is off", theme::DANGER)
    } else if searching {
        ("Searching on your network…", MUTED)
    } else {
        ("Nearby on your network", ONLINE)
    };
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(CornerRadius::same(18))
        .inner_margin(egui::Margin::symmetric(12, 6))
        .stroke(Stroke::new(1.0_f32, theme::STROKE))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if searching && !discovery_off {
                    ui.spinner();
                } else {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                }
                ui.label(RichText::new(label).size(12.5).color(MUTED));
            });
        });
}

pub struct CoworkerCard<'a> {
    pub name: &'a str,
    pub avatar: &'a Avatar,
    pub online: bool,
    pub headphones: bool,
    pub note: &'a str,
    pub selected: bool,
    pub enabled: bool,
}

pub fn coworker_card(ui: &mut egui::Ui, card: CoworkerCard<'_>) -> bool {
    let mut ding = false;
    let stroke = if card.selected {
        Stroke::new(2.0_f32, PURPLE)
    } else {
        Stroke::new(1.0_f32, theme::STROKE)
    };
    let fill = if card.selected {
        Color32::from_rgb(40, 34, 58)
    } else {
        theme::CARD
    };
    let alpha = if card.online { 1.0 } else { 0.55 };

    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(20))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .stroke(stroke)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                blob::paint_at(
                    ui.painter(),
                    rect.center(),
                    22.0,
                    card.avatar,
                    false,
                    alpha,
                );
                if card.online {
                    ui.painter().circle_filled(
                        rect.right_bottom() - Vec2::new(10.0, 10.0),
                        6.0,
                        theme::BG,
                    );
                    ui.painter().circle_filled(
                        rect.right_bottom() - Vec2::new(10.0, 10.0),
                        4.5,
                        ONLINE,
                    );
                }

                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(card.name)
                            .size(17.0)
                            .strong()
                            .color(if card.online { TEXT } else { MUTED }),
                    );
                    let status = if !card.online {
                        "Away".to_string()
                    } else if card.headphones {
                        "Online • In Headphones".to_string()
                    } else {
                        "Online".to_string()
                    };
                    ui.label(RichText::new(status).size(12.5).color(MUTED));
                    if card.online && card.note != "not in a team" {
                        ui.label(RichText::new(card.note).size(11.5).color(SOFT));
                    }
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ding = ding_button(ui, card.enabled).clicked();
                });
            });
        });
    ui.add_space(8.0);
    ding
}

pub fn ding_button(ui: &mut egui::Ui, enabled: bool) -> egui::Response {
    let fill = if enabled { PURPLE } else { theme::CARD_HOVER };
    let text = if enabled { TEXT } else { SOFT };
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new("Ding").color(text).strong().size(14.0))
            .fill(fill)
            .corner_radius(CornerRadius::same(18))
            .min_size(Vec2::new(84.0, 40.0)),
    )
}

pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(TEXT).strong().size(15.0))
            .fill(PURPLE)
            .corner_radius(CornerRadius::same(16))
            .min_size(Vec2::new(120.0, 40.0)),
    )
}

pub fn ghost_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(MUTED).size(14.0))
            .fill(theme::CARD)
            .stroke(Stroke::new(1.0_f32, theme::STROKE))
            .corner_radius(CornerRadius::same(14))
            .min_size(Vec2::new(100.0, 36.0)),
    )
}

pub fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(13.0).strong().color(MUTED));
    ui.add_space(6.0);
}

pub fn card_frame(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(theme::CARD)
        .corner_radius(CornerRadius::same(18))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .stroke(Stroke::new(1.0_f32, theme::STROKE))
        .show(ui, body);
    ui.add_space(8.0);
}

pub fn toast_bar(ui: &mut egui::Ui, toast: &Toast) -> ToastAction {
    let mut action = ToastAction::None;
    let progress = toast.progress();
    egui::Frame::new()
        .fill(Color32::from_rgb(28, 26, 36))
        .corner_radius(CornerRadius::same(22))
        .inner_margin(egui::Margin::symmetric(16, 12))
        .stroke(Stroke::new(1.0_f32, theme::STROKE))
        .shadow(egui::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(90),
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 12.0, PURPLE);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "✓",
                    egui::FontId::proportional(14.0),
                    TEXT,
                );
                ui.label(RichText::new(&toast.message).size(13.5).color(TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("✕").color(SOFT)).frame(false))
                        .clicked()
                    {
                        action = ToastAction::Dismiss;
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("Undo").color(PURPLE).strong())
                                .frame(false),
                        )
                        .clicked()
                    {
                        action = ToastAction::Undo;
                    }
                });
            });
            ui.add_space(6.0);
            let bar = ui.available_width() * progress;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 3.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(2), theme::STROKE);
            let fill = egui::Rect::from_min_size(rect.min, Vec2::new(bar, rect.height()));
            ui.painter()
                .rect_filled(fill, CornerRadius::same(2), PURPLE_DEEP);
        });
    action
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastAction {
    None,
    Undo,
    Dismiss,
}

pub fn incoming_banner(
    ui: &mut egui::Ui,
    from: &str,
    kind: &str,
    pulse: f32,
) -> BannerAction {
    let mut action = BannerAction::None;
    let glow = 40 + (pulse * 30.0) as u8;
    let fill = Color32::from_rgb(88, 50 + glow / 3, 180);
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(18))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            let headline = match kind {
                "airhorn" => format!("{from} deployed the airhorn"),
                "knock" => format!("{from} is knocking"),
                _ => format!("{from} dinged you"),
            };
            ui.label(RichText::new(headline).size(18.0).strong().color(TEXT));
            ui.label(
                RichText::new("They'll hear it if you ding back.")
                    .size(13.0)
                    .color(LAVENDER_SOFT),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Ding back").clicked() {
                    action = BannerAction::Back;
                }
                if ghost_button(ui, "Dismiss").clicked() {
                    action = BannerAction::Dismiss;
                }
            });
        });
    ui.add_space(10.0);
    action
}

const LAVENDER_SOFT: Color32 = Color32::from_rgb(220, 210, 255);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BannerAction {
    None,
    Back,
    Dismiss,
}
