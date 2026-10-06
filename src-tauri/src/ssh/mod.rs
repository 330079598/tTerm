pub mod agent;
pub mod auth;
mod client;
pub mod jump;
pub(crate) mod key_file;
pub mod known_host_commands;
mod rsa_signer;
pub mod secret_commands;
pub(crate) mod secret_store;
pub(crate) mod shell_integration;
pub(crate) mod store;
#[cfg(all(test, unix))]
pub(crate) mod test_sshd;
pub mod typed_passwords;
mod types;

pub use auth::{AuthMethod, AuthPromptMap, AuthPrompter};
pub use client::{measure_ssh_latency, run_single_ssh_connection, SshExitSignal};
pub use jump::{open_target_ssh_session, open_target_ssh_session_with_forwarding, JumpChain};
pub(crate) use rsa_signer::pem_key_file_is_usable;
pub use secret_store::{SecretLocation, SecretStoreState, VerificationPurpose};
pub use store::{load_legacy_password_store, now_unix_ms, remove_legacy_password_store};
pub use types::{
    emit_connection_progress, ConnectionStatusOptions, ForwardedTcpIp, HostKeyVerificationMode,
    SshClientHandler, SshConnectError, SshConnectionProgressPayload,
};
