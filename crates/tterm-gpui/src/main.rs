//! The tTerm GPUI client.
//!
//! For now it is the P0-14 prototype: one window running the local shell in
//! a real PTY. See [`tterm_gpui::terminal_demo`].

fn main() {
    tterm_gpui::terminal_demo::run();
}
