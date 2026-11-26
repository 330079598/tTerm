//! A saved target must earn host-key trust before it may ask for a password.
//!
//! Every launch here uses the disposable loopback `ShellServer` and the
//! existing saved-connection harness. The application database is the one the
//! harness isolates under `target/`, and the profile id and name are unique
//! per test; no OpenSSH `known_hosts` file is written or used as a fixture.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, TestAppContext, VisualTestContext};
use tterm_core::connections::profile_recents::{RecentProfile, read_recent_profiles};
use tterm_core::events::ssh::SshHostKeyPromptPayload;
use tterm_core::profiles::launch::SavedSshLaunch;
use tterm_core::ssh::store::{
    KnownHostRecord, load_known_host, now_unix_ms, save_known_host_entry,
};
use tterm_terminal::terminal::Terminal;

use super::server::ShellServer;
use super::{Prototype, direct_launch, ensure_storage, harness, output, pump_until};

use crate::terminal_demo::binding::scoped::ConnectionState;

const HOST: &str = "127.0.0.1";
const PASSWORD: &str = "disposable-test-password";

/// A profile id and display name that no parallel test or earlier run shares.
fn unique_target(prefix: &str) -> (String, String) {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    let serial = NEXT.fetch_add(1, Ordering::Relaxed);
    let id = format!("{prefix}-{}-{serial}", std::process::id());
    let name = format!("{prefix} {}-{serial}", std::process::id());
    (id, name)
}

/// The requested direct launch, with no password that could answer silently.
fn session_password_launch(id: &str, name: &str, server: &ShellServer) -> SavedSshLaunch {
    let mut launch = direct_launch(id, name, server);
    assert_eq!(
        launch.connection.ssh_shell_integration,
        Some(false),
        "the shared helper suppresses shell integration"
    );
    launch.connection.password = None;
    launch.connection.ignore_saved_password = true;
    launch
}

fn open_saved(
    visual: &mut VisualTestContext,
    root: &Entity<Prototype>,
    id: &str,
    name: &str,
    server: &ShellServer,
) -> (usize, Arc<Terminal>) {
    let launch = session_password_launch(id, name, server);
    visual.update(|window, cx| {
        let tab_id = root
            .update(cx, |workspace, cx| {
                workspace.open_saved_profile_launch(launch, window, cx)
            })
            .expect("the saved launch should open one tab");
        window.render_frame(cx);
        let model = root
            .read(cx)
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch should run a live session")
            .model();
        (tab_id, model)
    })
}

fn host_key_question(
    root: &Entity<Prototype>,
    tab_id: usize,
    cx: &App,
) -> Option<SshHostKeyPromptPayload> {
    root.read(cx)
        .tabs
        .get(tab_id)
        .and_then(|tab| tab.binding.as_ref())
        .and_then(|binding| binding.scoped_state().host_key_prompt().cloned())
}

fn wait_for_host_key(
    visual: &mut VisualTestContext,
    root: &Entity<Prototype>,
    tab_id: usize,
    model: &Arc<Terminal>,
) -> SshHostKeyPromptPayload {
    pump_until(visual, "the host-key question", |cx| {
        host_key_question(root, tab_id, cx).is_some() && output(model).contains("Fingerprint:")
    });
    visual
        .update(|_, cx| host_key_question(root, tab_id, cx))
        .expect("the host-key question stays in the tab state")
}

fn wait_for_password(visual: &mut VisualTestContext, model: &Arc<Terminal>) {
    pump_until(visual, "the password question", |_| {
        output(model).contains("Password:")
    });
}

fn wait_for_shell(visual: &mut VisualTestContext, model: &Arc<Terminal>) {
    pump_until(visual, "the SSH shell banner", |_| {
        output(model).contains("SAVED_SSH_READY")
    });
}

/// The recent entry for one saved profile, if the background writer stored it.
fn recent_profile(id: &str) -> Option<RecentProfile> {
    read_recent_profiles()
        .expect("the isolated recent profiles read")
        .into_iter()
        .find(|recent| recent.profile_id == id)
}

fn wait_for_recent(visual: &mut VisualTestContext, id: &str) -> RecentProfile {
    pump_until(visual, "the saved profile to become recent", |_| {
        recent_profile(id).is_some()
    });
    recent_profile(id).expect("the wait saw the recent profile")
}

/// Polls until the stored entry is strictly newer than `floor`, so a previous
/// use cannot satisfy the wait.
fn wait_for_recent_after(visual: &mut VisualTestContext, id: &str, floor: i64) -> RecentProfile {
    pump_until(visual, "a newer recent-use timestamp", |_| {
        recent_profile(id).is_some_and(|recent| recent.last_used_at > floor)
    });
    recent_profile(id).expect("the wait saw the newer recent profile")
}

/// Waits until the wall clock the recorder reads is strictly past `timestamp`.
/// The store ignores a use that is not newer than the stored one, so a second
/// launch in the same millisecond would otherwise never be recorded.
fn wait_for_clock_after(visual: &mut VisualTestContext, timestamp: i64) {
    pump_until(visual, "the clock to pass the first use", |_| {
        now_unix_ms() > timestamp
    });
}

/// Lets any worker that was wrongly started finish before checking it left the
/// first use untouched.
fn assert_recent_unchanged(visual: &mut VisualTestContext, id: &str, expected: i64) {
    std::thread::sleep(Duration::from_millis(100));
    visual.run_until_parked();
    let recent = recent_profile(id).expect("the first successful use stays recorded");
    assert_eq!(
        recent.last_used_at, expected,
        "a launch must not record a use before its shell is ready"
    );
}

/// Lets any worker that was wrongly started finish before checking it wrote
/// nothing.
fn assert_not_recent(visual: &mut VisualTestContext, id: &str) {
    std::thread::sleep(Duration::from_millis(100));
    visual.run_until_parked();
    assert!(
        recent_profile(id).is_none(),
        "a connection that never reached a shell must not be recent"
    );
}

/// Waits until a refused connection has ended, so later checks cannot pass on
/// state written before the refusal reached the session.
fn wait_for_session_end(visual: &mut VisualTestContext, root: &Entity<Prototype>, tab_id: usize) {
    pump_until(visual, "the refused session to end", |cx| {
        root.read(cx)
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .is_none_or(|binding| {
                matches!(
                    binding.scoped_state().connection_state(),
                    ConnectionState::Disconnected | ConnectionState::Error
                )
            })
    });
}

fn assert_password_not_echoed(model: &Terminal) {
    assert!(
        !output(model).contains(PASSWORD),
        "the session-only password must never be echoed into the terminal"
    );
}

#[gpui_kit::test]
fn an_untrusted_host_asks_first_and_each_session_asks_for_the_password_again(
    cx: &mut TestAppContext,
) {
    let harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let (id, name) = unique_target("first-trust");
    ensure_storage();
    assert!(
        load_known_host(&name, Some(&id), HOST, server.port())
            .expect("the isolated known-host store opens")
            .is_none(),
        "the disposable target starts unknown"
    );
    let root = harness.root.clone();

    // The unknown key is refused with an empty line, without touching auth.
    let (refused_id, refused_model) = open_saved(harness.visual, &root, &id, &name, &server);
    let question = wait_for_host_key(harness.visual, &root, refused_id, &refused_model);
    assert_eq!(question.profile_name, name);
    assert_eq!(question.host, HOST);
    assert_eq!(question.port, server.port());
    assert_eq!(question.reason, "unknown");
    assert_eq!(question.known_fingerprint, None);
    assert_eq!(question.fingerprint, server.fingerprint());

    harness.visual.simulate_keystrokes("enter");
    wait_for_session_end(harness.visual, &root, refused_id);
    assert_not_recent(harness.visual, &id);
    assert_eq!(server.connections(), 1);
    assert_eq!(
        server.shells(),
        0,
        "a refused host key never starts a shell"
    );
    assert!(
        load_known_host(&name, Some(&id), HOST, server.port())
            .expect("the isolated known-host store opens")
            .is_none(),
        "refusing an unknown key must not trust it"
    );

    // The same target is relaunched and explicitly trusted, then authenticates.
    let (trusted_id, trusted_model) = open_saved(harness.visual, &root, &id, &name, &server);
    let question = wait_for_host_key(harness.visual, &root, trusted_id, &trusted_model);
    assert_eq!(question.reason, "unknown");
    harness.visual.simulate_keystrokes("y enter");
    wait_for_password(harness.visual, &trusted_model);
    assert_password_not_echoed(&trusted_model);
    harness.visual.simulate_input(PASSWORD);
    assert_password_not_echoed(&trusted_model);
    harness.visual.simulate_keystrokes("enter");
    wait_for_shell(harness.visual, &trusted_model);
    assert_eq!(server.shells(), 1);
    assert_password_not_echoed(&trusted_model);
    let first_use = wait_for_recent(harness.visual, &id);

    let saved = load_known_host(&name, Some(&id), HOST, server.port())
        .expect("the isolated known-host store opens")
        .expect("explicitly trusting the host persists its fingerprint");
    assert_eq!(saved.profile_id.as_deref(), Some(id.as_str()));
    assert_eq!(saved.profile_name, name);
    assert_eq!(saved.host, HOST);
    assert_eq!(saved.port, server.port());
    assert_eq!(saved.algorithm, server.algorithm());
    assert_eq!(saved.fingerprint, server.fingerprint());

    // Reopening the same target skips the trust question but not authentication.
    let (reopened_id, reopened_model) = open_saved(harness.visual, &root, &id, &name, &server);
    wait_for_password(harness.visual, &reopened_model);
    assert_recent_unchanged(harness.visual, &id, first_use.last_used_at);
    harness.visual.update(|_, cx| {
        assert!(
            host_key_question(&root, reopened_id, cx).is_none(),
            "a pinned target must not ask for trust again"
        );
    });
    assert!(!output(&reopened_model).contains("Trust this host key?"));
    wait_for_clock_after(harness.visual, first_use.last_used_at);
    harness.visual.simulate_input(PASSWORD);
    assert_password_not_echoed(&reopened_model);
    harness.visual.simulate_keystrokes("enter");
    wait_for_shell(harness.visual, &reopened_model);
    assert_eq!(server.connections(), 3);
    assert_eq!(server.shells(), 2, "both accepted launches start a shell");
    assert_password_not_echoed(&reopened_model);
    let second_use = wait_for_recent_after(harness.visual, &id, first_use.last_used_at);
    assert!(
        second_use.last_used_at > first_use.last_used_at,
        "the second ready shell must record its own later use"
    );
    assert_eq!(
        read_recent_profiles()
            .expect("the isolated recent profiles read")
            .iter()
            .filter(|recent| recent.profile_id == id)
            .count(),
        1,
        "repeated successful launches keep one recent entry"
    );

    harness.visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn refusing_a_changed_host_key_keeps_the_saved_pin(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let server = ShellServer::start(harness._runtime._runtime.handle());
    let (id, name) = unique_target("changed-trust");
    ensure_storage();
    let pin = format!("SHA256:stale-pin-{id}");
    save_known_host_entry(KnownHostRecord {
        profile_id: Some(id.clone()),
        profile_name: name.clone(),
        host: HOST.to_string(),
        port: server.port(),
        algorithm: server.algorithm(),
        fingerprint: pin.clone(),
        trusted_at: now_unix_ms(),
    })
    .expect("the disposable stale pin should be stored");
    let root = harness.root.clone();

    let (tab_id, model) = open_saved(harness.visual, &root, &id, &name, &server);
    let question = wait_for_host_key(harness.visual, &root, tab_id, &model);
    assert_eq!(question.reason, "mismatch");
    assert_eq!(question.known_fingerprint, Some(pin.clone()));
    assert_eq!(question.fingerprint, server.fingerprint());

    harness.visual.simulate_keystrokes("ctrl-c");
    wait_for_session_end(harness.visual, &root, tab_id);
    assert_not_recent(harness.visual, &id);
    assert_eq!(server.connections(), 1);
    assert_eq!(
        server.shells(),
        0,
        "a refused changed key never starts a shell"
    );

    let unchanged = load_known_host(&name, Some(&id), HOST, server.port())
        .expect("the isolated known-host store opens")
        .expect("the old pin still exists");
    assert_eq!(
        unchanged.fingerprint, pin,
        "refusing a changed key must not replace the saved pin"
    );

    harness.visual.update(|window, _| window.remove_window());
}
