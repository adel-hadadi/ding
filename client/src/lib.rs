//! Shared client pieces: the team/LAN state, the terminal UI, and the window UI.

mod cli;
mod command;
mod discovery;
mod model;
mod session;
mod sound;

pub use model::{Command, Note, Session, Snapshot, TeamSnap};
pub use sound::play;

pub async fn cli_main() -> std::process::ExitCode {
    cli::cli_main().await
}
