use std::sync::Mutex;

/// Enough to keep a password prompt in view across a few tmux status-line
/// redraws, which arrive without a newline while the prompt waits.
const MAX_TAIL_BYTES: usize = 2048;

/// The most recent bytes of a session's output, so a saved-password write can
/// check that the prompt the frontend saw is still the one waiting for input.
#[derive(Debug, Default)]
pub struct OutputTail {
    bytes: Mutex<Vec<u8>>,
}

impl OutputTail {
    pub fn record(&self, data: &[u8]) {
        let Ok(mut bytes) = self.bytes.lock() else {
            return;
        };
        if data.len() >= MAX_TAIL_BYTES {
            bytes.clear();
            bytes.extend_from_slice(&data[data.len() - MAX_TAIL_BYTES..]);
            return;
        }
        let overflow = (bytes.len() + data.len()).saturating_sub(MAX_TAIL_BYTES);
        bytes.drain(..overflow);
        bytes.extend_from_slice(data);
    }

    /// True when `prompt` is the last line the session printed: it appears in
    /// the escape-stripped tail with no newline after it. Only prompts that end
    /// in a colon qualify, so a shell prompt can never receive the password.
    pub fn awaits_prompt(&self, prompt: &str) -> bool {
        let prompt = prompt.trim();
        if !(prompt.ends_with(':') || prompt.ends_with('：')) {
            return false;
        }
        let Ok(bytes) = self.bytes.lock() else {
            return false;
        };
        let text = strip_escape_sequences(&String::from_utf8_lossy(&bytes));
        text.rfind(prompt)
            .is_some_and(|index| !text[index + prompt.len()..].contains('\n'))
    }
}

/// Drops CSI, OSC, and two-byte escape sequences so cursor moves and colors
/// between the prompt's characters do not hide it.
fn strip_escape_sequences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for next in chars.by_ref() {
                    if ('\x40'..='\x7e').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(next) = chars.next() {
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "[sudo] password for stone:";

    #[test]
    fn accepts_prompt_as_last_line() {
        let tail = OutputTail::default();
        tail.record(b"$ sudo ls\r\n[sudo] password for stone: ");
        assert!(tail.awaits_prompt(PROMPT));
    }

    #[test]
    fn rejects_prompt_followed_by_newline() {
        let tail = OutputTail::default();
        tail.record(b"[sudo] password for stone: \r\nfile\r\n$ ");
        assert!(!tail.awaits_prompt(PROMPT));
    }

    #[test]
    fn accepts_prompt_split_by_escapes_and_status_redraw() {
        let tail = OutputTail::default();
        tail.record(b"\x1b[1m[sudo]\x1b[0m password for ");
        tail.record(b"stone: \x1b[?25l\x1b[24;1H\x1b]0;title\x07[0] bash\x1b[K\x1b[5;28H\x1b[?25h");
        assert!(tail.awaits_prompt(PROMPT));
    }

    #[test]
    fn accepts_localized_prompt_with_full_width_colon() {
        let tail = OutputTail::default();
        tail.record("[sudo] stone 的密码：".as_bytes());
        assert!(tail.awaits_prompt("[sudo] stone 的密码："));
    }

    #[test]
    fn rejects_prompt_without_colon() {
        let tail = OutputTail::default();
        tail.record(b"stone@host:~$ ");
        assert!(!tail.awaits_prompt("stone@host:~$"));
    }

    #[test]
    fn keeps_only_bounded_tail() {
        let tail = OutputTail::default();
        tail.record(PROMPT.as_bytes());
        tail.record(&vec![b'x'; MAX_TAIL_BYTES]);
        assert!(!tail.awaits_prompt(PROMPT));
        tail.record(PROMPT.as_bytes());
        assert!(tail.awaits_prompt(PROMPT));
    }
}
