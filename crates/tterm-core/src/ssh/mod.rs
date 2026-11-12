//! SSH: the connection, its authentication and the stores behind them.

pub mod openssh_known_hosts;
pub mod rsa_signer;
pub mod secret_store;
pub mod store;
pub mod typed_passwords;
