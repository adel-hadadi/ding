//! Who is nearby, which team we're in, and what just happened.
//!
//! The terminal and the window both drive this. It owns the sockets; they only draw.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use ding_server::{serve, App, LocalEvent};
use ding_shared::{name_key, normalize_team, ClientMessage, ServerMessage};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::discovery::{describe_note, primary_ipv4, Lan, LanEvent, Peer};
use crate::session::{direct_ding, spawn_team, TeamEvent, TeamNotice};

pub struct Session {
    me: String,
    port: u16,
    peers: HashMap<String, Peer>,
    team: Option<Team>,
    generation: u64,
    app: App,
    lan: Option<Lan>,
    notices: mpsc::UnboundedSender<TeamNotice>,
    ui_tx: mpsc::UnboundedSender<UiEvent>,
    events: mpsc::UnboundedReceiver<UiEvent>,
    discovery_off: bool,
    ip: String,
}

struct Team {
    code: String,
    hosting: bool,
    members: Vec<String>,
    saw_presence: bool,
    write: mpsc::UnboundedSender<ClientMessage>,
    generation: u64,
}

pub(crate) enum UiEvent {
    Lan(LanEvent),
    Local(LocalEvent),
    Team(TeamNotice),
    DirectResult {
        to: String,
        result: Result<(), String>,
    },
}

/// Something the screen should show. The terminal prints it; the window draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Status(String),
    Incoming { from: String, kind: String },
    Delivered { to: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerRow {
    pub name: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSnap {
    pub code: String,
    pub hosting: bool,
    pub members: Vec<String>,
    /// True until the hub has confirmed we're in the room.
    pub joining: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub me: String,
    pub ip: String,
    pub port: u16,
    pub discovery_off: bool,
    pub peers: Vec<PeerRow>,
    pub team: Option<TeamSnap>,
}

/// What the window asks the session to do.
#[derive(Debug)]
pub enum Command {
    Create(String),
    Join(String),
    JoinAddr {
        host: String,
        port: u16,
        team: String,
    },
    Leave,
    Ding {
        to: String,
        kind: String,
    },
    Shutdown,
}

impl Session {
    /// Open our own ding port, announce it, and start listening.
    pub async fn boot(name: String) -> Result<Self, String> {
        let app = App::new(name.clone());
        let mut local_events = app.subscribe();
        let listener = TcpListener::bind("0.0.0.0:0")
            .await
            .map_err(|err| format!("could not open a port: {err}"))?;
        let port = listener
            .local_addr()
            .map_err(|err| format!("could not read the local port: {err}"))?
            .port();
        let server = app.clone();
        tokio::spawn(async move {
            if let Err(err) = serve(listener, server).await {
                eprintln!("listener stopped: {err}");
            }
        });

        // One channel gathers LAN news, team messages, and direct dings.
        // Background tasks each hold a sender; this session is the only reader.
        let (ui_tx, events) = mpsc::unbounded_channel();

        let local_tx = ui_tx.clone();
        tokio::spawn(async move {
            loop {
                match local_events.recv().await {
                    Ok(event) => {
                        if local_tx.send(UiEvent::Local(event)).is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let (lan_tx, mut lan_rx) = mpsc::unbounded_channel();
        let lan = match Lan::start(&name, port, lan_tx) {
            Ok(lan) => Some(lan),
            Err(err) => {
                eprintln!("LAN discovery is off: {err}");
                None
            }
        };
        let lan_bridge = ui_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = lan_rx.recv().await {
                if lan_bridge.send(UiEvent::Lan(event)).is_err() {
                    break;
                }
            }
        });

        let (notices, mut team_rx) = mpsc::unbounded_channel();
        let team_bridge = ui_tx.clone();
        tokio::spawn(async move {
            while let Some(notice) = team_rx.recv().await {
                if team_bridge.send(UiEvent::Team(notice)).is_err() {
                    break;
                }
            }
        });

        let discovery_off = lan.is_none();
        Ok(Self {
            me: name,
            port,
            peers: HashMap::new(),
            team: None,
            generation: 0,
            app,
            lan,
            notices,
            ui_tx,
            events,
            discovery_off,
            ip: primary_ipv4(),
        })
    }

    /// Wait for the next network event. Cancel-safe: dropping this future does not lose a message.
    pub(crate) async fn next_event(&mut self) -> Option<UiEvent> {
        self.events.recv().await
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut peers: Vec<PeerRow> = self
            .peers
            .values()
            .map(|peer| PeerRow {
                name: peer.name.clone(),
                note: describe_note(peer),
            })
            .collect();
        peers.sort_by_key(|peer| name_key(&peer.name));
        Snapshot {
            me: self.me.clone(),
            ip: self.ip.clone(),
            port: self.port,
            discovery_off: self.discovery_off,
            peers,
            team: self.team.as_ref().map(|team| TeamSnap {
                code: team.code.clone(),
                hosting: team.hosting,
                members: team.members.clone(),
                joining: !team.saw_presence,
            }),
        }
    }

    /// Read network events and window commands until `Shutdown` or the channels close.
    pub async fn run_until(
        mut self,
        mut commands: mpsc::UnboundedReceiver<Command>,
        updates: std::sync::mpsc::Sender<(Snapshot, Vec<Note>)>,
    ) {
        loop {
            let wake = tokio::select! {
                event = self.events.recv() => Wake::Event(event),
                command = commands.recv() => Wake::Command(command),
            };
            let notes = match wake {
                Wake::Event(Some(event)) => self.apply(event),
                Wake::Command(Some(Command::Shutdown))
                | Wake::Event(None)
                | Wake::Command(None) => {
                    let notes = self.shutdown();
                    let _ = updates.send((self.snapshot(), notes));
                    break;
                }
                Wake::Command(Some(command)) => self.command(command).await,
            };
            if updates.send((self.snapshot(), notes)).is_err() {
                self.shutdown();
                break;
            }
        }
    }

    pub fn create_team(&mut self, raw: &str) -> Vec<Note> {
        let team = match normalize_team(raw) {
            Ok(team) => team,
            Err(message) => return vec![Note::Status(message.into())],
        };
        self.begin_team(SocketAddr::from(([127, 0, 0, 1], self.port)), team, true)
    }

    pub fn join_person(&mut self, target: &str) -> Vec<Note> {
        match resolve_join(&self.peers, target) {
            Ok((addr, team, host_name)) => {
                let mut notes = vec![Note::Status(format!("{host_name} is hosting {team}"))];
                notes.extend(self.begin_team(addr, team, false));
                notes
            }
            Err(message) => vec![Note::Status(message)],
        }
    }

    pub async fn join_addr(&mut self, host: &str, port: u16, raw_team: &str) -> Vec<Note> {
        let team = match normalize_team(raw_team) {
            Ok(team) => team,
            Err(message) => return vec![Note::Status(message.into())],
        };
        match resolve_host(host, port).await {
            Ok(addr) => self.begin_team(addr, team, false),
            Err(message) => vec![Note::Status(message)],
        }
    }

    pub fn leave(&mut self) -> Vec<Note> {
        let Some(code) = self.team.as_ref().map(|team| team.code.clone()) else {
            return vec![Note::Status("you are not in a team".into())];
        };
        self.disconnect();
        vec![Note::Status(format!("left {code}"))]
    }

    pub fn ding(&mut self, to: &str, kind: &str) -> Vec<Note> {
        if name_key(to) == name_key(&self.me) {
            return vec![Note::Status("you can't ding yourself".into())];
        }
        if self.team.as_ref().is_some_and(|team| in_team(team, to)) {
            let team = self.team.as_ref().unwrap();
            let message = ClientMessage::Ding {
                to: to.to_string(),
                kind: kind.to_string(),
            };
            if team.write.send(message).is_err() {
                return vec![Note::Status("disconnected from the team".into())];
            }
            return Vec::new();
        }

        let matches: Vec<&Peer> = self
            .peers
            .values()
            .filter(|peer| name_key(&peer.name) == name_key(to))
            .collect();
        match matches.as_slice() {
            [peer] => {
                let addr = peer.addr;
                let display = peer.name.clone();
                let from = self.me.clone();
                let kind = kind.to_string();
                let tx = self.ui_tx.clone();
                tokio::spawn(async move {
                    let result = direct_ding(addr, &from, &kind).await;
                    let _ = tx.send(UiEvent::DirectResult {
                        to: display,
                        result,
                    });
                });
                Vec::new()
            }
            [] => vec![Note::Status(format!("nobody named {to} is here"))],
            many => {
                let mut message = format!("more than one person nearby is named {to}:");
                for peer in many {
                    message.push_str(&format!("\n  {} at {}", peer.name, peer.addr));
                }
                vec![Note::Status(message)]
            }
        }
    }

    pub fn shutdown(&mut self) -> Vec<Note> {
        let notes = if let Some(code) = self.team.as_ref().map(|team| team.code.clone()) {
            self.disconnect();
            vec![Note::Status(format!("left {code}"))]
        } else {
            Vec::new()
        };
        if let Some(lan) = self.lan.take() {
            lan.shutdown();
        }
        notes
    }

    pub(crate) async fn handle_line(&mut self, line: String) -> (bool, Vec<Note>) {
        use crate::command::{parse_line, Action};
        match parse_line(&line) {
            Action::Nop => (false, Vec::new()),
            Action::Help => (false, vec![Note::Status(HELP.into())]),
            Action::List => (false, vec![Note::Status(format_list(&self.snapshot()))]),
            Action::Quit => (true, self.shutdown()),
            Action::Leave => (false, self.leave()),
            Action::Error(message) => {
                if message.is_empty() {
                    (false, Vec::new())
                } else {
                    (false, vec![Note::Status(message)])
                }
            }
            Action::Create { team } => (false, self.create_team(&team)),
            Action::JoinAddr { host, port, team } => {
                (false, self.join_addr(&host, port, &team).await)
            }
            Action::Join { target } => (false, self.join_person(&target)),
            Action::Ding { to, kind } => (false, self.ding(&to, &kind)),
        }
    }

    pub(crate) fn apply(&mut self, event: UiEvent) -> Vec<Note> {
        match event {
            UiEvent::Lan(event) => self.on_lan(event),
            UiEvent::Local(LocalEvent::DirectDing { from, kind }) => {
                vec![Note::Incoming { from, kind }]
            }
            UiEvent::Team(notice) => self.on_team(notice),
            UiEvent::DirectResult { to, result } => match result {
                Ok(()) => vec![Note::Delivered { to }],
                Err(message) => vec![Note::Status(message)],
            },
        }
    }

    async fn command(&mut self, command: Command) -> Vec<Note> {
        match command {
            Command::Create(team) => self.create_team(&team),
            Command::Join(target) => self.join_person(&target),
            Command::JoinAddr { host, port, team } => self.join_addr(&host, port, &team).await,
            Command::Leave => self.leave(),
            Command::Ding { to, kind } => self.ding(&to, &kind),
            Command::Shutdown => self.shutdown(),
        }
    }

    fn begin_team(&mut self, addr: SocketAddr, code: String, hosting: bool) -> Vec<Note> {
        if self
            .team
            .as_ref()
            .is_some_and(|team| team.code == code && team.hosting == hosting)
            && hosting
        {
            return vec![Note::Status(format!("you are already hosting {code}"))];
        }
        self.disconnect();
        self.generation += 1;
        let generation = self.generation;
        let write = spawn_team(
            addr,
            code.clone(),
            self.me.clone(),
            generation,
            self.notices.clone(),
        );
        self.team = Some(Team {
            code: code.clone(),
            hosting,
            members: Vec::new(),
            saw_presence: false,
            write,
            generation,
        });
        let mut notes = Vec::new();
        if let Some(lan) = &self.lan {
            if let Err(message) = lan.set_team(Some(&code), hosting) {
                notes.push(Note::Status(format!("team announcement failed: {message}")));
            }
        }
        if hosting {
            notes.push(Note::Status(format!(
                "hosting {code} — others can `join {}`",
                self.me
            )));
        } else {
            notes.push(Note::Status(format!("joining {code} at {addr}")));
        }
        notes
    }

    fn disconnect(&mut self) {
        let Some(team) = self.team.take() else {
            return;
        };
        if team.hosting {
            self.app.close_team(&team.code);
        }
        if let Some(lan) = &self.lan {
            let _ = lan.set_team(None, false);
        }
        drop(team);
    }

    fn on_team(&mut self, notice: TeamNotice) -> Vec<Note> {
        let current = self.team.as_ref().map(|team| team.generation);
        let Some(current) = current else {
            return Vec::new();
        };
        if current != notice.generation {
            return Vec::new();
        }
        match notice.event {
            TeamEvent::Failed(message) => {
                self.disconnect();
                vec![Note::Status(message)]
            }
            TeamEvent::Message(ServerMessage::Presence { users }) => {
                let me = self.me.clone();
                let code = self.team.as_ref().unwrap().code.clone();
                if let Some(team) = self.team.as_mut() {
                    team.members = users.clone();
                    team.saw_presence = true;
                }
                vec![Note::Status(format!(
                    "team {code}: {}",
                    format_members(&users, &me)
                ))]
            }
            TeamEvent::Message(ServerMessage::Ding { from, kind }) => {
                vec![Note::Incoming { from, kind }]
            }
            TeamEvent::Message(ServerMessage::Delivered { to }) => vec![Note::Delivered { to }],
            TeamEvent::Message(ServerMessage::Error { message }) => {
                let failed_join = self.team.as_ref().is_some_and(|team| !team.saw_presence);
                if failed_join {
                    self.disconnect();
                }
                vec![Note::Status(message)]
            }
        }
    }

    fn on_lan(&mut self, event: LanEvent) -> Vec<Note> {
        match event {
            LanEvent::Up(peer) => {
                let fullname = peer.fullname.clone();
                let name = peer.name.clone();
                let note = describe_note(&peer);
                let previous = self.peers.get(&fullname).map(describe_note);
                self.peers.insert(fullname, peer);
                match previous {
                    None => vec![Note::Status(format!("{name} is nearby ({note})"))],
                    Some(old) if old != note => vec![Note::Status(format!("{name} is now {note}"))],
                    Some(_) => Vec::new(),
                }
            }
            LanEvent::Down { fullname } => {
                if let Some(peer) = self.peers.remove(&fullname) {
                    vec![Note::Status(format!("{} left the network", peer.name))]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

enum Wake {
    Event(Option<UiEvent>),
    Command(Option<Command>),
}

fn in_team(team: &Team, to: &str) -> bool {
    let key = name_key(to);
    team.members.iter().any(|member| name_key(member) == key)
}

fn resolve_join(
    peers: &HashMap<String, Peer>,
    target: &str,
) -> Result<(SocketAddr, String, String), String> {
    let key = name_key(target);
    let by_name: Vec<&Peer> = peers
        .values()
        .filter(|peer| name_key(&peer.name) == key)
        .collect();
    if by_name.len() > 1 {
        return Err(format!(
            "more than one person nearby is named {target}. join <host:port> <team>"
        ));
    }
    if let Some(peer) = by_name.first() {
        if peer.hosting {
            let Some(team) = peer.team.clone() else {
                return Err(format!("{} is not hosting a team", peer.name));
            };
            return Ok((peer.addr, team, peer.name.clone()));
        }
    }

    let team_code = normalize_team(target).ok();
    let hosts: Vec<&Peer> = peers
        .values()
        .filter(|peer| {
            peer.hosting
                && team_code
                    .as_ref()
                    .is_some_and(|code| peer.team.as_deref() == Some(code.as_str()))
        })
        .collect();
    match hosts.as_slice() {
        [peer] => {
            let team = peer.team.clone().unwrap_or_default();
            Ok((peer.addr, team, peer.name.clone()))
        }
        [] => {
            if by_name.len() == 1 {
                Err(format!("{} is not hosting a team", by_name[0].name))
            } else {
                Err(format!("nobody nearby named {target} is hosting a team"))
            }
        }
        _ => Err(format!(
            "more than one person is hosting \"{}\". join them by name",
            team_code.unwrap_or_else(|| target.to_string())
        )),
    }
}

async fn resolve_host(host: &str, port: u16) -> Result<SocketAddr, String> {
    let lookup = timeout(
        Duration::from_secs(5),
        tokio::net::lookup_host((host, port)),
    )
    .await;
    let addrs = match lookup {
        Ok(Ok(addrs)) => addrs.collect::<Vec<_>>(),
        Ok(Err(err)) => return Err(format!("could not resolve {host}: {err}")),
        Err(_) => return Err(format!("timed out resolving {host}")),
    };
    addrs
        .iter()
        .copied()
        .find(|addr| addr.is_ipv4())
        .or_else(|| addrs.into_iter().next())
        .ok_or_else(|| format!("could not resolve {host}"))
}

fn format_members(members: &[String], me: &str) -> String {
    members
        .iter()
        .map(|name| {
            if name_key(name) == name_key(me) {
                format!("{name} (you)")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_list(snap: &Snapshot) -> String {
    let mut lines = vec!["On this network:".to_string()];
    if snap.discovery_off {
        lines.push("  (discovery is off)".into());
    } else if snap.peers.is_empty() {
        lines.push("  (nobody else yet)".into());
    } else {
        for peer in &snap.peers {
            lines.push(format!("  {:<16} {}", peer.name, peer.note));
        }
    }
    match &snap.team {
        Some(team) => {
            let role = if team.hosting { "hosting" } else { "joined" };
            lines.push(format!("Your team: {} ({role})", team.code));
            if team.members.is_empty() {
                lines.push("  (joining...)".into());
            } else {
                lines.push(format!("  {}", format_members(&team.members, &snap.me)));
            }
        }
        None => lines.push("Your team: (none)".into()),
    }
    lines.join("\n")
}

pub(crate) const HELP: &str = "\
list                     who's nearby, and your team
create <team>            start a team on this computer
join <person>            join the team that person is hosting
join <host:port> <team>  join a team by address
ding <person> [kind]     ding a teammate, or someone nearby (default kind: bell)
leave
quit
help";

pub(crate) fn print_notes(notes: &[Note]) {
    for note in notes {
        match note {
            Note::Status(text) if text.contains('\n') => println!("{text}"),
            Note::Status(text) => println!("* {text}"),
            Note::Incoming { from, kind } => println!("* ding from {from} ({kind})"),
            Note::Delivered { to } => println!("* delivered to {to}"),
        }
    }
}
