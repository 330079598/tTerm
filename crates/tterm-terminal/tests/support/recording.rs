//! P0-11 test recording reader, matching terminalLogReplay.ts's IPC frame format.

use std::time::Duration;

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::Terminal;

/// A timestamped output or resize frame, containing no application metadata.
#[derive(Clone, Debug)]
pub struct Frame {
    pub at: Duration,
    pub action: Action,
}

/// An operation recorded at the parser boundary.
#[derive(Clone, Debug)]
pub enum Action {
    Output(Vec<u8>),
    Resize(GridSize),
}

impl Frame {
    /// Applies one frame without waiting; the caller owns the replay clock.
    pub fn apply(&self, terminal: &Terminal) {
        match &self.action {
            Action::Output(bytes) => terminal.write(bytes),
            Action::Resize(grid) => terminal.resize(*grid, CellSize::new(8, 19)),
        }
    }
}

/// Rejects partial, unknown, oversized and nonmonotonic frames rather than silently losing output.
pub fn decode(bytes: &[u8]) -> Result<Vec<Frame>, String> {
    if bytes.len() > 32 * 1024 {
        return Err("recording exceeds 32 KiB".into());
    }
    let mut remaining = bytes;
    let mut frames: Vec<Frame> = Vec::new();
    while !remaining.is_empty() {
        let header = take(&mut remaining, 13)?;
        let at = Duration::from_micros(u64::from_le_bytes(
            header[1..9].try_into().map_err(|_| "invalid timestamp")?,
        ));
        if frames.last().is_some_and(|frame| frame.at > at) {
            return Err("timestamps decrease".into());
        }
        let action = match header[0] {
            0 => {
                let length =
                    u32::from_le_bytes(header[9..13].try_into().map_err(|_| "invalid length")?)
                        as usize;
                Action::Output(take(&mut remaining, length)?.to_vec())
            }
            1 => {
                let cols = u16::from_le_bytes([header[9], header[10]]) as usize;
                let rows = u16::from_le_bytes([header[11], header[12]]) as usize;
                if cols < 2 || rows == 0 || cols > 200 || rows > 100 {
                    return Err("recording grid outside 2..200 columns / 1..100 rows".into());
                }
                Action::Resize(GridSize::new(cols, rows))
            }
            _ => return Err("unknown frame kind".into()),
        };
        frames.push(Frame { at, action });
    }
    if !matches!(
        frames.first().map(|frame| &frame.action),
        Some(Action::Resize(_))
    ) {
        return Err("recording must begin with a size".into());
    }
    Ok(frames)
}

fn take<'a>(remaining: &mut &'a [u8], length: usize) -> Result<&'a [u8], String> {
    if remaining.len() < length {
        return Err("truncated recording".into());
    }
    let (taken, rest) = remaining.split_at(length);
    *remaining = rest;
    Ok(taken)
}
