//! Interactive blob avatar creator.

use ding_client::avatar::{Accessory, Avatar, BodyShape, Eyes, Mouth, COLOR_COUNT};
use eframe::egui::{self, RichText, Sense, Vec2};

use crate::gui::blob;
use crate::gui::components::{self, ghost_button, primary_button};
use crate::gui::theme::{self, MUTED, PURPLE, TEXT};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Shape,
    Color,
    Eyes,
    Mouth,
    Accessory,
}

pub struct CreatorView<'a> {
    pub avatar: &'a mut Avatar,
    pub headphones: bool,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub show_skip: bool,
}

pub enum CreatorAction {
    Save,
    Skip,
    Cancel,
}

pub fn show(ui: &mut egui::Ui, view: CreatorView<'_>) -> Option<CreatorAction> {
    let mut action = None;
    let mut tab = ui.ctx().data_mut(|data| {
        *data.get_temp_mut_or_insert_with(egui::Id::new("ding.creator.tab"), || Tab::Shape)
    });

    ui.label(RichText::new(view.title).size(26.0).strong().color(TEXT));
    ui.label(RichText::new(view.subtitle).size(13.5).color(MUTED));
    ui.add_space(14.0);

    components::card_frame(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);
            blob::paint(ui, view.avatar, 140.0, view.headphones);
            ui.add_space(8.0);
            ui.label(RichText::new("That's you.").size(13.0).color(MUTED));
            ui.add_space(8.0);
        });
    });

    ui.horizontal_wrapped(|ui| {
        for (value, label) in [
            (Tab::Shape, "Shape"),
            (Tab::Color, "Color"),
            (Tab::Eyes, "Eyes"),
            (Tab::Mouth, "Mouth"),
            (Tab::Accessory, "Extra"),
        ] {
            let selected = tab == value;
            let fill = if selected { PURPLE } else { theme::CARD };
            if ui
                .add(
                    egui::Button::new(RichText::new(label).color(TEXT).size(13.0))
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(12)),
                )
                .clicked()
            {
                tab = value;
            }
        }
    });
    ui.ctx().data_mut(|data| {
        data.insert_temp(egui::Id::new("ding.creator.tab"), tab);
    });

    ui.add_space(10.0);
    components::card_frame(ui, |ui| match tab {
        Tab::Shape => {
            ui.horizontal_wrapped(|ui| {
                for shape in [
                    BodyShape::Round,
                    BodyShape::Squish,
                    BodyShape::Tall,
                    BodyShape::Bean,
                ] {
                    let mut preview = view.avatar.clone();
                    preview.shape = shape;
                    if option_tile(ui, &preview, view.headphones, view.avatar.shape == shape) {
                        view.avatar.shape = shape;
                    }
                }
            });
        }
        Tab::Color => {
            ui.horizontal_wrapped(|ui| {
                for index in 0..COLOR_COUNT {
                    let selected = view.avatar.color == index;
                    let color = theme::color_at(index);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
                    ui.painter().circle_filled(rect.center(), 16.0, color);
                    if selected {
                        ui.painter().circle_stroke(
                            rect.center(),
                            17.5,
                            egui::Stroke::new(2.0_f32, TEXT),
                        );
                    }
                    if response.clicked() {
                        view.avatar.color = index;
                    }
                }
            });
        }
        Tab::Eyes => {
            ui.horizontal_wrapped(|ui| {
                for eyes in [Eyes::Dots, Eyes::Happy, Eyes::Wide, Eyes::Sleepy, Eyes::Wink] {
                    let mut preview = view.avatar.clone();
                    preview.eyes = eyes;
                    if option_tile(ui, &preview, view.headphones, view.avatar.eyes == eyes) {
                        view.avatar.eyes = eyes;
                    }
                }
            });
        }
        Tab::Mouth => {
            ui.horizontal_wrapped(|ui| {
                for mouth in [
                    Mouth::Smile,
                    Mouth::Grin,
                    Mouth::Oh,
                    Mouth::Flat,
                    Mouth::Smirk,
                ] {
                    let mut preview = view.avatar.clone();
                    preview.mouth = mouth;
                    if option_tile(ui, &preview, view.headphones, view.avatar.mouth == mouth) {
                        view.avatar.mouth = mouth;
                    }
                }
            });
        }
        Tab::Accessory => {
            ui.horizontal_wrapped(|ui| {
                for accessory in [
                    Accessory::None,
                    Accessory::Glasses,
                    Accessory::Cap,
                    Accessory::Sprout,
                    Accessory::Bow,
                ] {
                    let mut preview = view.avatar.clone();
                    preview.accessory = accessory;
                    if option_tile(
                        ui,
                        &preview,
                        view.headphones,
                        view.avatar.accessory == accessory,
                    ) {
                        view.avatar.accessory = accessory;
                    }
                }
            });
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ghost_button(ui, "Randomize").clicked() {
            *view.avatar = Avatar::random();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if primary_button(ui, "Save").clicked() {
                action = Some(CreatorAction::Save);
            }
            if view.show_skip && ghost_button(ui, "Skip").clicked() {
                *view.avatar = Avatar::random();
                action = Some(CreatorAction::Skip);
            }
            if !view.show_skip && ghost_button(ui, "Cancel").clicked() {
                action = Some(CreatorAction::Cancel);
            }
        });
    });

    action
}

fn option_tile(ui: &mut egui::Ui, avatar: &Avatar, headphones: bool, selected: bool) -> bool {
    let size = Vec2::splat(64.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if selected {
        egui::Color32::from_rgba_unmultiplied(139, 92, 246, 70)
    } else {
        theme::CARD_HOVER
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(14), fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(14),
            egui::Stroke::new(1.5_f32, PURPLE),
            egui::StrokeKind::Middle,
        );
    }
    blob::paint_at(ui.painter(), rect.center(), 20.0, avatar, headphones, 1.0);
    response.clicked()
}
