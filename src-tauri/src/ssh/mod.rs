//! The Tauri SSH commands; the connection itself lives in
//! [`tterm_core::ssh`].

pub(crate) mod auth_commands;
pub(crate) mod key_vault;
pub mod known_host_commands;
pub mod secret_commands;
pub(crate) mod typed_passwords_commands;
