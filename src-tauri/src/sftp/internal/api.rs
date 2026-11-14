//! The Tauri SFTP commands; the API they call lives in
//! [`tterm_core::sftp::internal::api`].

pub mod archive_commands;
pub mod base_commands;
pub mod delete;
pub mod download_commands;
pub mod edit_commands;
pub mod external;
pub mod permissions_commands;
pub mod search_commands;
pub mod upload;
