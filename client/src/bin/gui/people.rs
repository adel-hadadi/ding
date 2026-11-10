//! People screen: hero, discovery, coworker list.

use ding_client::avatar::Avatar;
use ding_client::Snapshot;
use ding_shared::name_key;
use eframe::egui::{self, RichText};

use crate::gui::blob;
use crate::gui::components::{self, CoworkerCard};
use crate::gui::theme::{MUTED, PURPLE, TEXT};

pub struct PeopleView<'a> {
    pub snap: &'a Snapshot,
    pub selected: Option<&'a str>,
}

pub enum PeopleAction {
    Ding { name: String },
}

pub fn show(ui: &mut egui::Ui, view: PeopleView<'_>) -> Option<PeopleAction> {
    let mut action = None;

    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            components::discovery_chip(ui, view.snap.discovery_off, view.snap.peers.is_empty());
        });
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        blob::paint_logo_bell(ui, 72.0, false);
        ui.add_space(8.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Ding ").size(30.0).strong().color(TEXT));
                ui.label(RichText::new("someone!").size(30.0).strong().color(PURPLE));
            });
            ui.label(
                RichText::new("Select a coworker and give them a ding")
                    .size(14.0)
                    .color(MUTED),
            );
        });
    });

    ui.add_space(18.0);

    let rows = coworker_rows(view.snap);
    if rows.is_empty() {
        components::card_frame(ui, |ui| {
            ui.label(
                RichText::new("Nobody else is running Ding yet.")
                    .size(15.0)
                    .color(TEXT),
            );
            ui.label(
                RichText::new(
                    "Leave this window open — teammates appear as they join your network.",
                )
                .size(13.0)
                .color(MUTED),
            );
        });
        return None;
    }

    for row in &rows {
        let avatar = row
            .avatar
            .as_deref()
            .and_then(Avatar::decode)
            .unwrap_or_else(|| Avatar::from_name(&row.name));
        let selected = view
            .selected
            .is_some_and(|name| name_key(name) == name_key(&row.name));
        if components::coworker_card(
            ui,
            CoworkerCard {
                name: &row.name,
                avatar: &avatar,
                online: row.online,
                headphones: row.headphones,
                note: &row.note,
                selected,
                enabled: row.online,
            },
        ) {
            action = Some(PeopleAction::Ding {
                name: row.name.clone(),
            });
        }
    }

    action
}

struct Row {
    name: String,
    note: String,
    headphones: bool,
    avatar: Option<String>,
    online: bool,
}

fn coworker_rows(snap: &Snapshot) -> Vec<Row> {
    let mut rows: Vec<Row> = snap
        .peers
        .iter()
        .map(|peer| Row {
            name: peer.name.clone(),
            note: peer.note.clone(),
            headphones: peer.headphones,
            avatar: peer.avatar.clone(),
            online: true,
        })
        .collect();

    if let Some(team) = &snap.team {
        for member in &team.members {
            if name_key(member) == name_key(&snap.me) {
                continue;
            }
            if rows
                .iter()
                .any(|row| name_key(&row.name) == name_key(member))
            {
                continue;
            }
            rows.push(Row {
                name: member.clone(),
                note: format!("in \"{}\"", team.code),
                headphones: false,
                avatar: None,
                online: false,
            });
        }
    }

    rows.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    rows
}
