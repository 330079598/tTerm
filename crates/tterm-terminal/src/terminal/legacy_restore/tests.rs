use crate::cell::Cell;
use crate::color::{Color, NamedColor};
use crate::grid::{CellSize, GridSize};
use crate::snapshot::Snapshot;
use crate::terminal::{Config, Scroll, Terminal};

fn model() -> Terminal {
    Terminal::new(
        Config::default(),
        GridSize::new(24, 6),
        CellSize::new(8, 16),
    )
}

fn display_text(snapshot: &Snapshot) -> String {
    let mut text = String::new();
    for row in snapshot.rows() {
        for cell in row.cells() {
            text.push(cell.ch);
        }
        text.push('\n');
    }
    text
}

fn find_cell(snapshot: &Snapshot, needle: char) -> Option<&Cell> {
    snapshot
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .find(|cell| cell.ch == needle)
}

#[test]
fn blank_content_is_a_complete_no_op() {
    let terminal = model();
    terminal.write(b"existing");
    let before = terminal.snapshot();

    terminal.restore_legacy_history(" \r\n\t", "must not appear");
    let after = terminal.snapshot();

    assert_eq!(after.rows(), before.rows());
    assert_eq!(after.content_version(), before.content_version());
    assert_eq!(
        after.scrollback_generation(),
        before.scrollback_generation()
    );
    assert_eq!(after.history_size(), before.history_size());
    assert_eq!(after.display_offset(), before.display_offset());
    assert!(!display_text(&after).contains("must not appear"));
}

#[test]
fn restored_colors_and_history_survive_with_a_dim_banner() {
    let reference = model();
    reference.write(b"\x1b[31mred\x1b[0m");
    let reference = reference.snapshot();
    let expected = reference.row(0).and_then(|row| row.cell(0));

    let terminal = model();
    let content = "\x1b[31mred\x1b[0m\r\none\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\n";
    terminal.restore_legacy_history(content, "Legacy tab");
    let bottom = terminal.snapshot();
    assert!(bottom.history_size() > 0);
    assert!(bottom.is_scrolled_to_bottom());
    let banner = find_cell(&bottom, '─');
    assert!(banner.is_some_and(|cell| cell.attributes.dim));

    terminal.scroll(Scroll::Top);
    let top = terminal.snapshot();
    let restored = top.row(0).and_then(|row| row.cell(0));
    assert_eq!(
        restored.map(|cell| &cell.foreground),
        expected.map(|cell| &cell.foreground)
    );
}

#[test]
fn reset_after_content_keeps_the_banner_and_later_output_unstyled() {
    let terminal = model();
    terminal.restore_legacy_history("\x1b[31mred", "tag");
    terminal.write(b"plain");
    let snapshot = terminal.snapshot();

    let banner = find_cell(&snapshot, '─');
    assert!(banner.is_some_and(|cell| cell.attributes.dim));

    let reference = model();
    reference.write(b"plain");
    let reference = reference.snapshot();
    let expected = reference.row(0).and_then(|row| row.cell(0));
    let plain = find_cell(&snapshot, 'p');
    assert_eq!(plain, expected);
}

#[test]
fn hostile_sequences_produce_no_intents_modes_or_hyperlinks() {
    let terminal = model();
    let modes = terminal.modes();
    let content = concat!(
        "\x1b]0;stolen title\x07",
        "\x1b]52;c;SGVsbG8=\x07",
        "\x1b]52;c;?\x07",
        "\x1b]8;;https://example.com\x07linked\x1b]8;;\x07",
        "\x1b]4;1;rgb:ff/00/00\x07",
        "\x1b]10;?\x07",
        "\x07",
        "\x1b[c",
        "\x1b[6n",
        "\x1b[14t",
        "\x1b[?1000h\x1b[?1002h\x1b[?1003h",
        "\x1b[?2004h\x1b[?25l\x1b[4h\x1b[?6h",
        "\x1b[?1h\x1b[?1049h",
        "safe",
    );

    terminal.restore_legacy_history(content, "banner");

    assert!(terminal.drain_intents().is_empty());
    assert_eq!(terminal.modes(), modes);
    let snapshot = terminal.snapshot();
    assert!(
        snapshot
            .rows()
            .iter()
            .flat_map(|row| row.cells())
            .all(|cell| cell.hyperlink.is_none())
    );
}

#[test]
fn live_incomplete_sequence_does_not_consume_restored_content() {
    let terminal = model();
    terminal.write(b"\x1b[");
    terminal.restore_legacy_history("restored\r\n", "legacy");

    let snapshot = terminal.snapshot();
    let text = display_text(&snapshot);
    assert!(text.contains("restored"));
    assert!(text.contains("legacy"));
}

#[test]
fn serializer_wrap_sequences_only_move_and_erase_the_grid() {
    let terminal = model();
    // The row-separator fallback in `SerializeAddon` pads with characters,
    // moves back, erases, moves up, pads again and moves down. These are the
    // only non-SGR commands besides text and CR/LF that a production snapshot
    // can contain.
    terminal.restore_legacy_history(
        "abcdefghij\x1b[2D\x1b[2X\x1b[1A\x1b[3C\x1b[2X\x1b[1D\x1b[1B",
        "legacy",
    );

    let snapshot = terminal.snapshot();
    let first_row: String = snapshot
        .row(0)
        .map(|row| row.cells().iter().take(8).map(|cell| cell.ch).collect())
        .unwrap_or_default();
    assert_eq!(first_row, "abcdefgh");
}

#[test]
fn incomplete_content_control_string_does_not_swallow_the_banner() {
    let terminal = model();
    terminal.restore_legacy_history("hello\x1b]0;unterminated", "banner");

    let snapshot = terminal.snapshot();
    let text = display_text(&snapshot);
    assert!(text.contains("hello"));
    assert!(text.contains("banner"));

    terminal.write(b"live");
    let snapshot = terminal.snapshot();
    assert!(display_text(&snapshot).contains("live"));
}

#[test]
fn imported_content_cannot_overwrite_the_live_saved_cursor() {
    let terminal = model();
    terminal.write(b"AB\x1b[31m\x1b7\r\n");
    terminal.restore_legacy_history("\x1b[32mgg\x1b7", "legacy");
    terminal.write(b"\x1b8X");

    let snapshot = terminal.snapshot();
    let row = snapshot.row(0);
    let text: String = row
        .map(|row| row.cells().iter().take(3).map(|cell| cell.ch).collect())
        .unwrap_or_default();
    assert_eq!(text, "ABX");
    assert_eq!(
        row.and_then(|row| row.cell(2)).map(|cell| cell.foreground),
        Some(Color::Named(NamedColor::Red))
    );
}

#[test]
fn banner_control_characters_are_removed_before_drawing() {
    let terminal = model();
    terminal.restore_legacy_history("text", "\x1b[31mred\nbanner\t");
    let snapshot = terminal.snapshot();
    let text = display_text(&snapshot);
    assert!(!text.contains('\u{1b}'));
    assert!(text.contains("── [31mredbanner ──"));
}
