//! Model snapshots, text extraction, staged waits and window capture.
//!
//! Captures draw the window exactly as the running app would, without forcing
//! a full repaint, so a screenshot shows what the real dirty-region path
//! produced instead of hiding a stale or missing row.

use std::path::Path;
use std::time::{Duration, Instant};

use gpui_kit::{AnyWindowHandle, AsyncApp};

use tterm_terminal::mode::Modes;
use tterm_terminal::snapshot::{Row, Snapshot};
use tterm_terminal::terminal::{Scroll, Terminal};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const OBSERVED_ROWS: usize = 40;

/// The visible screen, with trailing cell padding removed from each row.
pub(super) fn visible_text(model: &Terminal) -> String {
    model.scroll(Scroll::Bottom);
    let snapshot = model.snapshot();
    let start = snapshot.rows().len().saturating_sub(OBSERVED_ROWS);
    join_rows(&snapshot, start)
}

/// Every row of the screen, used where a marker must be absent from all of it.
fn screen_text(model: &Terminal) -> String {
    model.scroll(Scroll::Bottom);
    let snapshot = model.snapshot();
    join_rows(&snapshot, 0)
}

fn join_rows(snapshot: &Snapshot, start: usize) -> String {
    snapshot
        .rows()
        .iter()
        .skip(start)
        .map(row_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn row_text(row: &Row) -> String {
    let mut text = String::with_capacity(row.len());
    for cell in row.cells() {
        text.push(cell.ch);
        if let Some(extra) = &cell.zerowidth {
            text.extend(extra.iter().copied());
        }
    }
    while text.ends_with(' ') {
        text.pop();
    }
    text
}

/// Whether a text contains the line as a whole line, ignoring trailing
/// carriage returns and spaces.
pub(super) fn contains_line(text: &str, needle: &str) -> bool {
    text.split('\n')
        .any(|line| line.trim_end().trim() == needle)
}

fn after_line<'a>(text: &'a str, needle: &str) -> Option<&'a str> {
    let mut offset = 0;
    for line in text.split('\n') {
        if line.trim_end().trim() == needle {
            return Some(&text[(offset + line.len() + 1).min(text.len())..]);
        }
        offset += line.len() + 1;
    }
    None
}

/// Waits until a printed line appears, without assuming the output stopped.
pub(super) async fn wait_for_text(
    model: &Terminal,
    needle: &str,
    timeout: Duration,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = visible_text(model);
        if contains_line(&text, needle) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} seconds waiting for '{needle}'.\n{text}",
                timeout.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

/// Waits for an explicit success or failure marker printed after a sentinel.
///
/// The script builds every marker from pieces at run time, so the echoed
/// command line never contains the marker and cannot satisfy the wait by
/// itself.
pub(super) async fn wait_for_marker(
    model: &Terminal,
    sentinel: &str,
    ok: &str,
    fail: &str,
    timeout: Duration,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = visible_text(model);
        if let Some(after) = after_line(&text, sentinel) {
            if contains_line(after, fail) {
                return Err(format!(
                    "The stage reported '{fail}' after its sentinel.\n{text}"
                ));
            }
            if contains_line(after, ok) {
                return Ok(());
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} seconds waiting for '{ok}' after '{sentinel}'.\n{text}",
                timeout.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

/// Whether one row shows both pane markers with the pane border between them.
///
/// A vertical split puts the two panes side by side, so each marker is a
/// segment of the same row rather than the whole line. The border between the
/// two markers is the evidence that they came from separate panes; the script
/// still builds every marker at run time, so an echoed command line cannot
/// put a literal marker on a row in the first place.
pub(super) fn contains_pane_markers(text: &str, left: &str, right: &str) -> bool {
    text.split('\n')
        .any(|row| row_has_pane_markers(row, left, right))
}

fn row_has_pane_markers(row: &str, left: &str, right: &str) -> bool {
    let Some(left_at) = row.find(left) else {
        return false;
    };
    let Some(after_left) = row.get(left_at + left.len()..) else {
        return false;
    };
    let Some(right_at) = after_left.find(right) else {
        return false;
    };
    after_left[..right_at].chars().any(is_pane_border)
}

/// tmux draws a plain or heavy vertical rule depending on the terminal's
/// character set, so accept every form it can render.
fn is_pane_border(cell: char) -> bool {
    matches!(cell, '│' | '┃' | '║' | '|')
}

/// Waits until one row shows both panes' markers around the pane border.
pub(super) async fn wait_for_pane_markers(
    model: &Terminal,
    left: &str,
    right: &str,
    fail: &str,
    timeout: Duration,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = visible_text(model);
        if contains_pane_markers(&text, left, right) {
            return Ok(());
        }
        if contains_line(&text, fail) {
            return Err(format!(
                "The stage reported '{fail}' before both panes were visible.\n{text}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} seconds waiting for '{left}' and '{right}' on one row either side of the pane border.\n{text}",
                timeout.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

/// Waits until the whole screen shows the active pane's marker and hides the
/// other pane's.
///
/// Zoom repaints the client with a single pane, so the hidden pane's marker
/// must be absent from every row. Markers are matched as row segments because
/// the pane prompt shares their row; the script still builds them at run time,
/// so an echoed command line cannot satisfy either half.
pub(super) async fn wait_for_zoomed_pane(
    model: &Terminal,
    shown: &str,
    hidden: &str,
    timeout: Duration,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = screen_text(model);
        if text.contains(shown) && !text.contains(hidden) {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} seconds waiting for the zoomed pane to show '{shown}' and hide '{hidden}'.\n{text}",
                timeout.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

/// Waits until a program owns the alternate screen, or reports the stage's
/// failure marker if the program exits before it can.
pub(super) async fn wait_for_alternate_screen(
    model: &Terminal,
    fail_marker: &str,
    timeout: Duration,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = visible_text(model);
        if model.modes().contains(Modes::ALT_SCREEN) {
            return Ok(());
        }
        if contains_line(&text, fail_marker) {
            return Err(format!(
                "The program exited before it entered the alternate screen ({fail_marker}).\n{text}"
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Timed out after {} seconds waiting for the alternate screen.\n{text}",
                timeout.as_secs()
            ));
        }
        cx.background_executor().timer(POLL_INTERVAL).await;
    }
}

/// Draws the window and writes a PNG into the prepared field directory.
pub(super) fn capture(
    handle: AnyWindowHandle,
    output: &Path,
    name: &str,
    cx: &mut AsyncApp,
) -> Result<String, String> {
    let image = handle
        .update(cx, |_, window, app| {
            let _ = window.draw(app);
            window.render_to_image()
        })
        .map_err(|error| format!("The field window could not be drawn: {error}"))?
        .map_err(|error| format!("The field window could not be rendered: {error}"))?;
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| format!("The field screenshot could not be encoded: {error}"))?;
    std::fs::write(output.join(name), bytes)
        .map_err(|error| format!("The field screenshot '{name}' could not be written: {error}"))?;
    Ok(name.to_string())
}
