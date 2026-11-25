//! What the prototype does with the model's intents (terminal-design.md 3.3).
//!
//! Replies are built on the parser thread in intent order and are sent through
//! `AppServices`, so they use the same ordered path as user input.

use tterm_terminal::color::Rgb;
use tterm_terminal::event::Intent;
use tterm_terminal::palette::TerminalPalette;

use super::pipeline::SessionState;

/// The window title before a program sets one, and after it resets it.
pub(super) const DEFAULT_TITLE: &str = "tTerm GPUI Dev";

/// Answers or records one intent.
///
/// The prototype never changes the view's palette, so color queries are
/// answered from the default palette rather than a copy the view keeps in
/// sync. Clipboard writes cross to the UI through the session slot. Clipboard
/// reads get no answer; the bell is reserved for the later attention work.
pub(super) fn handle_intent(intent: Intent, state: &SessionState, mut write: impl FnMut(Vec<u8>)) {
    match intent {
        Intent::WriteToPty(bytes) => write(bytes),
        Intent::ColorRequest { index, format } => {
            if let Some(color) = palette_color(&TerminalPalette::default(), index) {
                write(format.format(color));
            }
        }
        Intent::WindowSizeRequest { format } => {
            if let Ok(size) = state.size.lock() {
                write(format.format(*size));
            }
        }
        Intent::Title(title) => set_title(state, title),
        Intent::TitleReset => set_title(state, DEFAULT_TITLE.to_string()),
        // Every OSC 52 target writes the ordinary system clipboard. Reading is
        // never requested here, so no query reply exists.
        Intent::ClipboardStore { text, .. } => {
            state.clipboard.store(text);
        }
        // The parser pipeline resolves this marker before ordinary intent
        // handling; it carries no user-visible action of its own.
        Intent::OrderedReply(_) => {}
        // The view reads the cursor's blinking from each snapshot.
        Intent::CursorBlinkingChanged | Intent::ClipboardRead { .. } | Intent::Bell => {}
    }
}

/// Keeps only the latest title, so a program that sets many titles quickly
/// leaves one for the window to show. An empty title uses the default marker,
/// which the window resolves in the current language.
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
    use tterm_terminal::terminal::{Config, Terminal};

    use super::{DEFAULT_TITLE, handle_intent};
    use crate::terminal_demo::pipeline::SessionState;

    fn state() -> (Arc<Terminal>, SessionState) {
        let model = Arc::new(Terminal::new(
            Config::default(),
            GridSize {
                columns: 80,
                screen_lines: 24,
            },
            CellSize {
                width: 8,
                height: 16,
            },
        ));
        let state = SessionState {
            terminal: Arc::clone(&model),
            size: Arc::new(Mutex::new(WindowSize {
                grid: GridSize {
                    columns: 80,
                    screen_lines: 24,
                },
                cell: CellSize {
                    width: 8,
                    height: 16,
                },
            })),
            title: Arc::new(Mutex::new(None)),
            clipboard: Arc::new(
                crate::terminal_demo::clipboard::store::ClipboardWrites::disabled(),
            ),
            session_nonce: 1,
            alive: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            visible: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            refresh: Arc::new(crate::terminal_demo::pipeline::RefreshSignal::new()),
            reports: Arc::new(crate::terminal_demo::session::reports::Reports::new()),
        };
        (model, state)
    }

    #[test]
    fn query_replies_are_written_in_the_order_the_program_asked() {
        let (model, state) = state();
        model.write(b"\x1b]11;?\x07\x1b[14t");
        let mut replies = Vec::new();
        for intent in model.drain_intents() {
            handle_intent(intent, &state, |bytes| replies.push(bytes));
        }

        assert_eq!(replies.len(), 2);
        assert!(replies[0].starts_with(b"\x1b]11;rgb:"));
        assert!(replies[0].ends_with(b"\x07"));
        assert_eq!(replies[1], b"\x1b[4;384;640t");
    }

    #[test]
    fn a_title_reset_returns_the_window_to_its_default() {
        let (model, state) = state();
        model.set_scrollback(500);
        for intent in model.drain_intents() {
            handle_intent(intent, &state, |_| {});
        }

        assert_eq!(
            state.title.lock().expect("title").as_deref(),
            Some(DEFAULT_TITLE)
        );
    }

    #[test]
    fn a_program_title_is_preserved_until_it_resets() {
        let (model, state) = state();
        model.write(b"\x1b]0;cargo test\x07");
        for intent in model.drain_intents() {
            handle_intent(intent, &state, |_| {});
        }

        assert_eq!(
            state.title.lock().expect("title").as_deref(),
            Some("cargo test")
        );

        model.write(b"\x1b]0;\x07");
        for intent in model.drain_intents() {
            handle_intent(intent, &state, |_| {});
        }

        assert_eq!(
            state.title.lock().expect("title").as_deref(),
            Some(DEFAULT_TITLE)
        );
    }
}
