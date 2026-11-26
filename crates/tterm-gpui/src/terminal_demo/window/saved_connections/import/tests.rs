//! Regressions for the SSH config import dialog over a live workspace.

use std::path::PathBuf;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{App, Entity, TestAppContext, VisualTestContext};
use tterm_core::config::AppConfig;
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::profiles::types::{
    SavedJumpHost, SshConfigImportHost, SshConfigImportOptions, SshConfigImportPreview,
    SshConfigImportResult,
};
use tterm_core::tunnel::forwards::ForwardSpec;
use tterm_core::tunnel::types::TunnelKind;

use super::super::{OPEN_BUTTON_ID, SavedConnectionsUi};
use super::{
    IMPORT_BUTTON_ID, IMPORT_CHOOSE_DIRECTORY_ID, IMPORT_CHOOSE_FILE_ID, IMPORT_PREVIEW_ID,
};
use crate::terminal_demo::command::CommandSpec;
use crate::terminal_demo::runtime::{GpuiRuntime, test_runtime};
use crate::terminal_demo::window::Prototype;
use crate::terminal_demo::window::split_session_tests::make_workspace_with;

struct Harness<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
}

fn shell() -> CommandSpec {
    #[cfg(windows)]
    let (program, args) = ("cmd.exe", vec!["/d".into(), "/q".into()]);
    #[cfg(not(windows))]
    let (program, args) = ("/bin/sh", vec!["-i".into()]);
    CommandSpec::Program {
        program: program.into(),
        args,
    }
}

fn harness<'a>(cx: &'a mut TestAppContext) -> Harness<'a> {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
    });
    let (root, visual) = cx.add_window_view(|_, cx| {
        make_workspace_with(&runtime, events, shell(), AppConfig::default(), cx)
    });
    Harness {
        _runtime: runtime,
        root,
        visual,
    }
}

fn summary(id: &str, name: &str) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some("deploy".to_string()),
        host: Some("host.example".to_string()),
        port: Some(22),
        group_path: None,
    }
}

fn picker<'a>(root: &'a Entity<Prototype>, cx: &'a App) -> &'a SavedConnectionsUi {
    root.read(cx).ui.saved_connections.read(cx)
}

fn with_picker(
    root: &Entity<Prototype>,
    cx: &mut App,
    update: impl FnOnce(&mut SavedConnectionsUi, &mut gpui_kit::Context<SavedConnectionsUi>),
) {
    let picker = root.read(cx).ui.saved_connections.clone();
    picker.update(cx, update);
}

fn click(visual: &mut VisualTestContext, selector: &'static str) {
    visual.update(|window, cx| window.render_frame(cx));
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be laid out"));
    let center = bounds.center();
    visual.simulate_mouse_down(center, gpui_kit::MouseButton::Left, Default::default());
    visual.simulate_mouse_up(center, gpui_kit::MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
}

fn open_picker(harness: &mut Harness<'_>) {
    click(harness.visual, OPEN_BUTTON_ID);
}

fn importing_host(pattern: &str, name: &str, username: &str, host: &str) -> SshConfigImportHost {
    SshConfigImportHost {
        host_pattern: pattern.to_string(),
        name: name.to_string(),
        host: Some(host.to_string()),
        port: 2222,
        username: Some(username.to_string()),
        auth_method: "key".to_string(),
        private_key_path: Some("~/.ssh/id_ed25519".to_string()),
        certificate_path: None,
        agent_forward: false,
        keepalive_interval_secs: 15,
        keepalive_count_max: 3,
        jump_hosts: vec![SavedJumpHost {
            host: "bastion.example".to_string(),
            port: 22,
            username: "deploy".to_string(),
            auth_method: "key".to_string(),
            private_key_path: None,
            private_key_passphrase: None,
            certificate_path: None,
            password: None,
            profile_id: None,
        }],
        proxy: None,
        warnings: vec![
            "ProxyJump entries without a User were imported with the user name 'local'; verify each jump host username.".to_string(),
        ],
        unsupported_options: Vec::new(),
        forwards: vec![ForwardSpec {
            kind: TunnelKind::Local,
            bind_host: "127.0.0.1".to_string(),
            bind_port: 8080,
            dest_host: "db.internal".to_string(),
            dest_port: 5432,
        }],
        skipped: false,
        skip_reason: None,
        existing_profile_id: None,
    }
}

fn skipped_host() -> SshConfigImportHost {
    let mut host = importing_host("*.internal", "", "", "");
    host.host = None;
    host.username = None;
    host.jump_hosts.clear();
    host.forwards.clear();
    host.warnings.clear();
    host.skipped = true;
    host.skip_reason = Some("Pattern hosts are not imported one by one.".to_string());
    host
}

fn preview() -> SshConfigImportPreview {
    SshConfigImportPreview {
        source_path: "/tmp/isolated/.ssh/config".to_string(),
        hosts: vec![
            importing_host("atlas", "Atlas", "deploy", "atlas.example"),
            skipped_host(),
        ],
        warnings: vec!["Include file 'conf.d/extra' has an unsupported directive.".to_string()],
    }
}

fn imported() -> SshConfigImportResult {
    SshConfigImportResult {
        imported: 1,
        updated: 0,
        skipped: 1,
        tunnels_imported: 1,
        tunnels_skipped: 0,
        profiles: Vec::new(),
    }
}

/// Opens the picker with one saved host and enters the import dialog.
fn open_import(harness: &mut Harness<'_>, preview: SshConfigImportPreview) {
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(vec![summary("saved-1", "Existing")]), cx);
            picker.set_import_preview_for_test(Ok(preview));
        });
    });
    // The picker's overlay covers the title bar, so clicking the open button
    // while it is already visible would dismiss it instead of reopening it.
    let already_open = harness.visual.update(|_, cx| picker(&root, cx).is_open());
    if !already_open {
        open_picker(harness);
    }
    click(harness.visual, IMPORT_BUTTON_ID);
}

#[gpui_kit::test]
fn opening_the_importer_previews_and_selects_every_importable_host(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    open_import(&mut harness, preview());

    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(picker.import_is_open());
        let dialog = picker
            .import_state_for_test()
            .expect("the import dialog should be open");
        assert!(!dialog.loading);
        assert_eq!(dialog.preview.as_ref().map(|p| p.hosts.len()), Some(2));
        assert_eq!(dialog.selectable_count(), 1);
        assert_eq!(dialog.selected.len(), 1);
        assert!(dialog.selected.contains("atlas"));
        assert!(dialog.all_selected());
        assert_eq!(dialog.forward_count(), 1);
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.as_str()),
            Some("/tmp/isolated/.ssh/config")
        );
    });
}

#[gpui_kit::test]
fn selection_toggles_and_submit_send_only_explicit_choices(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| picker.toggle_import_all(cx));
    });
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open");
        assert!(dialog.selected.is_empty());
        assert_eq!(dialog.forward_count(), 0);
    });
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.toggle_import_all(cx);
            picker.toggle_import_host("atlas", cx);
        });
    });
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open");
        assert!(dialog.selected.is_empty());
    });
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.toggle_import_host("atlas", cx);
        });
    });
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open");
        assert!(dialog.selected.contains("atlas"));
    });
    harness.visual.update(|window, cx| {
        let picker_entity = root.read(cx).ui.saved_connections.clone();
        picker_entity.update(cx, |picker, cx| {
            picker.toggle_import_overwrite(cx);
            picker
                .import_state_for_test()
                .expect("the import dialog should be open")
                .source
                .update(cx, |source, cx| {
                    source.set_value("/tmp/other/config", window, cx);
                });
            picker.set_import_result_for_test(Ok(imported()));
            picker.submit_import(window, cx);
        });
    });
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(!picker.import_is_open());
        let options: &SshConfigImportOptions = picker
            .import_options_for_test()
            .expect("submit should record its options");
        assert_eq!(
            options.source_path.as_deref(),
            Some("/tmp/isolated/.ssh/config"),
            "an edited path must not silently switch the import source"
        );
        assert_eq!(options.group.as_deref(), Some("Imported from SSH config"));
        assert!(options.overwrite_existing);
        assert!(options.import_forwards);
        assert_eq!(options.selected_hosts, vec!["atlas".to_string()]);
    });
}

#[gpui_kit::test]
fn a_finished_import_reports_its_counts_and_keeps_the_picker_open(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_import_result_for_test(Ok(imported()));
            picker.submit_import(window, cx);
        });
    });
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(!picker.import_is_open());
        assert!(picker.is_open());
        let notice = picker
            .notice()
            .expect("the picker should report the import");
        assert!(notice.contains("Imported 1"), "unexpected notice: {notice}");
        assert!(notice.contains("skipped 1"), "unexpected notice: {notice}");
    });
}

#[gpui_kit::test]
fn escape_cancels_the_import_without_closing_the_picker(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    harness.visual.simulate_keystrokes("escape");
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        assert!(!picker.import_is_open());
        assert!(picker.is_open());
    });
}

#[gpui_kit::test]
fn a_preview_failure_shows_a_friendly_message(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.set_profiles_for_test(Ok(vec![summary("saved-1", "Existing")]), cx);
            picker.set_import_preview_for_test(Err(
                "SSH config file not found: /tmp/missing/config".to_string(),
            ));
        });
    });
    open_picker(&mut harness);
    click(harness.visual, IMPORT_BUTTON_ID);
    harness.visual.update(|_, cx| {
        let picker = picker(&root, cx);
        let error = picker
            .import_error_for_test()
            .expect("the failure should be visible");
        assert!(
            error.to_lowercase().contains("not found"),
            "unexpected message: {error}"
        );
        assert!(picker.import_is_open());
    });
}

#[gpui_kit::test]
fn reopening_the_importer_ignores_a_late_result_from_the_old_dialog(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    let stale_generation = harness.visual.update(|_, cx| {
        picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open")
            .generation
    });
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| picker.cancel_import(window, cx));
    });
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| {
            picker.open_import(window, cx);
        });
    });
    let mut other = preview();
    other.hosts[0].name = "Replacement".to_string();
    harness.visual.update(|window, cx| {
        let picker_entity = root.read(cx).ui.saved_connections.clone();
        picker_entity.update(cx, |picker, cx| {
            picker.finish_import_preview(stale_generation, Ok(other), window, cx);
        });
    });
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should stay open");
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.hosts[0].name.as_str()),
            Some("Atlas"),
            "a result from the closed dialog must not replace the new preview"
        );
    });
}

#[gpui_kit::test]
fn choosing_a_file_prompts_updates_the_source_and_previews(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    let selected = std::env::temp_dir().join("p4-03-selected-config");
    let mut selected_preview = preview();
    selected_preview.source_path = selected.to_string_lossy().into_owned();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, _cx| {
            picker.set_import_preview_for_test(Ok(selected_preview));
        });
    });

    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    assert!(harness.visual.did_prompt_for_paths());
    harness.visual.simulate_path_prompt_response(|options| {
        assert!(options.files);
        assert!(!options.directories);
        assert!(!options.multiple);
        assert!(options.prompt.is_some());
        Some(vec![selected.clone()])
    });
    harness.visual.run_until_parked();

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert_eq!(
            dialog.source.read(cx).value(),
            selected.to_string_lossy().as_ref()
        );
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.as_str()),
            Some(selected.to_string_lossy().as_ref())
        );
        assert!(!dialog.loading);
        assert!(dialog.error.is_none());
    });
    assert!(!harness.visual.did_prompt_for_paths());
}

#[gpui_kit::test]
fn choosing_a_directory_prompts_updates_the_source_and_previews(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    let selected = std::env::temp_dir().join("p4-03-selected-directory");
    let config = selected.join("config");
    let mut selected_preview = preview();
    selected_preview.source_path = config.to_string_lossy().into_owned();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, _cx| {
            picker.set_import_preview_for_test(Ok(selected_preview));
        });
    });

    click(harness.visual, IMPORT_CHOOSE_DIRECTORY_ID);
    assert!(harness.visual.did_prompt_for_paths());
    harness.visual.simulate_path_prompt_response(|options| {
        assert!(!options.files);
        assert!(options.directories);
        assert!(!options.multiple);
        Some(vec![selected.clone()])
    });
    harness.visual.run_until_parked();

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert_eq!(
            dialog.source.read(cx).value(),
            config.to_string_lossy().as_ref()
        );
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.as_str()),
            Some(config.to_string_lossy().as_ref())
        );
        assert!(!dialog.loading);
        assert!(dialog.error.is_none());
    });
}

#[gpui_kit::test]
fn cancelling_the_source_picker_keeps_the_current_preview(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    let (source, preview_source, generation) = harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open");
        (
            dialog.source.read(cx).value().to_string(),
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.clone()),
            dialog.generation,
        )
    });

    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    harness.visual.simulate_path_prompt_response(|_| None);
    harness.visual.run_until_parked();

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert_eq!(dialog.source.read(cx).value(), source);
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.clone()),
            preview_source
        );
        assert_eq!(dialog.generation, generation);
        assert!(!dialog.picker_open);
        assert!(!dialog.loading);
        assert!(dialog.error.is_none());
    });
    assert!(!harness.visual.did_prompt_for_paths());
}

#[gpui_kit::test]
fn a_source_pick_after_the_dialog_reopens_is_ignored(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    assert!(harness.visual.did_prompt_for_paths());
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| picker.cancel_import(window, cx));
    });

    let mut reopened = preview();
    reopened.source_path = "/tmp/reopened/.ssh/config".to_string();
    open_import(&mut harness, reopened);
    let (source, preview_source, generation) = harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should be open");
        (
            dialog.source.read(cx).value().to_string(),
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.clone()),
            dialog.generation,
        )
    });

    harness
        .visual
        .simulate_path_prompt_response(|_| Some(vec![PathBuf::from("stale-config")]));
    harness.visual.run_until_parked();

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should stay open");
        assert_eq!(dialog.source.read(cx).value(), source);
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.clone()),
            preview_source
        );
        assert_eq!(dialog.generation, generation);
        assert!(!dialog.picker_open);
        assert!(dialog.error.is_none());
    });
}

#[gpui_kit::test]
fn an_old_dialog_pick_cannot_unlock_the_reopened_pickers_dialog(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    harness.visual.update(|window, cx| {
        with_picker(&root, cx, |picker, cx| picker.cancel_import(window, cx));
    });

    open_import(&mut harness, preview());
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should be open");
        assert!(dialog.picker_open);
    });

    harness
        .visual
        .simulate_path_prompt_response(|_| Some(vec![PathBuf::from("stale-config")]));
    harness.visual.run_until_parked();
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should stay open");
        assert!(
            dialog.picker_open,
            "a result from the closed dialog must not unlock the new picker"
        );
        assert_eq!(dialog.source.read(cx).value(), "");
    });
    assert!(harness.visual.did_prompt_for_paths());

    let selected = std::env::temp_dir().join("p4-03-reopened-selection");
    let mut selected_preview = preview();
    selected_preview.source_path = selected.to_string_lossy().into_owned();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, _cx| {
            picker.set_import_preview_for_test(Ok(selected_preview));
        });
    });
    harness
        .visual
        .simulate_path_prompt_response(|_| Some(vec![selected.clone()]));
    harness.visual.run_until_parked();
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the second import dialog should stay open");
        assert!(!dialog.picker_open);
        assert_eq!(
            dialog.source.read(cx).value(),
            selected.to_string_lossy().as_ref()
        );
    });
}

#[gpui_kit::test]
fn a_late_preview_does_not_overwrite_a_typed_source(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    let generation = harness.visual.update(|_, cx| {
        picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should be open")
            .generation
    });
    harness.visual.update(|window, cx| {
        let picker = root.read(cx).ui.saved_connections.clone();
        picker.update(cx, |picker, cx| {
            let dialog = picker
                .import
                .as_mut()
                .expect("the import dialog should be open");
            dialog.source.update(cx, |source, cx| {
                source.set_value("/tmp/typed-directory", window, cx);
            });
        });
    });
    let mut late = preview();
    late.source_path = "/tmp/typed-directory/config".to_string();
    harness.visual.update(|window, cx| {
        let picker = root.read(cx).ui.saved_connections.clone();
        picker.update(cx, |picker, cx| {
            picker.finish_import_preview(generation, Ok(late), window, cx);
        });
    });

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert_eq!(
            dialog.source.read(cx).value(),
            "/tmp/typed-directory",
            "a preview must not replace the path the user is typing"
        );
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.as_str()),
            Some("/tmp/typed-directory/config")
        );
    });
}

#[gpui_kit::test]
fn a_manual_preview_while_the_source_picker_is_open_releases_its_lock(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert!(dialog.picker_open);
    });

    click(harness.visual, IMPORT_PREVIEW_ID);
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert!(dialog.picker_open, "the picker is still waiting");
    });
    harness
        .visual
        .simulate_path_prompt_response(|_| Some(vec![PathBuf::from("stale-config")]));
    harness.visual.run_until_parked();

    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert!(
            !dialog.picker_open,
            "a changed generation must still release the picker lock"
        );
        assert_eq!(dialog.source.read(cx).value(), "");
        assert_eq!(
            dialog
                .preview
                .as_ref()
                .map(|preview| preview.source_path.as_str()),
            Some("/tmp/isolated/.ssh/config")
        );
    });
}

#[gpui_kit::test]
fn a_second_source_pick_does_not_open_another_dialog(cx: &mut TestAppContext) {
    let mut harness = harness(cx);
    let root = harness.root.clone();
    open_import(&mut harness, preview());
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    click(harness.visual, IMPORT_CHOOSE_FILE_ID);
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert!(dialog.picker_open);
    });

    let selected = std::env::temp_dir().join("p4-03-after-second-click");
    let mut selected_preview = preview();
    selected_preview.source_path = selected.to_string_lossy().into_owned();
    harness.visual.update(|_, cx| {
        with_picker(&root, cx, |picker, _cx| {
            picker.set_import_preview_for_test(Ok(selected_preview));
        });
    });
    harness
        .visual
        .simulate_path_prompt_response(|_| Some(vec![selected.clone()]));
    harness.visual.run_until_parked();
    harness.visual.update(|_, cx| {
        let dialog = picker(&root, cx)
            .import_state_for_test()
            .expect("the import dialog should stay open");
        assert_eq!(
            dialog.source.read(cx).value(),
            selected.to_string_lossy().as_ref()
        );
        assert!(!dialog.picker_open);
    });
    assert!(!harness.visual.did_prompt_for_paths());
}
