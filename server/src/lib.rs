//! In-memory team rooms and the WebSocket server that exposes them.
//!
//! The client embeds this same server when someone creates a team on their own machine.
//! `ding-server` is the same code, listening on a fixed port for people who join by address.

mod hub;
mod net;

pub use net::{serve, App, LocalEvent};
