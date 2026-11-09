//! WebSocket connections for one hub.
//!
//! Every connection runs as its own task. Tasks share one `World` through `Arc<Mutex<_>>`:
//! `Arc` so each task can own a pointer to the same rooms, and `Mutex` so two joins can't
//! take the same name at once. The lock is only held for memory work (update the map, enqueue
//! a message). The socket write happens later, on that person's task, so a slow client can't
//! freeze the room and we never hold the lock across `.await`.
//!
//! `std::sync::Mutex` is the right lock here because of that. An async mutex is for guards
//! you keep while waiting. We don't.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use ding_shared::{normalize_kind, normalize_name, normalize_team, ClientMessage, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc};

use crate::hub::{Hub, Outgoing};

/// Something that happened on this machine, outside a team room.
/// A direct ding from the LAN is the only one so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalEvent {
    DirectDing { from: String, kind: String },
}

/// Shared handle. Cheap to clone: the rooms live behind one `Arc`.
#[derive(Clone)]
pub struct App {
    local_name: String,
    inner: Arc<Mutex<World>>,
    events: broadcast::Sender<LocalEvent>,
}

struct World {
    hub: Hub,
    /// `(team, lowercase name)` → queue of messages waiting for that socket.
    senders: HashMap<(String, String), mpsc::UnboundedSender<Outbound>>,
}

enum Outbound {
    Msg(ServerMessage),
    Close,
}

struct Joined {
    team: String,
    name: String,
    name_key: String,
}

enum Ready {
    Incoming(Option<Result<Message, axum::Error>>),
    Outgoing(Option<Outbound>),
}

impl App {
    /// `local_name` is this process's display name. Direct dings name it in `Delivered`.
    pub fn new(local_name: impl Into<String>) -> Self {
        let (events, _) = broadcast::channel(32);
        Self {
            local_name: local_name.into(),
            inner: Arc::new(Mutex::new(World {
                hub: Hub::default(),
                senders: HashMap::new(),
            })),
            events,
        }
    }

    /// Listen for direct dings that land on this process.
    /// Call this before [`serve`], so a message can't arrive with nobody subscribed.
    pub fn subscribe(&self) -> broadcast::Receiver<LocalEvent> {
        self.events.subscribe()
    }

    /// Kick everyone out of a team this process is hosting.
    pub fn close_team(&self, team: &str) {
        let Ok(team) = normalize_team(team) else {
            return;
        };
        let mut world = self.world();
        let keys = world.hub.take_room(&team);
        for key in keys {
            if let Some(sender) = world.senders.remove(&(team.clone(), key)) {
                // Closing the channel's `Close` message makes that socket task exit.
                let _ = sender.send(Outbound::Close);
            }
        }
        tracing::info!(team = %team, "closed team");
    }

    fn world(&self) -> MutexGuard<'_, World> {
        // Poison means a task panicked while holding the room lock. That's a bug;
        // serving a half-updated room would be worse than stopping.
        self.inner.lock().unwrap()
    }

    fn try_join(
        &self,
        team: &str,
        name: &str,
        sender: mpsc::UnboundedSender<Outbound>,
    ) -> Result<Joined, String> {
        let mut world = self.world();
        let outcome = world
            .hub
            .join(team, name)
            .map_err(|err| err.message().to_string())?;
        // Register the sender before telling anyone, so the new person is included
        // in the presence broadcast we're about to enqueue.
        world
            .senders
            .insert((outcome.team.clone(), outcome.name_key.clone()), sender);
        dispatch(&world.senders, outcome.outgoing);
        tracing::info!(team = %outcome.team, name = %outcome.name, "joined");
        Ok(Joined {
            team: outcome.team,
            name: outcome.name,
            name_key: outcome.name_key,
        })
    }

    fn try_ding(&self, team: &str, from: &str, to: &str, kind: &str) -> Result<(), String> {
        let mut world = self.world();
        let outgoing = world
            .hub
            .ding(team, from, to, kind)
            .map_err(|err| err.message().to_string())?;
        dispatch(&world.senders, outgoing);
        tracing::info!(team, from, to, kind, "ding");
        Ok(())
    }

    fn leave_user(&self, joined: &Joined) {
        let mut world = self.world();
        world
            .senders
            .remove(&(joined.team.clone(), joined.name_key.clone()));
        let outgoing = world.hub.leave(&joined.team, &joined.name_key);
        dispatch(&world.senders, outgoing);
        tracing::info!(team = %joined.team, name = %joined.name, "left");
    }

    fn emit_direct(&self, from: String, kind: String) {
        tracing::info!(from = %from, kind = %kind, "direct ding");
        // Nobody listening is fine: the sender still gets `Delivered`.
        let _ = self.events.send(LocalEvent::DirectDing { from, kind });
    }
}

fn dispatch(
    senders: &HashMap<(String, String), mpsc::UnboundedSender<Outbound>>,
    outgoing: Vec<Outgoing>,
) {
    for item in outgoing {
        let Some(sender) = senders.get(&(item.team, item.name_key)) else {
            continue;
        };
        // `send` on an unbounded channel only touches memory. It does not wait for the socket.
        let _ = sender.send(Outbound::Msg(item.msg));
    }
}

/// Accept WebSocket connections on `/ws` until `listener` closes.
pub async fn serve(listener: TcpListener, app: App) -> std::io::Result<()> {
    let router = Router::new().route("/ws", get(ws_handler)).with_state(app);
    axum::serve(listener, router).await
}

async fn ws_handler(ws: WebSocketUpgrade, State(app): State<App>) -> impl IntoResponse {
    // `spawn` happens inside `on_upgrade`: this returns the HTTP 101 immediately,
    // and `handle_socket` runs as the connection's own task.
    ws.on_upgrade(move |socket| handle_socket(socket, app))
}

async fn handle_socket(socket: WebSocket, app: App) {
    let (mut sink, mut stream) = socket.split();
    // One channel of "please write this" messages. Other tasks (and this one) enqueue;
    // only the loop below writes the socket. Unbounded so the room lock never waits.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Outbound>();
    let (in_tx, mut in_rx) = mpsc::unbounded_channel();

    // The read half lives on its own task so a `select!` can't cancel a half-read frame.
    let reader = tokio::spawn(async move {
        while let Some(frame) = stream.next().await {
            if in_tx.send(frame).is_err() {
                break;
            }
        }
    });

    let mut joined: Option<Joined> = None;
    loop {
        // Decide what happened, then write. `send` stays outside `select!` so it can't
        // be cancelled halfway through a frame.
        let ready = tokio::select! {
            incoming = in_rx.recv() => Ready::Incoming(incoming),
            outgoing = out_rx.recv() => Ready::Outgoing(outgoing),
        };
        match ready {
            Ready::Outgoing(Some(Outbound::Msg(message))) => {
                let text = serde_json::to_string(&message).unwrap_or_default();
                if sink.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
            Ready::Outgoing(Some(Outbound::Close)) | Ready::Outgoing(None) => break,
            Ready::Incoming(None) | Ready::Incoming(Some(Err(_))) => break,
            Ready::Incoming(Some(Ok(Message::Close(_)))) => break,
            Ready::Incoming(Some(Ok(Message::Text(text)))) => {
                on_text(&app, text.as_str(), &out_tx, &mut joined);
            }
            Ready::Incoming(Some(Ok(_))) => {}
        }
    }

    reader.abort();
    if let Some(joined) = joined.take() {
        app.leave_user(&joined);
    }
}

fn on_text(
    app: &App,
    text: &str,
    out_tx: &mpsc::UnboundedSender<Outbound>,
    joined: &mut Option<Joined>,
) {
    let message = match serde_json::from_str::<ClientMessage>(text) {
        Ok(message) => message,
        Err(_) => {
            reply_error(out_tx, "could not understand that message");
            return;
        }
    };

    match message {
        ClientMessage::Join { team, name } => {
            if joined.is_some() {
                reply_error(out_tx, "you are already in a team");
                return;
            }
            match app.try_join(&team, &name, out_tx.clone()) {
                Ok(session) => *joined = Some(session),
                Err(message) => reply_error(out_tx, message),
            }
        }
        ClientMessage::Ding { to, kind } => {
            let Some(session) = joined.as_ref() else {
                reply_error(out_tx, "join a team before sending a ding");
                return;
            };
            if let Err(message) = app.try_ding(&session.team, &session.name, &to, &kind) {
                reply_error(out_tx, message);
            }
        }
        ClientMessage::DirectDing { from, kind } => {
            match (normalize_name(&from), normalize_kind(&kind)) {
                (Ok(from), Ok(kind)) => {
                    app.emit_direct(from, kind);
                    let _ = out_tx.send(Outbound::Msg(ServerMessage::Delivered {
                        to: app.local_name.clone(),
                    }));
                }
                (Err(message), _) | (_, Err(message)) => reply_error(out_tx, message),
            }
        }
        ClientMessage::SetDnd { .. } => {
            reply_error(out_tx, "do not disturb is not available yet");
        }
    }
}

fn reply_error(out_tx: &mpsc::UnboundedSender<Outbound>, message: impl Into<String>) {
    let _ = out_tx.send(Outbound::Msg(ServerMessage::Error {
        message: message.into(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use futures_util::{SinkExt, StreamExt};
    use tokio::time::timeout;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as ClientMessageFrame;

    async fn start(name: &str) -> (u16, App) {
        let app = App::new(name);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let serve_app = app.clone();
        tokio::spawn(async move {
            serve(listener, serve_app).await.unwrap();
        });
        (port, app)
    }

    async fn connect(
        port: u16,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let (socket, _) = connect_async(format!("ws://127.0.0.1:{port}/ws"))
            .await
            .unwrap();
        socket
    }

    async fn send(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        message: &ClientMessage,
    ) {
        let text = serde_json::to_string(message).unwrap();
        socket
            .send(ClientMessageFrame::Text(text.into()))
            .await
            .unwrap();
    }

    async fn next_message(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> ServerMessage {
        let frame = timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("timed out waiting for a message")
            .expect("socket closed")
            .expect("socket error");
        let ClientMessageFrame::Text(text) = frame else {
            panic!("expected a text frame, got {frame:?}");
        };
        serde_json::from_str(text.as_str()).unwrap()
    }

    #[tokio::test]
    async fn relays_a_ding_between_two_people() {
        let (port, _) = start("host").await;
        let mut ada = connect(port).await;
        let mut grace = connect(port).await;

        send(
            &mut ada,
            &ClientMessage::Join {
                team: "design".into(),
                name: "Ada".into(),
            },
        )
        .await;
        let ServerMessage::Presence { users } = next_message(&mut ada).await else {
            panic!("Ada should see herself first");
        };
        assert_eq!(users, vec!["Ada".to_string()]);

        send(
            &mut grace,
            &ClientMessage::Join {
                team: "design".into(),
                name: "Grace".into(),
            },
        )
        .await;
        let ServerMessage::Presence { users } = next_message(&mut grace).await else {
            panic!("Grace should get a presence list");
        };
        assert_eq!(users, vec!["Ada".to_string(), "Grace".to_string()]);

        // Ada also hears that Grace arrived.
        let ServerMessage::Presence { users } = next_message(&mut ada).await else {
            panic!("Ada should see the updated room");
        };
        assert_eq!(users, vec!["Ada".to_string(), "Grace".to_string()]);

        send(
            &mut ada,
            &ClientMessage::Ding {
                to: "grace".into(),
                kind: "bell".into(),
            },
        )
        .await;
        match next_message(&mut grace).await {
            ServerMessage::Ding { from, kind } => {
                assert_eq!(from, "Ada");
                assert_eq!(kind, "bell");
            }
            other => panic!("expected a ding, got {other:?}"),
        }
        match next_message(&mut ada).await {
            ServerMessage::Delivered { to } => assert_eq!(to, "Grace"),
            other => panic!("expected delivered, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn duplicate_name_and_early_ding_are_errors() {
        let (port, _) = start("host").await;
        let mut ada = connect(port).await;
        send(
            &mut ada,
            &ClientMessage::Ding {
                to: "Grace".into(),
                kind: "bell".into(),
            },
        )
        .await;
        match next_message(&mut ada).await {
            ServerMessage::Error { message } => {
                assert!(message.contains("join a team"), "{message}")
            }
            other => panic!("expected an error, got {other:?}"),
        }

        send(
            &mut ada,
            &ClientMessage::Join {
                team: "design".into(),
                name: "Ada".into(),
            },
        )
        .await;
        let _ = next_message(&mut ada).await;

        send(&mut ada, &ClientMessage::SetDnd { enabled: true }).await;
        match next_message(&mut ada).await {
            ServerMessage::Error { message } => {
                assert!(message.contains("not available"), "{message}")
            }
            other => panic!("expected an error, got {other:?}"),
        }

        let mut other = connect(port).await;
        send(
            &mut other,
            &ClientMessage::Join {
                team: "design".into(),
                name: "ada".into(),
            },
        )
        .await;
        match next_message(&mut other).await {
            ServerMessage::Error { message } => assert!(message.contains("already"), "{message}"),
            other => panic!("expected name taken, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn direct_ding_reaches_the_local_listener() {
        let (port, app) = start("Grace").await;
        let mut events = app.subscribe();
        let mut ada = connect(port).await;
        send(
            &mut ada,
            &ClientMessage::DirectDing {
                from: "Ada".into(),
                kind: "knock".into(),
            },
        )
        .await;

        match next_message(&mut ada).await {
            ServerMessage::Delivered { to } => assert_eq!(to, "Grace"),
            other => panic!("expected delivered, got {other:?}"),
        }
        let event = timeout(Duration::from_secs(2), events.recv())
            .await
            .expect("timed out")
            .unwrap();
        assert_eq!(
            event,
            LocalEvent::DirectDing {
                from: "Ada".into(),
                kind: "knock".into(),
            }
        );
    }

    #[tokio::test]
    async fn leaving_removes_you_from_presence() {
        let (port, _) = start("host").await;
        let mut ada = connect(port).await;
        let mut grace = connect(port).await;
        send(
            &mut ada,
            &ClientMessage::Join {
                team: "design".into(),
                name: "Ada".into(),
            },
        )
        .await;
        let _ = next_message(&mut ada).await;
        send(
            &mut grace,
            &ClientMessage::Join {
                team: "design".into(),
                name: "Grace".into(),
            },
        )
        .await;
        let _ = next_message(&mut grace).await;
        let _ = next_message(&mut ada).await;

        drop(ada);
        match next_message(&mut grace).await {
            ServerMessage::Presence { users } => assert_eq!(users, vec!["Grace".to_string()]),
            other => panic!("expected updated presence, got {other:?}"),
        }
    }
}
