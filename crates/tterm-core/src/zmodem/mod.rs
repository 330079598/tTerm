//! The I/O-free half of ZMODEM (`rz`/`sz`): the transfer engine, its frames
//! and the events the frontend hears about.

pub mod events;
pub mod protocol;
pub mod wire;
