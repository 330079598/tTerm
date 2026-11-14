//! Text the serial terminal echoes or timestamps, from `src/lib/serial.ts`.

use chrono::{DateTime, Local, Timelike};

use super::hex::format_hex_bytes;

fn format_clock(now: &DateTime<Local>) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        now.hour(),
        now.minute(),
        now.second(),
        now.nanosecond() / 1_000_000
    )
}

/// The dimmed time a line starts with when timestamps are on.
pub fn timestamp_prefix(now: &DateTime<Local>) -> String {
    format!("\x1b[2m[{}]\x1b[22m ", format_clock(now))
}

/// Adds the arrival time once to each line, including a line split across
/// output chunks.
#[derive(Debug, Clone)]
pub struct LineTimestamper {
    at_line_start: bool,
}

impl Default for LineTimestamper {
    fn default() -> Self {
        Self {
            at_line_start: true,
        }
    }
}

impl LineTimestamper {
    /// Starts the next output at a new line.
    pub fn start_line(&mut self) {
        self.at_line_start = true;
    }

    /// Stamps each line that starts in `text`.
    pub fn stamp(&mut self, text: &str, now: &DateTime<Local>) -> String {
        let stamp = timestamp_prefix(now);
        let mut stamped = String::new();
        for character in text.chars() {
            if self.at_line_start && !matches!(character, '\r' | '\n') {
                stamped.push_str(&stamp);
                self.at_line_start = false;
            }
            stamped.push(character);
            if character == '\n' {
                self.at_line_start = true;
            }
        }
        stamped
    }
}

/// What the terminal shows for typed text when the device does not echo it.
pub fn local_echo_text(data: &str) -> String {
    if data.starts_with('\x1b') {
        return String::new();
    }
    let normalized = data.replace("\r\n", "\r");
    let mut shown = String::new();
    for character in normalized.chars() {
        match character {
            '\r' => shown.push_str("\r\n"),
            '\x7f' | '\x08' => shown.push_str("\u{8} \u{8}"),
            _ => shown.push(character),
        }
    }
    shown
}

const CONTROL_ESCAPES: [(char, &str); 3] = [('\r', "\\r"), ('\n', "\\n"), ('\t', "\\t")];

/// Text as an echo line shows it: control characters as escapes.
pub fn visible_text(text: &str) -> String {
    let mut shown = String::new();
    for character in text.chars() {
        let code = character as u32;
        if code >= 0x20 && code != 0x7f {
            shown.push(character);
        } else if let Some((_, escaped)) = CONTROL_ESCAPES
            .iter()
            .find(|(control, _)| *control == character)
        {
            shown.push_str(escaped);
        } else {
            shown.push_str(&format!("\\x{code:02x}"));
        }
    }
    shown
}

/// What the send bar sends: text, or bytes it already encoded.
#[derive(Debug, Clone, Copy)]
pub enum SentEcho<'a> {
    Text(&'a str),
    Bytes(&'a [u8]),
}

/// The line the terminal shows for something the send bar sent.
pub fn sent_echo_line(sent: SentEcho<'_>, now: Option<&DateTime<Local>>) -> String {
    let shown = match sent {
        SentEcho::Text(text) => visible_text(text),
        SentEcho::Bytes(bytes) => format_hex_bytes(bytes),
    };
    let stamp = now.map(timestamp_prefix).unwrap_or_default();
    format!("{stamp}\x1b[36m→ {shown}\x1b[39m")
}

#[cfg(test)]
mod tests {
    use super::{LineTimestamper, SentEcho, local_echo_text, sent_echo_line};
    use chrono::{Local, TimeZone, Timelike};

    fn now() -> chrono::DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 1, 1, 9, 5, 7)
            .single()
            .expect("local time")
            .with_nanosecond(42_000_000)
            .expect("nanoseconds")
    }

    fn timestamp() -> &'static str {
        "\x1b[2m[09:05:07.042]\x1b[22m "
    }

    #[test]
    fn local_echo_starts_lines_and_rubs_out_backspace() {
        assert_eq!(local_echo_text("ls\r"), "ls\r\n");
        assert_eq!(local_echo_text("a\r\nb"), "a\r\nb");
        assert_eq!(local_echo_text("\x7f"), "\u{8} \u{8}");
    }

    #[test]
    fn local_echo_hides_the_terminals_own_replies() {
        assert_eq!(local_echo_text("\x1b[I"), "");
    }

    #[test]
    fn timestamps_each_line_once_even_split_across_chunks() {
        let mut stamp = LineTimestamper::default();
        assert_eq!(stamp.stamp("boot", &now()), format!("{}boot", timestamp()));
        assert_eq!(
            stamp.stamp("ing\r\nready\r\n", &now()),
            format!("ing\r\n{}ready\r\n", timestamp())
        );
        assert_eq!(stamp.stamp("> ", &now()), format!("{}> ", timestamp()));
    }

    #[test]
    fn leaves_empty_lines_alone() {
        assert_eq!(
            LineTimestamper::default().stamp("\r\n\r\n", &now()),
            "\r\n\r\n"
        );
    }

    #[test]
    fn stamps_after_a_line_written_by_someone_else() {
        let mut stamp = LineTimestamper::default();
        stamp.stamp("> ", &now());
        stamp.start_line();
        assert_eq!(stamp.stamp("ok", &now()), format!("{}ok", timestamp()));
    }

    #[test]
    fn shows_sent_text_with_control_characters_visible() {
        assert_eq!(
            sent_echo_line(SentEcho::Text("AT\r\x00"), None),
            "\x1b[36m→ AT\\r\\x00\x1b[39m"
        );
    }

    #[test]
    fn shows_sent_bytes_in_hex_after_the_time_when_asked() {
        assert_eq!(
            sent_echo_line(SentEcho::Bytes(&[0xaa, 0x05, 0x0d]), Some(&now())),
            format!("{}\x1b[36m→ AA 05 0D\x1b[39m", timestamp())
        );
    }
}
