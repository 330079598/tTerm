//! The tTerm GPUI client.
//!
//! P0-08 opens one window to prove the pinned `gpui-kit` facade builds, runs
//! and can be tested headlessly before the terminal work starts in P0-09.

#[cfg(test)]
mod app;
mod terminal_demo;

fn main() {
    terminal_demo::run();
}
