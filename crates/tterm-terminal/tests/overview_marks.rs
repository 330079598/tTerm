//! Shell-integration marks exposed to the overview ruler.
//!
//! Every test uses the production model and synthetic OSC 133 bytes. The
//! model's marks are in retained-history coordinates: zero is the oldest row
//! the model still holds, so a mark keeps its row when new output arrives and
//! moves only when older rows are dropped.

use tterm_terminal::command_marks::CommandMarkPosition;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};

const COLUMNS: usize = 20;

fn model(lines: usize, capacity: usize) -> Terminal {
    Terminal::new(
        Config {
            scrollback: capacity,
            ..Config::default()
        },
        GridSize::new(COLUMNS, lines),
        CellSize::new(8, 16),
    )
}

fn command(model: &Terminal, index: usize, status: i64) {
    model.write(format!("\x1b]133;A\x07$ cmd{index}\r\n").as_bytes());
    model.write(b"\x1b]133;C\x07");
    model.write(format!("out{index}\r\n").as_bytes());
    model.write(format!("\x1b]133;D;{status}\x07").as_bytes());
}

fn marks(model: &Terminal) -> Vec<CommandMarkPosition> {
    model.snapshot().command_marks().to_vec()
}

#[test]
fn disabling_command_marks_drops_them_and_stops_new_ones() {
    let model = model(5, 50);
    command(&model, 0, 0);
    assert_eq!(marks(&model).len(), 1);

    model.set_command_marks_enabled(false);
    assert!(marks(&model).is_empty(), "the existing marks are dropped");
    command(&model, 1, 0);
    assert!(
        marks(&model).is_empty(),
        "output while disabled creates no marks"
    );

    model.set_command_marks_enabled(true);
    command(&model, 2, 0);
    assert_eq!(marks(&model).len(), 1, "the next prompt is collected again");
}

#[test]
fn reopening_collection_does_not_complete_an_old_partial_prompt() {
    let model = model(5, 50);
    model.write(b"\x1b]133;");
    model.set_command_marks_enabled(false);
    model.write(b"A\x07");
    model.set_command_marks_enabled(true);
    model.write(b"A\x07");
    assert!(marks(&model).is_empty());
    command(&model, 1, 0);
    assert_eq!(marks(&model).len(), 1);
}

#[test]
fn three_commands_expose_prompt_output_and_end_lines() {
    let model = model(5, 50);
    for index in 0..3 {
        command(&model, index, 0);
    }

    let marks = marks(&model);
    assert_eq!(marks.len(), 3, "one read-only mark per finished command");
    assert_eq!(
        marks
            .iter()
            .map(|mark| mark.prompt_line)
            .collect::<Vec<_>>(),
        [0, 2, 4]
    );
    assert_eq!(
        marks
            .iter()
            .map(|mark| mark.output_line)
            .collect::<Vec<_>>(),
        [Some(1), Some(3), Some(5)]
    );
    assert_eq!(
        marks.iter().map(|mark| mark.end_line).collect::<Vec<_>>(),
        [Some(2), Some(4), Some(6)]
    );
    assert!(marks.iter().all(|mark| mark.exit_code == Some(0)));
    assert_eq!(
        model.snapshot().history_size(),
        2,
        "the fixture scrolled two rows into history"
    );
}

#[test]
fn output_marks_stay_on_their_rows_while_the_viewport_scrolls() {
    let model = model(5, 50);
    for index in 0..3 {
        command(&model, index, 0);
    }
    let before = marks(&model);

    model.scroll(tterm_terminal::terminal::Scroll::Top);
    assert_eq!(marks(&model), before, "scrolling does not move a mark");
    model.scroll(tterm_terminal::terminal::Scroll::Bottom);
    assert_eq!(marks(&model), before);
}

#[test]
fn trimming_known_history_shifts_and_drops_marks() {
    let model = model(3, 50);
    for index in 0..4 {
        command(&model, index, 0);
    }
    // A final prompt sits on the newest row, so one mark survives a shrink.
    model.write(b"\x1b]133;A\x07$ ");
    let before = marks(&model);
    let history_before = model.snapshot().history_size();
    assert!(history_before > 2, "the fixture has history to drop");

    model.set_scrollback(2);
    let removed = history_before - 2;
    let expected: Vec<_> = before
        .into_iter()
        .filter(|mark| mark.prompt_line >= removed as i64)
        .map(|mark| CommandMarkPosition {
            prompt_line: mark.prompt_line - removed as i64,
            output_line: mark
                .output_line
                .filter(|line| *line >= removed as i64)
                .map(|line| line - removed as i64),
            end_line: mark
                .end_line
                .filter(|line| *line >= removed as i64)
                .map(|line| line - removed as i64),
            exit_code: mark.exit_code,
        })
        .collect();

    assert_eq!(marks(&model), expected);
    assert!(
        expected.len() < 5,
        "marks whose prompt was evicted are dropped"
    );
}

#[test]
fn ed3_and_reset_clear_every_mark() {
    for clear in [b"\x1b[3J".as_slice(), b"\x1bc".as_slice()] {
        let model = model(5, 50);
        command(&model, 0, 0);
        assert_eq!(marks(&model).len(), 1);

        model.write(clear);
        assert!(
            marks(&model).is_empty(),
            "a main-history clear disposes every mark"
        );
    }
}

#[test]
fn clearing_only_the_viewport_keeps_marks() {
    let model = model(5, 50);
    command(&model, 0, 0);
    let before = marks(&model);

    model.write(b"\x1b[2J");
    assert_eq!(marks(&model), before);
}

#[test]
fn alternate_screen_marks_do_not_pollute_the_main_screen() {
    let model = model(5, 50);
    command(&model, 0, 0);
    let main = marks(&model);
    assert_eq!(main.len(), 1);

    model.write(b"\x1b[?1049h");
    command(&model, 1, 1);
    model.write(b"\x1b[3J");
    assert_eq!(
        marks(&model),
        main,
        "neither a mark nor a clear on the alternate screen touches the main marks"
    );

    model.write(b"\x1b[?1049l");
    assert_eq!(marks(&model), main);
}

#[test]
fn an_eviction_shifts_surviving_marks_by_the_exact_rows() {
    let model = model(3, 3);
    for index in 0..2 {
        command(&model, index, 0);
    }
    let before = marks(&model);
    assert_eq!(
        before
            .iter()
            .map(|mark| mark.prompt_line)
            .collect::<Vec<_>>(),
        [0, 2]
    );

    // Filling the last free history row moves no retained line.
    model.write(b"\n");
    assert_eq!(marks(&model), before);
    assert_eq!(model.snapshot().history_size(), 3);

    // The next line pushes the oldest row out. The first command is gone and
    // the second one moves up by exactly one row.
    model.write(b"\n");
    assert_eq!(
        marks(&model),
        vec![CommandMarkPosition {
            prompt_line: 1,
            output_line: Some(2),
            end_line: Some(3),
            exit_code: Some(0),
        }]
    );
}

#[test]
fn one_write_and_byte_writes_agree_on_clears_alternate_screens_and_evictions() {
    let mut stream = Vec::new();
    for index in 0..4 {
        stream.extend_from_slice(format!("\x1b]133;A\x07$ cmd{index}\r\n").as_bytes());
        stream.extend_from_slice(b"\x1b]133;C\x07");
        stream.extend_from_slice(format!("out{index}\r\n").as_bytes());
        stream.extend_from_slice(format!("\x1b]133;D;{index}\x07").as_bytes());
    }
    for index in 0..24 {
        stream.extend_from_slice(format!("fill-{index:02}\r\n").as_bytes());
    }
    stream.extend_from_slice(b"\x1b[?1049h");
    stream.extend_from_slice(b"\x1b]133;A\x07$ alt\r\n\x1b]133;C\x07alt\r\n\x1b]133;D;9\x07");
    stream.extend_from_slice(b"\x1b[3J\x1b[?1049l");
    stream.extend_from_slice(b"\x1b[3J\x1b[H");
    stream.extend_from_slice(b"\x1b]133;A\x07$ final\r\n\x1b]133;C\x07done\r\n\x1b]133;D;0\x07");

    let one = model(5, 50);
    one.write(&stream);
    let split = model(5, 50);
    for byte in &stream {
        split.write(&[*byte]);
    }

    assert_eq!(marks(&one), marks(&split));
    assert_eq!(
        one.snapshot().history_size(),
        split.snapshot().history_size()
    );
    let final_marks = marks(&one);
    assert_eq!(
        final_marks.len(),
        1,
        "the clear before the final command wins"
    );
    assert_eq!(final_marks[0].prompt_line, 0);
    assert_eq!(final_marks[0].exit_code, Some(0));
}

#[test]
fn a_clear_and_new_marks_in_one_write_keep_only_the_new_command() {
    let mut stream = b"\x1b]133;A\x07old\r\n\x1b]133;C\x07old\r\n\x1b]133;D;1\x07".to_vec();
    stream.extend_from_slice(b"\x1b[3J\x1b[H");
    stream.extend_from_slice(b"\x1b]133;A\x07$ new\r\n\x1b]133;C\x07new\r\n\x1b]133;D;0\x07");

    let single = model(5, 50);
    single.write(&stream);
    let found = marks(&single);
    assert_eq!(found.len(), 1, "the clear disposes the old command");
    assert_eq!(found[0].prompt_line, 0);
    assert_eq!(found[0].output_line, Some(1));
    assert_eq!(found[0].end_line, Some(2));
    assert_eq!(found[0].exit_code, Some(0));

    let split = model(5, 50);
    for byte in &stream {
        split.write(&[*byte]);
    }
    assert_eq!(
        split.snapshot().command_marks(),
        single.snapshot().command_marks()
    );
}

#[test]
fn control_string_payloads_do_not_create_command_marks() {
    let model = model(5, 50);
    for fake in [
        b"\x1b]0;title 133;A 133;C 133;D;0\x07".as_slice(),
        b"\x1bPpayload ]133;A\x07 133;C 133;D;0\x1b\\",
        b"\x1b]8;;https://example.com/133;A\x07",
    ] {
        model.write(fake);
        assert!(
            marks(&model).is_empty(),
            "payload text must not become a shell mark: {fake:?}"
        );
    }

    command(&model, 0, 0);
    assert_eq!(marks(&model).len(), 1, "a real mark still works");
}

#[test]
fn a_synchronized_update_does_not_give_a_buffered_mark_a_fake_position() {
    let stream = b"\x1b[?2026h\x1b]133;A\x07$ hidden\r\n\
        \x1b]133;C\x07out\r\n\x1b]133;D;0\x07\x1b[?2026l";
    let single = model(5, 50);
    single.write(stream);
    assert!(
        marks(&single).is_empty(),
        "a mark buffered by a synchronized update has no executed position"
    );

    let split = model(5, 50);
    for byte in stream {
        split.write(&[*byte]);
    }
    assert_eq!(marks(&split), marks(&single));

    command(&single, 0, 0);
    assert_eq!(marks(&single).len(), 1, "the next prompt re-synchronizes");
}

#[test]
fn a_window_resize_discards_marks_because_reflow_has_no_line_map() {
    let model = model(5, 50);
    command(&model, 0, 0);
    assert_eq!(marks(&model).len(), 1);

    model.resize(GridSize::new(10, 5), CellSize::new(8, 16));
    assert!(marks(&model).is_empty());
}

#[test]
fn an_empty_model_has_no_marks() {
    let model = model(5, 50);
    assert!(marks(&model).is_empty());
}
#[test]
fn a_mark_split_across_writes_is_applied_at_its_terminator() {
    let model = model(5, 50);
    model.write(b"\x1b]133;");
    assert!(
        marks(&model).is_empty(),
        "an unfinished report is not a mark"
    );
    model.write(b"A\x07$ split\r\n");
    model.write(b"\x1b]133;");
    model.write(b"C\x07out\r\n");
    model.write(b"\x1b]133;D;7\x07");

    let marks = marks(&model);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].prompt_line, 0);
    assert_eq!(marks[0].output_line, Some(1));
    assert_eq!(marks[0].end_line, Some(2));
    assert_eq!(marks[0].exit_code, Some(7));
}

#[test]
fn a_scroll_before_an_alternate_screen_switch_still_moves_main_marks() {
    let model = model(3, 3);
    for index in 0..2 {
        command(&model, index, 0);
    }
    let before = marks(&model);
    assert_eq!(
        before
            .iter()
            .map(|mark| mark.prompt_line)
            .collect::<Vec<_>>(),
        [0, 2]
    );

    model.write(b"\n\n\x1b[?1049h");
    assert_eq!(
        marks(&model),
        vec![CommandMarkPosition {
            prompt_line: 1,
            output_line: Some(2),
            end_line: Some(3),
            exit_code: Some(0),
        }],
        "the two main-screen scrolls happen before the grid is swapped"
    );
    model.write(b"\x1b[3J\x1b[?1049l");
    assert_eq!(
        marks(&model),
        vec![CommandMarkPosition {
            prompt_line: 1,
            output_line: Some(2),
            end_line: Some(3),
            exit_code: Some(0),
        }],
        "an alternate-screen clear does not touch the main marks"
    );
}
