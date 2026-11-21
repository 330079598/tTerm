use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshHostKeyPromptPayload};
use tterm_core::workspace::tab_attention::{TabAttentionLevel, TabAttentionStore};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Scroll, Terminal};

use super::{SessionManager, scoped::ConnectionState};
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};

const GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};
const WAIT: Duration = Duration::from_secs(15);

fn manager(runtime: &GpuiRuntime) -> SessionManager {
    SessionManager::new(
        Arc::clone(&runtime.services),
        Rc::new(RefCell::new(TabAttentionStore::new())),
    )
}

fn local_connection() -> PtyConnectionOptions {
    #[cfg(windows)]
    let command = CommandSpec::Program {
        program: std::env::var("COMSPEC")
            .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string()),
        args: vec!["/Q".to_string()],
    };
    #[cfg(not(windows))]
    let command = CommandSpec::Program {
        program: "/bin/sh".to_string(),
        args: vec!["-i".to_string()],
    };
    command
        .connection_options()
        .expect("local connection options")
}

/// A local command that stays alive without producing output, so a wake-up
/// can only come from the capacity change under test.
fn quiet_connection() -> PtyConnectionOptions {
    #[cfg(windows)]
    let command = CommandSpec::Program {
        program: std::env::var("COMSPEC")
            .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string()),
        args: vec![
            "/Q".to_string(),
            "/C".to_string(),
            "ping -n 60 127.0.0.1 >NUL".to_string(),
        ],
    };
    #[cfg(not(windows))]
    let command = CommandSpec::Program {
        program: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), "sleep 60".to_string()],
    };
    command
        .connection_options()
        .expect("quiet connection options")
}

fn flood_connection() -> PtyConnectionOptions {
    #[cfg(windows)]
    let command = CommandSpec::Program {
        program: std::env::var("COMSPEC")
            .unwrap_or_else(|_| r"C:\Windows\System32\cmd.exe".to_string()),
        args: vec![
            "/Q".to_string(),
            "/C".to_string(),
            "for /L %i in (1,1,1000000000) do @echo P2-02-FLOOD-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_string(),
        ],
    };
    #[cfg(not(windows))]
    let command = CommandSpec::Program {
        program: "/bin/sh".to_string(),
        args: vec![
            "-c".to_string(),
            "while :; do printf 'P2-02-FLOOD-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n'; done".to_string(),
        ],
    };
    command
        .connection_options()
        .expect("flood connection options")
}

fn print_command(marker: &str) -> Vec<u8> {
    #[cfg(windows)]
    return format!("echo {marker}\r\n").into_bytes();
    #[cfg(not(windows))]
    return format!("printf '{marker}\n'\n").into_bytes();
}

fn text(model: &Terminal) -> String {
    model
        .snapshot()
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .map(|cell| cell.ch)
        .collect()
}

fn wait_for_text(model: &Terminal, needle: &str) -> bool {
    let deadline = Instant::now() + WAIT;
    loop {
        if text(model).contains(needle) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_text_after(
    model: &Terminal,
    needle: &str,
    started: &Instant,
    timeout: Duration,
) -> bool {
    let deadline = *started + timeout;
    loop {
        if text(model).contains(needle) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn dropping_a_view_attachment_leaves_the_core_session_running() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-view-destruction";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let attachment = binding.attach().expect("attach the first view");
    let session = Arc::clone(&attachment.session);
    let key = attachment.key().clone();
    let model = attachment.model();

    let input = session.input();
    input(&print_command("P2-01-BEFORE-DETACH"));
    assert!(wait_for_text(&model, "P2-01-BEFORE-DETACH"));

    drop(attachment);

    let surviving = binding.session().expect("the binding keeps the session");
    let input = surviving.input();
    input(&print_command("P2-01-AFTER-DETACH"));
    assert!(
        wait_for_text(&model, "P2-01-AFTER-DETACH"),
        "output still reaches the model after the view is gone"
    );
    assert!(
        runtime
            .services
            .session
            .ptys
            .blocking_read()
            .contains_key(&key.tab_id),
        "core still owns the session"
    );

    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn reattaching_a_view_keeps_content_parsed_while_it_was_gone() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-reattach";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let first = binding.attach().expect("attach the first view");
    let first_model = first.model();
    let first_input = first.session().input();
    first_input(&print_command("P2-01-CONTENT-BEFORE"));
    assert!(wait_for_text(&first_model, "P2-01-CONTENT-BEFORE"));
    drop(first);

    let detached_session = binding.session().expect("session survives detach");
    let detached_input = detached_session.input();
    detached_input(&print_command("P2-01-CONTENT-DURING"));
    assert!(wait_for_text(&first_model, "P2-01-CONTENT-DURING"));

    let second = binding.attach().expect("reattach the view");
    let second_model = second.model();
    assert!(
        Arc::ptr_eq(&first_model, &second_model),
        "the replacement view draws the same model"
    );
    let content = text(&second_model);
    assert!(content.contains("P2-01-CONTENT-BEFORE"));
    assert!(
        content.contains("P2-01-CONTENT-DURING"),
        "content parsed while no view was attached is still there"
    );

    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn late_output_from_an_old_nonce_is_discarded() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-stale-nonce";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let old = binding.attach().expect("attach the old generation");
    let old_nonce = old.session_nonce();
    let old_session = Arc::clone(&old.session);
    drop(old);

    let replacement = manager.open(tab_id, local_connection(), GRID, CELL);
    let new = replacement.attach().expect("attach the new generation");
    let new_nonce = new.session_nonce();
    let new_model = new.model();
    assert_ne!(old_nonce, new_nonce, "a reconnect starts a new generation");
    let before = text(&new_model);
    let stale = b"P2-01-STALE-OUTPUT\r\n";

    assert!(
        !old_session.accept_output(old_nonce, stale),
        "the closed generation rejects its parser's late batch"
    );
    assert!(
        !new.accept_output(old_nonce, stale),
        "a replacement view rejects output tagged with the old nonce"
    );
    let after = text(&new_model);
    assert_eq!(after, before, "the new model never saw the stale bytes");
    assert!(!after.contains("P2-01-STALE-OUTPUT"));

    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn closing_a_tab_forgets_typed_passwords_and_prompt_state() {
    let runtime = test_runtime();
    let attention = Rc::new(RefCell::new(TabAttentionStore::new()));
    let manager = SessionManager::new(Arc::clone(&runtime.services), Rc::clone(&attention));
    let tab_id = "p2-01-close-tab";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let key = binding.session().expect("local session").key().clone();
    let tasks = runtime.services.session.ptys.blocking_read()[&key.tab_id]
        .tasks
        .clone();
    tterm_core::ssh::typed_passwords::remember(tab_id, "db.example", 22, "root", "secret");
    assert!(
        runtime
            .services
            .shared
            .prompts
            .register_owner(Some(key.clone()))
            .is_ok()
    );
    attention
        .borrow_mut()
        .mark_tab_attention(tab_id, TabAttentionLevel::Bell);

    manager.close_tab(tab_id).expect("close the tab");

    assert!(
        tterm_core::ssh::typed_passwords::get(tab_id, "db.example", 22, "root").is_none(),
        "typed passwords are released with the tab"
    );
    assert!(
        runtime
            .services
            .shared
            .prompts
            .register_owner(Some(key))
            .is_err(),
        "the prompt generation is closed"
    );
    assert!(
        !runtime
            .services
            .session
            .ptys
            .blocking_read()
            .contains_key(tab_id)
    );
    assert!(attention.borrow().get_tab_attention(tab_id).is_none());
    assert!(tasks.wait_idle(WAIT), "all session tasks finish");
}

#[test]
fn a_new_session_resets_scoped_state_and_ignores_old_updates() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-scoped-reset";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let old_nonce = binding.session().expect("first session").session_nonce();
    let host_key = SshHostKeyPromptPayload {
        request_id: "host-1".to_string(),
        profile_name: "Production".to_string(),
        host: "db.example".to_string(),
        port: 22,
        algorithm: "ssh-ed25519".to_string(),
        fingerprint: "SHA256:old".to_string(),
        reason: "not known".to_string(),
        known_fingerprint: None,
    };
    assert!(binding.set_host_key_prompt(old_nonce, Some(host_key)));
    assert!(binding.set_connection_state(old_nonce, ConnectionState::Error));
    assert!(binding.set_connection_progress(
        old_nonce,
        Some(SshConnectionProgressPayload::new("failed", "old failure"))
    ));
    assert!(binding.set_start_error(old_nonce, Some("old start failure".to_string())));
    assert_eq!(
        binding.scoped_state().connection_state(),
        ConnectionState::Error
    );

    let replacement = manager.open(tab_id, local_connection(), GRID, CELL);
    let new_nonce = replacement
        .session()
        .expect("second session")
        .session_nonce();
    assert_ne!(old_nonce, new_nonce);
    let reset = replacement.scoped_state();
    assert_eq!(reset.session_nonce(), new_nonce);
    assert_eq!(reset.connection_state(), ConnectionState::Connecting);
    assert!(reset.host_key_prompt().is_none());
    assert!(reset.connection_progress().is_none());
    assert!(reset.start_error().is_none());

    assert!(
        !replacement.set_host_key_prompt(
            old_nonce,
            Some(SshHostKeyPromptPayload {
                request_id: "host-2".to_string(),
                profile_name: "Production".to_string(),
                host: "db.example".to_string(),
                port: 22,
                algorithm: "ssh-ed25519".to_string(),
                fingerprint: "SHA256:late".to_string(),
                reason: "not known".to_string(),
                known_fingerprint: None,
            })
        ),
        "an old prompt cannot replace the new session's state"
    );
    assert!(!replacement.set_connection_state(old_nonce, ConnectionState::Connected));
    assert!(!replacement.set_start_error(old_nonce, Some("late".to_string())));
    let unchanged = replacement.scoped_state();
    assert_eq!(unchanged.connection_state(), ConnectionState::Connecting);
    assert!(unchanged.host_key_prompt().is_none());
    assert!(unchanged.start_error().is_none());

    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn entering_a_tab_clears_its_attention_state() {
    let runtime = test_runtime();
    let attention = Rc::new(RefCell::new(TabAttentionStore::new()));
    let manager = SessionManager::new(Arc::clone(&runtime.services), Rc::clone(&attention));
    let tab_id = "p2-01-attention";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    {
        let mut store = attention.borrow_mut();
        store.mark_tab_attention(tab_id, TabAttentionLevel::Bell);
        store.count_tab_notification(tab_id);
        assert_eq!(
            store.get_tab_attention(tab_id),
            Some(TabAttentionLevel::Bell)
        );
    }
    assert!(!binding.is_active());
    let session = binding.session().expect("local session");
    assert!(!session.is_visible());

    assert!(binding.enter(), "the first entry changes the active state");
    assert!(binding.is_active());
    assert!(session.is_visible());
    {
        let store = attention.borrow();
        assert!(store.get_tab_attention(tab_id).is_none());
        assert_eq!(store.unseen_notification_total(), 0);
    }
    assert!(!binding.enter(), "entering again is not a new transition");

    binding.leave();
    assert!(!binding.is_active());
    assert!(!session.is_visible());
    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn a_program_that_ends_by_itself_still_writes_its_exit_notice() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-process-exit";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let attachment = binding.attach().expect("attach the view");
    let model = attachment.model();
    let input = attachment.session().input();
    #[cfg(windows)]
    input(b"exit\r\n");
    #[cfg(not(windows))]
    input(b"exit\n");
    assert!(
        wait_for_text(&model, "[Process exited]"),
        "queued output and the exit notice are parsed after input closes"
    );
    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn input_and_ctrl_c_bypass_a_saturated_output_parser() {
    const INPUT_BUDGET: Duration = Duration::from_millis(100);
    const INTERRUPT_BUDGET: Duration = Duration::from_secs(5);

    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-02-input-under-output";
    let binding = manager.open(tab_id, flood_connection(), GRID, CELL);
    let attachment = binding.attach().expect("attach the view");
    let model = attachment.model();
    assert!(
        wait_for_text(&model, "P2-02-FLOOD"),
        "the output flood reaches the parser"
    );
    std::thread::sleep(Duration::from_millis(250));

    let input = attachment.session().input();
    let input_started = Instant::now();
    input(b"x");
    let input_latency = input_started.elapsed();
    assert!(
        input_latency < INPUT_BUDGET,
        "ordinary input waited {input_latency:?} behind terminal output"
    );

    let interrupt_started = Instant::now();
    input(&[0x03]);
    let interrupt_write = interrupt_started.elapsed();
    assert!(
        interrupt_write < INPUT_BUDGET,
        "Ctrl+C waited {interrupt_write:?} behind terminal output before reaching the PTY"
    );
    assert!(
        wait_for_text_after(
            &model,
            "[Process exited]",
            &interrupt_started,
            INTERRUPT_BUDGET
        ),
        "Ctrl+C did not stop the flood within {INTERRUPT_BUDGET:?}"
    );
    eprintln!(
        "P2-02 input latency: input={input_latency:?}, ctrl_c_write={interrupt_write:?}, \
         ctrl_c_to_exit={:?}",
        interrupt_started.elapsed()
    );

    manager.close_tab(tab_id).expect("close the tab");
}

#[gpui_kit::test]
fn recreating_the_gpui_view_keeps_the_sessions_content(cx: &mut gpui_kit::TestAppContext) {
    use gpui_kit::AppContext as _;
    use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-01-view-entity";
    let binding = manager.open(tab_id, local_connection(), GRID, CELL);
    let first = binding.attach().expect("attach the first view");
    let first_model = first.model();
    let input = first.session().input();
    input(&print_command("P2-01-BEFORE-ENTITY-DROP"));
    assert!(wait_for_text(&first_model, "P2-01-BEFORE-ENTITY-DROP"));
    let first_view = cx.update(|cx| {
        cx.new(|cx| TerminalView::new(cx, Arc::clone(&first_model), TerminalViewConfig::default()))
    });
    let first_id = first_view.entity_id();
    drop(first_view);
    let second = binding.attach().expect("attach the replacement view");
    let second_model = second.model();
    assert!(Arc::ptr_eq(&first_model, &second_model));
    let second_view = cx.update(|cx| {
        cx.new(|cx| TerminalView::new(cx, Arc::clone(&second_model), TerminalViewConfig::default()))
    });
    assert_ne!(first_id, second_view.entity_id());
    assert!(text(&second_model).contains("P2-01-BEFORE-ENTITY-DROP"));
    manager.close_tab(tab_id).expect("close the tab");
}

#[test]
fn changing_scrollback_capacity_updates_every_open_session_and_future_sessions() {
    let runtime = test_runtime();
    let manager = SessionManager::new(
        Arc::clone(&runtime.services),
        Rc::new(RefCell::new(TabAttentionStore::new())),
    )
    .with_terminal_config(Config {
        scrollback: 8,
        ..Config::default()
    });
    let first_id = "p2-06-scrollback-first";
    let second_id = "p2-06-scrollback-second";
    let first = manager.open(first_id, local_connection(), GRID, CELL);
    let second = manager.open(second_id, local_connection(), GRID, CELL);
    let first_attachment = first.attach().expect("attach the first view");
    let second_attachment = second.attach().expect("attach the second view");
    let first_session = Arc::clone(&first_attachment.session);
    let second_session = Arc::clone(&second_attachment.session);
    let first_nonce = first_session.session_nonce();
    let second_nonce = second_session.session_nonce();
    let first_model = first_attachment.model();
    let second_model = second_attachment.model();
    let first_input = first_attachment.session().input();
    let second_input = second_attachment.session().input();

    for index in 0..40 {
        first_input(&print_command(&format!("first-{index}")));
        second_input(&print_command(&format!("second-{index}")));
    }
    assert!(wait_for_text(&first_model, "first-39"));
    assert!(wait_for_text(&second_model, "second-39"));
    assert!(first_model.snapshot().history_size() > 4);
    assert!(second_model.snapshot().history_size() > 4);

    first_model.scroll(Scroll::Top);
    second_model.scroll(Scroll::Top);
    let first_before = first_model.snapshot();
    let second_before = second_model.snapshot();
    assert!(first_before.display_offset() > 4);
    assert!(second_before.display_offset() > 4);

    manager.set_scrollback(4);

    assert_eq!(first_model.scrollback(), 4);
    assert_eq!(second_model.scrollback(), 4);
    let first_after = first_model.snapshot();
    let second_after = second_model.snapshot();
    assert_eq!(first_after.history_size(), 4);
    assert_eq!(second_after.history_size(), 4);
    assert!(
        first_after.damage().is_full(),
        "evicting history changes every visible row"
    );
    assert!(
        second_after.damage().is_full(),
        "every managed session is updated"
    );
    assert!(
        first_after.display_offset() <= 4,
        "the viewport is clamped to the remaining history"
    );
    assert!(second_after.display_offset() <= 4);
    assert!(
        first_after.scrollback_generation() > first_before.scrollback_generation(),
        "the view cache is invalidated when rows are evicted"
    );
    assert!(
        second_after.scrollback_generation() > second_before.scrollback_generation(),
        "each attached view sees its own generation advance"
    );

    let first_current = first.session().expect("the first session survives");
    let second_current = second.session().expect("the second session survives");
    assert!(Arc::ptr_eq(&first_current, &first_session));
    assert!(Arc::ptr_eq(&second_current, &second_session));
    assert_eq!(first_current.session_nonce(), first_nonce);
    assert_eq!(second_current.session_nonce(), second_nonce);

    let later_id = "p2-06-scrollback-later";
    let later = manager.open(later_id, local_connection(), GRID, CELL);
    let later_model = later.attach().expect("attach the later view").model();
    assert_eq!(
        later_model.scrollback(),
        4,
        "sessions opened after the change inherit the resolved capacity"
    );

    manager.close_tab(first_id).expect("close the first tab");
    manager.close_tab(second_id).expect("close the second tab");
    manager.close_tab(later_id).expect("close the later tab");
}

#[test]
fn a_capacity_change_wakes_an_attached_visible_view() {
    let runtime = test_runtime();
    let manager = manager(&runtime);
    let tab_id = "p2-06-scrollback-wake";
    let binding = manager.open(tab_id, quiet_connection(), GRID, CELL);
    assert!(binding.enter(), "the view is visible");
    let attachment = binding.attach().expect("attach the view");
    let mut watch = attachment.subscribe();
    watch.acknowledge();

    manager.set_scrollback(4);

    let changed = runtime
        ._runtime
        .block_on(async { tokio::time::timeout(Duration::from_secs(5), watch.changed()).await });
    assert!(
        matches!(changed, Ok(true)),
        "the capacity change requests a frame without new PTY output"
    );

    manager.close_tab(tab_id).expect("close the tab");
}
