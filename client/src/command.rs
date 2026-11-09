//! The words typed at the Ding prompt.
//!
//! Parsing is separate from the network so these tests don't open a socket.

use ding_shared::{normalize_kind, normalize_name, normalize_team};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Nop,
    Help,
    List,
    Quit,
    Leave,
    Create {
        team: String,
    },
    /// A person on the LAN, or the team code they are hosting.
    Join {
        target: String,
    },
    JoinAddr {
        host: String,
        port: u16,
        team: String,
    },
    Ding {
        to: String,
        kind: String,
    },
    Error(String),
}

pub fn parse_line(line: &str) -> Action {
    let line = line.trim();
    if line.is_empty() {
        return Action::Nop;
    }

    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap().to_ascii_lowercase();
    let rest: Vec<&str> = parts.collect();

    match cmd.as_str() {
        "help" => no_args(&rest, Action::Help, "usage: help"),
        "list" => no_args(&rest, Action::List, "usage: list"),
        "quit" | "exit" => no_args(&rest, Action::Quit, "usage: quit"),
        "leave" => no_args(&rest, Action::Leave, "usage: leave"),
        "create" => match rest.as_slice() {
            [team] => match normalize_team(team) {
                Ok(team) => Action::Create { team },
                Err(message) => Action::Error(message.into()),
            },
            _ => Action::Error("usage: create <team>".into()),
        },
        "join" => parse_join(&rest),
        "ding" => match rest.as_slice() {
            [to] => ding(to, "bell"),
            [to, kind] => ding(to, kind),
            _ => Action::Error("usage: ding <person> [kind]".into()),
        },
        other => Action::Error(format!("unknown command \"{other}\". type help")),
    }
}

fn no_args(rest: &[&str], action: Action, usage: &str) -> Action {
    if rest.is_empty() {
        action
    } else {
        Action::Error(usage.into())
    }
}

fn parse_join(rest: &[&str]) -> Action {
    match rest {
        [target] => Action::Join {
            target: (*target).to_string(),
        },
        [host_port, team] => match split_host_port(host_port) {
            Some((host, port)) => match normalize_team(team) {
                Ok(team) => Action::JoinAddr { host, port, team },
                Err(message) => Action::Error(message.into()),
            },
            None => Action::Error("usage: join <person>  or  join <host:port> <team>".into()),
        },
        _ => Action::Error("usage: join <person>  or  join <host:port> <team>".into()),
    }
}

fn split_host_port(raw: &str) -> Option<(String, u16)> {
    let (host, port) = raw.rsplit_once(':')?;
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() {
        return None;
    }
    let port = port.parse::<u16>().ok()?;
    Some((host.to_string(), port))
}

fn ding(to: &str, kind: &str) -> Action {
    match (normalize_name(to), normalize_kind(kind)) {
        (Ok(to), Ok(kind)) => Action::Ding { to, kind },
        (Err(message), _) | (_, Err(message)) => Action::Error(message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_commands() {
        assert_eq!(parse_line("   "), Action::Nop);
        assert_eq!(parse_line("LIST"), Action::List);
        assert_eq!(
            parse_line("create Design"),
            Action::Create {
                team: "design".into()
            }
        );
        assert_eq!(
            parse_line("ding Grace"),
            Action::Ding {
                to: "Grace".into(),
                kind: "bell".into(),
            }
        );
        assert_eq!(
            parse_line("ding Grace airhorn"),
            Action::Ding {
                to: "Grace".into(),
                kind: "airhorn".into(),
            }
        );
        assert_eq!(
            parse_line("join 127.0.0.1:3000 Design"),
            Action::JoinAddr {
                host: "127.0.0.1".into(),
                port: 3000,
                team: "design".into(),
            }
        );
        assert_eq!(
            parse_line("join Grace"),
            Action::Join {
                target: "Grace".into()
            }
        );
        assert!(matches!(parse_line("nope"), Action::Error(_)));
        assert!(matches!(parse_line("create"), Action::Error(_)));
        assert!(matches!(parse_line("ding"), Action::Error(_)));
    }
}
