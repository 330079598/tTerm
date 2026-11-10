//! Domain services both clients call: SSH sessions, authentication, SFTP,
//! tunnels, storage, backup and sync.
//!
//! The code lives in `src-tauri/src` today and moves here in P1, so this crate
//! is empty until then.

/// Typed events and delivery interfaces shared by client adapters.
pub mod events;
/// Keyboard shortcut parsing, display, conflicts and dispatch.
pub mod keymap;
/// Application directories resolved by the client at startup.
pub mod paths;

/// Shared SFTP value and streaming types (P1-04).
pub mod sftp;
