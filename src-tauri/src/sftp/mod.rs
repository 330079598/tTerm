mod store;
pub mod internal {
    pub mod api;
    pub mod connection;
    pub mod paths;
    pub mod types;
}

pub use internal::types::{SftpConnectionPool, TransferCancelMap};