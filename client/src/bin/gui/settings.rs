//! Settings: headphones, ding kind, avatar entry, teams.

use ding_client::avatar::Avatar;
use ding_client::Snapshot;
use eframe::egui::{self, RichText};

use crate::gui::blob;
use crate::gui::components::{self, section_title};
use crate::gui::theme::{MUTED, ONLINE, TEXT};
use crate::gui::Kind;

pub struct SettingsView<'a> {
    pub snap: Option<&'a Snapshot>,
    pub avatar: &'a Avatar,
    pub headphones: bool,
    pub kind: Kind,
    pub team_name: &'a mut String,
    pub join_name: &'a mut String,
    pub address: &'a mut String,
    pub address_team: &'a mut String,
}

pub enum SettingsAction {
    ToggleHeadphones,
    SetKind(Kind),
    EditAvatar,
    CreateTeam,
    JoinPerson,
    JoinAddr { host: String, port: u16 },
    LeaveTeam,
    BadAddress,
}

pub fn show(ui: &mut egui::Ui, view: SettingsView<'_>) -> Option<SettingsAction> {
    let mut action = None;

    ui.label(RichText::new("Settings").size(28.0).strong().color(TEXT));
    ui.label(
        RichText::new("Make Ding feel like you, then invite the chaos.")
            .size(13.5)
            .color(MUTED),
    );
    ui.add_space(16.0);

    section_title(ui, "Your character");
    components::card_frame(ui, |ui| {
        ui.horizontal(|ui| {
            blob::paint(ui, view.avatar, 72.0, view.headphones);
            ui.add_space(10.0);
            ui.vertical(|ui| {
                if let Some(snap) = view.snap {
                    ui.label(RichText::new(&snap.me).size(18.0).strong().color(TEXT));
                } else {
                    ui.label(RichText::new("You").size(18.0).strong().color(TEXT));
                }
                let status = if view.headphones {
                    "Advertising: In Headphones"
                } else {
                    "Advertising: Online"
                };
                ui.label(RichText::new(status).size(12.5).color(MUTED));
                ui.add_space(8.0);
                if components::primary_button(ui, "Edit avatar").clicked() {
                    action = Some(SettingsAction::EditAvatar);
                }
            });
        });
    });

    section_title(ui, "Presence");
    components::card_frame(ui, |ui| {
        ui.horizontal(|ui| {
            let label = if view.headphones {
                "Wearing headphones"
            } else {
                "Not in headphones"
            };
            ui.label(RichText::new(label).size(14.5).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if view.headphones { "On" } else { "Off" };
                let fill = if view.headphones {
                    crate::gui::theme::PURPLE
                } else {
                    crate::gui::theme::CARD_HOVER
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(label).color(TEXT).strong().size(13.0),
                        )
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(14))
                        .min_size(egui::vec2(64.0, 32.0)),
                    )
                    .clicked()
                {
                    action = Some(SettingsAction::ToggleHeadphones);
                }
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    4.0,
                    if view.headphones { ONLINE } else { MUTED },
                );
            });
        });
        ui.label(
            RichText::new("Coworkers see this on your card over the local network.")
                .size(12.0)
                .color(MUTED),
        );
    });

    section_title(ui, "Weapon of choice");
    components::card_frame(ui, |ui| {
        ui.horizontal(|ui| {
            for kind in [Kind::Bell, Kind::Knock, Kind::Airhorn] {
                let selected = view.kind == kind;
                let fill = if selected {
                    crate::gui::theme::PURPLE
                } else {
                    crate::gui::theme::CARD_HOVER
                };
                let button = egui::Button::new(
                    RichText::new(kind.label())
                        .color(TEXT)
                        .strong()
                        .size(14.0),
                )
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(14))
                .min_size(egui::vec2(88.0, 36.0));
                if ui.add(button).clicked() {
                    action = Some(SettingsAction::SetKind(kind));
                }
            }
        });
    });

    section_title(ui, "Teams");
    if let Some(snap) = view.snap {
        if let Some(team) = &snap.team {
            components::card_frame(ui, |ui| {
                let role = if team.hosting { "Hosting" } else { "In" };
                ui.label(
                    RichText::new(format!("{role} “{}”", team.code))
                        .size(15.0)
                        .strong()
                        .color(TEXT),
                );
                if team.joining {
                    ui.label(RichText::new("Saving you a seat…").size(12.5).color(MUTED));
                } else {
                    ui.label(
                        RichText::new(format!("{} people in the room", team.members.len()))
                            .size(12.5)
                            .color(MUTED),
                    );
                }
                ui.add_space(8.0);
                if components::ghost_button(ui, "Leave team").clicked() {
                    action = Some(SettingsAction::LeaveTeam);
                }
            });
        } else {
            components::card_frame(ui, |ui| {
                ui.label(
                    RichText::new("Teams are optional. Being loud is not.")
                        .size(13.0)
                        .color(MUTED),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(view.team_name)
                            .hint_text("team name")
                            .desired_width(160.0),
                    );
                    if ui.button("Create").clicked() && !view.team_name.trim().is_empty() {
                        action = Some(SettingsAction::CreateTeam);
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(view.join_name)
                            .hint_text("who's hosting?")
                            .desired_width(160.0),
                    );
                    if ui.button("Join").clicked() && !view.join_name.trim().is_empty() {
                        action = Some(SettingsAction::JoinPerson);
                    }
                });
            });

            egui::CollapsingHeader::new("Join by address")
                .default_open(snap.discovery_off)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(view.address)
                                .hint_text("10.0.0.5:3000")
                                .desired_width(150.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(view.address_team)
                                .hint_text("team")
                                .desired_width(80.0),
                        );
                        if ui.button("Go").clicked() {
                            if let Some((host, port)) = split_host(view.address) {
                                action = Some(SettingsAction::JoinAddr { host, port });
                            } else {
                                action = Some(SettingsAction::BadAddress);
                            }
                        }
                    });
                });
        }

        ui.add_space(12.0);
        ui.label(
            RichText::new(format!("You are {} · {}:{}", snap.me, snap.ip, snap.port))
                .size(12.0)
                .color(MUTED),
        );
    } else {
        components::card_frame(ui, |ui| {
            ui.label(
                RichText::new("Join the network first to manage teams.")
                    .size(13.0)
                    .color(MUTED),
            );
        });
    }

    action
}

fn split_host(raw: &str) -> Option<(String, u16)> {
    let (host, port) = raw.trim().rsplit_once(':')?;
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port.trim().parse().ok()?))
}
