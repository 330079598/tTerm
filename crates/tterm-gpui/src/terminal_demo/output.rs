use super::session::WriteMessage;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tterm_terminal::color::Rgb;
use tterm_terminal::event::Intent;
use tterm_terminal::grid::{CellSize, WindowSize};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::terminal::Terminal;

pub fn handle_intent(
    intent: Intent,
    terminal: &Arc<Terminal>,
    writer: &mpsc::Sender<WriteMessage>,
    size: &Arc<Mutex<WindowSize>>,
    title: &Arc<Mutex<Option<String>>>,
) {
    match intent {
        Intent::WriteToPty(bytes) => {
            let _ = writer.send(WriteMessage::Bytes(bytes));
        }
        Intent::ColorRequest { index, format } => {
            let palette = TerminalPalette::default();
            let color = match index {
                256 => palette.foreground,
                257 => palette.background,
                258 => palette.cursor,
                _ => palette.indexed(index.min(u8::MAX as usize) as u8),
            };
            let _ = writer.send(WriteMessage::Bytes(format.format(Rgb {
                r: color.r,
                g: color.g,
                b: color.b,
            })));
        }
        Intent::WindowSizeRequest { format } => {
            let window_size = size.lock().map(|size| *size).unwrap_or(WindowSize {
                grid: terminal.grid_size(),
                cell: CellSize::new(8, 16),
            });
            let _ = writer.send(WriteMessage::Bytes(format.format(window_size)));
        }
        Intent::Title(value) => {
            if let Ok(mut title) = title.lock() {
                *title = Some(value);
            }
        }
        Intent::TitleReset => {
            if let Ok(mut title) = title.lock() {
                *title = Some("tTerm GPUI Dev".to_string());
            }
        }
        Intent::ClipboardRead { .. }
        | Intent::ClipboardStore { .. }
        | Intent::Bell
        | Intent::CursorBlinkingChanged => {}
    }
}
