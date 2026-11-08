//! Semantic comparison with xterm, independent of fonts, theme RGB and internal cell flags.

use tterm_terminal::cell::{Cell, Underline};
use tterm_terminal::color::{Color, NamedColor};
use tterm_terminal::event::Intent;
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::Terminal;

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".into();
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn color(color: Color) -> String {
    match color {
        Color::Spec(rgb) => format!("rgb:{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(index) => format!("index:{index}"),
        Color::Named(named) => {
            let palette = [
                NamedColor::Black,
                NamedColor::Red,
                NamedColor::Green,
                NamedColor::Yellow,
                NamedColor::Blue,
                NamedColor::Magenta,
                NamedColor::Cyan,
                NamedColor::White,
                NamedColor::BrightBlack,
                NamedColor::BrightRed,
                NamedColor::BrightGreen,
                NamedColor::BrightYellow,
                NamedColor::BrightBlue,
                NamedColor::BrightMagenta,
                NamedColor::BrightCyan,
                NamedColor::BrightWhite,
            ];
            palette
                .iter()
                .position(|entry| *entry == named)
                .map_or_else(|| "default".into(), |index| format!("index:{index}"))
        }
    }
}

fn cell_line(row: usize, col: usize, cell: &Cell) -> String {
    let attrs = cell.attributes;
    let width = if attrs.wide_spacer {
        0
    } else if attrs.wide {
        2
    } else {
        1
    };
    let mut text = if width == 0 {
        String::new()
    } else {
        cell.ch.to_string()
    };
    if let Some(marks) = &cell.zerowidth {
        text.extend(marks);
    }
    let flags: String = [
        attrs.bold,
        attrs.dim,
        attrs.italic,
        attrs.inverse,
        attrs.hidden,
        attrs.strikeout,
    ]
    .map(|flag| if flag { '1' } else { '0' })
    .into_iter()
    .collect();
    let underline = match attrs.underline {
        Underline::None => 0,
        Underline::Single => 1,
        Underline::Double => 2,
        Underline::Curly => 3,
        Underline::Dotted => 4,
        Underline::Dashed => 5,
    };
    let ul_color = if underline == 0 {
        "default".into()
    } else {
        cell.underline_color.map_or_else(|| "default".into(), color)
    };
    let (id, uri) = cell
        .hyperlink
        .as_ref()
        .map_or(("", ""), |link| (link.id(), link.uri()));
    format!(
        "{row}\t{col}\t{}\t{width}\t{}\t{}\t{flags}\t{underline}\t{ul_color}\t{}\t{}",
        hex(text.as_bytes()),
        color(cell.foreground),
        color(cell.background),
        hex(id.as_bytes()),
        hex(uri.as_bytes())
    )
}

/// Exports every visible cell, including blank cells' styles, plus ordered protocol replies.
pub fn screen(terminal: &Terminal) -> String {
    let snapshot = terminal.snapshot();
    let grid = snapshot.grid();
    let cursor = snapshot.cursor().point;
    let screen = if snapshot.modes().contains(Modes::ALT_SCREEN) {
        "alternate"
    } else {
        "normal"
    };
    let mut lines = vec![
        format!("grid\t{}\t{}", grid.columns, grid.screen_lines),
        format!("cursor\t{}\t{}", cursor.line, cursor.column),
        format!("screen\t{screen}"),
    ];
    for (row, line) in snapshot.rows().iter().enumerate() {
        for (col, cell) in line.cells().iter().enumerate() {
            lines.push(cell_line(row, col, cell));
        }
    }
    let replies: Vec<u8> = terminal
        .drain_intents()
        .into_iter()
        .filter_map(|intent| match intent {
            Intent::WriteToPty(bytes) => Some(bytes),
            _ => None,
        })
        .flatten()
        .collect();
    lines.push(format!("replies\t{}", hex(&replies)));
    lines.join("\n") + "\n"
}

/// Lists exact changed records with both values, without accepting unexpected differences.
pub fn differences(reference: &str, actual: &str) -> String {
    let expected: Vec<_> = reference.lines().collect();
    let actual: Vec<_> = actual.lines().collect();
    let mut diff = String::new();
    for index in 0..expected.len().max(actual.len()) {
        let left = expected.get(index).copied().unwrap_or("<missing>");
        let right = actual.get(index).copied().unwrap_or("<missing>");
        if left != right {
            diff.push_str(&format!("xterm\t{left}\ntterm\t{right}\n"));
        }
    }
    diff
}
