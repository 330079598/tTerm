//! Answers host-key and authentication questions emitted by core SSH sessions.

use std::sync::{Arc, Mutex};

use tterm_core::events::ssh::{AuthPromptField, AuthPromptPayload, SshHostKeyPromptPayload};
use tterm_core::prompts::PromptBroker;
use tterm_terminal::terminal::Terminal;

use super::pipeline::RefreshSignal;

/// Turns keyboard input into prompt replies until the session is connected.
pub(super) struct PromptControl {
    state: Mutex<PromptState>,
    model: Arc<Terminal>,
    prompts: Arc<PromptBroker>,
    refresh: Arc<RefreshSignal>,
}

enum PromptState {
    Idle,
    HostKey {
        request_id: String,
        input: Vec<u8>,
    },
    Auth {
        request_id: String,
        fields: Vec<AuthPromptField>,
        answers: Vec<String>,
        field: usize,
        input: Vec<u8>,
    },
}

enum Submission {
    HostKey {
        request_id: String,
        trust: bool,
    },
    Auth {
        request_id: String,
        answers: Vec<String>,
    },
    Cancel {
        request_id: String,
        auth: bool,
    },
}

impl PromptControl {
    pub(super) fn new(
        model: Arc<Terminal>,
        prompts: Arc<PromptBroker>,
        refresh: Arc<RefreshSignal>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(PromptState::Idle),
            model,
            prompts,
            refresh,
        })
    }

    pub(super) fn show_host_key(&self, payload: SshHostKeyPromptPayload) {
        let mut text = format!(
            "\r\nHost key for {} ({}) at {}:{}\r\n{}\r\nFingerprint: {}\r\n",
            payload.profile_name,
            payload.algorithm,
            payload.host,
            payload.port,
            payload.reason,
            payload.fingerprint
        );
        if let Some(known) = payload.known_fingerprint {
            text.push_str(&format!("Previously saved: {known}\r\n"));
        }
        text.push_str("Trust this host key? [y/N] ");
        self.write(text.as_bytes());
        if let Ok(mut state) = self.state.lock() {
            *state = PromptState::HostKey {
                request_id: payload.request_id,
                input: Vec::new(),
            };
        }
        self.notify();
    }

    pub(super) fn show_auth(&self, payload: AuthPromptPayload) {
        let mut text = format!(
            "\r\nAuthentication for {}@{}:{}",
            payload.username, payload.host, payload.port
        );
        if payload.retry {
            text.push_str(" (the previous answer was rejected)");
        }
        if !payload.name.trim().is_empty() {
            text.push_str("\r\n");
            text.push_str(payload.name.trim());
        }
        if !payload.instructions.trim().is_empty() {
            text.push_str("\r\n");
            text.push_str(payload.instructions.trim());
        }
        if let Some(field) = payload.prompts.first() {
            text.push_str("\r\n");
            text.push_str(field.text.trim());
            if field.text.trim().is_empty() {
                text.push_str(if field.echo {
                    "Response: "
                } else {
                    "Password: "
                });
            }
        }
        self.write(text.as_bytes());
        if let Ok(mut state) = self.state.lock() {
            *state = PromptState::Auth {
                request_id: payload.request_id,
                fields: payload.prompts,
                answers: Vec::new(),
                field: 0,
                input: Vec::new(),
            };
        }
        self.notify();
    }

    pub(super) fn expire(&self, request_id: &str, auth: bool) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        let current = match &*state {
            PromptState::HostKey {
                request_id: current,
                ..
            }
            | PromptState::Auth {
                request_id: current,
                ..
            } => current,
            PromptState::Idle => return,
        };
        if current != request_id {
            return;
        }
        *state = PromptState::Idle;
        let text = if auth {
            "\r\nAuthentication prompt expired.\r\n"
        } else {
            "\r\nHost-key prompt expired.\r\n"
        };
        self.write(text.as_bytes());
        self.notify();
    }

    /// Consumes input while a prompt is active. Returns false when the bytes
    /// should continue to the session.
    pub(super) fn handle_input(&self, bytes: &[u8]) -> bool {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => return false,
        };
        if matches!(*state, PromptState::Idle) {
            return false;
        }

        let mut submission = None;
        for &byte in bytes {
            match byte {
                0x03 => {
                    submission = cancel(&mut state, &self.model);
                    break;
                }
                b'\r' | b'\n' => {
                    submission = submit(&mut state, &self.model);
                    if submission.is_some() {
                        break;
                    }
                }
                0x08 | 0x7f => erase(&mut state, &self.model),
                byte if byte >= 0x20 => {
                    append(&mut state, byte, &self.model);
                }
                _ => {}
            }
        }
        drop(state);

        match submission {
            Some(Submission::HostKey { request_id, trust }) => {
                let _ = self.prompts.reply_host_key(&request_id, trust);
            }
            Some(Submission::Auth {
                request_id,
                answers,
            }) => {
                let _ = self.prompts.reply_auth(&request_id, Some(answers));
            }
            Some(Submission::Cancel { request_id, auth }) => {
                if auth {
                    let _ = self.prompts.reply_auth(&request_id, None);
                } else {
                    let _ = self.prompts.reply_host_key(&request_id, false);
                }
            }
            None => {}
        }
        self.notify();
        true
    }

    /// Whether no host-key or authentication question is waiting.
    pub(super) fn is_idle(&self) -> bool {
        self.state
            .lock()
            .is_ok_and(|state| matches!(*state, PromptState::Idle))
    }

    fn write(&self, bytes: &[u8]) {
        self.model.write(bytes);
    }

    fn notify(&self) {
        self.refresh.notify();
    }
}

fn append(state: &mut PromptState, byte: u8, model: &Terminal) {
    let echo = match state {
        PromptState::HostKey { input, .. } => {
            input.push(byte);
            true
        }
        PromptState::Auth {
            fields,
            field,
            input,
            ..
        } => {
            input.push(byte);
            fields.get(*field).is_some_and(|prompt| prompt.echo)
        }
        PromptState::Idle => false,
    };
    if echo {
        model.write(&[byte]);
    }
}

fn erase(state: &mut PromptState, model: &Terminal) {
    let echo = match state {
        PromptState::HostKey { input, .. } => input.pop().is_some(),
        PromptState::Auth {
            fields,
            field,
            input,
            ..
        } => {
            if input.pop().is_none() {
                false
            } else {
                while input.last().is_some_and(|byte| byte & 0xc0 == 0x80) {
                    input.pop();
                }
                fields.get(*field).is_some_and(|prompt| prompt.echo)
            }
        }
        PromptState::Idle => false,
    };
    if echo {
        model.write(b"\x08 \x08");
    }
}

fn submit(state: &mut PromptState, model: &Terminal) -> Option<Submission> {
    match state {
        PromptState::HostKey { request_id, input } => {
            let answer = String::from_utf8_lossy(input).trim().to_ascii_lowercase();
            if !matches!(answer.as_str(), "" | "y" | "yes" | "n" | "no") {
                model.write(b"\r\nType y to trust it or n to cancel: ");
                input.clear();
                return None;
            }
            let trust = matches!(answer.as_str(), "y" | "yes");
            let request_id = std::mem::take(request_id);
            *state = PromptState::Idle;
            model.write(b"\r\n");
            Some(Submission::HostKey { request_id, trust })
        }
        PromptState::Auth {
            request_id,
            fields,
            answers,
            field,
            input,
        } => {
            answers.push(String::from_utf8_lossy(input).into_owned());
            input.clear();
            model.write(b"\r\n");
            *field += 1;
            if let Some(next) = fields.get(*field) {
                model.write(next.text.trim().as_bytes());
                if next.text.trim().is_empty() {
                    model.write(if next.echo {
                        b"Response: "
                    } else {
                        b"Password: "
                    });
                }
                return None;
            }
            let request_id = std::mem::take(request_id);
            let answers = std::mem::take(answers);
            *state = PromptState::Idle;
            Some(Submission::Auth {
                request_id,
                answers,
            })
        }
        PromptState::Idle => None,
    }
}

fn cancel(state: &mut PromptState, model: &Terminal) -> Option<Submission> {
    let (request_id, auth) = match state {
        PromptState::HostKey { request_id, .. } => (std::mem::take(request_id), false),
        PromptState::Auth { request_id, .. } => (std::mem::take(request_id), true),
        PromptState::Idle => return None,
    };
    *state = PromptState::Idle;
    model.write(b"\r\n");
    Some(Submission::Cancel { request_id, auth })
}

#[cfg(test)]
mod tests {
    use super::{PromptControl, PromptState};
    use std::sync::Arc;
    use tterm_core::events::{AppEvent, EventSink};
    use tterm_core::prompts::PromptBroker;
    use tterm_terminal::grid::{CellSize, GridSize};
    use tterm_terminal::terminal::{Config, Terminal};

    fn control() -> PromptControl {
        let model = Arc::new(Terminal::new(
            Config::default(),
            GridSize {
                columns: 80,
                screen_lines: 24,
            },
            CellSize {
                width: 8,
                height: 16,
            },
        ));
        let events = Arc::new(NoopEvents);
        let prompts = Arc::new(PromptBroker::new(events));
        let refresh = Arc::new(crate::terminal_demo::pipeline::RefreshSignal::new());
        Arc::try_unwrap(PromptControl::new(model, prompts, refresh))
            .unwrap_or_else(|_| panic!("single prompt control"))
    }

    struct NoopEvents;

    impl EventSink for NoopEvents {
        fn emit(&self, _: AppEvent) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn idle_input_continues_to_the_session() {
        assert!(!control().handle_input(b"ls\r"));
    }

    #[test]
    fn prompt_input_is_consumed_even_when_it_is_an_invalid_answer() {
        let control = control();
        if let Ok(mut state) = control.state.lock() {
            *state = PromptState::HostKey {
                request_id: "host".to_string(),
                input: Vec::new(),
            };
        }
        assert!(control.handle_input(b"maybe\r"));
        assert!(matches!(
            *control.state.lock().expect("prompt state"),
            PromptState::HostKey { .. }
        ));
    }

    #[test]
    fn a_valid_host_key_answer_finishes_the_prompt() {
        let control = control();
        if let Ok(mut state) = control.state.lock() {
            *state = PromptState::HostKey {
                request_id: "host".to_string(),
                input: Vec::new(),
            };
        }

        assert!(control.handle_input(b"yes\r"));
        assert!(matches!(
            *control.state.lock().expect("prompt state"),
            PromptState::Idle
        ));
    }

    #[test]
    fn cancelling_a_prompt_releases_input_for_the_session() {
        let control = control();
        if let Ok(mut state) = control.state.lock() {
            *state = PromptState::Auth {
                request_id: "auth".to_string(),
                fields: Vec::new(),
                answers: Vec::new(),
                field: 0,
                input: Vec::new(),
            };
        }

        assert!(control.handle_input(&[0x03]));
        assert!(matches!(
            *control.state.lock().expect("prompt state"),
            PromptState::Idle
        ));
        assert!(!control.handle_input(b"ls\r"));
    }
}
