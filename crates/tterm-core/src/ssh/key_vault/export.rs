//! Writing a key from the vault back out as key files.

use super::entry::VaultKey;
use crate::config::atomic::{atomic_write, atomic_write_private};
use std::path::{Path, PathBuf};

/// Writes the private key to `path`, readable only by its owner as `ssh`
/// insists, and its certificate, if it has one, beside it as
/// `<path>-cert.pub`, where `ssh` looks for it.
pub(super) fn write_key_files(path: &Path, key: &VaultKey) -> Result<(), String> {
    atomic_write_private(path, key.private_key.as_bytes())?;
    if let Some(certificate) = key.certificate.as_deref() {
        atomic_write(
            &certificate_path(path),
            format!("{}\n", certificate.trim_end()),
        )?;
    }
    Ok(())
}

pub(super) fn certificate_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push("-cert.pub");
    PathBuf::from(name)
}
