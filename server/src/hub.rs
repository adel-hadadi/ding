//! Who is in each team, and which messages a ding should produce.
//!
//! `Hub` is ordinary data. It does not own sockets. The WebSocket layer in `net` delivers
//! the [`Outgoing`] messages it returns. Keeping the network out of here is why the tests
//! can call `join` and `ding` like normal functions.

use std::collections::HashMap;

use ding_shared::{name_key, normalize_kind, normalize_name, normalize_team, ServerMessage};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HubError {
    Invalid(&'static str),
    NameTaken,
    UnknownUser,
    SelfDing,
    NotInRoom,
}

impl HubError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            HubError::Invalid(message) => message,
            HubError::NameTaken => "that name is already in this team",
            HubError::UnknownUser => "that person is not in this team",
            HubError::SelfDing => "you can't ding yourself",
            HubError::NotInRoom => "join a team before sending a ding",
        }
    }
}

/// One message the socket layer should write to one member.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Outgoing {
    pub(crate) team: String,
    pub(crate) name_key: String,
    pub(crate) msg: ServerMessage,
}

#[derive(Debug)]
pub(crate) struct JoinOutcome {
    pub(crate) team: String,
    pub(crate) name: String,
    pub(crate) name_key: String,
    pub(crate) outgoing: Vec<Outgoing>,
}

#[derive(Default)]
struct Member {
    name: String,
}

#[derive(Default)]
struct Room {
    members: HashMap<String, Member>,
}

impl Room {
    fn display_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .members
            .values()
            .map(|member| member.name.clone())
            .collect();
        names.sort_by_key(|name| name_key(name));
        names
    }
}

#[derive(Default)]
pub(crate) struct Hub {
    rooms: HashMap<String, Room>,
}

impl Hub {
    pub(crate) fn join(&mut self, team: &str, name: &str) -> Result<JoinOutcome, HubError> {
        let team = normalize_team(team).map_err(HubError::Invalid)?;
        let name = normalize_name(name).map_err(HubError::Invalid)?;
        let key = name_key(&name);

        let room = self.rooms.entry(team.clone()).or_default();
        if room.members.contains_key(&key) {
            return Err(HubError::NameTaken);
        }
        room.members
            .insert(key.clone(), Member { name: name.clone() });
        let outgoing = presence_effects(room, &team);

        Ok(JoinOutcome {
            team,
            name,
            name_key: key,
            outgoing,
        })
    }

    /// Remove someone. An unknown person, or an unknown team, is a no-op so a
    /// disconnect that races with a kick is harmless.
    pub(crate) fn leave(&mut self, team: &str, key: &str) -> Vec<Outgoing> {
        // End the mutable borrow before we either delete the room or read it again.
        let removed = {
            let Some(room) = self.rooms.get_mut(team) else {
                return Vec::new();
            };
            room.members.remove(key).is_some()
        };
        if !removed {
            return Vec::new();
        }
        let empty = self
            .rooms
            .get(team)
            .is_some_and(|room| room.members.is_empty());
        if empty {
            self.rooms.remove(team);
            return Vec::new();
        }
        presence_effects(self.rooms.get(team).expect("room still has members"), team)
    }

    /// Drop a whole room and return the member keys that were in it.
    pub(crate) fn take_room(&mut self, team: &str) -> Vec<String> {
        self.rooms
            .remove(team)
            .map(|room| room.members.into_keys().collect())
            .unwrap_or_default()
    }

    pub(crate) fn ding(
        &mut self,
        team: &str,
        from: &str,
        to: &str,
        kind: &str,
    ) -> Result<Vec<Outgoing>, HubError> {
        let team = normalize_team(team).map_err(HubError::Invalid)?;
        let from_name = normalize_name(from).map_err(HubError::Invalid)?;
        let to_name = normalize_name(to).map_err(HubError::Invalid)?;
        let kind = normalize_kind(kind).map_err(HubError::Invalid)?;
        let from_key = name_key(&from_name);
        let to_key = name_key(&to_name);

        let room = self.rooms.get(&team).ok_or(HubError::NotInRoom)?;
        let sender = room.members.get(&from_key).ok_or(HubError::NotInRoom)?;
        if from_key == to_key {
            return Err(HubError::SelfDing);
        }
        let target = room.members.get(&to_key).ok_or(HubError::UnknownUser)?;
        let from_display = sender.name.clone();
        let to_display = target.name.clone();

        Ok(vec![
            Outgoing {
                team: team.clone(),
                name_key: to_key,
                msg: ServerMessage::Ding {
                    from: from_display,
                    kind,
                },
            },
            Outgoing {
                team,
                name_key: from_key,
                msg: ServerMessage::Delivered { to: to_display },
            },
        ])
    }
}

fn presence_effects(room: &Room, team: &str) -> Vec<Outgoing> {
    let users = room.display_names();
    room.members
        .keys()
        .map(|key| Outgoing {
            team: team.to_string(),
            name_key: key.clone(),
            msg: ServerMessage::Presence {
                users: users.clone(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(users: &[&str]) -> ServerMessage {
        ServerMessage::Presence {
            users: users.iter().map(|user| (*user).to_string()).collect(),
        }
    }

    #[test]
    fn join_broadcasts_sorted_presence() {
        let mut hub = Hub::default();
        let ada = hub.join("Design", "Ada").unwrap();
        assert_eq!(ada.team, "design");
        assert_eq!(ada.name, "Ada");
        assert_eq!(ada.outgoing.len(), 1);
        assert_eq!(ada.outgoing[0].msg, presence(&["Ada"]));

        let both = hub.join("design", "Grace").unwrap();
        assert_eq!(both.outgoing.len(), 2);
        assert!(both
            .outgoing
            .iter()
            .all(|item| item.msg == presence(&["Ada", "Grace"])));
        let mut keys: Vec<_> = both
            .outgoing
            .iter()
            .map(|item| item.name_key.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["ada", "grace"]);
    }

    #[test]
    fn duplicate_names_are_rejected_without_changing_the_room() {
        let mut hub = Hub::default();
        hub.join("design", "Ada").unwrap();
        assert_eq!(hub.join("design", "ada").unwrap_err(), HubError::NameTaken);
        assert_eq!(
            hub.join("design", " ADA ").unwrap_err(),
            HubError::NameTaken
        );

        let room = hub.join("design", "Grace").unwrap();
        assert!(room
            .outgoing
            .iter()
            .all(|item| item.msg == presence(&["Ada", "Grace"])));
    }

    #[test]
    fn leave_updates_presence_and_drops_an_empty_room() {
        let mut hub = Hub::default();
        hub.join("design", "Ada").unwrap();
        hub.join("design", "Grace").unwrap();

        let left = hub.leave("design", "ada");
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].name_key, "grace");
        assert_eq!(left[0].msg, presence(&["Grace"]));

        assert!(hub.leave("design", "grace").is_empty());
        assert!(hub.rooms.is_empty());
        // Leaving twice, or leaving a team that is already gone, does nothing.
        assert!(hub.leave("design", "grace").is_empty());
    }

    #[test]
    fn a_name_can_be_used_again_after_that_person_leaves() {
        let mut hub = Hub::default();
        hub.join("design", "Ada").unwrap();
        hub.leave("design", "ada");
        let rejoined = hub.join("design", "Ada").unwrap();
        assert_eq!(rejoined.outgoing[0].msg, presence(&["Ada"]));
    }

    #[test]
    fn teams_do_not_share_members() {
        let mut hub = Hub::default();
        hub.join("design", "Ada").unwrap();
        hub.join("lunch", "Grace").unwrap();

        let err = hub.ding("design", "Ada", "Grace", "bell").unwrap_err();
        assert_eq!(err, HubError::UnknownUser);
        assert_eq!(hub.rooms.len(), 2);
    }

    #[test]
    fn ding_tells_the_target_and_confirms_the_sender() {
        let mut hub = Hub::default();
        hub.join("design", "Ada").unwrap();
        hub.join("design", "Grace").unwrap();

        let effects = hub.ding("Design", "ada", "GRACE", "bell").unwrap();
        assert!(effects.iter().any(|item| {
            item.name_key == "grace"
                && item.msg
                    == ServerMessage::Ding {
                        from: "Ada".into(),
                        kind: "bell".into(),
                    }
        }));
        assert!(effects.iter().any(|item| {
            item.name_key == "ada" && item.msg == ServerMessage::Delivered { to: "Grace".into() }
        }));
    }

    #[test]
    fn ding_rejects_unknown_self_and_missing_sender() {
        let mut hub = Hub::default();
        assert_eq!(
            hub.ding("design", "Ada", "Grace", "bell").unwrap_err(),
            HubError::NotInRoom
        );

        hub.join("design", "Ada").unwrap();
        assert_eq!(
            hub.ding("design", "Ada", "Ada", "bell").unwrap_err(),
            HubError::SelfDing
        );
        assert_eq!(
            hub.ding("design", "Ada", "Grace", "bell").unwrap_err(),
            HubError::UnknownUser
        );
        assert_eq!(
            hub.ding("design", "Ada", "Grace", "  ").unwrap_err(),
            HubError::Invalid("kind cannot be empty")
        );
        let emoji = hub.ding("design", "Ada", "Ada", "🎉");
        // Self-ding is still rejected, but only after the kind has been accepted.
        // A legal kind to someone else is covered below.
        assert_eq!(emoji.unwrap_err(), HubError::SelfDing);

        hub.join("design", "Grace").unwrap();
        assert!(hub.ding("design", "Ada", "Grace", "🎉").is_ok());
    }

    #[test]
    fn blank_or_huge_names_do_not_create_a_room() {
        let mut hub = Hub::default();
        assert!(hub.join("design", " ").is_err());
        assert!(hub.join("design", "has space").is_err());
        assert!(hub.join(" ", "Ada").is_err());
        assert!(hub.join("design", &"a".repeat(25)).is_err());
        assert!(hub.rooms.is_empty());
    }
}
