//! Connection profiles: the saved hosts, their groups and the `~/.ssh/config`
//! import that reads them.

pub mod connection_check;
pub mod group_service;
pub mod groups;
pub mod import;
pub mod parser;
pub mod service;
pub mod storage;
pub mod types;
