//! What the model hands back for the caller to do.
//!
//! The model owns no PTY, no clipboard and no theme. When a program asks for
//! something only the application can provide, the parser records an
//! [`Intent`]; the session layer resolves it and, where the protocol expects
//! an answer, writes the result through the same ordered channel as user
//! input (design note 3.3). Bytes the model can produce itself — device
//! attributes, cursor position, mode reports — arrive as
//! [`Intent::WriteToPty`].
//!
//! The three formatter types wrap closures from Alacritty itself. They encode
//! the exact escape sequence the program asked for, terminator included, so
//! this crate never has to reimplement a reply format; the caller only
//! supplies the data that the model does not have.

use std::fmt;
use std::sync::Arc;

use alacritty_terminal::event::{Event as AnsiEvent, WindowSize as AnsiWindowSize};
use alacritty_terminal::term::ClipboardType as AnsiClipboardType;
use alacritty_terminal::vte::ansi::Rgb as AnsiRgb;

use crate::color::Rgb;
use crate::grid::WindowSize;

/// Which clipboard a program addressed with OSC 52.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clipboard {
    /// The system clipboard, the one a user pastes from everywhere.
    Clipboard,
    /// The selection buffer, which X11 fills as text is marked.
    Selection,
}

impl From<AnsiClipboardType> for Clipboard {
    fn from(clipboard: AnsiClipboardType) -> Self {
        match clipboard {
            AnsiClipboardType::Clipboard => Self::Clipboard,
            AnsiClipboardType::Selection => Self::Selection,
        }
    }
}

/// Turns clipboard text into the reply a program asked for.
///
/// The program may have chosen the terminator and the clipboard letter, so the
/// answer is the closure Alacritty built for that request. It is handed back
/// through an [`Arc`] so a queued intent stays cheap to clone.
#[derive(Clone)]
pub struct ClipboardFormatter(Arc<dyn Fn(&str) -> String + Send + Sync + 'static>);

impl ClipboardFormatter {
    /// Encodes `text` as the answer to the program's request.
    pub fn format(&self, text: &str) -> Vec<u8> {
        (self.0)(text).into_bytes()
    }
}

impl fmt::Debug for ClipboardFormatter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClipboardFormatter")
    }
}

/// Turns a palette color into the reply to an OSC 10, 11 or 12 query.
#[derive(Clone)]
pub struct ColorFormatter(Arc<dyn Fn(Rgb) -> String + Send + Sync + 'static>);

impl ColorFormatter {
    /// Encodes `color` as the answer to the program's request.
    pub fn format(&self, color: Rgb) -> Vec<u8> {
        (self.0)(color).into_bytes()
    }
}

impl fmt::Debug for ColorFormatter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ColorFormatter")
    }
}

/// Turns the text area size into the reply to a `CSI 14 t` query.
#[derive(Clone)]
pub struct WindowSizeFormatter(Arc<dyn Fn(WindowSize) -> String + Send + Sync + 'static>);

impl WindowSizeFormatter {
    /// Encodes `size` as the answer to the program's request.
    pub fn format(&self, size: WindowSize) -> Vec<u8> {
        (self.0)(size).into_bytes()
    }
}

impl fmt::Debug for WindowSizeFormatter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WindowSizeFormatter")
    }
}

/// Something the caller has to do for the terminal.
#[derive(Clone)]
pub enum Intent {
    /// The program set the window title (OSC 0, 1 or 2).
    Title(String),
    /// The program reset the title to the default.
    TitleReset,
    /// The program rang the terminal bell.
    Bell,
    /// The program stored text in a clipboard (OSC 52); storing is the
    /// program's request, not a paste.
    ClipboardStore {
        /// The clipboard the program addressed.
        clipboard: Clipboard,
        /// The text to store.
        text: String,
    },
    /// The program asked for the clipboard's contents (OSC 52). Read the
    /// clipboard, call [`ClipboardFormatter::format`] and write the bytes back
    /// to the session.
    ClipboardRead {
        /// The clipboard the program addressed.
        clipboard: Clipboard,
        /// Encodes the text as the program's reply.
        format: ClipboardFormatter,
    },
    /// The program asked for the terminal's color (OSC 10, 11 or 12). Resolve
    /// the palette entry, call [`ColorFormatter::format`] and write the bytes
    /// back to the session.
    ColorRequest {
        /// Index into the 256-color palette; 256 and above are the special
        /// slots such as the foreground, background and cursor.
        index: usize,
        /// Encodes the color as the program's reply.
        format: ColorFormatter,
    },
    /// The program asked for the text area size in pixels (`CSI 14 t`). Call
    /// [`WindowSizeFormatter::format`] with
    /// [`crate::snapshot::Snapshot::window_size`] and write the bytes back to
    /// the session.
    WindowSizeRequest {
        /// Encodes the size as the program's reply.
        format: WindowSizeFormatter,
    },
    /// Bytes that must go back to the session, in order with user input.
    ///
    /// The model never holds the PTY or SSH writer: the session layer queues
    /// these on its ordered input channel so two query answers cannot swap
    /// places.
    WriteToPty(Vec<u8>),
    /// The cursor's blinking state changed; the view restarts or stops its
    /// cursor timer.
    CursorBlinkingChanged,
}

impl fmt::Debug for Intent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Title(title) => formatter.debug_tuple("Title").field(title).finish(),
            Self::TitleReset => formatter.write_str("TitleReset"),
            Self::Bell => formatter.write_str("Bell"),
            Self::ClipboardStore { clipboard, text } => formatter
                .debug_struct("ClipboardStore")
                .field("clipboard", clipboard)
                .field("text", text)
                .finish(),
            Self::ClipboardRead { clipboard, .. } => formatter
                .debug_struct("ClipboardRead")
                .field("clipboard", clipboard)
                .finish(),
            Self::ColorRequest { index, .. } => formatter
                .debug_struct("ColorRequest")
                .field("index", index)
                .finish(),
            Self::WindowSizeRequest { .. } => formatter.write_str("WindowSizeRequest"),
            Self::WriteToPty(bytes) => formatter.debug_tuple("WriteToPty").field(bytes).finish(),
            Self::CursorBlinkingChanged => formatter.write_str("CursorBlinkingChanged"),
        }
    }
}

/// Translates one Alacritty event into tTerm's intent, or drops it.
///
/// Four events are dropped deliberately:
///
/// - `Wakeup` and `MouseCursorDirty` only tell the application to redraw. The
///   caller already knows that a write happened, and the mouse mode it would
///   derive from is part of every snapshot.
/// - `Exit` and `ChildExit` belong to whoever owns the PTY; this model has no
///   child process to report and never calls `Term::exit`.
pub(crate) fn from_ansi(event: AnsiEvent) -> Option<Intent> {
    Some(match event {
        AnsiEvent::Title(title) => Intent::Title(title),
        AnsiEvent::ResetTitle => Intent::TitleReset,
        AnsiEvent::Bell => Intent::Bell,
        AnsiEvent::ClipboardStore(clipboard, text) => Intent::ClipboardStore {
            clipboard: clipboard.into(),
            text,
        },
        AnsiEvent::ClipboardLoad(clipboard, format) => Intent::ClipboardRead {
            clipboard: clipboard.into(),
            format: ClipboardFormatter(format),
        },
        AnsiEvent::ColorRequest(index, format) => Intent::ColorRequest {
            index,
            format: ColorFormatter(Arc::new(move |color: Rgb| {
                format(AnsiRgb {
                    r: color.r,
                    g: color.g,
                    b: color.b,
                })
            })),
        },
        AnsiEvent::TextAreaSizeRequest(format) => Intent::WindowSizeRequest {
            format: WindowSizeFormatter(Arc::new(move |size: WindowSize| {
                format(AnsiWindowSize {
                    num_lines: to_u16(size.grid.screen_lines),
                    num_cols: to_u16(size.grid.columns),
                    cell_width: size.cell.width,
                    cell_height: size.cell.height,
                })
            })),
        },
        AnsiEvent::PtyWrite(text) => Intent::WriteToPty(text.into_bytes()),
        AnsiEvent::CursorBlinkingChange => Intent::CursorBlinkingChanged,
        AnsiEvent::MouseCursorDirty
        | AnsiEvent::Wakeup
        | AnsiEvent::Exit
        | AnsiEvent::ChildExit(_) => return None,
    })
}

/// Alacritty's window size is expressed in `u16`, so a grid larger than that
/// saturates instead of wrapping. No real window comes close.
fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}
