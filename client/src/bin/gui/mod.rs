//! Dark playful Ding window.

mod blob;
mod components;
mod creator;
mod onboarding;
mod people;
mod settings;
mod theme;

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ding_client::avatar::Avatar;
use ding_client::{play, Command, Note, Prefs, Session, Snapshot};
use ding_shared::normalize_name;
use eframe::egui::{self, Vec2};

use components::{BannerAction, Nav, Toast, ToastAction};
use creator::CreatorAction;
use onboarding::NameAction;
use people::PeopleAction;
use settings::SettingsAction;

pub fn run() -> eframe::Result {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(4)
        .thread_name("ding-net")
        .enable_all()
        .build()
        .expect("tokio runtime");
    let handle = runtime.handle().clone();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(780.0, 640.0))
            .with_min_inner_size(Vec2::new(640.0, 520.0))
            .with_title("Ding")
            .with_app_id("com.adelhadadi.ding"),
        vsync: true,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Ding",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, handle)))),
    );
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bell,
    Knock,
    Airhorn,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::Bell => "bell",
            Kind::Knock => "knock",
            Kind::Airhorn => "airhorn",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Bell => "Ding",
            Kind::Knock => "Knock",
            Kind::Airhorn => "HONK",
        }
    }

    fn from_id(id: &str) -> Self {
        match id {
            "knock" => Kind::Knock,
            "airhorn" => Kind::Airhorn,
            _ => Kind::Bell,
        }
    }
}

enum Phase {
    Name {
        draft: String,
        error: Option<String>,
        pending: bool,
    },
    Creator {
        draft_name: String,
        first_run: bool,
    },
    Running,
}

enum Update {
    Ready(Snapshot),
    Failed(String),
    Tick { snap: Snapshot, notes: Vec<Note> },
}

struct Banner {
    from: String,
    kind: String,
    at: Instant,
    played: bool,
}

struct App {
    runtime: tokio::runtime::Handle,
    updates_tx: Sender<Update>,
    updates: Receiver<Update>,
    commands: Sender<Command>,
    command_rx: Option<Receiver<Command>>,
    done: Receiver<()>,
    done_tx: Option<Sender<()>>,
    phase: Phase,
    nav: Nav,
    prefs: Prefs,
    kind: Kind,
    team_name: String,
    join_name: String,
    address: String,
    address_team: String,
    banner: Option<Banner>,
    toast: Option<Toast>,
    status: String,
    snap: Option<Snapshot>,
    selected: Option<String>,
    editing_avatar: Option<Avatar>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Handle) -> Self {
        theme::apply(&cc.egui_ctx);
        let prefs = Prefs::load();
        let argv_name = std::env::args().nth(1).unwrap_or_default();
        let draft = if !argv_name.is_empty() {
            argv_name
        } else {
            prefs.name.clone()
        };
        let (updates_tx, updates) = mpsc::channel();
        let (commands, command_rx) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        Self {
            runtime,
            updates_tx,
            updates,
            commands,
            command_rx: Some(command_rx),
            done,
            done_tx: Some(done_tx),
            phase: Phase::Name {
                draft,
                error: None,
                pending: false,
            },
            nav: Nav::People,
            kind: Kind::from_id(&prefs.kind),
            prefs,
            team_name: String::new(),
            join_name: String::new(),
            address: String::new(),
            address_team: String::new(),
            banner: None,
            toast: None,
            status: "Ready when you are.".into(),
            snap: None,
            selected: None,
            editing_avatar: None,
        }
    }

    fn pump(&mut self) -> bool {
        let mut fresh = false;
        while let Ok(update) = self.updates.try_recv() {
            fresh = true;
            match update {
                Update::Ready(snap) => {
                    self.snap = Some(snap);
                    self.phase = Phase::Running;
                    self.status = "Listening on your network.".into();
                    self.push_presence();
                }
                Update::Failed(message) => {
                    if let Phase::Name { error, pending, .. } = &mut self.phase {
                        *error = Some(message);
                        *pending = false;
                    }
                }
                Update::Tick { snap, notes } => {
                    self.snap = Some(snap);
                    for note in notes {
                        match note {
                            Note::Incoming { from, kind } => {
                                self.banner = Some(Banner {
                                    from,
                                    kind,
                                    at: Instant::now(),
                                    played: false,
                                });
                            }
                            Note::Delivered { to } => {
                                self.status = format!("Ding delivered to {to}.");
                                self.toast = Some(Toast::new(format!(
                                    "Ding sent to {to}! They'll hear it in their headphones."
                                )));
                            }
                            Note::Status(text) if !text.is_empty() => self.status = text,
                            Note::Status(_) => {}
                        }
                    }
                }
            }
        }
        if self
            .banner
            .as_ref()
            .is_some_and(|banner| banner.at.elapsed() > Duration::from_secs(6))
        {
            self.banner = None;
        }
        if self.toast.as_ref().is_some_and(|toast| !toast.alive()) {
            self.toast = None;
        }
        if let Some(banner) = &mut self.banner {
            if !banner.played {
                play(&banner.kind);
                banner.played = true;
            }
        }
        fresh
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn push_presence(&self) {
        self.send(Command::SetPresence {
            headphones: self.prefs.headphones,
            avatar: self.prefs.avatar.encode(),
        });
    }

    fn save_prefs(&mut self) {
        self.prefs.kind = self.kind.id().to_string();
        self.prefs.save();
    }

    fn start_session(&mut self, name: String) {
        let Some(commands) = self.command_rx.take() else {
            return;
        };
        let Some(done_tx) = self.done_tx.take() else {
            return;
        };
        if let Phase::Name { pending, error, .. } = &mut self.phase {
            *pending = true;
            *error = None;
        }

        let updates = self.updates_tx.clone();
        let headphones = self.prefs.headphones;
        let avatar = self.prefs.avatar.encode();
        self.runtime.spawn(async move {
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            std::thread::spawn(move || {
                while let Ok(command) = commands.recv() {
                    let stop = matches!(command, Command::Shutdown);
                    if cmd_tx.send(command).is_err() {
                        break;
                    }
                    if stop {
                        break;
                    }
                }
            });

            match Session::boot_with(name, headphones, avatar).await {
                Ok(session) => {
                    let _ = updates.send(Update::Ready(session.snapshot()));
                    let ticks = updates.clone();
                    let (tick_tx, tick_rx) = mpsc::channel();
                    std::thread::spawn(move || {
                        while let Ok((snap, notes)) = tick_rx.recv() {
                            if ticks.send(Update::Tick { snap, notes }).is_err() {
                                break;
                            }
                        }
                    });
                    session.run_until(cmd_rx, tick_tx).await;
                }
                Err(message) => {
                    let _ = updates.send(Update::Failed(message));
                }
            }
            let _ = done_tx.send(());
        });
    }

    fn submit_name(&mut self) {
        let draft = match &self.phase {
            Phase::Name { draft, .. } => draft.clone(),
            _ => return,
        };
        let name = match normalize_name(&draft) {
            Ok(name) => name,
            Err(message) => {
                if let Phase::Name { error, .. } = &mut self.phase {
                    *error = Some(message.into());
                }
                return;
            }
        };
        self.prefs.name = name.clone();
        self.save_prefs();

        if self.prefs.avatar_saved {
            self.start_session(name);
        } else {
            self.editing_avatar = Some(self.prefs.avatar.clone());
            self.phase = Phase::Creator {
                draft_name: name,
                first_run: true,
            };
        }
    }

    fn finish_creator(&mut self, avatar: Avatar, start: bool) {
        self.prefs.avatar = avatar;
        self.prefs.avatar_saved = true;
        self.save_prefs();
        self.editing_avatar = None;
        if start {
            let name = match &self.phase {
                Phase::Creator { draft_name, .. } => draft_name.clone(),
                _ => self.prefs.name.clone(),
            };
            self.phase = Phase::Name {
                draft: name.clone(),
                error: None,
                pending: true,
            };
            self.start_session(name);
        } else {
            self.phase = Phase::Running;
            self.push_presence();
            self.nav = Nav::Settings;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let fresh = self.pump();
        let animating = self.banner.is_some() || self.toast.is_some();
        let wait = if animating {
            Duration::from_millis(50)
        } else if fresh {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(250)
        };
        ctx.request_repaint_after(wait);

        let narrow = ctx.content_rect().width() < 720.0;
        let first_run_creator =
            matches!(self.phase, Phase::Creator { first_run: true, .. });
        let onboarding = matches!(self.phase, Phase::Name { .. }) || first_run_creator;
        let editing = matches!(self.phase, Phase::Creator { first_run: false, .. })
            || self.editing_avatar.is_some();

        if !onboarding {
            let sidebar_w = if narrow {
                theme::SIDEBAR_NARROW
            } else {
                theme::SIDEBAR_W
            };
            egui::SidePanel::left("ding.sidebar")
                .exact_width(sidebar_w)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(theme::SIDEBAR)
                        .inner_margin(egui::Margin::symmetric(14, 12)),
                )
                .show(ctx, |ui| {
                    components::sidebar(
                        ui,
                        narrow,
                        &mut self.nav,
                        self.banner.is_some(),
                        self.snap.is_some(),
                    );
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(22, 16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if matches!(self.phase, Phase::Name { .. }) {
                            self.draw_name(ui);
                        } else if first_run_creator {
                            self.draw_creator(ui, true);
                        } else if editing {
                            self.draw_creator(ui, false);
                        } else {
                            self.draw_main(ui);
                        }
                    });
            });

        if let Some(toast) = &self.toast {
            let action = egui::Area::new(egui::Id::new("ding.toast"))
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| components::toast_bar(ui, toast))
                .inner;
            match action {
                ToastAction::Undo | ToastAction::Dismiss => self.toast = None,
                ToastAction::None => {}
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.commands.send(Command::Shutdown);
        let _ = self.done.recv_timeout(Duration::from_secs(1));
    }
}

impl App {
    fn draw_name(&mut self, ui: &mut egui::Ui) {
        let (error, pending) = match &self.phase {
            Phase::Name {
                error, pending, ..
            } => (error.clone(), *pending),
            _ => return,
        };
        let draft = match &mut self.phase {
            Phase::Name { draft, .. } => draft,
            _ => return,
        };
        if let Some(NameAction::Submit) = onboarding::name_screen(
            ui,
            onboarding::NameView {
                draft,
                error: error.as_deref(),
                pending,
            },
        ) {
            self.submit_name();
        }
    }

    fn draw_creator(&mut self, ui: &mut egui::Ui, first_run: bool) {
        if self.editing_avatar.is_none() {
            self.editing_avatar = Some(self.prefs.avatar.clone());
        }
        let Some(avatar) = self.editing_avatar.as_mut() else {
            return;
        };
        let action = creator::show(
            ui,
            creator::CreatorView {
                avatar,
                headphones: self.prefs.headphones,
                title: if first_run {
                    "Make your blob"
                } else {
                    "Edit your blob"
                },
                subtitle: if first_run {
                    "Pick a look coworkers will recognize on the network."
                } else {
                    "Changes sync to nearby Ding windows."
                },
                show_skip: first_run,
            },
        );
        match action {
            Some(CreatorAction::Save) => {
                let avatar = self.editing_avatar.clone().unwrap_or_default();
                self.finish_creator(avatar, first_run);
            }
            Some(CreatorAction::Skip) => {
                let avatar = self
                    .editing_avatar
                    .clone()
                    .unwrap_or_else(Avatar::random);
                self.finish_creator(avatar, true);
            }
            Some(CreatorAction::Cancel) => {
                self.editing_avatar = None;
                if matches!(self.phase, Phase::Creator { first_run: false, .. }) {
                    self.phase = Phase::Running;
                }
            }
            None => {}
        }
    }

    fn draw_main(&mut self, ui: &mut egui::Ui) {
        if let Some(banner) = &self.banner {
            let from = banner.from.clone();
            let kind = banner.kind.clone();
            let pulse = (banner.at.elapsed().as_secs_f32() * 6.0).sin().abs();
            match components::incoming_banner(ui, &from, &kind, pulse) {
                BannerAction::Back => {
                    self.send(Command::Ding {
                        to: from,
                        kind: self.kind.id().into(),
                    });
                    self.banner = None;
                    self.status = "Returned fire.".into();
                }
                BannerAction::Dismiss => self.banner = None,
                BannerAction::None => {}
            }
        }

        match self.nav {
            Nav::People => {
                let Some(snap) = self.snap.clone() else {
                    ui.label("One moment…");
                    return;
                };
                if let Some(PeopleAction::Ding { name }) = people::show(
                    ui,
                    people::PeopleView {
                        snap: &snap,
                        selected: self.selected.as_deref(),
                    },
                ) {
                    self.selected = Some(name.clone());
                    self.send(Command::Ding {
                        to: name.clone(),
                        kind: self.kind.id().into(),
                    });
                    self.status = format!("Sent a {} toward {name}.", self.kind.id());
                    self.toast = Some(Toast::new(format!(
                        "Ding sent to {name}! They'll hear it in their headphones."
                    )));
                }
            }
            Nav::Settings => {
                if let Some(action) = settings::show(
                    ui,
                    settings::SettingsView {
                        snap: self.snap.as_ref(),
                        avatar: &self.prefs.avatar,
                        headphones: self.prefs.headphones,
                        kind: self.kind,
                        team_name: &mut self.team_name,
                        join_name: &mut self.join_name,
                        address: &mut self.address,
                        address_team: &mut self.address_team,
                    },
                ) {
                    self.handle_settings(action);
                }
            }
        }

        ui.add_space(16.0);
        ui.label(
            egui::RichText::new(&self.status)
                .size(12.5)
                .color(theme::SOFT),
        );
    }

    fn handle_settings(&mut self, action: SettingsAction) {
        match action {
            SettingsAction::ToggleHeadphones => {
                self.prefs.headphones = !self.prefs.headphones;
                self.save_prefs();
                self.push_presence();
            }
            SettingsAction::SetKind(kind) => {
                self.kind = kind;
                self.save_prefs();
            }
            SettingsAction::EditAvatar => {
                self.editing_avatar = Some(self.prefs.avatar.clone());
                self.phase = Phase::Creator {
                    draft_name: self.prefs.name.clone(),
                    first_run: false,
                };
            }
            SettingsAction::CreateTeam => {
                self.send(Command::Create(self.team_name.clone()));
            }
            SettingsAction::JoinPerson => {
                self.send(Command::Join(self.join_name.clone()));
            }
            SettingsAction::JoinAddr { host, port } => {
                self.send(Command::JoinAddr {
                    host,
                    port,
                    team: self.address_team.clone(),
                });
            }
            SettingsAction::LeaveTeam => self.send(Command::Leave),
            SettingsAction::BadAddress => {
                self.status = "Use host:port, like 10.0.0.5:3000.".into();
            }
        }
    }
}
