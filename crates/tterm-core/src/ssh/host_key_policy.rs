//! The process-wide host-key policy for SSH connections.
//!
//! Production connections use `PromptAndPersist`. A disposable field run can
//! install a stricter policy before it starts any connection, and the first
//! install wins so a later caller cannot weaken it.

use std::sync::OnceLock;

use crate::ssh::types::HostKeyVerificationMode;

static MODE: OnceLock<HostKeyVerificationMode> = OnceLock::new();

/// The mode new connections use for this process.
pub fn current() -> HostKeyVerificationMode {
    MODE.get()
        .copied()
        .unwrap_or(HostKeyVerificationMode::PromptAndPersist)
}

/// Requires every connection in this process to match an application-store
/// pin before the first connection starts.
///
/// This exists for debug field runs that must fail closed instead of asking
/// about an unknown or changed key. It deliberately takes no mode, so a caller
/// can only strengthen the default `PromptAndPersist` policy. Release builds
/// never call it.
#[cfg(debug_assertions)]
pub fn require_pinned() -> Result<(), String> {
    MODE.set(HostKeyVerificationMode::RequirePinnedHostKey)
        .map_err(|_| "The SSH host-key policy is already set for this process.".to_string())
}
