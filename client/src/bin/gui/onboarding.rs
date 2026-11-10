//! Name entry before the network boots.

use eframe::egui::{self, RichText};

use crate::gui::blob;
use crate::gui::components;
use crate::gui::theme::{self, MUTED, PURPLE, TEXT};

pub struct NameView<'a> {
    pub draft: &'a mut String,
    pub error: Option<&'a str>,
    pub pending: bool,
}

pub enum NameAction {
    Submit,
}

pub fn name_screen(ui: &mut egui::Ui, view: NameView<'_>) -> Option<NameAction> {
    let mut action = None;
    ui.vertical_centered(|ui| {
        ui.add_space(28.0);
        blob::paint_logo_bell(ui, 96.0, false);
        ui.add_space(10.0);
        ui.label(RichText::new("ding").size(36.0).strong().color(TEXT));
        ui.label(
            RichText::new("Tap a coworker. Wake their headphones.")
                .size(14.0)
                .color(MUTED),
        );
        ui.add_space(28.0);
    });

    components::card_frame(ui, |ui| {
        ui.label(
            RichText::new("What should people yell?")
                .size(16.0)
                .strong()
                .color(TEXT),
        );
        ui.add_space(8.0);
        let edit = egui::TextEdit::singleline(view.draft)
            .hint_text("Ada")
            .desired_width(f32::INFINITY)
            .font(egui::TextStyle::Heading);
        let response = ui.add(edit);
        let enter =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if let Some(error) = view.error {
            ui.add_space(6.0);
            ui.label(RichText::new(error).color(theme::DANGER));
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let button = egui::Button::new(
                RichText::new("That's me").color(TEXT).strong().size(15.0),
            )
            .fill(PURPLE)
            .corner_radius(egui::CornerRadius::same(16))
            .min_size(egui::vec2(140.0, 42.0));
            if ui.add_enabled(!view.pending, button).clicked() || (enter && !view.pending) {
                action = Some(NameAction::Submit);
            }
            if view.pending {
                ui.spinner();
                ui.label(RichText::new("Warming up the bell…").color(MUTED));
            }
        });
    });

    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Next you'll pick a cute blob. Or skip and get a surprise.")
                .size(12.5)
                .color(MUTED),
        );
    });

    action
}
