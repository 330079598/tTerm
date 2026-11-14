//! The SFTP connection, its transfers and the commands that drive them,
//! as far as the GPUI client shares them.

pub mod api;
pub mod paths;
pub mod transfer;

pub mod connection;
pub mod exec;
pub mod launcher;
pub mod sudo;
pub mod types;
