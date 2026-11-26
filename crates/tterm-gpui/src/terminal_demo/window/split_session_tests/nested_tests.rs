//! Nested trees keep every pane live while the visible leaves take turns with input.

use gpui_kit::{InputEvent, MouseButton, MouseMoveEvent, MouseUpEvent, point, px};
use tterm_core::workspace::tab_groups::{PaneNode, SplitOrientation};

use super::*;

struct NestedWorkspace<'a> {
    _runtime: GpuiRuntime,
    root: Entity<Prototype>,
    visual: &'a mut VisualTestContext,
}

fn nested_workspace(cx: &mut TestAppContext) -> NestedWorkspace<'_> {
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
    NestedWorkspace {
        _runtime: runtime,
        root,
        visual,
    }
}

fn open_nested(visual: &mut VisualTestContext, root: &Entity<Prototype>) {
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.new_tab(window, cx);
            workspace.select_tab(2, window, cx);
            workspace.split_tab(3, 2, Edge::Bottom, window, cx);
        });
    });
}

fn open_standalone_third(visual: &mut VisualTestContext, root: &Entity<Prototype>) {
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, shell(), &workspace.ui.locale, cx)
            });
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, Edge::Right, window, cx);
            workspace.new_tab(window, cx);
            workspace.select_tab(2, window, cx);
        });
    });
}

fn press(visual: &mut VisualTestContext, key: &str) {
    visual.update(|window, cx| window.render_frame(cx));
    visual.simulate_keystrokes(key);
    visual.update(|window, cx| window.render_frame(cx));
}

fn focus(visual: &mut VisualTestContext, root: &Entity<Prototype>, id: usize) {
    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.focus_pane(id, window, cx)));
}

fn model(workspace: &Prototype, id: usize) -> Arc<Terminal> {
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
}

fn session_is_live(workspace: &Prototype, id: usize) -> bool {
    workspace
        .tabs
        .get(id)
        .unwrap()
        .binding
        .as_ref()
        .unwrap()
        .session()
        .unwrap()
        .is_live()
}

fn leaf(id: usize) -> PaneNode {
    PaneNode::Pane {
        tab_id: id.to_string(),
    }
}

fn row(children: Vec<PaneNode>, sizes: &[f64]) -> PaneNode {
    PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        children,
        sizes: sizes.to_vec(),
    }
}

fn column(children: Vec<PaneNode>, sizes: &[f64]) -> PaneNode {
    PaneNode::Split {
        orientation: SplitOrientation::Vertical,
        children,
        sizes: sizes.to_vec(),
    }
}

fn nested_shape() -> PaneNode {
    row(
        vec![leaf(1), column(vec![leaf(2), leaf(3)], &[0.5, 0.5])],
        &[0.5, 0.5],
    )
}

fn assert_nested_shape(workspace: &Prototype) {
    let expected = nested_shape();
    assert_eq!(workspace.splits.root(1), Some(&expected));
    assert_eq!(workspace.splits.root(2), Some(&expected));
    assert_eq!(workspace.splits.root(3), Some(&expected));
    assert!(workspace.splits.is_split(1));
    assert!(workspace.splits.is_split(2));
    assert!(workspace.splits.is_split(3));
    assert_eq!(workspace.splits.zoomed(3), None);
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "sizes {actual:?} differ from {expected:?}"
        );
    }
}

#[gpui_kit::test]
fn dragging_the_third_tab_onto_the_right_pane_nests_it_below(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_standalone_third(visual, &root);
    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.select_tab(2, window, cx)));
    visual.update(|window, cx| window.render_frame(cx));
    visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2], 2, window, cx);
        assert_eq!(
            workspace.splits.root(2),
            Some(&row(vec![leaf(1), leaf(2)], &[0.5, 0.5]))
        );
        assert_eq!(workspace.splits.root(3), Some(&leaf(3)));
        assert!(!workspace.splits.is_split(3));
    });

    let source = visual.update(|window, cx| {
        window.render_frame(cx);
        root.read(cx)
            .overflow
            .handle
            .bounds_for_item(2)
            .expect("third tab is laid out")
            .center()
    });
    visual.simulate_mouse_down(source, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        point(source.x + px(24.), source.y),
        MouseButton::Left,
        Default::default(),
    );
    visual.update(|window, cx| window.render_frame(cx));

    let drop = visual
        .debug_bounds("workspace-pane-drop-2")
        .expect("the right-hand pane owns a drop surface");
    let pane = visual
        .debug_bounds("workspace-pane-2")
        .expect("the right-hand pane is laid out");
    assert!(
        pane.contains(&drop.center()),
        "drop surface follows the leaf"
    );
    let release = point(drop.center().x, drop.top() + drop.size.height * 0.8);
    visual.simulate_mouse_move(release, MouseButton::Left, Default::default());
    visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2], 2, window, cx);
        assert_eq!(
            workspace.splits.root(2),
            Some(&row(vec![leaf(1), leaf(2)], &[0.5, 0.5]))
        );
        assert_eq!(workspace.splits.root(3), Some(&leaf(3)));
    });

    visual.simulate_mouse_up(release, MouseButton::Left, Default::default());
    visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        assert_nested_shape(workspace);
        let models = [
            model(workspace, 1),
            model(workspace, 2),
            model(workspace, 3),
        ];
        for (index, first) in models.iter().enumerate() {
            for second in &models[index + 1..] {
                assert!(!Arc::ptr_eq(first, second));
            }
        }
    });
    assert!(visual.debug_bounds("workspace-pane-drop-2").is_none());
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn releasing_a_tab_drag_before_the_next_frame_still_splits_the_pane(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_standalone_third(visual, &root);
    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.select_tab(2, window, cx)));
    let (source, release) = visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let source = workspace
            .overflow
            .handle
            .bounds_for_item(2)
            .expect("third tab is laid out")
            .center();
        let pane = workspace
            .tabs
            .get(2)
            .and_then(|tab| tab.view.read(cx).content_bounds())
            .expect("the right-hand pane is laid out");
        (
            source,
            point(pane.center().x, pane.top() + pane.size.height * 0.8),
        )
    });

    visual.simulate_mouse_down(source, MouseButton::Left, Default::default());
    // Dispatch both events in one update. The test harness draws dirty windows when an
    // update finishes, so separate simulated events always insert a frame; macOS can
    // deliver mouse-up before the frame that installs the active-drag drop surfaces.
    visual.update(|window, cx| {
        window.dispatch_event(
            MouseMoveEvent {
                position: release,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position: release,
                modifiers: Default::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
    });
    visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        assert_nested_shape(workspace);
    });
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn three_nested_ptys_route_input_only_to_the_focused_pane(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_nested(visual, &root);
    let models = visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        assert_nested_shape(workspace);
        assert!(
            [1, 2, 3]
                .into_iter()
                .all(|id| session_is_live(workspace, id))
        );
        [
            model(workspace, 1),
            model(workspace, 2),
            model(workspace, 3),
        ]
    });

    send_marker(visual, &models[2], "thirdpane");
    assert!(!output(&models[0]).contains("thirdpane"));
    assert!(!output(&models[1]).contains("thirdpane"));

    focus(visual, &root, 2);
    visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], 2, window, cx));
    send_marker(visual, &models[1], "secondpane");
    assert!(!output(&models[0]).contains("secondpane"));
    assert!(!output(&models[2]).contains("secondpane"));

    focus(visual, &root, 1);
    visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], 1, window, cx));
    send_marker(visual, &models[0], "firstpane");
    assert!(!output(&models[1]).contains("firstpane"));
    assert!(!output(&models[2]).contains("firstpane"));

    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn closing_a_nested_pane_restores_the_root_ratio(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_nested(visual, &root);
    visual.update(|_, cx| {
        root.update(cx, |workspace, _| {
            assert!(workspace.splits.resize(1, &[], &[0.3, 0.7]));
            assert!(workspace.splits.resize(2, &[1], &[0.4, 0.6]));
        })
    });
    let (first, second) = visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        (model(workspace, 1), model(workspace, 2))
    });
    visual.update(|window, cx| window.render_frame(cx));
    let before_first = visual
        .debug_bounds("workspace-pane-1")
        .expect("left pane is laid out")
        .size
        .width;
    let before_right = visual
        .debug_bounds("workspace-pane-2")
        .expect("right pane is laid out")
        .size
        .width;
    let before_ratio = f32::from(before_first) / f32::from(before_first + before_right);
    assert!((before_ratio - 0.3).abs() < 0.02);

    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.close_tab(3, window, cx);
            check_visible(workspace, &[1, 2], 2, window, cx);
            let sizes = workspace
                .splits
                .split_sizes(2, &[])
                .expect("the surviving root split");
            assert_close(&sizes, &[0.3, 0.7]);
            assert_eq!(
                workspace.splits.root(2),
                Some(&row(vec![leaf(1), leaf(2)], &[0.3, 0.7]))
            );
            assert!(workspace.factory.sessions.tab("gpui-3").is_none());
            assert!(Arc::ptr_eq(&first, &model(workspace, 1)));
            assert!(Arc::ptr_eq(&second, &model(workspace, 2)));
        });
    });
    visual.update(|window, cx| window.render_frame(cx));
    assert!(visual.debug_bounds("workspace-sash").is_some());
    assert!(visual.debug_bounds("workspace-sash-1").is_none());
    let after_first = visual
        .debug_bounds("workspace-pane-1")
        .expect("left pane survives")
        .size
        .width;
    let after_right = visual
        .debug_bounds("workspace-pane-2")
        .expect("right pane survives")
        .size
        .width;
    let after_ratio = f32::from(after_first) / f32::from(after_first + after_right);
    assert!((after_ratio - 0.3).abs() < 0.02);

    send_marker(visual, &second, "survivorpane");
    assert!(!output(&first).contains("survivorpane"));
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn nested_sashes_resize_only_their_own_split(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_nested(visual, &root);
    let models = visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        [
            model(workspace, 1),
            model(workspace, 2),
            model(workspace, 3),
        ]
    });
    let snapshot = |visual: &mut VisualTestContext| {
        visual.update(|window, cx| {
            window.render_frame(cx);
            let workspace = root.read(cx);
            check_visible(workspace, &[1, 2, 3], 3, window, cx);
            (
                workspace.splits.split_sizes(3, &[]).unwrap(),
                workspace.splits.split_sizes(3, &[1]).unwrap(),
                models.each_ref().map(|model| model.grid_size()),
            )
        })
    };

    let (root_before, nested_before, grids_before) = snapshot(visual);
    let sash = visual
        .debug_bounds("workspace-sash")
        .expect("the root sash is laid out");
    let start = sash.center();
    let end = point(start.x * 1.2, start.y);
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
    let (root_after, nested_after, grids_after) = snapshot(visual);
    assert!(root_after[0] > root_before[0] && root_after[0] < 1.);
    assert_close(&nested_after, &nested_before);
    assert!(grids_after[0].columns > grids_before[0].columns);
    assert!(grids_after[1].columns < grids_before[1].columns);
    assert!(grids_after[2].columns < grids_before[2].columns);
    for (after, before) in grids_after.iter().zip(&grids_before) {
        assert_eq!(after.screen_lines, before.screen_lines);
    }
    visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(start, None, Default::default());
    let (root_released, nested_released, grids_released) = snapshot(visual);
    assert_close(&root_released, &root_after);
    assert_close(&nested_released, &nested_after);
    assert_eq!(grids_released, grids_after);

    let nested_sash = visual
        .debug_bounds("workspace-sash-1")
        .expect("the nested sash is laid out");
    let nested_start = nested_sash.center();
    let nested_end = point(nested_start.x, nested_start.y * 1.2);
    visual.simulate_mouse_down(nested_start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(nested_end, MouseButton::Left, Default::default());
    let (root_nested, nested_nested, grids_nested) = snapshot(visual);
    assert_close(&root_nested, &root_after);
    assert!(nested_nested[0] > nested_released[0] && nested_nested[0] < 1.);
    assert!(grids_nested[1].screen_lines > grids_released[1].screen_lines);
    assert!(grids_nested[2].screen_lines < grids_released[2].screen_lines);
    assert_eq!(
        (
            grids_nested[0].screen_lines,
            grids_nested[1].columns,
            grids_nested[2].columns,
        ),
        (
            grids_released[0].screen_lines,
            grids_released[1].columns,
            grids_released[2].columns,
        )
    );
    visual.simulate_mouse_up(nested_end, MouseButton::Left, Default::default());
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn nested_zoom_and_navigation_keep_every_session_isolated(cx: &mut TestAppContext) {
    let NestedWorkspace {
        _runtime,
        root,
        visual,
    } = nested_workspace(cx);
    open_nested(visual, &root);
    let models = visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 3, window, cx);
        [
            model(workspace, 1),
            model(workspace, 2),
            model(workspace, 3),
        ]
    });
    visual.update(|_, cx| {
        root.update(cx, |workspace, _| {
            assert!(workspace.splits.resize(1, &[], &[0.3, 0.7]));
        })
    });

    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    press(visual, &format!("{modifier}-alt-enter"));
    visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[3], 3, window, cx);
        assert_eq!(workspace.splits.zoomed(3), Some(3));
        assert_close(&workspace.splits.split_sizes(3, &[]).unwrap(), &[0.3, 0.7]);
        assert_eq!(
            workspace.splits.root(3),
            Some(&row(
                vec![leaf(1), column(vec![leaf(2), leaf(3)], &[0.5, 0.5])],
                &[0.3, 0.7]
            ))
        );
    });
    visual.update(|window, cx| window.render_frame(cx));
    assert!(visual.debug_bounds("workspace-sash").is_none());
    assert!(visual.debug_bounds("workspace-sash-1").is_none());
    send_marker(visual, &models[2], "zoomedpane");
    assert!(!output(&models[0]).contains("zoomedpane"));
    assert!(!output(&models[1]).contains("zoomedpane"));

    visual
        .update(|window, cx| root.update(cx, |workspace, cx| workspace.select_tab(2, window, cx)));
    visual.update(|window, cx| {
        let workspace = root.read(cx);
        check_visible(workspace, &[1, 2, 3], 2, window, cx);
        assert_eq!(workspace.splits.zoomed(2), None);
        assert_close(&workspace.splits.split_sizes(2, &[]).unwrap(), &[0.3, 0.7]);
    });
    visual.update(|window, cx| window.render_frame(cx));
    assert!(visual.debug_bounds("workspace-sash").is_some());
    assert!(visual.debug_bounds("workspace-sash-1").is_some());

    let direction = |key: &str| format!("{modifier}-alt-{key}");
    focus(visual, &root, 3);
    press(visual, &direction("up"));
    visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], 2, window, cx));
    press(visual, &direction("down"));
    visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], 3, window, cx));
    press(visual, &direction("left"));
    visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], 1, window, cx));

    let cycle = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl-shift"
    };
    let next = format!("{cycle}-]");
    let prev = format!("{cycle}-[");
    for (key, id) in [(&next, 2), (&next, 3), (&next, 1), (&prev, 3)] {
        press(visual, key);
        visual.update(|window, cx| check_visible(root.read(cx), &[1, 2, 3], id, window, cx));
    }

    send_marker(visual, &models[2], "nestedinput");
    assert!(!output(&models[0]).contains("nestedinput"));
    assert!(!output(&models[1]).contains("nestedinput"));
    assert!(visual.update(|_, cx| {
        [1, 2, 3]
            .into_iter()
            .all(|id| session_is_live(root.read(cx), id))
    }));
    visual.update(|window, _| window.remove_window());
}
