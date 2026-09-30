pub mod agent;
pub mod auth;
mod client;
pub mod jump;
pub mod known_host_commands;
pub(crate) mod key_file;
pub mod secret_commands;
pub(crate) mod secret_store;
pub(crate) mod store;
#[cfg(all(test, unix))]
pub(crate) mod test_sshd;
mod types;

pub use auth::{AuthPromptMap, AuthPrompter};
pub use client::{measure_ssh_latency, run_single_ssh_connection, SshExitSignal};
pub use jump::{open_target_ssh_session, open_target_ssh_session_with_forwarding, JumpChain};
pub use secret_store::{SecretLocation, SecretStoreState};
pub use store::{load_legacy_password_store, now_unix_ms, remove_legacy_password_store};
pub use types::{
    emit_connection_progress, ConnectionStatusOptions, ForwardedTcpIp, HostKeyVerificationMode,
    SshClientHandler, SshConnectError, SshConnectionProgressPayload,
};
