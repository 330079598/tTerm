//! Headless tests for the local-shells editor and its save behavior.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::list_actions::{can_hide, can_move_custom};
use super::path_status::EditorStatus;
use super::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{IntoElement, MouseButton, Render, TestAppContext};
use tterm_core::config::AppConfig;
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::SavedLocalShells;
use tterm_core::i18n::language::Language;
use tterm_core::terminal::pty::TerminalShellProfile;
use tterm_core::workspace::local_shells::default_shell::default_shell_updates;
use tterm_core::workspace::local_shells::types::{DefaultShellSettings, ShellEntryKind};
use tterm_ui::tokens::palette::DARK;

use crate::config_state::{ConfigState, LocalShellsWriter};
use crate::locale::Locale;

struct Probe {
    setting: Entity<LocalShellsSetting>,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.setting.update(cx, |setting, cx| setting.section(cx))
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test runtime")
}

fn shell(id: &str, name: &str, path: &str, args: &str) -> LocalShell {
    LocalShell {
        id: id.to_string(),
        name: name.to_string(),
        path: path.to_string(),
        args: args.to_string(),
    }
}

fn profile(shell: &str, label: &str, source: &str) -> TerminalShellProfile {
    TerminalShellProfile {
        shell: shell.to_string(),
        label: label.to_string(),
        source: source.to_string(),
        ..TerminalShellProfile::default()
    }
}

fn editor_with_detected(
    cx: &mut TestAppContext,
    config: AppConfig,
    writer: LocalShellsWriter,
    detected: Vec<TerminalShellProfile>,
) -> (tokio::runtime::Runtime, Entity<LocalShellsSetting>) {
    let (runtime, state) = bare_editor(cx, config, writer);
    (runtime, finish_editor(cx, state, detected))
}

fn editor_with_default(
    cx: &mut TestAppContext,
    config: AppConfig,
    detected: Vec<TerminalShellProfile>,
    writer: crate::config_state::DefaultShellWriter,
) -> (tokio::runtime::Runtime, Entity<LocalShellsSetting>) {
    let (runtime, state) = bare_state(cx, |runtime| {
        ConfigState::with_default_shell_writer(config, runtime, writer)
    });
    (runtime, finish_editor(cx, state, detected))
}

fn editor(
    cx: &mut TestAppContext,
    config: AppConfig,
    writer: LocalShellsWriter,
) -> (tokio::runtime::Runtime, Entity<LocalShellsSetting>) {
    let (runtime, state) = bare_editor(cx, config, writer);
    (runtime, finish_editor(cx, state, Vec::new()))
}

fn bare_editor(
    cx: &mut TestAppContext,
    config: AppConfig,
    writer: LocalShellsWriter,
) -> (tokio::runtime::Runtime, Entity<ConfigState>) {
    bare_state(cx, |runtime| {
        ConfigState::with_writers(
            config,
            runtime,
            Arc::new(|_| Ok(())),
            Arc::new(|_| Ok(())),
            writer,
        )
    })
}

fn bare_state(
    cx: &mut TestAppContext,
    build: impl FnOnce(&tokio::runtime::Handle) -> ConfigState,
) -> (tokio::runtime::Runtime, Entity<ConfigState>) {
    cx.update(gpui_kit::init);
    cx.executor().allow_parking();
    let runtime = runtime();
    let handle = runtime.handle().clone();
    let state = cx.new(|_| build(&handle));
    (runtime, state)
}

fn finish_editor(
    cx: &mut TestAppContext,
    config: Entity<ConfigState>,
    detected: Vec<TerminalShellProfile>,
) -> Entity<LocalShellsSetting> {
    let locale = cx.new(|_| Locale::new(tterm_core::i18n::language::Language::En));
    let setting = cx.new(|cx| LocalShellsSetting::new(config, locale, DARK, cx));
    setting.update(cx, |setting, cx| {
        setting.config.update(cx, |config, _| {
            config.set_detected_shells_for_test(detected)
        });
        setting.open_page(cx);
    });
    setting
}

fn show(
    cx: &mut TestAppContext,
    setting: Entity<LocalShellsSetting>,
) -> &mut gpui_kit::VisualTestContext {
    let (_, visual) = cx.add_window_view(|_, _| Probe { setting });
    visual
}

fn exists(visual: &mut gpui_kit::VisualTestContext, id: String) -> bool {
    // The debug-selector lookup needs a `'static` id, so tests leak the
    // generated name once per query.
    let id: &'static str = Box::leak(id.into_boxed_str());
    visual.update(|window, cx| window.render_frame(cx));
    visual.debug_bounds(id).is_some()
}

fn click(visual: &mut gpui_kit::VisualTestContext, id: String) {
    let id: &'static str = Box::leak(id.into_boxed_str());
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(id)
        .unwrap_or_else(|| panic!("{id} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

#[gpui_kit::test]
fn editing_loads_the_saved_values_and_keeps_the_same_id(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded.lock().expect("attempts").push(saved.clone());
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    let original = shell("nu", "my nu", "/bin/nu", "--login");
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            local_shells: vec![original.clone()],
            ..AppConfig::default()
        },
        writer,
    );
    let visual = show(cx, setting.clone());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_edit(original.clone(), window, cx)
        });
    });
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).editor_values(cx),
            Some((
                "/bin/nu".to_string(),
                "my nu".to_string(),
                "--login".to_string()
            ))
        );
    });

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            let name = setting.editor.as_ref().expect("editor").name.clone();
            name.update(cx, |state, cx| state.set_value("renamed nu", window, cx));
            setting.submit(window, cx);
        });
    });
    wait_until(visual, |cx| setting.read(cx).editor_values(cx).is_none());
    visual.update(|_, cx| {
        let config = setting.read(cx).config.read(cx).config();
        assert_eq!(config.local_shells.len(), 1);
        assert_eq!(config.local_shells[0].id, "nu");
        assert_eq!(config.local_shells[0].name, "renamed nu");
    });
    assert_eq!(
        attempts.lock().expect("attempts")[0].id,
        original.id,
        "editing must reuse the saved id instead of adding a second shell"
    );
}

#[gpui_kit::test]
fn a_failed_save_keeps_the_draft_and_a_retry_commits_it(cx: &mut TestAppContext) {
    let attempts = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&attempts);
    let failures = Arc::new(AtomicUsize::new(1));
    let remaining = Arc::clone(&failures);
    let writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded.lock().expect("attempts").push(saved.clone());
        if remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            return Err("the config disk is full".to_string());
        }
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    let (_runtime, setting) = editor(cx, AppConfig::default(), writer);
    let visual = show(cx, setting.clone());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_add(window, cx);
            let path = setting.editor.as_ref().expect("editor").path.clone();
            path.update(cx, |state, cx| state.set_value("/bin/nu", window, cx));
            setting.submit(window, cx);
        });
    });
    wait_until(visual, |cx| setting.read(cx).error().is_some());
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert!(
            setting.editor.is_some(),
            "the draft must stay after a failure"
        );
        assert_eq!(
            setting.editor_values(cx),
            Some(("/bin/nu".to_string(), String::new(), String::new()))
        );
        assert_eq!(setting.error(), Some("the config disk is full"));
        assert!(setting.config.read(cx).config().local_shells.is_empty());
        assert!(!setting.is_saving());
    });

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.submit(window, cx));
    });
    wait_until(visual, |cx| setting.read(cx).editor_values(cx).is_none());
    visual.update(|_, cx| {
        assert!(setting.read(cx).error().is_none());
        let shells = &setting.read(cx).config.read(cx).config().local_shells;
        assert_eq!(shells.len(), 1);
        assert_eq!(shells[0].path, "/bin/nu");
    });
    assert_eq!(attempts.lock().expect("attempts").len(), 2);
}

#[gpui_kit::test]
fn an_empty_program_is_rejected_without_writing(cx: &mut TestAppContext) {
    let writes = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&writes);
    let writer: LocalShellsWriter = Arc::new(move |saved| {
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    let (_runtime, setting) = editor(cx, AppConfig::default(), writer);
    let visual = show(cx, setting.clone());

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_add(window, cx);
            setting.submit(window, cx);
        });
    });
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert!(setting.editor.is_some());
        assert!(setting.error().is_some());
        assert!(!setting.is_saving());
    });
    assert_eq!(writes.load(Ordering::SeqCst), 0);
}

#[gpui_kit::test]
fn the_list_renders_auto_detected_and_custom_rows_with_the_shared_labels(cx: &mut TestAppContext) {
    let (writer, _writes) = ok_writer();
    let (_runtime, setting) = editor_with_detected(
        cx,
        AppConfig {
            local_shells: vec![shell("nu", "my nu", "/bin/nu", "--login")],
            ..AppConfig::default()
        },
        writer,
        vec![profile("custom", "zsh", "/bin/zsh")],
    );
    setting.update(cx, |setting, cx| {
        setting
            .locale
            .update(cx, |locale, cx| locale.set_language(Language::Zh, cx));
    });

    let entries = setting.update(cx, |setting, cx| {
        shell_entries(setting.config.read(cx), setting.locale.read(cx))
    });
    let listed: Vec<(&str, ShellEntryKind, &str)> = entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry.kind, entry.label.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("auto", ShellEntryKind::Auto, "自动（推荐）"),
            ("/bin/zsh", ShellEntryKind::Detected, "zsh"),
            ("custom:nu", ShellEntryKind::Custom, "my nu"),
        ]
    );

    let texts = setting.update(cx, |setting, cx| {
        let locale = setting.locale.read(cx);
        (
            locale
                .text(keys::LOCAL_SHELLS_DEFAULT)
                .expect("default label"),
            locale
                .text(keys::LOCAL_SHELLS_DETECTED)
                .expect("detected label"),
            locale
                .text(keys::LOCAL_SHELLS_MAKE_DEFAULT)
                .expect("make-default label"),
        )
    });
    assert_eq!(
        texts,
        (
            "默认".to_string(),
            "已检测到".to_string(),
            "设为默认".to_string()
        )
    );

    let visual = show(cx, setting.clone());
    for index in 0..3 {
        assert!(
            exists(visual, format!("{ROW_PREFIX}{index}")),
            "the list must render every core entry, including Auto"
        );
    }
    assert!(exists(visual, format!("{DEFAULT_PREFIX}0")));
    assert!(exists(visual, format!("{DETECTED_PREFIX}1")));
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}1")));
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}2")));
    assert!(exists(visual, format!("{EDIT_PREFIX}0")));
    assert!(
        !exists(visual, format!("{SET_DEFAULT_PREFIX}0")),
        "the default row must not offer to become the default"
    );
}

#[gpui_kit::test]
fn a_custom_default_marks_its_own_row_and_leaves_auto_actionable(cx: &mut TestAppContext) {
    let (writer, _writes) = ok_writer();
    let custom = shell("nu", "my nu", "/bin/nu", "--login");
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            terminal_shell: "custom".to_string(),
            terminal_shell_id: custom.id.clone(),
            local_shells: vec![custom],
            ..AppConfig::default()
        },
        writer,
    );
    let visual = show(cx, setting.clone());
    visual.update(|window, cx| window.render_frame(cx));

    assert!(exists(visual, format!("{DEFAULT_PREFIX}1")));
    assert!(
        !exists(visual, format!("{DEFAULT_PREFIX}0")),
        "Auto is not the default here"
    );
    assert!(!exists(visual, format!("{SET_DEFAULT_PREFIX}1")));
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}0")));
}

#[gpui_kit::test]
fn choosing_a_default_writes_the_choice_and_moves_the_badge(cx: &mut TestAppContext) {
    let choices = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&choices);
    let custom = shell("nu", "my nu", "/bin/nu", "--login");
    let local_shells = vec![custom.clone()];
    let written = local_shells.clone();
    let writer: crate::config_state::DefaultShellWriter = Arc::new(move |choice| {
        recorded
            .lock()
            .expect("default choices")
            .push(choice.clone());
        Ok(SavedLocalShells {
            local_shells: written.clone(),
            default_shell: default_shell_updates(&choice),
        })
    });
    let (_runtime, setting) = editor_with_default(
        cx,
        AppConfig {
            local_shells,
            ..AppConfig::default()
        },
        vec![profile("custom", "zsh", "/bin/zsh")],
        writer,
    );
    let visual = show(cx, setting.clone());
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}2")));

    click(visual, format!("{SET_DEFAULT_PREFIX}2"));
    wait_until(visual, |cx| {
        setting.read(cx).config.read(cx).config().terminal_shell_id == custom.id
    });
    let choices = choices.lock().expect("default choices");
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].shell, "custom");
    assert_eq!(choices[0].shell_id.as_deref(), Some(custom.id.as_str()));
    assert_eq!(choices[0].custom_path.as_deref(), Some("/bin/nu"));
    assert_eq!(choices[0].custom_args.as_deref(), Some("--login"));
    drop(choices);

    visual.update(|window, cx| window.render_frame(cx));
    assert!(exists(visual, format!("{DEFAULT_PREFIX}2")));
    assert!(!exists(visual, format!("{DEFAULT_PREFIX}0")));
    assert!(!exists(visual, format!("{SET_DEFAULT_PREFIX}2")));
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}0")));
}

#[gpui_kit::test]
fn the_empty_state_remains_when_only_auto_and_detected_rows_are_listed(cx: &mut TestAppContext) {
    let (writer, _writes) = ok_writer();
    let (_runtime, setting) = editor_with_detected(
        cx,
        AppConfig::default(),
        writer,
        vec![profile("custom", "zsh", "/bin/zsh")],
    );
    let visual = show(cx, setting.clone());
    visual.update(|window, cx| window.render_frame(cx));

    assert!(exists(visual, PAGE_EMPTY_ID.to_string()));
}

#[gpui_kit::test]
fn a_default_write_error_stays_visible_without_an_editor_and_keeps_a_draft(
    cx: &mut TestAppContext,
) {
    let attempts = Arc::new(Mutex::new(0));
    let counted = Arc::clone(&attempts);
    let writer: crate::config_state::DefaultShellWriter = Arc::new(move |_| {
        let mut attempts = counted.lock().expect("default attempts");
        *attempts += 1;
        Err(if *attempts == 1 {
            "the config disk is full".to_string()
        } else {
            "the config disk is still full".to_string()
        })
    });
    let (_runtime, setting) = editor_with_default(
        cx,
        AppConfig::default(),
        vec![profile("custom", "zsh", "/bin/zsh")],
        writer,
    );
    let visual = show(cx, setting.clone());
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}1")));

    click(visual, format!("{SET_DEFAULT_PREFIX}1"));
    wait_until(visual, |cx| {
        setting.read(cx).list_error.as_deref() == Some("the config disk is full")
    });
    assert!(exists(visual, LIST_ERROR_ID.to_string()));
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert!(setting.editor.is_none());
    });

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_add(window, cx);
            let path = setting.editor.as_ref().expect("editor").path.clone();
            path.update(cx, |state, cx| state.set_value("/bin/nu", window, cx));
        });
    });
    assert!(exists(visual, format!("{SET_DEFAULT_PREFIX}1")));
    click(visual, format!("{SET_DEFAULT_PREFIX}1"));
    wait_until(visual, |cx| {
        setting.read(cx).list_error.as_deref() == Some("the config disk is still full")
    });
    assert!(exists(visual, LIST_ERROR_ID.to_string()));
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert_eq!(
            setting.editor_values(cx),
            Some(("/bin/nu".to_string(), String::new(), String::new()))
        );
    });
    assert_eq!(*attempts.lock().expect("default attempts"), 2);
}

#[gpui_kit::test]
fn row_actions_are_offered_only_where_they_apply(cx: &mut TestAppContext) {
    let (writer, _writes) = ok_writer();
    let (_runtime, setting) = editor_with_detected(
        cx,
        AppConfig {
            terminal_shell: "pwsh".to_string(),
            hidden_shells: vec!["/bin/zsh".to_string()],
            local_shells: vec![
                shell("one", "one", "/bin/one", ""),
                shell("two", "two", "/bin/two", ""),
            ],
            ..AppConfig::default()
        },
        writer,
        vec![
            profile("pwsh", "PowerShell", "/bin/pwsh"),
            profile("custom", "zsh", "/bin/zsh"),
        ],
    );
    let visual = show(cx, setting.clone());

    // Auto has no shell management action; the visible default and a hidden
    // detected shell keep their copy and visibility controls.
    assert!(!exists(visual, format!("{COPY_PREFIX}0")));
    assert!(!exists(visual, format!("{VISIBILITY_PREFIX}0")));
    assert!(exists(visual, format!("{COPY_PREFIX}1")));
    assert!(exists(visual, format!("{VISIBILITY_PREFIX}1")));
    assert!(exists(visual, format!("{COPY_PREFIX}2")));
    assert!(exists(visual, format!("{VISIBILITY_PREFIX}2")));
    assert!(exists(visual, format!("{HIDDEN_PREFIX}2")));
    assert!(!exists(visual, format!("{HIDDEN_PREFIX}1")));

    // The user's own shells move, edit and remove, but are not copied.
    assert!(!exists(visual, format!("{COPY_PREFIX}3")));
    assert!(exists(visual, format!("{MOVE_UP_PREFIX}0")));
    assert!(exists(visual, format!("{MOVE_DOWN_PREFIX}0")));
    assert!(exists(visual, format!("{EDIT_PREFIX}0")));
    assert!(exists(visual, format!("{REMOVE_PREFIX}0")));
    assert!(exists(visual, format!("{MOVE_UP_PREFIX}1")));
    assert!(exists(visual, format!("{MOVE_DOWN_PREFIX}1")));

    // The edge controls are disabled, and the visible default cannot hide.
    assert!(!can_move_custom(0, 2, -1));
    assert!(can_move_custom(0, 2, 1));
    assert!(can_move_custom(1, 2, -1));
    assert!(!can_move_custom(1, 2, 1));
    assert!(!can_hide(true, false));
    assert!(can_hide(true, true));
    assert!(can_hide(false, false));
}

#[gpui_kit::test]
fn copying_a_detected_shell_opens_a_fresh_draft_and_cancel_writes_nothing(cx: &mut TestAppContext) {
    let (writer, writes) = ok_writer();
    let mut wsl = profile("wsl", "Ubuntu (WSL)", "wsl.exe");
    wsl.wsl_distro = Some("Ubuntu".to_string());
    let (_runtime, setting) = editor_with_detected(
        cx,
        AppConfig {
            local_shells: vec![shell("existing", "existing", "/bin/existing", "")],
            ..AppConfig::default()
        },
        writer,
        vec![wsl],
    );
    let visual = show(cx, setting.clone());
    assert!(exists(visual, format!("{COPY_PREFIX}1")));

    click(visual, format!("{COPY_PREFIX}1"));
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).editor_values(cx),
            Some((
                "wsl.exe".to_string(),
                "Ubuntu (WSL)".to_string(),
                "-d Ubuntu".to_string()
            ))
        );
        let id = &setting.read(cx).editor.as_ref().expect("copied draft").id;
        assert_ne!(id, "existing", "a copy must not replace the saved shell");
    });

    click(visual, CANCEL_ID.to_string());
    visual.update(|_, cx| {
        let setting = setting.read(cx);
        assert!(setting.editor.is_none());
        assert_eq!(
            setting.config.read(cx).config().local_shells,
            vec![shell("existing", "existing", "/bin/existing", "")]
        );
    });
    assert!(
        writes.lock().expect("writes").is_empty(),
        "cancelling a copy must not write the config file"
    );
}

fn wait_until(
    visual: &mut gpui_kit::VisualTestContext,
    mut condition: impl FnMut(&gpui_kit::App) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !visual.update(|_, cx| condition(cx)) {
        assert!(Instant::now() < deadline, "timed out waiting for the save");
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn ok_writer() -> (LocalShellsWriter, Arc<Mutex<Vec<LocalShell>>>) {
    let writes = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&writes);
    let writer: LocalShellsWriter = Arc::new(move |saved| {
        recorded.lock().expect("writes").push(saved.clone());
        Ok(SavedLocalShells {
            local_shells: vec![saved],
            default_shell: DefaultShellSettings::default(),
        })
    });
    (writer, writes)
}

/// An absolute path that no program occupies on this machine.
fn missing_program_path() -> String {
    std::env::temp_dir()
        .join(format!("tterm-gpui-path-check-{}", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

#[gpui_kit::test]
fn the_editor_waits_for_typing_to_pause_before_it_checks_the_path(cx: &mut TestAppContext) {
    let (writer, _writes) = ok_writer();
    let (_runtime, setting) = editor(cx, AppConfig::default(), writer);
    let visual = show(cx, setting.clone());
    let missing = missing_program_path();

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_add(window, cx);
            setting.begin_editor_check(&missing, cx);
        });
    });
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).paths.editor_status(),
            &EditorStatus::Checking
        );
    });

    // Poll what can run without moving the clock, so the check reaches its
    // debounce timer and stops there.
    while visual.executor().tick() {}
    visual
        .executor()
        .advance_clock(PATH_CHECK_DELAY - Duration::from_millis(1));
    while visual.executor().tick() {}
    visual.update(|_, cx| {
        assert_eq!(
            setting.read(cx).paths.editor_status(),
            &EditorStatus::Checking,
            "a check before typing pauses would run for every letter typed"
        );
    });

    visual.executor().advance_clock(Duration::from_millis(1));
    wait_until(visual, |cx| {
        setting.read(cx).paths.editor_status() == &EditorStatus::NotFound
    });
}

#[gpui_kit::test]
fn a_program_that_leads_nowhere_is_advisory_and_still_saves(cx: &mut TestAppContext) {
    let (writer, writes) = ok_writer();
    let (_runtime, setting) = editor(cx, AppConfig::default(), writer);
    let visual = show(cx, setting.clone());
    let missing = missing_program_path();

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| {
            setting.start_add(window, cx);
            let path = setting.editor.as_ref().expect("editor").path.clone();
            path.update(cx, |state, cx| state.set_value(missing.clone(), window, cx));
            setting.begin_editor_check(&missing, cx);
        });
    });
    visual.executor().advance_clock(PATH_CHECK_DELAY);
    wait_until(visual, |cx| {
        setting.read(cx).paths.editor_status() == &EditorStatus::NotFound
    });

    visual.update(|window, cx| {
        setting.update(cx, |setting, cx| setting.submit(window, cx));
    });
    wait_until(visual, |cx| setting.read(cx).editor_values(cx).is_none());
    assert_eq!(writes.lock().expect("writes").len(), 1);
    visual.update(|_, cx| {
        let config = setting.read(cx).config.read(cx).config();
        assert_eq!(config.local_shells[0].path, missing);
    });
}

#[gpui_kit::test]
fn a_saved_shell_that_leads_nowhere_keeps_a_cached_missing_badge(cx: &mut TestAppContext) {
    let missing = missing_program_path();
    let (writer, _writes) = ok_writer();
    let (_runtime, setting) = editor(
        cx,
        AppConfig {
            local_shells: vec![shell("gone", "gone", &missing, "")],
            ..AppConfig::default()
        },
        writer,
    );
    let visual = show(cx, setting.clone());

    wait_until(visual, |cx| setting.read(cx).paths.saved_missing(&missing));
    visual.update(|window, cx| window.render_frame(cx));
    assert!(
        visual.debug_bounds(MISSING_ID).is_some(),
        "the saved row must show what the check found"
    );

    // Rendering again reads the cache rather than starting another check.
    visual.update(|window, cx| window.render_frame(cx));
    assert!(visual.debug_bounds(MISSING_ID).is_some());
    visual.update(|_, cx| {
        assert!(setting.read(cx).paths.saved_missing(&missing));
    });
}
