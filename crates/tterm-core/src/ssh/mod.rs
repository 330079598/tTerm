//! SSH: the connection, its authentication and the stores behind them.

pub mod agent;
pub mod auth;
pub mod client;
pub mod context;
pub mod host_key_policy;
pub mod host_key_prompt;
pub mod jump;
pub mod key_file;
pub mod key_vault;
pub mod openssh_known_hosts;
pub mod remote_os;
pub mod rsa_signer;
pub mod secret_store;
pub mod shell_integration;
pub mod store;
// Only the crate's own unit tests use this ephemeral server. Keeping it out
// of `test-support` builds avoids compiling, and warning about, helpers that
// no other crate calls.
#[cfg(all(test, unix))]
pub mod test_sshd;
pub mod typed_passwords;
pub mod types;
