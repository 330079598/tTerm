//! The Tauri SFTP API surface; the connection itself lives in
//! [`tterm_core::sftp::internal`].

pub mod api;
#[cfg(test)]
pub(crate) mod test_support;
