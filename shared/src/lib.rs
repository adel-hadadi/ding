//! Messages shared by the Ding server and client.
//!
//! Each WebSocket text frame is one JSON object. The `"type"` field picks the variant:
//!
//! ```json
//! {"type":"join","team":"design","name":"Ada"}
//! {"type":"presence","users":["Ada","Grace"]}
//! {"type":"ding","to":"Grace","kind":"bell"}
//! {"type":"ding","from":"Ada","kind":"bell"}
//! {"type":"delivered","to":"Grace"}
//! {"type":"direct_ding","from":"Ada","kind":"bell"}
//! {"type":"error","message":"that person is not in this team"}
//! ```
//!
//! `ding` looks different in each direction on purpose. A client says who the ding is *to*.
//! The server fills in who it is *from*, because it already knows which socket joined.

use serde::{Deserialize, Serialize};

pub const MAX_NAME_CHARS: usize = 24;
pub const MAX_TEAM_CHARS: usize = 32;
pub const MAX_KIND_CHARS: usize = 16;

/// What a client sends to a server (a team hub, or one peer for a direct ding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Enter a team room. The team code is the room name, and `name` is what others see.
    Join { team: String, name: String },
    /// Ask the hub to pass a ding to someone in the same room.
    Ding { to: String, kind: String },
    /// A one-off ding straight to another person's app, with no room involved.
    /// `from` is our display name, because that other app has no join session for us.
    DirectDing { from: String, kind: String },
    /// Part of the schema now. The server rejects it until Do Not Disturb exists.
    SetDnd { enabled: bool },
}

/// What a server sends back to a client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Everyone currently in the room, including you. Names are sorted.
    Presence {
        users: Vec<String>,
    },
    /// Someone dinged you.
    Ding {
        from: String,
        kind: String,
    },
    /// The ding was handed to that person's open socket.
    Delivered {
        to: String,
    },
    Error {
        message: String,
    },
}

/// Trim and check a display name. The original casing is kept.
pub fn normalize_name(raw: &str) -> Result<String, &'static str> {
    normalize_label(raw, MAX_NAME_CHARS, "name")
}

/// Trim and check a team code. Comparison is case-insensitive, so this returns lowercase.
pub fn normalize_team(raw: &str) -> Result<String, &'static str> {
    Ok(normalize_label(raw, MAX_TEAM_CHARS, "team code")?.to_lowercase())
}

/// Trim and check a ding kind. `"bell"` is the usual one; anything short is forwarded as-is.
pub fn normalize_kind(raw: &str) -> Result<String, &'static str> {
    normalize_label(raw, MAX_KIND_CHARS, "kind")
}

/// Key used to compare names. `Ada` and `ada` are the same person in a room.
pub fn name_key(name: &str) -> String {
    name.to_lowercase()
}

fn normalize_label(
    raw: &str,
    max_chars: usize,
    label: &'static str,
) -> Result<String, &'static str> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(match label {
            "name" => "name cannot be empty",
            "team code" => "team code cannot be empty",
            _ => "kind cannot be empty",
        });
    }
    if value.chars().count() > max_chars {
        return Err(match label {
            "name" => "name is too long",
            "team code" => "team code is too long",
            _ => "kind is too long",
        });
    }
    if value.chars().any(|ch| ch.is_control()) {
        return Err(match label {
            "name" => "name cannot contain control characters",
            "team code" => "team code cannot contain control characters",
            _ => "kind cannot contain control characters",
        });
    }
    if value.chars().any(|ch| ch.is_whitespace()) {
        return Err(match label {
            "name" => "name cannot contain spaces",
            "team code" => "team code cannot contain spaces",
            _ => "kind cannot contain spaces",
        });
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip_as_tagged_json() {
        let messages = [
            ClientMessage::Join {
                team: "design".into(),
                name: "Ada".into(),
            },
            ClientMessage::Ding {
                to: "Grace".into(),
                kind: "bell".into(),
            },
            ClientMessage::DirectDing {
                from: "Ada".into(),
                kind: "🎉".into(),
            },
            ClientMessage::SetDnd { enabled: true },
        ];
        for message in messages {
            let json = serde_json::to_string(&message).unwrap();
            let back: ClientMessage = serde_json::from_str(&json).unwrap();
            assert_eq!(back, message);
        }

        let presence = ServerMessage::Presence {
            users: vec!["Ada".into(), "Grace".into()],
        };
        let json = serde_json::to_value(&presence).unwrap();
        assert_eq!(json["type"], "presence");
        let back: ServerMessage = serde_json::from_value(json).unwrap();
        assert_eq!(back, presence);

        assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"nope"}"#).is_err());
    }

    #[test]
    fn names_keep_case_but_team_codes_do_not() {
        assert_eq!(normalize_name("  Ada  ").unwrap(), "Ada");
        assert_eq!(normalize_team("  Design  ").unwrap(), "design");
        assert_eq!(name_key("Ada"), name_key("ada"));
        assert!(normalize_name("a".repeat(MAX_NAME_CHARS).as_str()).is_ok());
        assert!(normalize_name("a".repeat(MAX_NAME_CHARS + 1).as_str()).is_err());
        assert!(normalize_name("has space").is_err());
        assert!(normalize_kind("🎉").is_ok());
        assert!(normalize_kind("").is_err());
    }
}
