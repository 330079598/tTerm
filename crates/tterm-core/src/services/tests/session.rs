//! Integration tests through the client-neutral `AppServices` session API.

use crate::events::{AppEvent, EventSink};
use crate::prompts::SessionKey;
use crate::services::test_support;
use crate::session::sink::SessionSink;
use crate::terminal::io_batcher::OutputFlow;
use crate::terminal::output::TerminalOutput;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(15);

#[derive(Default)]
struct SinkState {
    output: Vec<u8>,
    events: Vec<AppEvent>,
    exited: bool,
    exit_reason: Option<String>,
    closed: bool,
    input_notes: usize,
}

#[derive(Default)]
struct TestSink {
    state: Mutex<SinkState>,
    changed: Condvar,
    output: Mutex<Option<FinaleOutput>>,
}

type FinaleOutput = TerminalOutput;

impl TestSink {
    fn wait_for(&self, predicate: impl Fn(&SinkState) -> bool) -> bool {
        let deadline = Instant::now() + WAIT;
        let mut state = self.state.lock().expect("sink state");
        while !predicate(&state) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, remaining)
                .expect("wait for sink");
            state = next;
        }
        true
    }

    fn output_contains(&self, needle: &[u8]) -> bool {
        self.wait_for(|state| {
            state
                .output
                .windows(needle.len())
                .any(|window| window == needle)
        })
    }
}

impl EventSink for TestSink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.state.lock().expect("sink state").events.push(event);
        self.changed.notify_all();
        Ok(())
    }
}

impl SessionSink for TestSink {
    fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), String> {
        let output = self
            .output
            .lock()
            .expect("output handle")
            .as_ref()
            .cloned()
            .ok_or("output already released")?;
        output.send(bytes.clone())?;
        output.flow().ack(bytes.len());
        self.state.lock().expect("sink state").output.extend(bytes);
        self.changed.notify_all();
        Ok(())
    }

    fn state_changed(
        &self,
        _: crate::events::ssh::SshConnectionProgressPayload,
    ) -> Result<(), String> {
        Ok(())
    }

    fn exited(&self, reason: Option<String>) -> Result<(), String> {
        let mut state = self.state.lock().expect("sink state");
        state.exited = true;
        state.exit_reason = reason;
        drop(state);
        self.changed.notify_all();
        Ok(())
    }

    fn ack_output(&self, _: usize) {}

    fn close_output(&self) {
        self.output.lock().expect("output handle").take();
        self.state.lock().expect("sink state").closed = true;
        self.changed.notify_all();
    }

    fn note_input(&self) {
        self.state.lock().expect("sink state").input_notes += 1;
        self.changed.notify_all();
    }

    fn answers_input(&self) -> bool {
        false
    }
}

fn local_connection() -> crate::core::session::types::PtyConnectionOptions {
    #[cfg(windows)]
    let (path, args) = (
        std::env::var("COMSPEC").unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".into()),
        "/Q",
    );
    #[cfg(not(windows))]
    let (path, args) = ("/bin/sh".to_string(), "-i");

    crate::core::session::types::PtyConnectionOptions {
        connection_type: Some("terminal".into()),
        terminal_shell: Some("custom".into()),
        terminal_shell_custom_path: Some(path),
        terminal_shell_custom_args: Some(args.into()),
        session_log: Some("never".into()),
        ..Default::default()
    }
}

fn print_command(marker: &str) -> String {
    #[cfg(windows)]
    return format!("echo {marker}\r\n");
    #[cfg(not(windows))]
    return format!("printf '{marker}\\n'\n");
}

fn exit_command() -> &'static str {
    #[cfg(windows)]
    return "exit\r\n";
    #[cfg(not(windows))]
    return "exit\n";
}

fn start(
    app: &test_support::TestApp,
    sink: Arc<dyn SessionSink>,
    output: FinaleOutput,
    tab_id: &str,
) -> (u32, SessionKey) {
    let key = SessionKey {
        tab_id: tab_id.into(),
        session_nonce: 12,
    };
    let pid = app
        .services
        .create_pty_blocking(
            crate::core::create::PtyIdentity {
                tab_id: key.tab_id.clone(),
                session_nonce: key.session_nonce,
                rows: 24,
                cols: 80,
            },
            Some(local_connection()),
            output,
            sink,
        )
        .expect("create local PTY");
    (pid, key)
}

fn sink_and_output() -> (FinaleOutput, Arc<TestSink>) {
    let sink = Arc::new(TestSink::default());
    let output = FinaleOutput::new(
        Arc::new(|_: Vec<u8>| Ok(())),
        Arc::new(OutputFlow::default()),
    );
    *sink.output.lock().expect("output handle") = Some(output.clone());
    (output, sink)
}

#[test]
fn app_services_local_pty_starts_round_trips_completes_and_releases() {
    let app = test_support::app();
    let (output, sink) = sink_and_output();
    let typed_sink: Arc<dyn SessionSink> = sink.clone();
    let (pid, key) = start(&app, typed_sink, output, "p1-12-local-round-trip");
    assert_ne!(pid, 0);

    let tasks = {
        let sessions = app.services.session.ptys.blocking_read();
        sessions
            .get(&key.tab_id)
            .expect("registered session")
            .tasks
            .clone()
    };
    assert!(
        tasks.pending() >= 3,
        "reader, input and supervisor are tracked"
    );

    let marker = "P1-12-ROUNDTRIP";
    app.services
        .write_pty(&key, print_command(marker).as_bytes())
        .expect("write command");
    assert!(sink.output_contains(marker.as_bytes()), "command output");
    app.services.resize_pty(&key, 40, 120).expect("resize PTY");
    assert_eq!(
        app.services.session.ptys.blocking_read()[&key.tab_id]
            .size
            .load(),
        (40, 120)
    );

    app.services
        .write_pty(&key, exit_command().as_bytes())
        .expect("exit command");
    assert!(
        sink.wait_for(|state| state.exited),
        "process completion is reported"
    );
    assert_eq!(sink.state.lock().expect("sink state").exit_reason, None);

    app.services.kill_pty(&key).expect("close PTY");
    assert!(sink.wait_for(|state| state.closed), "output is closed");
    assert!(tasks.wait_idle(WAIT), "all session tasks finish");
    assert!(
        !app.services
            .session
            .ptys
            .blocking_read()
            .contains_key(&key.tab_id)
    );
}

#[test]
fn app_services_local_pty_manual_close_releases_a_running_session() {
    let app = test_support::app();
    let (output, sink) = sink_and_output();
    let typed_sink: Arc<dyn SessionSink> = sink.clone();
    let (_, key) = start(&app, typed_sink, output, "p1-12-local-close");
    let tasks = app.services.session.ptys.blocking_read()[&key.tab_id]
        .tasks
        .clone();

    app.services.kill_pty(&key).expect("close running PTY");
    assert!(sink.wait_for(|state| state.closed));
    assert!(
        tasks.wait_idle(WAIT),
        "closed session leaves no running task"
    );
    assert!(
        !app.services
            .session
            .ptys
            .blocking_read()
            .contains_key(&key.tab_id)
    );
}

/// Runs against this card's disposable OrbStack containers. It never reads a
/// saved tTerm profile or the user's known_hosts store.
#[test]
#[ignore = "requires the P1-05 two-hop OrbStack containers and P1-12_ORBSTACK=1"]
fn app_services_ssh_through_jump_hosts_uses_prompts_and_round_trips() {
    assert_eq!(
        std::env::var("P1_12_ORBSTACK").as_deref(),
        Ok("1"),
        "set P1_12_ORBSTACK=1 only after starting the disposable P1-05 host containers"
    );
    let app = test_support::app();
    let (output, sink) = sink_and_output();
    let broker = app.services.shared.prompts.clone();
    let events = app.events.clone();
    let stop_prompter = Arc::new(AtomicBool::new(false));
    let prompt_count = Arc::new(Mutex::new((0usize, 0usize)));
    let prompter = {
        let stop_prompter = stop_prompter.clone();
        let prompt_count = prompt_count.clone();
        std::thread::spawn(move || {
            let mut handled = HashSet::new();
            while !stop_prompter.load(Ordering::Acquire) {
                for event in events.events() {
                    let AppEvent::Ssh(event) = event else {
                        continue;
                    };
                    match event {
                        crate::events::ssh::SshEvent::HostKeyPrompt { payload, .. }
                            if handled.insert(format!("key:{}", payload.request_id)) =>
                        {
                            prompt_count.lock().expect("prompt count").0 += 1;
                            broker
                                .reply_host_key(&payload.request_id, true)
                                .expect("trust host key");
                        }
                        crate::events::ssh::SshEvent::AuthPrompt { payload, .. }
                            if handled.insert(format!("auth:{}", payload.request_id)) =>
                        {
                            prompt_count.lock().expect("prompt count").1 += 1;
                            let answers = payload
                                .prompts
                                .iter()
                                .map(|field| {
                                    if field.text.to_ascii_lowercase().contains("user") {
                                        "alice".to_string()
                                    } else {
                                        "alice-pass".to_string()
                                    }
                                })
                                .collect();
                            broker
                                .reply_auth(&payload.request_id, Some(answers))
                                .expect("answer authentication");
                        }
                        _ => {}
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })
    };

    let jump = |host: &str, port: u16| crate::core::session::types::JumpHostOptions {
        host: Some(host.into()),
        port: Some(port),
        username: Some("alice".into()),
        password: None,
        auth_method: Some("keyboard-interactive".into()),
        private_key_path: None,
        private_key_passphrase: None,
        certificate_path: None,
        profile_id: None,
    };
    let connection = crate::core::session::types::PtyConnectionOptions {
        connection_type: Some("ssh".into()),
        profile_name: Some("P1-12 disposable SSH".into()),
        host: Some("127.0.0.1".into()),
        port: Some(22),
        username: Some("alice".into()),
        password: None,
        auth_method: Some("keyboard-interactive".into()),
        jump_hosts: vec![jump("127.0.0.1", 22506), jump("tterm-P1-05-host", 22)],
        ssh_shell_integration: Some(false),
        session_log: Some("never".into()),
        ..Default::default()
    };
    let key = SessionKey {
        tab_id: "p1-12-ssh-jump".into(),
        session_nonce: 12,
    };
    let typed_sink: Arc<dyn SessionSink> = sink.clone();
    app.services
        .create_pty_blocking(
            crate::core::create::PtyIdentity {
                tab_id: key.tab_id.clone(),
                session_nonce: key.session_nonce,
                rows: 24,
                cols: 80,
            },
            Some(connection),
            output,
            typed_sink,
        )
        .expect("start SSH session");
    let tasks = app.services.session.ptys.blocking_read()[&key.tab_id]
        .tasks
        .clone();

    app.services
        .write_pty(&key, print_command("P1-12-SSH-JUMP-OK").as_bytes())
        .expect("write SSH command");
    assert!(
        sink.output_contains(b"P1-12-SSH-JUMP-OK"),
        "SSH command output"
    );
    let (host_keys, auth_prompts) = *prompt_count.lock().expect("prompt count");
    assert!(host_keys >= 3, "each hop and target key is confirmed");
    assert!(
        auth_prompts >= 3,
        "each hop and target authentication is answered"
    );

    app.services.kill_pty(&key).expect("close SSH session");
    assert!(sink.wait_for(|state| state.closed));
    assert!(tasks.wait_idle(WAIT), "all SSH session tasks finish");
    stop_prompter.store(true, Ordering::Release);
    prompter.join().expect("prompt task");
}
