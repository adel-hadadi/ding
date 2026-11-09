//! Standalone Ding hub. Teammates connect with `join <host:port> <team>`.

use std::net::SocketAddr;
use std::process::ExitCode;

use ding_server::{serve, App, LocalEvent};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("ding_server=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let port = match parse_port() {
        Ok(port) => port,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };

    let app = App::new("ding-server");
    // Subscribe before accepting connections so a direct ding can't arrive unheard.
    let mut events = app.subscribe();
    tokio::spawn(async move {
        loop {
            match events.recv().await {
                Ok(LocalEvent::DirectDing { from, kind }) => {
                    tracing::info!(from, kind, "direct ding");
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let listen: SocketAddr = ([0, 0, 0, 0], port).into();
    let listener = match TcpListener::bind(listen).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("could not listen on port {port}: {err}");
            return ExitCode::from(1);
        }
    };
    let bound = listener
        .local_addr()
        .map(|addr| addr.port())
        .unwrap_or(port);
    println!("Ding server listening on ws://0.0.0.0:{bound}/ws");
    println!("Teams live in memory and disappear when this process stops.");

    if let Err(err) = serve(listener, app).await {
        eprintln!("server stopped: {err}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn parse_port() -> Result<u16, String> {
    let raw = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("PORT").ok());
    match raw {
        None => Ok(3000),
        Some(value) => value
            .parse::<u16>()
            .map_err(|_| format!("port must be a number, got {value}")),
    }
}
