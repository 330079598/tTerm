//! The plain-text log: typed lines, screen output and events in the order
//! they happened.

use super::screen::{OutputLog, ScreenLine};
use super::MAX_QUEUED_PLAIN_LINES;
use chrono::{DateTime, Local};
use std::collections::VecDeque;
use vte::{Params, Perform};

/// The plain-text log's lines in the order things happened. An output line
/// is known only once it leaves the screen, so typed lines and events wait
/// until every output line that started before them has been written.
pub(super) struct PlainLog {
    input: InputStream,
    output: OutputLog,
    queue: VecDeque<(DateTime<Local>, String)>,
    timestamps: bool,
}

impl PlainLog {
    pub(super) fn new(rows: u16, cols: u16, timestamps: bool) -> Self {
        Self {
            input: InputStream::new(),
            output: OutputLog::new(rows, cols),
            queue: VecDeque::new(),
            timestamps,
        }
    }

    /// `[time] [OUTPUT] text`, or without the time just the text; typed lines
    /// and events keep their tag either way.
    fn line(&self, at: DateTime<Local>, direction: &str, text: &str) -> String {
        match (self.timestamps, direction) {
            (true, _) => format!("[{}] [{direction}] {text}\n", plain_timestamp(at)),
            (false, "OUTPUT") => format!("{text}\n"),
            (false, _) => format!("[{direction}] {text}\n"),
        }
    }

    fn event_line(
        &self,
        at: DateTime<Local>,
        event_type: &str,
        detail: &serde_json::Value,
    ) -> String {
        self.line(at, "EVENT", &format!("{event_type} {detail}"))
    }

    pub(super) fn input(&mut self, at: DateTime<Local>, data: &[u8]) -> Vec<String> {
        for line in self.input.advance(data) {
            let line = self.line(at, "INPUT", &line);
            self.queue.push_back((at, line));
        }
        self.settle(Vec::new())
    }

    pub(super) fn output(&mut self, at: DateTime<Local>, data: &[u8]) -> Vec<String> {
        let lines = self.output.advance(at, data);
        let ready = self.place(lines);
        self.settle(ready)
    }

    pub(super) fn event(
        &mut self,
        at: DateTime<Local>,
        event_type: &str,
        detail: &serde_json::Value,
    ) -> Vec<String> {
        let line = self.event_line(at, event_type, detail);
        self.queue.push_back((at, line));
        self.settle(Vec::new())
    }

    pub(super) fn resize(&mut self, at: DateTime<Local>, rows: u16, cols: u16) -> Vec<String> {
        let lines = self.output.resize(at, rows, cols);
        let ready = self.place(lines);
        self.settle(ready)
    }

    /// Everything left: the line being typed and what is still on screen.
    pub(super) fn finish(&mut self, at: DateTime<Local>) -> Vec<String> {
        for line in self.input.finish() {
            let line = self.line(at, "INPUT", &line);
            self.queue.push_back((at, line));
        }
        let lines = self.output.finish(at);
        let ready = self.place(lines);
        self.settle(ready)
    }

    /// Takes queued lines from the front while `due` says so, or while the
    /// queue is over its limit.
    fn take_queued(&mut self, ready: &mut Vec<String>, due: impl Fn(DateTime<Local>) -> bool) {
        while let Some((at, _)) = self.queue.front() {
            if !due(*at) && self.queue.len() <= MAX_QUEUED_PLAIN_LINES {
                break;
            }
            let (_, line) = self.queue.pop_front().expect("front checked");
            ready.push(line);
        }
    }

    /// Output lines in order, each after the queued lines older than it.
    fn place(&mut self, lines: Vec<ScreenLine>) -> Vec<String> {
        let mut ready = Vec::new();
        for line in lines {
            match line {
                ScreenLine::Text { at, text } => {
                    self.take_queued(&mut ready, |queued| queued <= at);
                    ready.push(if text.is_empty() {
                        "\n".to_string()
                    } else {
                        self.line(at, "OUTPUT", &text)
                    });
                }
                ScreenLine::Event { at, name } => {
                    let line = self.event_line(at, name, &serde_json::json!({}));
                    self.queue.push_back((at, line));
                }
            }
        }
        ready
    }

    /// Adds the queued lines that no output line still on screen can precede.
    fn settle(&mut self, mut ready: Vec<String>) -> Vec<String> {
        match self.output.oldest_pending() {
            Some(oldest) => self.take_queued(&mut ready, |queued| queued < oldest),
            None => self.take_queued(&mut ready, |_| true),
        }
        ready
    }
}

/// The submitted lines in keyboard input.
pub(super) struct InputStream {
    parser: vte::Parser,
    line: InputLine,
}

impl InputStream {
    pub(super) fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            line: InputLine::default(),
        }
    }

    pub(super) fn advance(&mut self, data: &[u8]) -> Vec<String> {
        self.parser.advance(&mut self.line, data);
        std::mem::take(&mut self.line.completed)
    }

    pub(super) fn finish(&mut self) -> Vec<String> {
        if self
            .line
            .text
            .iter()
            .any(|character| !character.is_whitespace())
        {
            self.line.complete();
        }
        std::mem::take(&mut self.line.completed)
    }
}

/// The line being typed, edited by the keys that edit it.
#[derive(Default)]
struct InputLine {
    text: Vec<char>,
    cursor: usize,
    completed: Vec<String>,
    just_committed_cr: bool,
}

impl InputLine {
    fn complete(&mut self) {
        let line = self.text.iter().collect::<String>();
        self.completed.push(line.trim_end().to_string());
        self.text.clear();
        self.cursor = 0;
    }

    fn first_param(params: &Params, default: u16) -> usize {
        params
            .iter()
            .next()
            .and_then(|param| param.first())
            .copied()
            .filter(|value| *value != 0)
            .unwrap_or(default) as usize
    }
}

impl InputLine {
    fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            if self.cursor < self.text.len() {
                self.text.remove(self.cursor);
            }
        }
    }
}

impl Perform for InputLine {
    fn print(&mut self, character: char) {
        self.just_committed_cr = false;
        // The parser hands DEL, the usual Backspace key, over as printable.
        if character == '\x7f' {
            self.backspace();
            return;
        }
        if self.cursor < self.text.len() {
            self.text[self.cursor] = character;
        } else {
            self.text.resize(self.cursor, ' ');
            self.text.push(character);
        }
        self.cursor = self.cursor.saturating_add(1);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' if self.just_committed_cr => self.just_committed_cr = false,
            b'\n' => self.complete(),
            b'\r' => {
                if self.text.is_empty() {
                    self.completed.push("<Enter>".to_string());
                } else {
                    self.complete();
                }
                self.just_committed_cr = true;
            }
            0x08 => self.backspace(),
            0x03 => {
                if !self.text.is_empty() {
                    self.complete();
                }
                self.completed.push("<Ctrl+C>".to_string());
            }
            0x04 => self.completed.push("<Ctrl+D>".to_string()),
            0x0c => self.completed.push("<Ctrl+L>".to_string()),
            _ => {}
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'C' => {
                self.cursor = (self.cursor + Self::first_param(params, 1)).min(self.text.len());
            }
            'D' => self.cursor = self.cursor.saturating_sub(Self::first_param(params, 1)),
            'H' => self.cursor = 0,
            'F' => self.cursor = self.text.len(),
            // Delete.
            '~' if Self::first_param(params, 0) == 3 && self.cursor < self.text.len() => {
                self.text.remove(self.cursor);
            }
            _ => {}
        }
    }
}

fn plain_timestamp(at: DateTime<Local>) -> impl std::fmt::Display {
    at.format("%Y-%m-%d %H:%M:%S%.3f")
}
