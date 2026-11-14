//! Domain services both clients call: SSH sessions, authentication, SFTP,
//! tunnels, storage, backup and sync.
//!
//! The code lives in `src-tauri/src` today and moves here in P1, so this crate
//! is empty until then.

/// Hooks and plugins installed into AI coding agents so they report what they do.
pub mod agent_integration;
/// Agent status reports and the tracker that follows them.
pub mod agent_status;
/// The state the app keeps for itself rather than as user settings.
pub mod app_state;
/// Notifications and agent reports carried by terminal OSC sequences.
pub mod attention_reports;
/// Backup: the WebDAV client a remote backup drives.
pub mod backup;
/// The saved command library every client shares.
pub mod command_library;
/// Command lines, timing and shell-integration marks for one terminal.
pub mod command_tracking;
/// Configuration values both clients keep: the config directory, keymap
/// overrides, keyword highlight sets, split pane layouts and tab icons.
pub mod config;
/// Cross-cutting services shared by the clients, such as the blocking worker.
pub mod core;
/// Follows local and remote shell directory reports.
pub mod cwd_reports;
/// The app's SQLite database and the stores that keep their queries in it.
pub mod db;
/// Distribution identity for the server monitor's icons.
pub mod distro;
/// Typed events and delivery interfaces shared by client adapters.
pub mod events;
/// The fonts installed on this computer, and their families.
pub mod fonts;
/// Application identity supplied by the client.
pub mod identity;
/// Keyboard shortcut parsing, display, conflicts and dispatch.
pub mod keymap;
/// Keyword highlight configuration and editing.
pub mod keyword_highlight;
/// Shared launch forwarding entry point.
pub mod launch;
/// Directories handed to a running tTerm by a second launch.
pub mod launch_dir;
/// Types a login script into a shell one command at a time.
pub mod login_script;
/// Local and remote system monitoring: the metric collectors and snapshots.
pub mod monitor;
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
/// Prompt lifetimes shared by host keys, authentication and proxies.
pub mod prompt_scope;
/// One-shot interactive requests with scoped cancellation.
pub mod prompts;
/// Proxies an SSH connection reaches its first host through.
pub mod proxy;
/// Per-tab directories and hosts reported by remote shells.
pub mod remote_cwd;
/// Saved-password offers at SSH sudo prompts.
pub mod saved_password_prompts;
/// Serial connections: their settings, ports and line controls.
pub mod serial;
/// The services both clients build once and hand to their commands.
pub mod services;
/// Session output endpoints shared by client adapters (P1-06).
pub mod session;
/// Terminal session logs and the screen they replay.
pub mod session_log;
/// The tabs and layout to restore on the next launch.
pub mod session_restore;
/// Shared SFTP value and streaming types (P1-04).
pub mod sftp;
/// SSH connections, their authentication and the stores behind them.
pub mod ssh;
/// Merging the records two devices wrote during sync.
pub mod sync;
/// Installing the shell-integration helpers on this computer.
pub mod system_integration;
/// Local and remote terminal sessions and their output.
pub mod terminal;
/// Notification requests and the rules for announcing them.
pub mod terminal_notifications;
/// Theme definitions, the bundled catalog, imports and custom themes.
pub mod theme;
/// The custom themes the user saved as rows in the database.
pub mod themes;
/// SSH tunnel rules, forwarding specs and the SOCKS5 handshake.
pub mod tunnel;
/// The pending update downloads and the services that drive them.
pub mod updater;
/// Update routing and cache policy.
pub mod updates;
/// Saved window geometry and the monitors it fits.
pub mod window_state;
/// Workspace panes, tab navigation and tab menu models.
pub mod workspace;
/// The I/O-free half of the ZMODEM file transfer.
pub mod zmodem;
