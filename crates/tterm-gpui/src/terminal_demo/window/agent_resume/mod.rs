//! Resuming the AI agent session a restored tab was running.
//!
//! The capture half follows every session and keeps the tab's saved resume
//! record current. The restore half waits for the new shell's prompt and
//! sends the command the core builder approves. Saved text is never sent on
//! its own.

pub(super) mod controller;
pub(super) mod state;

#[cfg(test)]
mod tests;
