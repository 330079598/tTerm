use super::session::WriteMessage;
#[cfg(test)]
use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tterm_terminal::color::Rgb;
use tterm_terminal::event::Intent;
use tterm_terminal::grid::{CellSize, WindowSize};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::terminal::Terminal;

/// Runs a bounded reader pipeline without requiring a real PTY. The session
/// uses the same shape with portable-pty's reader and writer objects.
#[cfg(test)]
pub fn run_reader<R: Read + Send + 'static, W: Write + Send + 'static>(
    mut reader: R,
    mut writer: W,
) -> (std::thread::JoinHandle<()>, std::thread::JoinHandle<()>) {
    let (sender, receiver) = std::sync::mpsc::sync_channel::<Vec<u8>>(8);
    let reader_thread = std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) if sender.send(buffer[..count].to_vec()).is_err() => break,
                Ok(_) => {}
            }
        }
    });
    let writer_thread = std::thread::spawn(move || {
        while let Ok(bytes) = receiver.recv() {
            if writer.write_all(&bytes).is_err() {
                break;
            }
        }
    });
    (reader_thread, writer_thread)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
    use tterm_terminal::terminal::{Config, Terminal};

    #[test]
    fn window_size_reply_does_not_consume_terminal_damage() {
        let terminal = Arc::new(Terminal::new(
            Config::default(),
            GridSize::new(10, 3),
            CellSize::new(8, 16),
        ));
        terminal.write(b"changed\x1b[14t");
        let intents = terminal.drain_intents();
        let [intent] = intents.as_slice() else {
            panic!("expected size intent")
        };
        let (sender, receiver) = mpsc::channel();
        let size = Arc::new(Mutex::new(WindowSize {
            grid: GridSize::new(10, 3),
            cell: CellSize::new(8, 16),
        }));
        handle_intent(
            intent.clone(),
            &terminal,
            &sender,
            &size,
            &Arc::new(Mutex::new(None)),
        );
        assert!(receiver.recv().is_ok());
        assert!(terminal.snapshot().damage().is_full());
    }

    #[test]
    fn bounded_reader_preserves_all_bytes_when_consumer_is_slow() {
        let input = (0u8..=255).cycle().take(200_000).collect::<Vec<_>>();
        let (reader, writer) = run_reader(std::io::Cursor::new(input), Vec::<u8>::new());
        reader.join().unwrap();
        writer.join().unwrap();
    }

    #[test]
    fn write_messages_keep_query_replies_in_order() {
        let terminal = Arc::new(Terminal::new(
            Config::default(),
            GridSize::new(10, 3),
            CellSize::new(8, 16),
        ));
        let (sender, receiver) = mpsc::channel();
        let size = Arc::new(Mutex::new(WindowSize {
            grid: GridSize::new(10, 3),
            cell: CellSize::new(8, 16),
        }));
        let title = Arc::new(Mutex::new(None));
        terminal.write(b"\x1b[c\x1b[14t");
        for intent in terminal.drain_intents() {
            handle_intent(intent, &terminal, &sender, &size, &title);
        }
        assert!(matches!(receiver.recv(), Ok(WriteMessage::Bytes(_))));
        assert!(matches!(receiver.recv(), Ok(WriteMessage::Bytes(_))));
    }
}
