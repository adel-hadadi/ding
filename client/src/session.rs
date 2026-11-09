//! A client connection to a team hub (ours, or someone else's).

use std::net::SocketAddr;
use std::time::Duration;

use ding_shared::{ClientMessage, ServerMessage};
use futures_util::sink::Sink;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug)]
pub struct TeamNotice {
    pub generation: u64,
    pub event: TeamEvent,
}

#[derive(Debug)]
pub enum TeamEvent {
    Message(ServerMessage),
    /// The socket died, or we never connected. The UI still thinks this generation is current.
    Failed(String),
}

enum Ready {
    Incoming(Option<Result<Message, tokio_tungstenite::tungstenite::Error>>),
    Outgoing(Option<ClientMessage>),
}

/// Open `ws://addr/ws`, send Join, and keep the socket alive.
///
/// The returned sender is how the UI asks to ding someone. Dropping it closes the team
/// connection. `generation` lets the UI ignore notices from a team it has already left.
pub fn spawn_team(
    addr: SocketAddr,
    team: String,
    name: String,
    generation: u64,
    notices: mpsc::UnboundedSender<TeamNotice>,
) -> mpsc::UnboundedSender<ClientMessage> {
    let (write_tx, write_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_team(addr, team, name, generation, notices, write_rx));
    write_tx
}

async fn run_team(
    addr: SocketAddr,
    team: String,
    name: String,
    generation: u64,
    notices: mpsc::UnboundedSender<TeamNotice>,
    mut write_rx: mpsc::UnboundedReceiver<ClientMessage>,
) {
    let url = format!("ws://{addr}/ws");
    let socket = match timeout(Duration::from_secs(5), connect_async(&url)).await {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(err)) => {
            let _ = notices.send(failed(
                generation,
                format!("could not connect to {addr}: {err}"),
            ));
            return;
        }
        Err(_) => {
            let _ = notices.send(failed(
                generation,
                format!("timed out connecting to {addr}"),
            ));
            return;
        }
    };

    // Split the socket so one task can sit blocked on reading while this one writes.
    // A channel is the handoff: only one task owns each half.
    let (mut sink, mut stream) = socket.split();
    let (inbound_tx, mut inbound_rx) = mpsc::unbounded_channel();
    let reader = tokio::spawn(async move {
        while let Some(frame) = stream.next().await {
            if inbound_tx.send(frame).is_err() {
                break;
            }
        }
    });

    let join = ClientMessage::Join { team, name };
    if !send_client(&mut sink, &join).await {
        reader.abort();
        let _ = notices.send(failed(generation, format!("could not join {addr}")));
        return;
    }

    let mut failure: Option<String> = None;
    loop {
        let ready = tokio::select! {
            incoming = inbound_rx.recv() => Ready::Incoming(incoming),
            outgoing = write_rx.recv() => Ready::Outgoing(outgoing),
        };
        match ready {
            // The UI dropped its sender on purpose (`leave`). Don't report that as an error.
            Ready::Outgoing(None) => break,
            Ready::Outgoing(Some(message)) => {
                if !send_client(&mut sink, &message).await {
                    failure = Some("disconnected from the team".into());
                    break;
                }
            }
            Ready::Incoming(None) | Ready::Incoming(Some(Err(_))) => {
                failure = Some("disconnected from the team".into());
                break;
            }
            Ready::Incoming(Some(Ok(Message::Close(_)))) => {
                failure = Some("disconnected from the team".into());
                break;
            }
            Ready::Incoming(Some(Ok(Message::Text(text)))) => {
                match serde_json::from_str::<ServerMessage>(text.as_str()) {
                    Ok(message) => {
                        let notice = TeamNotice {
                            generation,
                            event: TeamEvent::Message(message),
                        };
                        if notices.send(notice).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        failure = Some("could not understand the team server".into());
                        break;
                    }
                }
            }
            Ready::Incoming(Some(Ok(_))) => {}
        }
    }

    reader.abort();
    if let Some(message) = failure {
        let _ = notices.send(failed(generation, message));
    }
}

async fn send_client(
    sink: &mut (impl Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin),
    message: &ClientMessage,
) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    sink.send(Message::Text(text.into())).await.is_ok()
}

fn failed(generation: u64, message: String) -> TeamNotice {
    TeamNotice {
        generation,
        event: TeamEvent::Failed(message),
    }
}

/// One direct ding to a peer. Success means they accepted it.
pub async fn direct_ding(addr: SocketAddr, from: &str, kind: &str) -> Result<(), String> {
    let url = format!("ws://{addr}/ws");
    let (mut socket, _) = timeout(Duration::from_secs(3), connect_async(&url))
        .await
        .map_err(|_| format!("timed out reaching {addr}"))?
        .map_err(|err| format!("could not reach {addr}: {err}"))?;

    let message = ClientMessage::DirectDing {
        from: from.to_string(),
        kind: kind.to_string(),
    };
    let text = serde_json::to_string(&message).map_err(|err| err.to_string())?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|err| format!("could not send the ding: {err}"))?;

    let reply = timeout(Duration::from_secs(3), socket.next())
        .await
        .map_err(|_| "no reply to the ding".to_string())?
        .ok_or_else(|| "connection closed".to_string())?
        .map_err(|err| format!("connection error: {err}"))?;

    let Message::Text(body) = reply else {
        return Err("unexpected reply".into());
    };
    match serde_json::from_str::<ServerMessage>(body.as_str()) {
        Ok(ServerMessage::Delivered { .. }) => Ok(()),
        Ok(ServerMessage::Error { message }) => Err(message),
        _ => Err("unexpected reply".into()),
    }
}
