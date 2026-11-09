//! A small window for Ding. egui draws it the same way on Ubuntu and macOS.

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ding_client::{play, Command, Note, Session, Snapshot};
use ding_shared::{name_key, normalize_name};
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};

const INK: Color32 = Color32::from_rgb(48, 32, 24);
const CREAM: Color32 = Color32::from_rgb(255, 244, 230);
const CARD: Color32 = Color32::from_rgb(255, 250, 244);
const ACCENT: Color32 = Color32::from_rgb(255, 107, 53);
const SOFT: Color32 = Color32::from_rgb(120, 90, 72);

fn main() -> eframe::Result {
    // Two workers is enough for a handful of sockets. A default runtime
    // would spawn one thread per CPU, which is a lot of noise for a bell.
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
            .with_inner_size(Vec2::new(440.0, 700.0))
            .with_min_inner_size(Vec2::new(380.0, 520.0))
            .with_title("Ding")
            .with_app_id("com.adelhadadi.ding"),
        vsync: true,
        ..Default::default()
    };
    let result = eframe::run_native(
        "Ding",
        options,
        Box::new(move |cc| Ok(Box::new(Gui::new(cc, handle)))),
    );
    runtime.shutdown_timeout(Duration::from_secs(1));
    result
}

struct Gui {
    runtime: tokio::runtime::Handle,
    updates_tx: Sender<Update>,
    updates: Receiver<Update>,
    commands: Sender<Command>,
    command_rx: Option<Receiver<Command>>,
    done: Receiver<()>,
    done_tx: Option<Sender<()>>,
    phase: Phase,
    kind: Kind,
    team_name: String,
    join_name: String,
    address: String,
    address_team: String,
    banner: Option<Banner>,
    status: String,
    snap: Option<Snapshot>,
}

enum Phase {
    Name {
        draft: String,
        error: Option<String>,
        pending: bool,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
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
}

impl Gui {
    fn new(cc: &eframe::CreationContext<'_>, runtime: tokio::runtime::Handle) -> Self {
        style(&cc.egui_ctx);
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
                draft: std::env::args().nth(1).unwrap_or_default(),
                error: None,
                pending: false,
            },
            kind: Kind::Bell,
            team_name: String::new(),
            join_name: String::new(),
            address: String::new(),
            address_team: String::new(),
            banner: None,
            status: "Ready to be annoying.".into(),
            snap: None,
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
                    self.status = "Listening. Headphones are a suggestion.".into();
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
                                self.status = format!("{to} has been disturbed.");
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
        if let Some(banner) = &mut self.banner {
            if !banner.played {
                play(&banner.kind);
                banner.played = true;
            }
        }
        fresh
    }

    fn start(&mut self, raw: String) {
        let name = match normalize_name(&raw) {
            Ok(name) => name,
            Err(message) => {
                if let Phase::Name { error, .. } = &mut self.phase {
                    *error = Some(message.into());
                }
                return;
            }
        };
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
        self.runtime.spawn(async move {
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            // `recv` blocks, so it lives on a plain thread and only forwards into tokio.
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

            match Session::boot(name).await {
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

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn name_screen(&mut self, ui: &mut egui::Ui) {
        ui.add_space(24.0);
        header(ui, false);
        ui.add_space(8.0);
        ui.label(
            RichText::new("What should people yell?")
                .size(18.0)
                .color(INK),
        );
        ui.add_space(8.0);

        let mut submit = false;
        let mut pending = false;
        if let Phase::Name {
            draft,
            error,
            pending: waiting,
        } = &mut self.phase
        {
            let edit = egui::TextEdit::singleline(draft)
                .hint_text("Ada")
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Heading);
            let response = ui.add(edit);
            submit = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.add_space(6.0);
            if let Some(error) = error {
                ui.label(RichText::new(error.as_str()).color(ACCENT));
            }
            pending = *waiting;
        }
        ui.add_space(8.0);
        let button = egui::Button::new(RichText::new("That's me").color(Color32::WHITE).size(18.0))
            .fill(ACCENT)
            .min_size(Vec2::new(140.0, 40.0));
        if ui.add_enabled(!pending, button).clicked() {
            submit = true;
        }
        if pending {
            ui.label(RichText::new("Warming up the bell…").color(SOFT));
        }
        if submit && !pending {
            let draft = match &self.phase {
                Phase::Name { draft, .. } => draft.clone(),
                Phase::Running => return,
            };
            self.start(draft);
        }
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let Some(snap) = self.snap.clone() else {
            ui.label("One moment…");
            return;
        };
        header(ui, self.banner.is_some());
        ui.label(
            RichText::new(format!("You are {}", snap.me))
                .size(16.0)
                .color(SOFT),
        );
        ui.add_space(8.0);
        self.banner(ui);
        self.kinds(ui);
        ui.add_space(8.0);

        if snap.discovery_off {
            ui.label(
                RichText::new("The network is hiding everyone. Join a team by address below.")
                    .color(SOFT),
            );
        }

        section(ui, "Nearby");
        if snap.peers.is_empty() {
            card(ui, |ui| {
                ui.label("Nobody else is running Ding.");
                ui.label(RichText::new("Go tap a shoulder the old-fashioned way.").color(SOFT));
            });
        } else {
            let people: Vec<_> = snap
                .peers
                .iter()
                .map(|peer| (peer.name.clone(), peer.note.clone()))
                .collect();
            for (name, note) in people {
                self.person(ui, &name, &note);
            }
        }

        ui.add_space(8.0);
        section(ui, "Your team");
        if let Some(team) = &snap.team {
            let code = team.code.clone();
            let hosting = team.hosting;
            let joining = team.joining;
            let members = team.members.clone();
            card(ui, |ui| {
                let role = if hosting { "hosting" } else { "in" };
                ui.label(RichText::new(format!("You're {role} {code}")).strong());
                if joining {
                    ui.label(RichText::new("Saving you a seat…").color(SOFT));
                }
            });
            for member in members {
                if ding_client_name_eq(&member, &snap.me) {
                    continue;
                }
                self.person(ui, &member, "in your team");
            }
            if accent_button(ui, "Leave, dramatically").clicked() {
                self.send(Command::Leave);
            }
        } else {
            self.team_form(ui);
        }

        ui.add_space(12.0);
        ui.separator();
        ui.label(RichText::new(&self.status).color(SOFT));
    }

    fn kinds(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Weapon of choice").color(SOFT));
            for kind in [Kind::Bell, Kind::Knock, Kind::Airhorn] {
                let selected = self.kind == kind;
                if ui.selectable_label(selected, kind.label()).clicked() {
                    self.kind = kind;
                }
            }
        });
    }

    fn person(&mut self, ui: &mut egui::Ui, name: &str, note: &str) {
        let mut ding = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                avatar(ui, name);
                ui.vertical(|ui| {
                    ui.label(RichText::new(name).size(18.0).strong());
                    ui.label(RichText::new(note).color(SOFT));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ding = accent_button(ui, self.kind.label()).clicked();
                });
            });
        });
        if ding {
            self.send(Command::Ding {
                to: name.to_string(),
                kind: self.kind.id().to_string(),
            });
            self.status = format!("Sent a {} toward {name}.", self.kind.id());
        }
    }

    fn team_form(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            ui.label("Teams are optional. Being loud is not.");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.team_name)
                        .hint_text("lunch, chaos, …")
                        .desired_width(180.0),
                );
                if ui.button("Create").clicked() && !self.team_name.trim().is_empty() {
                    self.send(Command::Create(self.team_name.clone()));
                }
            });
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.join_name)
                        .hint_text("who's hosting?")
                        .desired_width(180.0),
                );
                if ui.button("Join").clicked() && !self.join_name.trim().is_empty() {
                    self.send(Command::Join(self.join_name.clone()));
                }
            });
        });
        egui::CollapsingHeader::new("Join by address").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.address)
                        .hint_text("10.0.0.5:3000")
                        .desired_width(160.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.address_team)
                        .hint_text("team")
                        .desired_width(80.0),
                );
                if ui.button("Go").clicked() {
                    if let Some((host, port)) = split_host(&self.address) {
                        self.send(Command::JoinAddr {
                            host,
                            port,
                            team: self.address_team.clone(),
                        });
                    } else {
                        self.status = "Use host:port, like 10.0.0.5:3000.".into();
                    }
                }
            });
        });
    }

    fn banner(&mut self, ui: &mut egui::Ui) {
        let Some(banner) = &self.banner else {
            return;
        };
        let from = banner.from.clone();
        let kind = banner.kind.clone();
        let age = banner.at.elapsed().as_secs_f32();
        let pulse = (age * 6.0).sin().abs();
        let fill = Color32::from_rgb(255, 90 + (40.0_f32 * pulse) as u8, 40);
        let headline = match kind.as_str() {
            "airhorn" => format!("{from} deployed the airhorn"),
            "knock" => format!("{from} is knocking. On your soul."),
            _ => format!("{from} dinged you"),
        };
        let mut action = BannerAction::None;
        egui::Frame::new()
            .fill(fill)
            .corner_radius(16.0)
            .inner_margin(14.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(headline)
                        .size(20.0)
                        .color(Color32::WHITE)
                        .strong(),
                );
                ui.label(RichText::new("Headphones were a mistake.").color(Color32::WHITE));
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Ding back, obviously").color(INK))
                        .clicked()
                    {
                        action = BannerAction::Back;
                    }
                    if ui.button("I'll allow it").clicked() {
                        action = BannerAction::Dismiss;
                    }
                });
            });
        ui.add_space(8.0);
        match action {
            BannerAction::Back => {
                self.send(Command::Ding { to: from, kind });
                self.banner = None;
                self.status = "Returned fire.".into();
            }
            BannerAction::Dismiss => self.banner = None,
            BannerAction::None => {}
        }
    }
}

enum BannerAction {
    None,
    Back,
    Dismiss,
}

impl eframe::App for Gui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let fresh = self.pump();
        // Idle at 4 fps so LAN names show up. The banner animates at 20 fps,
        // not every vsync, so a ding does not pin the GPU.
        let wait = if self.banner.is_some() {
            Duration::from_millis(50)
        } else if fresh {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(250)
        };
        ctx.request_repaint_after(wait);
        egui::CentralPanel::default().show(ctx, |ui| {
            let running = matches!(self.phase, Phase::Running);
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Frame::new().inner_margin(18.0).show(ui, |ui| {
                    if running {
                        self.home(ui);
                    } else {
                        self.name_screen(ui);
                    }
                });
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.commands.send(Command::Shutdown);
        let _ = self.done.recv_timeout(Duration::from_secs(1));
    }
}

fn header(ui: &mut egui::Ui, awake: bool) {
    ui.horizontal(|ui| {
        bell(ui, awake);
        ui.vertical(|ui| {
            ui.label(RichText::new("DING").size(36.0).strong().color(INK));
            ui.label(RichText::new("Headphones on? Not for long.").color(SOFT));
        });
    });
}

fn bell(ui: &mut egui::Ui, awake: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(56.0), egui::Sense::hover());
    let time = ui.input(|input| input.time as f32);
    let wobble = if awake {
        (time * 14.0).sin() * 5.0
    } else {
        0.0
    };
    let painter = ui.painter();
    let center = rect.center() + Vec2::new(wobble, 2.0);
    painter.circle_filled(center, 16.0, ACCENT);
    painter.circle_stroke(center, 16.0, Stroke::new(3.0_f32, INK));
    painter.line_segment(
        [
            center + Vec2::new(0.0, -18.0),
            center + Vec2::new(0.0, -10.0),
        ],
        Stroke::new(3.0_f32, INK),
    );
    painter.circle_filled(center + Vec2::new(0.0, 18.0), 4.0, INK);
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).size(14.0).strong().color(SOFT));
    ui.add_space(4.0);
}

fn card(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(14.0)
        .inner_margin(12.0)
        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(235, 214, 196)))
        .show(ui, body);
    ui.add_space(6.0);
}

fn avatar(ui: &mut egui::Ui, name: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
    let color = color_for(name);
    ui.painter().circle_filled(rect.center(), 18.0, color);
    let letter = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_ascii_uppercase()
        .to_string();
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(16.0),
        Color32::WHITE,
    );
}

fn accent_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
            .fill(ACCENT)
            .min_size(Vec2::new(84.0, 32.0)),
    )
}

fn color_for(name: &str) -> Color32 {
    const COLORS: [Color32; 5] = [
        ACCENT,
        Color32::from_rgb(36, 140, 150),
        Color32::from_rgb(190, 70, 120),
        Color32::from_rgb(70, 140, 80),
        Color32::from_rgb(120, 90, 200),
    ];
    let mut hash = 0u32;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    COLORS[(hash as usize) % COLORS.len()]
}

fn split_host(raw: &str) -> Option<(String, u16)> {
    let (host, port) = raw.trim().rsplit_once(':')?;
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port.trim().parse().ok()?))
}

fn ding_client_name_eq(left: &str, right: &str) -> bool {
    name_key(left) == name_key(right)
}

fn style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = CREAM;
    visuals.window_fill = CREAM;
    visuals.extreme_bg_color = CARD;
    visuals.override_text_color = Some(INK);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, INK);
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(255, 228, 210);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.selection.bg_fill = ACCENT;
    ctx.set_visuals(visuals);
}
