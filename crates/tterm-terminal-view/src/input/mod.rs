//! Keyboard and mouse encoders for the terminal view.
//!
//! Each encoder is a pure function over the terminal mode bits and a GPUI
//! event, so it can be tested without a window. The view calls the encoder
//! first and only then writes the returned bytes to the session.

pub mod focus;
pub mod keyboard;
pub mod mouse;
pub mod wheel;
