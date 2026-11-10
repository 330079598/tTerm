//! Domain services both clients call: SSH sessions, authentication, SFTP,
//! tunnels, storage, backup and sync.
//!
//! The code lives in `src-tauri/src` today and moves here in P1, so this crate
//! is empty until then.

/// Typed events and delivery interfaces shared by client adapters.
pub mod events;
/// Application identity supplied by the client.
pub mod identity;
/// Keyboard shortcut parsing, display, conflicts and dispatch.
pub mod keymap;
/// Shared launch forwarding entry point.
pub mod launch;
/// Application directories resolved by the client at startup.
pub mod paths;
/// Update routing and cache policy.
pub mod updates;
