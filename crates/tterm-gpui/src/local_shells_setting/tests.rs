//! Headless tests for the local-shells editor and its save behavior.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::*;
use gpui_kit::{IntoElement, Render, TestAppContext};
use tterm_core::config::AppConfig;
use tterm_core::config::local_shells::LocalShell;
use tterm_core::config::local_shells_file::SavedLocalShells;
use tterm_core::workspace::local_shells::types::DefaultShellSettings;
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

fn editor(
    cx: &mut TestAppContext,
    config: AppConfig,
    writer: LocalShellsWriter,
) -> (tokio::runtime::Runtime, Entity<LocalShellsSetting>) {
    cx.update(gpui_kit::init);
    cx.executor().allow_parking();
    let runtime = runtime();
    let config = cx.new(|_| {
        ConfigState::with_writers(
            config,
            runtime.handle(),
            Arc::new(|_| Ok(())),
            Arc::new(|_| Ok(())),
            writer,
        )
    });
    let locale = cx.new(|_| Locale::new(tterm_core::i18n::language::Language::En));
    let setting = cx.new(|cx| LocalShellsSetting::new(config, locale, DARK, cx));
    setting.update(cx, |setting, cx| setting.open_page(cx));
    (runtime, setting)
}

fn show(
    cx: &mut TestAppContext,
    setting: Entity<LocalShellsSetting>,
) -> &mut gpui_kit::VisualTestContext {
    let (_, visual) = cx.add_window_view(|_, _| Probe { setting });
    visual
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
