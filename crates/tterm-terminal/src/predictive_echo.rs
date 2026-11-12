//! Predicts the echo of typing at a remote shell.
//!
//! The predictor follows plain typing and backspace on one line. Masked
//! prompts, full-screen programs and keys it cannot place stop it until the
//! line is ended. Drawing belongs to the native terminal view.

/// How predictive echo is enabled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PredictiveEchoMode {
    /// Show predictions only once a measured round trip is slow.
    #[default]
    Auto,
    /// Always show predictions.
    Always,
    /// Never show predictions.
    Off,
}

impl PredictiveEchoMode {
    /// Reads the persisted configuration value.
    pub fn from_config(value: &str) -> Self {
        match value {
            "always" => Self::Always,
            "off" => Self::Off,
            _ => Self::Auto,
        }
    }
}

/// What the predictor reads of the terminal.
///
/// Rows count from the top of the scrollback, as xterm's buffer does. A
/// native caller maps them from [`crate::snapshot::Snapshot`] grid lines.
pub trait PredictionScreen {
    /// The cursor's absolute row.
    fn cursor_row(&self) -> i64;
    /// The cursor's zero-based column.
    fn cursor_col(&self) -> usize;
    /// The number of columns.
    fn cols(&self) -> usize;
    /// The text in a cell, or a space when it is empty.
    fn char_at(&self, row: i64, col: usize) -> String;
}

/// A cell the predicted echo changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredictedCell {
    /// The zero-based column.
    pub col: usize,
    /// The character shown, or a space when the prediction erases a cell.
    pub char: String,
    /// Whether the echo is overdue.
    pub stalled: bool,
}

/// The predicted echo of what is typed but not yet echoed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PredictionView {
    /// The absolute row.
    pub row: i64,
    /// The cells to draw, in first-insertion order.
    pub cells: Vec<PredictedCell>,
    /// The column the predicted cursor moves to.
    pub cursor_col: usize,
}

/// A guess is marked as one once its echo is this late, or two round trips.
const STALL_MS: f64 = 500.0;
/// Guesses still not echoed this long after are dropped, or four round trips.
const TIMEOUT_MS: f64 = 2000.0;
/// The line stopped matching the guesses this long ago.
const DOUBT_MS: f64 = 150.0;

#[derive(Clone, Debug)]
struct Keystroke {
    col: usize,
    value: String,
    cursor_after: usize,
    at: f64,
    erases: bool,
}

fn is_predictable(character: char) -> bool {
    (' '..='~').contains(&character)
}

/// Whether `data` ends the line typed on: Enter, Ctrl+C, or Ctrl+D.
pub fn ends_line(data: &str) -> bool {
    data.chars()
        .any(|character| matches!(character, '\r' | '\n' | '\u{0003}' | '\u{0004}'))
}

/// Guesses the echo of keys typed at a remote shell's line editor.
pub struct EchoPredictor {
    now: Box<dyn Fn() -> f64>,
    row: Option<i64>,
    line_start: usize,
    base: usize,
    pending: Vec<Keystroke>,
    confirmed: bool,
    doubt_since: Option<f64>,
    srtt: Option<f64>,
    slow: bool,
    masked_row: Option<i64>,
    tentative: bool,
    suspect: Option<(i64, String)>,
}

impl EchoPredictor {
    /// Creates a predictor that reads time from `now`.
    pub fn new(now: impl Fn() -> f64 + 'static) -> Self {
        Self {
            now: Box::new(now),
            row: None,
            line_start: 0,
            base: 0,
            pending: Vec::new(),
            confirmed: false,
            doubt_since: None,
            srtt: None,
            slow: false,
            masked_row: None,
            tentative: false,
            suspect: None,
        }
    }

    /// The smoothed time from a key to its echo, once one was measured.
    pub fn round_trip_ms(&self) -> Option<f64> {
        self.srtt
    }

    /// The round trip is over `threshold_ms`, with hysteresis below two
    /// thirds of it.
    pub fn slower_than(&mut self, threshold_ms: f64) -> bool {
        let Some(srtt) = self.srtt else {
            return false;
        };
        if srtt > threshold_ms {
            self.slow = true;
        } else if srtt < threshold_ms * 2.0 / 3.0 {
            self.slow = false;
        }
        self.slow
    }

    /// The row being predicted on, or none.
    pub fn active_row(&self) -> Option<i64> {
        self.row
    }

    /// Whether any keys wait for an echo or the line is in doubt.
    pub fn busy(&self) -> bool {
        !self.pending.is_empty() || self.doubt_since.is_some()
    }

    /// Forgets every guess. Keys sent meanwhile may still arrive, so guesses
    /// after this are placed tentatively.
    pub fn reset(&mut self) {
        self.clear();
        self.tentative = true;
    }

    /// Notes `data` typed, before it is sent.
    pub fn type_input(&mut self, data: &str, screen: &impl PredictionScreen) {
        if ends_line(data) {
            self.masked_row = None;
            self.suspect = None;
        }
        for character in data.chars() {
            let typed = if character == '\u{007F}' || character == '\u{0008}' {
                self.erase(screen, (self.now)())
            } else {
                is_predictable(character) && self.insert(character, screen, (self.now)())
            };
            if !typed {
                self.reset();
                return;
            }
        }
    }

    /// Checks the guesses against the terminal after output was parsed.
    pub fn reconcile(&mut self, screen: &impl PredictionScreen) {
        let Some(row) = self.row else {
            return;
        };
        if self.pending.is_empty() {
            return;
        }
        let echoed = if screen.cursor_row() == row {
            self.pending.iter().position(|key| {
                key.cursor_after == screen.cursor_col() && screen.char_at(row, key.col) == key.value
            })
        } else {
            None
        };
        let Some(echoed) = echoed else {
            if self.tentative {
                self.place(row, screen);
                return;
            }
            if self.echoed_as(row, screen).is_some() {
                self.clear();
                self.masked_row = Some(row);
                return;
            }
            let waiting = screen.cursor_row() == row && screen.cursor_col() == self.base;
            self.doubt_since = if waiting {
                None
            } else {
                Some(self.doubt_since.unwrap_or_else(|| (self.now)()))
            };
            return;
        };
        let key = self.pending[echoed].clone();
        self.sample((self.now)() - key.at);
        let masked = Some(row) == self.masked_row
            || self
                .suspect
                .as_ref()
                .is_some_and(|(suspect_row, character)| {
                    *suspect_row == row && *character == key.value
                });
        if !key.erases && !masked {
            self.confirmed = true;
        }
        self.tentative = false;
        self.doubt_since = None;
        self.base = key.cursor_after;
        self.pending.drain(..=echoed);
    }

    /// Drops guesses the remote never confirmed.
    pub fn expire(&mut self, screen: &impl PredictionScreen) {
        let now = (self.now)();
        if let Some(doubt_since) = self.doubt_since
            && now - doubt_since > DOUBT_MS
        {
            self.reset();
            return;
        }
        let Some(oldest) = self.pending.first().cloned() else {
            return;
        };
        if now - oldest.at <= TIMEOUT_MS.max(4.0 * self.srtt.unwrap_or(0.0)) {
            return;
        }
        if self.matches(screen) {
            self.base = screen.cursor_col();
            self.pending.clear();
        } else {
            self.clear();
        }
        self.tentative = false;
    }

    /// The guesses to show, or none while none may be shown.
    pub fn view(&self) -> Option<PredictionView> {
        if self.row.is_none() || !self.confirmed || self.tentative || self.doubt_since.is_some() {
            return None;
        }
        let last = self.pending.last()?;
        let now = (self.now)();
        let stall_ms = STALL_MS.max(2.0 * self.srtt.unwrap_or(0.0));
        let mut cells: Vec<PredictedCell> = Vec::new();
        for key in &self.pending {
            let cell = PredictedCell {
                col: key.col,
                char: key.value.clone(),
                stalled: now - key.at > stall_ms,
            };
            match cells.iter_mut().find(|existing| existing.col == key.col) {
                Some(existing) => *existing = cell,
                None => cells.push(cell),
            }
        }
        Some(PredictionView {
            row: self.row?,
            cells,
            cursor_col: last.cursor_after,
        })
    }

    fn clear(&mut self) {
        self.row = None;
        self.pending.clear();
        self.confirmed = false;
        self.doubt_since = None;
    }

    fn place(&mut self, row: i64, screen: &impl PredictionScreen) {
        if let Some(other) = self.echoed_as(row, screen) {
            self.suspect = Some((row, other));
        }
        let keep_from = self.echoed_before(screen);
        let keys = self.pending.split_off(keep_from);
        self.pending.clear();
        if screen.cursor_row() != row {
            self.row = None;
        }
        for key in keys {
            let placed = if key.erases {
                self.erase(screen, key.at)
            } else {
                self.insert_char(&key.value, screen, key.at)
            };
            if !placed {
                self.reset();
                return;
            }
        }
    }

    fn echoed_before(&self, screen: &impl PredictionScreen) -> usize {
        let first_erase = self.pending.iter().position(|key| key.erases);
        let typed = first_erase.unwrap_or(self.pending.len());
        let mut count = typed.min(screen.cursor_col());
        while count > 0 {
            let start = screen.cursor_col() - count;
            let shown = self.pending[..count]
                .iter()
                .enumerate()
                .all(|(index, key)| {
                    screen.char_at(screen.cursor_row(), start + index) == key.value
                });
            if shown {
                return count;
            }
            count -= 1;
        }
        0
    }

    fn insert(&mut self, character: char, screen: &impl PredictionScreen, at: f64) -> bool {
        self.insert_char(&character.to_string(), screen, at)
    }

    fn insert_char(&mut self, character: &str, screen: &impl PredictionScreen, at: f64) -> bool {
        let col = self.next_col(screen);
        if col >= screen.cols().saturating_sub(1) {
            return false;
        }
        let current = self.value_at(col, screen);
        if current != " " && current != character {
            return false;
        }
        self.pending.push(Keystroke {
            col,
            value: character.to_owned(),
            cursor_after: col + 1,
            at,
            erases: false,
        });
        true
    }

    fn erase(&mut self, screen: &impl PredictionScreen, at: f64) -> bool {
        let col = self.next_col(screen);
        if col <= self.line_start {
            return false;
        }
        self.pending.push(Keystroke {
            col: col - 1,
            value: " ".to_owned(),
            cursor_after: col - 1,
            at,
            erases: true,
        });
        true
    }

    fn next_col(&mut self, screen: &impl PredictionScreen) -> usize {
        if let Some(last) = self.pending.last() {
            return last.cursor_after;
        }
        if self.row != Some(screen.cursor_row()) {
            self.clear();
            self.row = Some(screen.cursor_row());
            self.line_start = screen.cursor_col();
        }
        self.line_start = self.line_start.min(screen.cursor_col());
        self.base = screen.cursor_col();
        self.doubt_since = None;
        screen.cursor_col()
    }

    fn value_at(&self, col: usize, screen: &impl PredictionScreen) -> String {
        for key in self.pending.iter().rev() {
            if key.col == col {
                return key.value.clone();
            }
        }
        screen.char_at(self.row.unwrap_or_else(|| screen.cursor_row()), col)
    }

    fn echoed_as(&self, row: i64, screen: &impl PredictionScreen) -> Option<String> {
        if screen.cursor_row() != row {
            return None;
        }
        self.pending
            .iter()
            .find(|key| {
                !key.erases
                    && key.cursor_after == screen.cursor_col()
                    && screen.char_at(row, key.col) != key.value
            })
            .map(|key| screen.char_at(row, key.col))
    }

    fn matches(&self, screen: &impl PredictionScreen) -> bool {
        let Some(last) = self.pending.last() else {
            return true;
        };
        let Some(row) = self.row else {
            return true;
        };
        if screen.cursor_row() != row || screen.cursor_col() != last.cursor_after {
            return false;
        }
        self.pending
            .iter()
            .all(|key| screen.char_at(row, key.col) == self.value_at(key.col, screen))
    }

    fn sample(&mut self, rtt_ms: f64) {
        self.srtt = Some(match self.srtt {
            None => rtt_ms,
            Some(srtt) => srtt + (rtt_ms - srtt) / 8.0,
        });
    }
}

/// The terminal state that decides whether prediction is allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredictiveEchoState {
    /// Whether the program hid the cursor.
    pub cursor_hidden: bool,
    /// Whether the normal screen is active.
    pub normal_screen: bool,
    /// Whether the program enabled mouse tracking.
    pub mouse_tracking: bool,
    /// Whether typing may echo at the cursor.
    pub can_predict: bool,
}

/// Connects the predictor to the rules that decide when its view is shown.
pub struct PredictiveEcho {
    predictor: EchoPredictor,
    mode: PredictiveEchoMode,
    threshold_ms: f64,
    prompt_row: Option<i64>,
}

impl PredictiveEcho {
    /// Creates the predictor for one terminal.
    pub fn new(
        now: impl Fn() -> f64 + 'static,
        mode: PredictiveEchoMode,
        threshold_ms: f64,
    ) -> Self {
        Self {
            predictor: EchoPredictor::new(now),
            mode,
            threshold_ms,
            prompt_row: None,
        }
    }

    /// Replaces the configured mode and threshold.
    pub fn configure(&mut self, mode: PredictiveEchoMode, threshold_ms: f64) {
        self.mode = mode;
        self.threshold_ms = threshold_ms;
    }

    /// Notes input before it is sent and returns the view to draw.
    pub fn note_input(
        &mut self,
        data: &str,
        screen: &impl PredictionScreen,
        state: PredictiveEchoState,
    ) -> Option<PredictionView> {
        if ends_line(data) {
            self.prompt_row = None;
        } else if !state.can_predict {
            self.prompt_row = Some(screen.cursor_row());
        }
        if !self.allowed(state) || Some(screen.cursor_row()) == self.prompt_row {
            // The key went out unguessed, so keys after it must be placed
            // tentatively even when there was no row to forget.
            self.predictor.reset();
            return None;
        }
        self.predictor.type_input(data, screen);
        self.render(screen)
    }

    /// Reconciles after the terminal parsed output and returns the view.
    pub fn on_output_parsed(&mut self, screen: &impl PredictionScreen) -> Option<PredictionView> {
        self.predictor.reconcile(screen);
        self.render(screen)
    }

    /// Drops predictions after a resize, buffer change or marker move.
    pub fn reset(&mut self, screen: &impl PredictionScreen) -> Option<PredictionView> {
        self.predictor.reset();
        self.render(screen)
    }

    /// Expires overdue guesses and returns the view.
    pub fn tick(&mut self, screen: &impl PredictionScreen) -> Option<PredictionView> {
        self.predictor.expire(screen);
        self.render(screen)
    }

    /// The current view, if the mode allows one.
    pub fn render(&mut self, _screen: &impl PredictionScreen) -> Option<PredictionView> {
        let shown = self.mode == PredictiveEchoMode::Always
            || (self.mode == PredictiveEchoMode::Auto
                && self.predictor.slower_than(self.threshold_ms));
        shown.then(|| self.predictor.view()).flatten()
    }

    /// Whether prediction still waits for an echo or is in doubt.
    pub fn busy(&self) -> bool {
        self.predictor.busy()
    }

    /// The predictor, for callers that need its measured round trip.
    pub fn predictor(&self) -> &EchoPredictor {
        &self.predictor
    }

    fn allowed(&self, state: PredictiveEchoState) -> bool {
        self.mode != PredictiveEchoMode::Off
            && !state.cursor_hidden
            && state.normal_screen
            && !state.mouse_tracking
            && state.can_predict
    }
}

#[cfg(test)]
mod tests;
