//! What the prototype does with the model's intents (terminal-design.md 3.3).
//!
//! Replies are built on the parser thread in intent order and go to the same
//! ordered write channel as user input, so two answers can never swap places.

use std::sync::mpsc::Sender;

use tterm_terminal::color::Rgb;
use tterm_terminal::event::Intent;
use tterm_terminal::palette::TerminalPalette;

use super::pipeline::{SessionState, WriteMessage};

/// The window title before a program sets one, and after it resets it.
pub(super) const DEFAULT_TITLE: &str = "tTerm GPUI Dev";

/// Answers or records one intent.
///
/// The prototype never changes the view's palette, so color queries are
/// answered from the default palette rather than a copy the view keeps in
/// sync. Clipboard reads get no answer and clipboard stores and the bell are
/// ignored; P2 decides how the product handles them.
pub(super) fn handle_intent(intent: Intent, state: &SessionState, writes: &Sender<WriteMessage>) {
    match intent {
        Intent::WriteToPty(bytes) => send(writes, bytes),
        Intent::ColorRequest { index, format } => {
            if let Some(color) = palette_color(&TerminalPalette::default(), index) {
                send(writes, format.format(color));
            }
        }
        Intent::WindowSizeRequest { format } => {
            if let Ok(size) = state.size.lock() {
                send(writes, format.format(*size));
            }
        }
        Intent::Title(title) => set_title(state, title),
        Intent::TitleReset => set_title(state, DEFAULT_TITLE.to_string()),
        // The view reads the cursor's blinking from each snapshot.
        Intent::CursorBlinkingChanged
        | Intent::ClipboardRead { .. }
        | Intent::ClipboardStore { .. }
        | Intent::Bell => {}
    }
}

fn send(writes: &Sender<WriteMessage>, bytes: Vec<u8>) {
    // The writer is gone only while the session closes.
    let _ = writes.send(WriteMessage::Bytes(bytes));
}

/// Keeps only the latest title, so a program that sets many titles quickly
/// leaves one for the window to show. An empty title shows the default one,
/// since a window without a title cannot be told apart in the task bar.
fn set_title(state: &SessionState, title: String) {
    let title = if title.trim().is_empty() {
        DEFAULT_TITLE.to_string()
    } else {
        title
    };
    if let Ok(mut slot) = state.title.lock() {
        *slot = Some(title);
    }
}

/// The color a query asks for: 0 to 255 are the indexed colors, then
/// Alacritty numbers the foreground, background and cursor 256, 257 and 258.
/// The dim and bright variants above that are not queried by OSC 4, 10, 11
/// or 12, so they get no answer.
fn palette_color(palette: &TerminalPalette, index: usize) -> Option<Rgb> {
    let color = match index {
        0..=255 => palette.indexed(u8::try_from(index).ok()?),
        256 => palette.foreground,
        257 => palette.background,
        258 => palette.cursor,
        _ => return None,
    };
    Some(Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    })
}
