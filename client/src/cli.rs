//! The original terminal client. The window app uses the same [`crate::Session`].

use std::io::BufRead;
use std::process::ExitCode;

use tokio::sync::mpsc;

use crate::model::{print_notes, Session, HELP};
use ding_shared::normalize_name;

pub async fn cli_main() -> ExitCode {
    let name = match startup_name() {
        Ok(name) => name,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };

    let mut session = match Session::boot(name).await {
        Ok(session) => session,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(1);
        }
    };

    let snap = session.snapshot();
    println!("Ding");
    println!("You are {} at {}:{}", snap.me, snap.ip, snap.port);
    if snap.discovery_off {
        println!("Join a team by address with: join <host:port> <team>");
    } else {
        println!("People running Ding on this network can see you.");
    }
    println!();
    println!("{HELP}");

    let (line_tx, mut lines) = mpsc::unbounded_channel();
    let stdin_tx = line_tx.clone();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if stdin_tx.send(Line::Text(line)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = stdin_tx.send(Line::End);
    });
    let quit_tx = line_tx;
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = quit_tx.send(Line::End);
        }
    });

    loop {
        let wake = tokio::select! {
            event = session.next_event() => Wake::Event(event),
            line = lines.recv() => Wake::Line(line),
        };
        match wake {
            Wake::Event(Some(event)) => print_notes(&session.apply(event)),
            Wake::Line(Some(Line::Text(line))) => {
                let (quit, notes) = session.handle_line(line).await;
                print_notes(&notes);
                if quit {
                    println!("bye");
                    break;
                }
            }
            Wake::Event(None) | Wake::Line(Some(Line::End)) | Wake::Line(None) => {
                print_notes(&session.shutdown());
                println!("bye");
                break;
            }
        }
    }

    ExitCode::SUCCESS
}

enum Line {
    Text(String),
    End,
}

enum Wake {
    Event(Option<crate::model::UiEvent>),
    Line(Option<Line>),
}

fn startup_name() -> Result<String, String> {
    let mut args = std::env::args().skip(1);
    let raw = match args.next() {
        Some(flag) if flag == "-h" || flag == "--help" => {
            eprintln!("Usage: ding [name]");
            eprintln!("If you omit the name, Ding uses your username.");
            std::process::exit(0);
        }
        Some(name) => {
            if args.next().is_some() {
                return Err("Usage: ding [name]".into());
            }
            name
        }
        None => std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "someone".into()),
    };
    normalize_name(&raw).map_err(|message| message.to_string())
}
