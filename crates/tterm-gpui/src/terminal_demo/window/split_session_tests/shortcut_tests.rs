use super::*;
use tterm_core::workspace::tab_groups::SplitOrientation;

fn with_workspace(
    cx: &mut TestAppContext,
    command: CommandSpec,
    config: AppConfig,
    run: impl FnOnce(Entity<Prototype>, &mut VisualTestContext),
) {
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
    let (root, visual) =
        cx.add_window_view(|_, cx| make_workspace_with(&runtime, events, command, config, cx));
    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.select_tab(1, window, cx)));
    run(root, visual);
    visual.update(|window, _| window.remove_window());
}

fn press(visual: &mut VisualTestContext, suffix: &str) {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    visual.update(|window, cx| window.render_frame(cx));
    visual.simulate_keystrokes(&format!("{modifier}-{suffix}"));
}

fn verify_split(cx: &mut TestAppContext, config: AppConfig, key: &str, axis: SplitOrientation) {
    with_workspace(cx, CommandSpec::Shell, config, |root, visual| {
        let source = visual.update(|_, cx| {
            root.read(cx)
                .tabs
                .active()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
        });
        press(visual, key);
        let second = visual.update(|window, cx| {
            let workspace = root.read(cx);
            check_visible(workspace, [Some(1), Some(2)], 2, window, cx);
            assert_eq!(workspace.splits.pair_for(1).unwrap().axis, axis);
            workspace
                .tabs
                .active()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
        });
        assert!(!Arc::ptr_eq(&source, &second));
        send_marker(visual, &second.model(), "newpane");
        assert!(!output(&source.model()).contains("newpane"));
        press(visual, key);
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                check_visible(workspace, [Some(1), Some(2)], 2, window, cx);
                assert_eq!(workspace.tabs.ordered().len(), 2);
                assert!(workspace.factory.sessions.tab("gpui-3").is_none());
                workspace.close_tab(2, window, cx);
                check_visible(workspace, [Some(1), None], 1, window, cx);
                assert!(workspace.factory.sessions.tab("gpui-2").is_none());
            })
        });
        send_marker(visual, &source.model(), "survivorpane");
    });
}

#[gpui_kit::test]
fn default_shortcut_splits_right(cx: &mut TestAppContext) {
    verify_split(cx, AppConfig::default(), "\\", SplitOrientation::Horizontal);
}

#[gpui_kit::test]
fn default_shortcut_splits_below(cx: &mut TestAppContext) {
    verify_split(
        cx,
        AppConfig::default(),
        "shift-\\",
        SplitOrientation::Vertical,
    );
}

#[gpui_kit::test]
fn split_honors_custom_binding(cx: &mut TestAppContext) {
    let mut config = AppConfig::default();
    config.keymap.bindings.insert(
        "workspace.splitBelow".into(),
        Some(vec!["mod+alt+y".into()]),
    );
    verify_split(cx, config, "alt-y", SplitOrientation::Vertical);
}

#[gpui_kit::test]
fn split_does_not_duplicate_exec_programs(cx: &mut TestAppContext) {
    with_workspace(cx, shell(), AppConfig::default(), |root, visual| {
        press(visual, "\\");
        visual.update(|window, cx| {
            let workspace = root.read(cx);
            check_visible(workspace, [Some(1), None], 1, window, cx);
            assert!(workspace.factory.sessions.tab("gpui-2").is_none());
        });
    });
}
