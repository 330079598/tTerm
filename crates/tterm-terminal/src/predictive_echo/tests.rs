use std::cell::Cell;
use std::rc::Rc;

use super::{
    EchoPredictor, PredictionScreen, PredictiveEcho, PredictiveEchoMode, PredictiveEchoState,
    ends_line,
};

struct FakeLine {
    text: Vec<char>,
    cursor_row: i64,
    cursor_col: usize,
    cols: usize,
}

impl FakeLine {
    fn new(text: &str, cursor_col: usize) -> Self {
        Self {
            text: text.chars().collect(),
            cursor_row: 5,
            cursor_col,
            cols: 80,
        }
    }

    fn at_end(text: &str) -> Self {
        let text: Vec<char> = text.chars().collect();
        let cursor_col = text.len();
        Self {
            text,
            cursor_row: 5,
            cursor_col,
            cols: 80,
        }
    }

    fn echo(&mut self, typed: &str) {
        for character in typed.chars() {
            if character == '\u{007F}' {
                self.cursor_col = self.cursor_col.saturating_sub(1);
                self.text.truncate(self.cursor_col);
            } else {
                if self.cursor_col < self.text.len() {
                    self.text[self.cursor_col] = character;
                } else {
                    self.text.push(character);
                }
                self.cursor_col += 1;
            }
        }
    }

    fn text(&self) -> String {
        self.text.iter().collect()
    }

    fn set_text(&mut self, text: &str) {
        self.text = text.chars().collect();
    }
}

impl PredictionScreen for FakeLine {
    fn cursor_row(&self) -> i64 {
        self.cursor_row
    }

    fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    fn cols(&self) -> usize {
        self.cols
    }

    fn char_at(&self, row: i64, col: usize) -> String {
        if row == self.cursor_row {
            self.text
                .get(col)
                .map(ToString::to_string)
                .unwrap_or_else(|| " ".to_string())
        } else {
            " ".to_string()
        }
    }
}

struct Harness {
    clock: Rc<Cell<f64>>,
    line: FakeLine,
    predictor: EchoPredictor,
}

impl Harness {
    fn tick(&mut self, milliseconds: f64) {
        self.clock.set(self.clock.get() + milliseconds);
    }

    fn type_and_echo(&mut self, keys: &str, rtt: f64) {
        self.predictor.type_input(keys, &self.line);
        self.tick(rtt);
        self.line.echo(keys);
        self.predictor.reconcile(&self.line);
    }

    fn shown(&self) -> Option<(String, usize)> {
        self.predictor.view().map(|view| {
            (
                view.cells.iter().map(|cell| cell.char.as_str()).collect(),
                view.cursor_col,
            )
        })
    }
}

fn harness_with(line: FakeLine) -> Harness {
    let clock = Rc::new(Cell::new(0.0));
    let read_clock = Rc::clone(&clock);
    Harness {
        clock: Rc::clone(&clock),
        line,
        predictor: EchoPredictor::new(move || read_clock.get()),
    }
}

#[test]
fn shows_nothing_on_a_line_until_the_remote_has_echoed_a_key_typed_there() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.predictor.type_input("l", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.tick(100.0);
    harness.line.echo("l");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("s -", &harness.line);
    assert_eq!(harness.shown(), Some(("s -".to_string(), 6)));
}

#[test]
fn drops_guesses_as_their_echo_arrives_even_several_at_once() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("l", 100.0);
    harness.predictor.type_input("s -la", &harness.line);
    harness.line.echo("s -");
    harness.predictor.reconcile(&harness.line);
    assert_eq!(harness.shown(), Some(("la".to_string(), 8)));

    harness.line.echo("la");
    harness.predictor.reconcile(&harness.line);
    assert_eq!(harness.shown(), None);
    assert!(!harness.predictor.busy());
}

#[test]
fn never_shows_what_is_typed_at_a_prompt_that_does_not_echo() {
    let mut harness = harness_with(FakeLine::at_end("Password: "));
    harness.predictor.type_input("secret", &harness.line);
    harness.tick(5000.0);
    harness.predictor.reconcile(&harness.line);
    harness.predictor.expire(&harness.line);
    harness.predictor.type_input("more", &harness.line);
    assert_eq!(harness.shown(), None);
}

#[test]
fn does_not_take_a_line_that_looks_backspaced_for_an_echo() {
    let mut harness = harness_with(FakeLine::at_end("Password: "));
    harness.predictor.type_input("x\u{007F}", &harness.line);
    harness.tick(100.0);
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("qr", &harness.line);
    assert_eq!(harness.shown(), None);
}

#[test]
fn shows_nothing_more_on_a_line_that_echoed_a_key_as_something_else() {
    let mut harness = harness_with(FakeLine::at_end("Password: "));
    harness.predictor.type_input("p", &harness.line);
    harness.tick(100.0);
    harness.line.echo("*");
    harness.predictor.reconcile(&harness.line);
    harness.type_and_echo("*", 100.0);
    harness.predictor.type_input("x", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.predictor.type_input("\r", &harness.line);
    harness.line = FakeLine::at_end("$ ");
    harness.type_and_echo("l", 100.0);
    harness.predictor.type_input("s", &harness.line);
    assert_eq!(harness.shown(), Some(("s".to_string(), 4)));
}

#[test]
fn starts_over_on_enter_so_the_next_line_must_echo_again() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("ls", 100.0);
    harness.predictor.type_input("\r", &harness.line);
    harness.line.cursor_row += 1;
    harness.line.set_text("$ ");
    harness.line.cursor_col = 2;
    harness.predictor.type_input("p", &harness.line);
    assert_eq!(harness.shown(), None);
}

#[test]
fn predicts_backspace_over_typed_characters_but_not_into_the_prompt() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("ab", 100.0);
    harness.predictor.type_input("\u{007F}", &harness.line);
    assert_eq!(harness.shown(), Some((" ".to_string(), 3)));

    harness.tick(100.0);
    harness.line.echo("\u{007F}");
    harness.predictor.reconcile(&harness.line);
    harness
        .predictor
        .type_input("\u{007F}\u{007F}", &harness.line);
    assert_eq!(harness.shown(), None);
    assert_eq!(harness.predictor.active_row(), None);
}

#[test]
fn keeps_a_key_and_its_backspace_that_echo_as_no_change_at_all() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b\u{007F}", &harness.line);
    harness.line.echo("b\u{007F}");
    harness.predictor.reconcile(&harness.line);
    harness.tick(3000.0);
    harness.predictor.expire(&harness.line);
    assert!(!harness.predictor.busy());

    harness.predictor.type_input("c", &harness.line);
    assert_eq!(harness.shown(), Some(("c".to_string(), 4)));
}

#[test]
fn does_not_guess_typing_in_the_middle_of_text() {
    let mut harness = harness_with(FakeLine::new("$ echo hi", 4));
    harness.type_and_echo("x", 100.0);
    assert_eq!(harness.predictor.active_row(), None);
}

#[test]
fn follows_a_grayed_out_suggestion_while_it_is_typed() {
    let mut harness = harness_with(FakeLine::new("$ git status", 3));
    harness.type_and_echo("i", 100.0);
    harness.predictor.type_input("t", &harness.line);
    assert_eq!(harness.shown(), Some(("t".to_string(), 5)));
}

#[test]
fn stops_at_the_last_column_where_the_cursor_waits_to_wrap() {
    let line = format!("$ {}", "x".repeat(16));
    let mut harness = harness_with(FakeLine::at_end(&line));
    harness.line.cols = 20;
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b", &harness.line);
    assert_eq!(harness.shown(), None);
}

#[test]
fn drops_guesses_once_the_line_stops_matching_them() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b", &harness.line);
    harness.line.set_text("$ something else");
    harness.line.cursor_col = 16;
    harness.predictor.reconcile(&harness.line);
    assert_eq!(harness.shown(), None);
    assert!(harness.predictor.busy());

    harness.tick(200.0);
    harness.predictor.expire(&harness.line);
    assert_eq!(harness.predictor.active_row(), None);
}

#[test]
fn rides_out_a_redraw_caught_halfway() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b", &harness.line);
    harness.line.cursor_col = 0;
    harness.predictor.reconcile(&harness.line);
    harness.line.cursor_col = 3;
    harness.line.echo("b");
    harness.predictor.reconcile(&harness.line);
    harness.tick(200.0);
    harness.predictor.expire(&harness.line);
    assert!(!harness.predictor.busy());
    assert_eq!(harness.predictor.active_row(), Some(5));
}

#[test]
fn marks_a_guess_whose_echo_is_overdue() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b", &harness.line);
    harness.tick(600.0);
    assert!(harness.predictor.view().expect("a view exists").cells[0].stalled);
}

#[test]
fn measures_the_round_trip_and_reports_a_slow_link() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    assert!(!harness.predictor.slower_than(30.0));
    harness.type_and_echo("a", 10.0);
    assert!(!harness.predictor.slower_than(30.0));
    for _ in 0..20 {
        harness.type_and_echo("b", 80.0);
    }
    assert!(harness.predictor.round_trip_ms().expect("a sample") > 30.0);
    assert!(harness.predictor.slower_than(30.0));
    assert!(!harness.predictor.slower_than(200.0));
}

#[test]
fn keeps_a_link_near_the_threshold_slow_until_it_is_clearly_faster() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 120.0);
    assert!(harness.predictor.slower_than(100.0));
    for _ in 0..5 {
        harness.type_and_echo("b", 75.0);
    }
    assert!(harness.predictor.round_trip_ms().expect("a sample") < 100.0);
    assert!(harness.predictor.slower_than(100.0));
    for _ in 0..30 {
        harness.type_and_echo("c", 40.0);
    }
    assert!(!harness.predictor.slower_than(100.0));
}

#[test]
fn places_keys_typed_after_tab_once_the_keys_before_it_are_echoed() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("c", 100.0);
    harness.predictor.type_input("d", &harness.line);
    harness.predictor.type_input("\t", &harness.line);
    harness.predictor.type_input("x", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.tick(100.0);
    harness.line.echo("d");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("y", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.tick(100.0);
    harness.line.echo("x");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("z", &harness.line);
    assert_eq!(harness.shown(), Some(("yz".to_string(), 7)));
}

#[test]
fn places_keys_typed_after_tab_past_what_the_completion_adds() {
    let mut harness = harness_with(FakeLine::at_end("$ cd sr"));
    harness.type_and_echo("c", 100.0);
    harness.predictor.type_input("\t", &harness.line);
    harness.predictor.type_input("l", &harness.line);
    harness.tick(100.0);
    harness.line.echo("/");
    harness.predictor.reconcile(&harness.line);
    harness.line.echo("l");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("ib", &harness.line);
    assert_eq!(harness.shown(), Some(("ib".to_string(), 12)));
}

#[test]
fn guesses_the_next_command_when_it_is_typed_before_the_prompt_returns() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("ls", 100.0);
    harness.predictor.type_input("\r", &harness.line);
    harness.predictor.type_input("p", &harness.line);
    harness.tick(300.0);
    harness.line.cursor_row += 2;
    harness.line.set_text("$ ");
    harness.line.cursor_col = 2;
    harness.predictor.reconcile(&harness.line);
    harness.tick(200.0);
    harness.predictor.expire(&harness.line);
    harness.predictor.type_input("w", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.tick(100.0);
    harness.line.echo("p");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("d", &harness.line);
    assert_eq!(harness.shown(), Some(("wd".to_string(), 5)));
}

#[test]
fn keeps_a_password_hidden_after_enter_at_a_prompt_that_masks_with_star() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("sudo -s", 100.0);
    harness.predictor.type_input("\r", &harness.line);
    harness.line.cursor_row += 1;
    harness.line.set_text("Password: ");
    harness.line.cursor_col = harness.line.text().chars().count();
    harness.predictor.type_input("a", &harness.line);
    harness.tick(100.0);
    harness.line.echo("*");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("*", &harness.line);
    harness.tick(100.0);
    harness.line.echo("*");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("x", &harness.line);
    assert_eq!(harness.shown(), None);

    harness.tick(5000.0);
    harness.predictor.expire(&harness.line);
    harness.predictor.type_input("y", &harness.line);
    harness.tick(100.0);
    harness.line.echo("**");
    harness.predictor.reconcile(&harness.line);
    harness.predictor.type_input("z", &harness.line);
    assert_eq!(harness.shown(), None);
}

#[test]
fn guesses_nothing_for_keys_other_than_typing_and_backspace() {
    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    for key in ["\t", "\x1b[D", "\u{0003}", "你"] {
        harness.predictor.type_input(key, &harness.line);
        assert_eq!(harness.predictor.active_row(), None);
    }
}

#[test]
fn oracle_ends_line_and_unpredictable_unicode_stop_guessing() {
    assert!(!ends_line(""));
    assert!(!ends_line("abc"));
    assert!(ends_line("a\r"));
    assert!(ends_line("a\n"));
    assert!(ends_line("a\u{0003}"));
    assert!(ends_line("a\u{0004}"));
    assert!(ends_line("😀\n"));

    let mut harness = harness_with(FakeLine::at_end("$ "));
    harness.type_and_echo("a", 100.0);
    harness.predictor.type_input("b你", &harness.line);
    assert_eq!(harness.predictor.active_row(), None);
    assert_eq!(harness.shown(), None);
    assert!(!harness.predictor.busy());
}

#[test]
fn oracle_chinese_emoji_and_combining_inputs_all_stop_guessing() {
    for input in ["你", "😀", "e\u{0301}"] {
        let mut harness = harness_with(FakeLine::at_end("$ "));
        harness.type_and_echo("a", 100.0);
        harness.predictor.type_input(input, &harness.line);
        assert_eq!(harness.predictor.active_row(), None, "{input}");
        assert_eq!(harness.shown(), None, "{input}");
        assert!(!harness.predictor.busy(), "{input}");
    }
}

#[test]
fn reads_the_predictive_echo_mode_configured_by_the_backend() {
    assert_eq!(
        PredictiveEchoMode::from_config("always"),
        PredictiveEchoMode::Always
    );
    assert_eq!(
        PredictiveEchoMode::from_config("off"),
        PredictiveEchoMode::Off
    );
    assert_eq!(
        PredictiveEchoMode::from_config("auto"),
        PredictiveEchoMode::Auto
    );
    assert_eq!(
        PredictiveEchoMode::from_config("sometimes"),
        PredictiveEchoMode::Auto
    );
    assert_eq!(
        PredictiveEchoMode::from_config(""),
        PredictiveEchoMode::Auto
    );
}

fn echo_state(can_predict: bool) -> PredictiveEchoState {
    PredictiveEchoState {
        cursor_hidden: false,
        normal_screen: true,
        mouse_tracking: false,
        can_predict,
    }
}

fn predicted_text(view: Option<super::PredictionView>, line: &FakeLine) -> String {
    let Some(view) = view else {
        return String::new();
    };
    let mut text: String = view.cells.iter().map(|cell| cell.char.as_str()).collect();
    text.push_str(
        &view
            .cells
            .iter()
            .find(|cell| cell.col == view.cursor_col)
            .map(|cell| cell.char.clone())
            .unwrap_or_else(|| line.char_at(view.row, view.cursor_col)),
    );
    text
}

#[test]
fn draws_typing_once_the_line_has_echoed_and_takes_it_away_with_the_echo() {
    let clock = Rc::new(Cell::new(0.0));
    let read_clock = Rc::clone(&clock);
    let mut line = FakeLine::at_end("$ ");
    let mut echo = PredictiveEcho::new(move || read_clock.get(), PredictiveEchoMode::Always, 30.0);
    let mut view = echo.note_input("l", &line, echo_state(true));
    assert_eq!(predicted_text(view.take(), &line), "");

    line.echo("l");
    let _ = echo.on_output_parsed(&line);
    view = echo.note_input("s", &line, echo_state(true));
    assert_eq!(predicted_text(view, &line), "s ");

    line.echo("s");
    view = echo.on_output_parsed(&line);
    assert_eq!(predicted_text(view, &line), "");
}

#[test]
fn leaves_programs_that_hide_the_cursor_alone() {
    let clock = Rc::new(Cell::new(0.0));
    let read_clock = Rc::clone(&clock);
    let mut line = FakeLine::at_end("$ ");
    let mut echo = PredictiveEcho::new(move || read_clock.get(), PredictiveEchoMode::Always, 30.0);
    echo.note_input("l", &line, echo_state(true));
    line.echo("l");
    let mut state = echo_state(true);
    state.cursor_hidden = true;
    echo.on_output_parsed(&line);
    let view = echo.note_input("s", &line, state);
    assert_eq!(predicted_text(view, &line), "");
}

#[test]
fn leaves_full_screen_programs_alone() {
    let clock = Rc::new(Cell::new(0.0));
    let read_clock = Rc::clone(&clock);
    let mut line = FakeLine::at_end("$ ");
    let mut echo = PredictiveEcho::new(move || read_clock.get(), PredictiveEchoMode::Always, 30.0);
    let mut state = echo_state(true);
    state.normal_screen = false;
    echo.note_input("l", &line, state);
    line.echo("l");
    echo.on_output_parsed(&line);
    let view = echo.note_input("s", &line, state);
    assert_eq!(predicted_text(view, &line), "");
}

#[test]
fn draws_nothing_when_off_on_a_fast_link_or_when_typing_may_not_echo() {
    for (mode, can_predict) in [
        (PredictiveEchoMode::Off, true),
        (PredictiveEchoMode::Auto, true),
        (PredictiveEchoMode::Always, false),
    ] {
        let clock = Rc::new(Cell::new(0.0));
        let read_clock = Rc::clone(&clock);
        let mut line = FakeLine::at_end("$ ");
        let mut echo = PredictiveEcho::new(move || read_clock.get(), mode, 30.0);
        echo.note_input("l", &line, echo_state(can_predict));
        line.echo("l");
        echo.on_output_parsed(&line);
        let view = echo.note_input("s", &line, echo_state(can_predict));
        assert_eq!(predicted_text(view, &line), "", "{mode:?}");
    }
}
