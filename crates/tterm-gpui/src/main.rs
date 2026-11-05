//! The tTerm GPUI client.
//!
//! P0-08 opens one window to prove the pinned `gpui-kit` facade builds, runs
//! and can be tested headlessly before the terminal work starts in P0-09.

mod app;

fn main() {
    app::run();
}
