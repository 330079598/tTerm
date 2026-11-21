use super::*;

fn verify_focus_keys(cx: &mut TestAppContext, edge: Edge) {
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
    let (root, visual) = cx.add_window_view(|_, cx| make_workspace(&runtime, events, cx));
    let (inward, outward, across) = if edge == Edge::Right {
        ("left", "right", "up")
    } else {
        ("up", "down", "left")
    };
    let platform = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let cycle = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl-shift"
    };
    let direction = |key: &str| format!("{platform}-alt-{key}");
    let next = format!("{cycle}-]");
    let prev = format!("{cycle}-[");
    let press = |visual: &mut VisualTestContext, key: &str, visible, focused| {
        visual.update(|window, cx| window.render_frame(cx));
        visual.simulate_keystrokes(key);
        visual.update(|window, cx| check_visible(root.read(cx), visible, focused, window, cx));
    };
    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.select_tab(1, window, cx)));
    for key in [direction(inward), next.clone(), prev.clone()] {
        press(visual, &key, [Some(1), None], 1);
    }
    let models = visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, edge, window, cx);
            [1, 2].map(|id| {
                workspace
                    .tabs
                    .get(id)
                    .unwrap()
                    .binding
                    .as_ref()
                    .unwrap()
                    .session()
                    .unwrap()
                    .model()
            })
        })
    });
    let pair = [Some(1), Some(2)];
    for key in [outward, across] {
        press(visual, &direction(key), pair, 2);
    }
    press(visual, &direction(inward), pair, 1);
    send_marker(visual, &models[0], "firstfocus");
    press(visual, &direction(outward), pair, 2);
    send_marker(visual, &models[1], "secondfocus");
    assert!(!output(&models[1]).contains("firstfocus"));
    assert!(!output(&models[0]).contains("secondfocus"));
    for (key, id) in [(&next, 1), (&next, 2), (&prev, 1), (&prev, 2)] {
        press(visual, key, pair, id);
    }
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn pane_focus_keys_target_left_and_right_and_wrap(cx: &mut TestAppContext) {
    verify_focus_keys(cx, Edge::Right);
}

#[gpui_kit::test]
fn pane_focus_keys_target_up_and_down_and_wrap(cx: &mut TestAppContext) {
    verify_focus_keys(cx, Edge::Bottom);
}
