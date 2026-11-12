//! Domain services both clients call: SSH sessions, authentication, SFTP,
//! tunnels, storage, backup and sync.
//!
//! The code lives in `src-tauri/src` today and moves here in P1, so this crate
//! is empty until then.

/// Agent status reports and the tracker that follows them.
pub mod agent_status;
/// Notifications and agent reports carried by terminal OSC sequences.
pub mod attention_reports;
/// The saved command library every client shares.
pub mod command_library;
/// Command lines, timing and shell-integration marks for one terminal.
pub mod command_tracking;
/// Configuration values both clients keep: the config directory, keymap
/// overrides, keyword highlight sets, split pane layouts and tab icons.
pub mod config;
/// Follows local and remote shell directory reports.
pub mod cwd_reports;
/// The app's SQLite database and the stores that keep their queries in it.
pub mod db;
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
/// Distribution reports from a local WSL launcher.
pub mod os_reports;
/// The number of characters a password prompt holds.
pub mod password_feedback;
/// Rules for confirming a multi-line paste.
pub mod paste_guard;
/// Application directories resolved by the client at startup.
pub mod paths;

/// Saved connection profiles and the OpenSSH config import.
pub mod profiles;
/// One-shot interactive requests with scoped cancellation.
pub mod prompts;
/// Per-tab directories and hosts reported by remote shells.
pub mod remote_cwd;
/// Saved-password offers at SSH sudo prompts.
pub mod saved_password_prompts;
/// Session output endpoints shared by client adapters (P1-06).
pub mod session;
/// Shared SFTP value and streaming types (P1-04).
pub mod sftp;
/// SSH connections, their authentication and the stores behind them.
pub mod ssh;
/// Notification requests and the rules for announcing them.
pub mod terminal_notifications;
/// Theme definitions, the bundled catalog, imports and custom themes.
pub mod theme;
/// Update routing and cache policy.
pub mod updates;
/// Workspace panes, tab navigation and tab menu models.
pub mod workspace;
