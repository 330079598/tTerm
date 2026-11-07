use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::mpsc::TrySendError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::CommandBuilder;
use tterm_terminal::grid::{CellSize, GridSize, WindowSize};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::snapshot::Snapshot;
use tterm_terminal::terminal::{Config, Terminal};

use super::command::{CommandSpec, home_dir, windows_shell};
use super::intents::DEFAULT_TITLE;
use super::pipeline::{self, QUEUED_READS, SessionState, WriteMessage};
use super::session::{EXIT_NOTICE, Session, pty_size};

const GRID: GridSize = GridSize {
    columns: 40,
    screen_lines: 10,
};
const CELL: CellSize = CellSize {
    width: 9,
    height: 18,
};

/// A writer the test can read back after the writer thread is done.
#[derive(Clone, Default)]
struct Written(Arc<Mutex<Vec<u8>>>);

impl Write for Written {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn state() -> SessionState {
    SessionState {
        terminal: Arc::new(Terminal::new(Config::default(), GRID, CELL)),
        size: Arc::new(Mutex::new(WindowSize {
            grid: GRID,
            cell: CELL,
        })),
        title: Arc::new(Mutex::new(None)),
    }
}

/// Runs the production pipeline over `input` until the reader reaches the
/// end, then returns what the writer wrote and how many wakes are pending.
fn run_pipeline(state: &SessionState, input: Vec<u8>) -> (Vec<u8>, usize) {
    let written = Written::default();
    let pipeline = pipeline::start(
        Box::new(Cursor::new(input)),
        Box::new(written.clone()),
        state.clone(),
    );
    pipeline.reader.join().unwrap();
    drop(pipeline.queue);
    pipeline.parser.join().unwrap();
    pipeline.writes.send(WriteMessage::Close).unwrap();
    pipeline.writer.join().unwrap();
    let bytes = written.0.lock().unwrap().clone();
    (bytes, pipeline.wake.len())
}

fn screen_text(frame: &Snapshot) -> String {
    (0..frame.line_count())
        .filter_map(|line| frame.row(line))
        .map(|row| {
            let text: String = row.cells().iter().map(|cell| cell.ch).collect();
            text.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_pipeline_applies_every_byte_and_the_history_stays_bounded() {
    let state = state();
    let mut input = Vec::new();
    for line in 0..200_000 {
        input.extend_from_slice(format!("line {line}\r\n").as_bytes());
    }
    input.extend_from_slice(b"end");

    let (written, wakes) = run_pipeline(&state, input);

    let frame = state.terminal.snapshot();
    let text = screen_text(&frame);
    assert!(text.contains("line 199999\nend"), "{text}");
    // Far more lines than the scrollback went through; the model kept only
    // its configured history.
    assert_eq!(frame.history_size(), state.terminal.scrollback());
    assert!(written.is_empty());
    // Many batches were applied, and they left a single wake behind.
    assert_eq!(wakes, 1);
}

#[test]
fn the_byte_queue_holds_a_fixed_number_of_reads() {
    let (queue, _queued) = std::sync::mpsc::sync_channel::<Vec<u8>>(QUEUED_READS);
    for _ in 0..QUEUED_READS {
        queue.try_send(vec![0; pipeline::READ_SIZE]).unwrap();
    }
    // Pipeline::start builds its queue the same way: one read more and the
    // reader would block instead of buffering it.
    assert!(matches!(
        queue.try_send(Vec::new()),
        Err(TrySendError::Full(_))
    ));
}

#[test]
fn query_replies_reach_the_writer_in_order() {
    let state = state();
    // Device attributes, then the background color, then the text area size.
    let (written, _) = run_pipeline(&state, b"\x1b[c\x1b]11;?\x1b\\\x1b[14t".to_vec());

    let background = TerminalPalette::default().background;
    let (r, g, b) = (background.r, background.g, background.b);
    let expected = format!(
        "\x1b[?6c\x1b]11;rgb:{r:02x}{r:02x}/{g:02x}{g:02x}/{b:02x}{b:02x}\x1b\\\x1b[4;180;360t"
    );
    assert_eq!(String::from_utf8_lossy(&written), expected);
}

#[test]
fn a_size_reply_leaves_the_damage_for_the_view() {
    let state = state();
    // The view's first snapshot takes the initial full damage.
    let _ = state.terminal.snapshot();

    run_pipeline(&state, b"x\x1b[14t".to_vec());

    // Answering from a snapshot would have consumed the damaged first row.
    let frame = state.terminal.snapshot();
    assert!(frame.damage().is_full() || !frame.damage().rows().is_empty());
}

#[test]
fn intents_are_drained_and_only_the_last_title_is_kept() {
    let state = state();
    let mut input = Vec::new();
    for index in 0..1_000 {
        input.extend_from_slice(format!("\x1b]0;title {index}\x07\x07").as_bytes());
    }
    input.extend_from_slice(b"\x1b]0;last\x07");

    run_pipeline(&state, input);

    assert!(state.terminal.drain_intents().is_empty());
    assert_eq!(state.title.lock().unwrap().as_deref(), Some("last"));
}

#[test]
fn a_title_reset_restores_the_default_title() {
    let state = state();
    // Push the unset title, set one, pop: Alacritty reports a reset.
    run_pipeline(&state, b"[22;0t]0;vim[23;0t".to_vec());
    assert_eq!(state.title.lock().unwrap().as_deref(), Some(DEFAULT_TITLE));

    run_pipeline(&state, b"]0;vim]0;".to_vec());
    assert_eq!(state.title.lock().unwrap().as_deref(), Some(DEFAULT_TITLE));
}

#[test]
fn windows_prefers_powershell_7_then_windows_powershell_then_cmd() {
    let path = |name: &str| Some(PathBuf::from(name));
    let shell = |pwsh, powershell, cmd| windows_shell(pwsh, powershell, cmd);

    assert_eq!(
        shell(path("pwsh.exe"), path("powershell.exe"), path("cmd.exe")),
        (PathBuf::from("pwsh.exe"), vec!["-NoLogo"])
    );
    assert_eq!(
        shell(None, path("powershell.exe"), path("cmd.exe")),
        (PathBuf::from("powershell.exe"), vec!["-NoLogo"])
    );
    assert_eq!(
        shell(None, None, path(r"C:\Windows\System32\cmd.exe")),
        (PathBuf::from(r"C:\Windows\System32\cmd.exe"), Vec::new())
    );
    assert_eq!(
        shell(None, None, None),
        (PathBuf::from("cmd.exe"), Vec::new())
    );
}

#[test]
fn the_command_line_picks_the_shell_ssh_or_a_program() {
    let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    assert_eq!(CommandSpec::from_args(&[]), Ok(CommandSpec::Shell));
    assert_eq!(
        CommandSpec::from_args(&args(&["--ssh", "me@host", "-p", "2222"])),
        Ok(CommandSpec::Program {
            program: "ssh".to_string(),
            args: args(&["me@host", "-p", "2222"]),
        })
    );
    assert_eq!(
        CommandSpec::from_args(&args(&["--exec", "vim", "notes.txt"])),
        Ok(CommandSpec::Program {
            program: "vim".to_string(),
            args: args(&["notes.txt"]),
        })
    );
    assert!(CommandSpec::from_args(&args(&["--ssh"])).is_err());
    assert!(CommandSpec::from_args(&args(&["--exec"])).is_err());
    assert!(CommandSpec::from_args(&args(&["--unknown"])).is_err());
}

#[test]
fn the_home_directory_follows_the_tauri_order() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |key: &str| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        }
    };
    if cfg!(windows) {
        assert_eq!(
            home_dir(env(&[("USERPROFILE", r"C:\Users\a"), ("HOME", r"D:\h")])),
            r"C:\Users\a"
        );
        assert_eq!(
            home_dir(env(&[
                ("USERPROFILE", " "),
                ("HOMEDRIVE", "E:"),
                ("HOMEPATH", r"\home")
            ])),
            r"E:\home"
        );
        assert_eq!(home_dir(env(&[("HOME", r"D:\h")])), r"D:\h");
        assert_eq!(home_dir(env(&[])), r"C:\");
    } else {
        assert_eq!(home_dir(env(&[("HOME", "/home/a")])), "/home/a");
        assert_eq!(home_dir(env(&[("HOME", "")])), "/");
    }
}

/// A command that prints `ready` and ends at once.
fn short_command() -> CommandBuilder {
    if cfg!(windows) {
        let mut command = CommandBuilder::new("cmd.exe");
        command.args(["/d", "/c", "echo ready"]);
        command
    } else {
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "echo ready"]);
        command
    }
}

/// A command that runs for a minute unless something ends it.
fn long_command() -> CommandBuilder {
    if cfg!(windows) {
        let mut command = CommandBuilder::new("ping.exe");
        command.args(["-n", "60", "127.0.0.1"]);
        command
    } else {
        let mut command = CommandBuilder::new("/bin/sh");
        command.args(["-c", "sleep 60"]);
        command
    }
}

fn wait_for(deadline: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

#[test]
fn a_program_that_ends_by_itself_leaves_the_exit_notice_and_stops_input() {
    let state = state();
    let (session, _wake) =
        Session::spawn(short_command(), pty_size(GRID, CELL), state.clone()).expect("start a PTY");
    let notice = String::from_utf8_lossy(EXIT_NOTICE);
    let notice = notice.trim().trim_start_matches("\x1b[33m");
    let notice = notice.trim_end_matches("\x1b[0m").to_string();

    let shown = wait_for(Duration::from_secs(20), || {
        screen_text(&state.terminal.snapshot()).contains(&notice)
    });
    let text = screen_text(&state.terminal.snapshot());
    assert!(shown, "{text}");
    assert!(text.contains("ready"), "{text}");
    assert!(!session.is_running());
    session.shutdown().join().unwrap();
}

#[test]
fn closing_the_session_ends_the_program_without_blocking() {
    let state = state();
    let (session, _wake) =
        Session::spawn(long_command(), pty_size(GRID, CELL), state).expect("start a PTY");
    assert!(session.is_running());

    let started = Instant::now();
    let closing = session.shutdown();
    // Killing the program and closing the PTY happen before shutdown
    // returns; only the waiting is left to the background thread.
    assert!(started.elapsed() < Duration::from_secs(2));
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = done.send(closing.join().is_ok());
    });
    assert_eq!(finished.recv_timeout(Duration::from_secs(20)), Ok(true));
}
