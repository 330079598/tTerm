//! The tTerm GPUI client.
//!
//! For now it is a runtime prototype: one window using the shared
//! `AppServices` for a local or saved SSH session. See
//! [`tterm_gpui::terminal_demo`].

fn main() {
    tterm_gpui::terminal_demo::run();
}
