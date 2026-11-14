//! The SFTP drawer's listing and navigation models, from
//! `src/components/SftpDrawer/sftpView.ts` and `sftpHistory.ts`.
//!
//! The WebView still owns the `localStorage` read and write of the view
//! preferences; [`sort::normalize_sftp_view_preferences`] validates the stored
//! text.

pub mod format;
pub mod history;
pub mod paths;
pub mod sort;
#[cfg(test)]
mod tests;
