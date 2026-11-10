//! Shared client pieces: the team/LAN state, the terminal UI, and the window UI.

pub mod avatar;
mod cli;
mod command;
mod discovery;
mod model;
pub mod prefs;
mod session;
mod sound;

pub use avatar::Avatar;
pub use model::{Command, Note, PeerRow, Session, Snapshot, TeamSnap};
pub use prefs::Prefs;
pub use sound::play;

pub async fn cli_main() -> std::process::ExitCode {
    cli::cli_main().await
}
