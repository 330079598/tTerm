//! The terminal model the clients share: the screen, the scrollback history,
//! the selection, search, and the escape-sequence answers a session has to
//! send back.
//!
//! It depends on neither Tauri nor GPUI; the GPUI element that draws it lives
//! in its own crate, `tterm-terminal-view`. Parsing, the grid, the scrollback
//! buffer, the selection and the search matcher all come from the official
//! `alacritty_terminal` 0.26.0 crate the workspace pins since P0-06. This crate
//! owns the state behind one fair mutex and hands the rest of tTerm a view of
//! it that never mentions Alacritty's types.
//!
//! # Layers
//!
//! - [`terminal::Terminal`] owns the byte parser and Alacritty's terminal
//!   behind one lock. UTF-8 bytes go in from the session's parse thread; the
//!   model never converts other character sets, because that happens in core
//!   before the bytes get here (design note 1.2).
//! - [`snapshot::Snapshot`] is one immutable frame of the visible area: rows,
//!   cursor, modes, display offset, selection, and the rows that changed since
//!   the previous frame. It is copied under the lock and read after the lock
//!   is released, so drawing never waits for the parser.
//! - [`event::Intent`] carries everything only the application can do: set a
//!   title, ring a bell, read or write a clipboard, answer a query through the
//!   same ordered channel as user input. The model holds no PTY writer.
//!
//! # Reading a frame
//!
//! ```text
//! parse thread:  terminal.write(bytes);                 // bytes from the session
//!                terminal.drain_intents();              // title, bell, replies, ...
//! view thread:   let frame = terminal.snapshot();       // short lock, visible rows only
//!                ... draw from `frame` without the lock
//! ```
//!
//! The snapshot copies exactly the visible cells: a session with a hundred
//! thousand scrollback lines costs the same per frame as one with a thousand.
//! The scrollback is only touched when the caller scrolls back, in which case
//! the visible window simply moves over it.
//!
//! # What is deliberately missing
//!
//! Fonts, layout, colors resolved against a theme, drawing, keyboard and mouse
//! encoding, input methods, character-set conversion, PTY and SSH. Those live
//! in the view crate, `tterm-platform`, `tterm-core` or P0-28's input module,
//! depending on which side of the session they belong to.
//!
//! # Tests
//!
//! The behaviour tests live in `tests.rs`. Alongside them is a seeded
//! random-input test: a hundred thousand groups of hostile bytes (truncated
//! escape sequences, overlong parameters, invalid UTF-8) must be absorbed
//! without a panic, and the seed is printed so a failure can be replayed.

pub mod cell;
pub mod color;
/// Shell integration command marks shared with the notification rules.
pub mod command_mark;
/// Prompt and command positions reported by shell integration.
pub mod command_marks;
/// Buffering rules for spinner cursor moves.
pub mod cursor_move_hold;
pub mod damage;
pub mod event;
pub mod grid;
pub mod hint;
/// The frontend's base64 and URI-component decoders.
pub mod js_encoding;
/// JavaScript string semantics over Rust's UTF-8 strings.
pub mod js_text;
pub mod keyword_highlight;
pub mod mode;
/// The OSC 52 clipboard request.
pub mod osc52;
pub mod palette;
/// Predicts the echo of typing at a slow remote shell.
pub mod predictive_echo;
pub mod search;
pub mod selection;
pub mod snapshot;
/// Recognizes password prompts on the cursor line.
pub mod sudo_prompt;
pub mod terminal;
/// Captures command lines from the terminal's input stream.
pub mod terminal_command_capture;
/// Working-directory reports from shell integration.
pub mod terminal_cwd;
/// Rolling scan of decoded output for the connecting status line.
pub mod terminal_output_scanner;

#[cfg(test)]
mod tests;
