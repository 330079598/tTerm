//! Domain services both clients call: SSH sessions, authentication, SFTP,
//! tunnels, storage, backup and sync.
//!
//! The code lives in `src-tauri/src` today and moves here in P1, so this crate
//! is empty until then.

/// Agent status reports and the tracker that follows them.
pub mod agent_status;
/// Distribution identity for the server monitor's icons.
pub mod distro;
/// Typed events and delivery interfaces shared by client adapters.
pub mod events;
/// Application identity supplied by the client.
pub mod identity;
/// Keyboard shortcut parsing, display, conflicts and dispatch.
pub mod keymap;
/// Keyword highlight configuration and editing.
pub mod keyword_highlight;
/// Shared launch forwarding entry point.
pub mod launch;
/// Types a login script into a shell one command at a time.
pub mod login_script;
/// The number of characters a password prompt holds.
pub mod password_feedback;
/// Rules for confirming a multi-line paste.
pub mod paste_guard;
/// Application directories resolved by the client at startup.
pub mod paths;

/// One-shot interactive requests with scoped cancellation.
pub mod prompts;
/// Per-tab directories and hosts reported by remote shells.
pub mod remote_cwd;
/// Session output endpoints shared by client adapters (P1-06).
pub mod session;
/// Shared SFTP value and streaming types (P1-04).
pub mod sftp;
/// Notification requests and the rules for announcing them.
pub mod terminal_notifications;
/// Theme definitions, the bundled catalog, imports and custom themes.
pub mod theme;
/// Update routing and cache policy.
pub mod updates;
/// Workspace panes, tab navigation and tab menu models.
pub mod workspace;
