use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::mouse_reporting::MOUSE_REPORTING_OFF;
use tterm_terminal::terminal::{Config, Terminal};

use super::{ForegroundProbe, MouseGuard, SETTLE_DELAY};
use crate::terminal_demo::pipeline::{RefreshSignal, SessionState};

const GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};
const MOUSE_ON: &[u8] = b"\x1b[?1002h\x1b[?1006h";

struct Fixture {
    _runtime: tokio::runtime::Runtime,
    guard: MouseGuard,
    model: Arc<Terminal>,
}

fn fixture(local: bool) -> Fixture {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    let model = Arc::new(Terminal::new(Config::default(), GRID, CELL));
    let state = SessionState {
        terminal: Arc::clone(&model),
        size: Arc::new(Mutex::new(WindowSize {
            grid: GRID,
            cell: CELL,
        })),
        title: Arc::new(Mutex::new(None)),
        palette: Arc::new(Mutex::new(
            tterm_terminal::palette::TerminalPalette::default(),
        )),
        clipboard: Arc::new(crate::terminal_demo::clipboard::store::ClipboardWrites::disabled()),
        session_nonce: 1,
        alive: Arc::new(AtomicBool::new(true)),
        visible: Arc::new(AtomicBool::new(true)),
        refresh: Arc::new(RefreshSignal::new()),
        reports: Arc::new(crate::terminal_demo::session::reports::Reports::new()),
    };
    let foreground = local.then(|| ForegroundProbe::Local {
        ptys: Arc::new(RwLock::new(HashMap::new())),
        tab_id: "p2-04-guard".to_string(),
        session_nonce: 1,
    });
    let handle = runtime.handle().clone();
    Fixture {
        _runtime: runtime,
        guard: MouseGuard::new(state, handle, foreground),
        model,
    }
}

impl Fixture {
    fn apply(&self, bytes: &[u8], now: Instant) -> bool {
        self.guard.apply_batch_at(bytes, now)
    }

    fn reporting(&self) -> bool {
        self.model.is_mouse_reporting()
    }
}

#[test]
fn the_off_sequence_is_exact() {
    assert_eq!(
        MOUSE_REPORTING_OFF,
        b"\x1b[?9l\x1b[?1000l\x1b[?1001l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?1015l\x1b[?1016l"
    );
}

#[test]
fn a_prompt_mark_closes_reporting_at_its_stream_position() {
    let fixture = fixture(false);
    let now = Instant::now();
    assert!(fixture.apply(MOUSE_ON, now));
    assert!(fixture.reporting());
    assert!(fixture.apply(b"$ \x1b]133;A\x07", now));
    assert!(!fixture.reporting());
}

#[test]
fn guarded_output_delivers_split_prompt_reports_once_to_the_resume_controller() {
    use tterm_terminal::session_reports::SessionReport;

    let fixture = fixture(false);
    let (_, mut receiver, signal) = fixture.guard.state.reports.attach();
    let now = Instant::now();
    fixture.apply(b"\x1b]133;", now);
    assert!(!signal.at_prompt());
    fixture.apply(b"A\x07", now);
    assert!(signal.at_prompt());
    assert_eq!(receiver.try_recv().unwrap(), SessionReport::Prompt);
    fixture.apply(b"\x1b]133;C\x07", now);
    assert!(!signal.at_prompt());
    assert_eq!(receiver.try_recv().unwrap(), SessionReport::CommandStarted);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn osc_633_prompt_marks_close_reporting_too() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b]633;A\x1b\\", now);
    assert!(!fixture.reporting());
}

#[test]
fn a_prompt_mark_split_across_batches_still_closes_reporting() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b]13", now);
    assert!(fixture.reporting());
    fixture.apply(b"3;A\x07", now);
    assert!(!fixture.reporting());
}

#[test]
fn an_st_split_between_esc_and_backslash_is_applied_in_order() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b]133;A\x1b", now);
    assert!(fixture.reporting());
    fixture.apply(b"\\", now);
    assert!(!fixture.reporting());
}

#[test]
fn a_program_that_turns_reporting_on_after_the_prompt_keeps_it() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b]133;A\x07\x1b[?1000h\x1b[?1003h", now);
    assert!(fixture.reporting());
}

#[test]
fn a_prompt_followed_by_a_mode_enable_in_one_batch_stays_enabled() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(
        b"\x1b]133;A\x07\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h",
        now,
    );
    assert!(fixture.reporting());
}

#[test]
fn a_second_prompt_after_a_mode_enable_closes_it_again() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(b"\x1b]133;A\x07\x1b[?1002h\x1b[?1006h\x1b]133;A\x07", now);
    assert!(!fixture.reporting());
}

#[test]
fn prompt_attributes_longer_than_the_buffer_are_ignored() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let mut input = b"\x1b]133;A;".to_vec();
    input.extend(std::iter::repeat_n(b'x', 4096));
    input.push(0x07);
    assert!(fixture.apply(&input, now));
    assert!(!fixture.reporting());
}

#[test]
fn output_that_is_not_a_prompt_leaves_reporting_on() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b]133;C\x07\x1b]133;D;1\x07", now);
    assert!(fixture.reporting());
}

#[test]
fn output_settles_for_exactly_two_hundred_milliseconds() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    assert_eq!(fixture.guard.timeout_at(now), Some(SETTLE_DELAY));
    assert_eq!(
        fixture.guard.timeout_at(now + Duration::from_millis(199)),
        Some(Duration::from_millis(1))
    );
    assert!(
        fixture
            .guard
            .begin_check_at(now + Duration::from_millis(199))
            .is_none()
    );
    assert_eq!(
        fixture.guard.timeout_at(now + Duration::from_millis(199)),
        Some(Duration::from_millis(1)),
        "a sync timeout settling early must leave the mouse check scheduled"
    );
    assert!(fixture.guard.begin_check_at(now + SETTLE_DELAY).is_some());
    assert_eq!(fixture.guard.timeout_at(now + SETTLE_DELAY), None);
}

#[test]
fn a_foreground_shell_closes_stale_reporting() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.guard.resolve(token, true);
    assert!(!fixture.reporting());
}

#[test]
fn a_foreground_program_keeps_reporting() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.guard.resolve(token, false);
    assert!(fixture.reporting());
}

#[test]
fn an_answer_overtaken_by_output_is_dropped() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.apply(b"more output", now + SETTLE_DELAY);
    fixture.guard.resolve(token, true);
    assert!(fixture.reporting());
}

#[test]
fn a_prompt_during_a_check_keeps_reporting_off() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.apply(b"\x1b]133;A\x07", now + SETTLE_DELAY);
    assert!(!fixture.reporting());
    fixture.guard.resolve(token, true);
    assert!(!fixture.reporting());
}

#[test]
fn a_stale_session_nonce_cannot_apply_output_or_an_answer() {
    let mut fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.guard.state.session_nonce = 2;
    assert!(!fixture.apply(b"stale output", now));
    fixture.guard.resolve(token, true);
    assert!(fixture.reporting());
}

#[test]
fn a_closed_session_cannot_apply_output_or_an_answer() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    let token = fixture
        .guard
        .begin_check_at(now + SETTLE_DELAY)
        .expect("settled check");
    fixture.guard.state.alive.store(false, Ordering::Release);
    assert!(!fixture.apply(b"late output", now));
    fixture.guard.resolve(token, true);
    assert!(fixture.reporting());
}

#[test]
fn a_nonlocal_session_never_schedules_a_foreground_check() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    assert_eq!(fixture.guard.timeout_at(now), None);
    assert!(fixture.guard.begin_check_at(now + SETTLE_DELAY).is_none());
}

#[test]
fn no_check_is_scheduled_while_reporting_is_off() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(b"plain output", now);
    assert_eq!(fixture.guard.timeout_at(now), None);
    assert!(fixture.guard.begin_check_at(now + SETTLE_DELAY).is_none());
}

#[test]
fn a_mouse_off_sequence_cancels_the_settle_check() {
    let fixture = fixture(true);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1b[?1000l\x1b[?1002l\x1b[?1003l", now);
    assert!(!fixture.reporting());
    assert_eq!(fixture.guard.timeout_at(now), None);
}

#[test]
fn a_prompt_inside_a_dcs_after_esc_is_a_real_prompt() {
    let fixture = fixture(false);
    let now = Instant::now();
    fixture.apply(MOUSE_ON, now);
    fixture.apply(b"\x1bPpayload\x1b]133;A\x07", now);
    assert!(!fixture.reporting());
}
