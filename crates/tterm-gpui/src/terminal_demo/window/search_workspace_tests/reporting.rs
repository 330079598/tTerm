//! Search-grip event ownership across production split panes with SGR any-motion enabled.

use gpui_kit::{MouseButton, Pixels, Point, ScrollDelta, ScrollEvent, point, size};

use super::*;

type Bytes = Rc<RefCell<Vec<u8>>>;

fn synthetic_view(cx: &mut Context<Prototype>, bytes: &Bytes) -> Entity<TerminalView> {
    let model = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
    model.write(b"\x1b[?1003h\x1b[?1006h");
    let sink = bytes.clone();
    cx.new(|cx| {
        TerminalView::new(
            cx,
            model,
            TerminalViewConfig {
                input: Some(Rc::new(move |data| {
                    sink.borrow_mut().extend_from_slice(data)
                })),
                ..Default::default()
            },
        )
    })
}

fn with_reporting_panes(
    cx: &mut TestAppContext,
    run: impl FnOnce(&Entity<Prototype>, &mut VisualTestContext, &[Bytes; 2]),
) {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let command = CommandSpec::Program {
        program: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("Cargo.toml")
            .join("no-program")
            .to_string_lossy()
            .into_owned(),
        args: vec![],
    };
    let (root, visual) = cx.add_window_view(|_, cx| {
        split_session_tests::make_workspace_with(
            &runtime,
            events,
            command.clone(),
            AppConfig::default(),
            cx,
        )
    });
    let sinks: [Bytes; 2] = Default::default();
    visual.simulate_resize(size(px(1000.), px(600.)));
    visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace.tabs.push(|id| {
                workspace
                    .factory
                    .open(id, command, &workspace.ui.locale, cx)
            });
            for (id, sink) in [1, 2].into_iter().zip(&sinks) {
                workspace.tabs.get_mut(id).unwrap().view = synthetic_view(cx, sink);
            }
            workspace.select_tab(1, window, cx);
            workspace.split_tab(2, 1, split_state::Edge::Right, window, cx);
            workspace.focus_pane(1, window, cx);
            workspace.open_search(window, cx);
        });
        window.render_frame(cx);
    });
    for (selector, sink) in ["workspace-pane-1", "workspace-pane-2"]
        .into_iter()
        .zip(&sinks)
    {
        let position = visual.debug_bounds(selector).unwrap().center();
        hover_report(visual, position, sink);
    }
    run(&root, visual, &sinks);
    visual.update(|window, _| window.remove_window());
}

fn hover_report(visual: &mut VisualTestContext, position: Point<Pixels>, sink: &Bytes) {
    sink.borrow_mut().clear();
    visual.simulate_mouse_move(position, None, Default::default());
    let report = sink.take();
    assert!(
        report.starts_with(b"\x1b[<35;"),
        "the terminal must report an unpressed SGR motion at this point: {report:?}"
    );
}

fn begin_grip(visual: &mut VisualTestContext) {
    let handle = visual
        .debug_bounds(search::HANDLE_SELECTOR)
        .unwrap()
        .center();
    visual.simulate_mouse_down(handle, MouseButton::Left, Default::default());
}

fn drag_to(visual: &mut VisualTestContext, end: Point<Pixels>) -> gpui_kit::Bounds<Pixels> {
    begin_grip(visual);
    visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
    visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.render_frame(cx));
    visual.debug_bounds(search::BAR_SELECTOR).unwrap()
}

#[gpui_kit::test]
fn six_hundred_pixel_viewport_starts_at_ten_then_drags_to_eight(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |root, visual, _| {
        visual.simulate_resize(size(px(600.), px(500.)));
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                assert!(workspace.splits.toggle_zoom(1));
                cx.notify();
            });
            window.render_frame(cx);
        });
        visual.run_until_parked();
        let pane = visual.debug_bounds("workspace-pane-1").unwrap();
        let initial = visual.debug_bounds(search::BAR_SELECTOR).unwrap();
        assert_eq!(pane.size.width, px(600.));
        assert_eq!(initial.left() - pane.left(), px(10.));
        assert_eq!(pane.right() - initial.right(), px(10.));
        assert_eq!(initial.top() - pane.top(), px(10.));
        let left = drag_to(visual, pane.origin - point(px(200.), px(200.)));
        assert_eq!(left.size, initial.size);
        assert_eq!(left.origin - pane.origin, point(px(8.), px(8.)));
        let right = drag_to(visual, pane.bottom_right() + point(px(200.), px(200.)));
        assert_eq!(right.size, initial.size);
        assert_eq!(
            pane.bottom_right() - right.bottom_right(),
            point(px(8.), px(8.))
        );
    });
}

#[gpui_kit::test]
fn narrow_wrapped_split_drags_its_actual_box_to_eight(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |_, visual, _| {
        visual.simulate_resize(size(px(600.), px(500.)));
        visual.update(|window, cx| window.render_frame(cx));
        visual.run_until_parked();
        let pane = visual.debug_bounds("workspace-pane-1").unwrap();
        let grip = visual.debug_bounds(search::HANDLE_SELECTOR).unwrap();
        let close = visual.debug_bounds(search::CLOSE_SELECTOR).unwrap();
        assert!(
            close.top() >= grip.bottom(),
            "the narrow split must wrap its buttons"
        );
        let actual = drag_to(visual, pane.bottom_right() + point(px(200.), px(200.)));
        assert_eq!(
            pane.bottom_right() - actual.bottom_right(),
            point(px(8.), px(8.))
        );
    });
}

#[gpui_kit::test]
fn extra_button_during_grip_neither_focuses_nor_reports_to_other_pane(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |root, visual, sinks| {
        for (id, neighbour) in [(1, "workspace-pane-2"), (2, "workspace-pane-1")] {
            visual.update(|window, cx| {
                root.update(cx, |workspace, cx| {
                    workspace.focus_pane(id, window, cx);
                    workspace.open_search(window, cx);
                });
                window.render_frame(cx);
            });
            let end = visual.debug_bounds(neighbour).unwrap().center();
            begin_grip(visual);
            visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
            visual.simulate_mouse_down(end, MouseButton::Right, Default::default());
            visual.simulate_mouse_up(end, MouseButton::Right, Default::default());
            visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
            assert!(
                sinks.iter().all(|sink| sink.borrow().is_empty()),
                "extra button reached a PTY: {sinks:?}"
            );
            visual.update(|_, cx| assert_eq!(root.read(cx).tabs.active_id(), id));
        }
    });
}

#[gpui_kit::test]
fn deactivating_window_ends_grip_even_if_release_was_outside(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |root, visual, sinks| {
        visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        begin_grip(visual);
        let is_dragging = |visual: &mut VisualTestContext| {
            visual.update(|_, cx| active_search(root, cx).unwrap().read(cx).drag_active())
        };
        assert!(is_dragging(visual));
        visual.deactivate_window();
        assert!(
            !is_dragging(visual),
            "a window blur must end the grip without a local release"
        );
        visual.update(|window, _| window.activate_window());
        visual.run_until_parked();
        let neighbour = visual.debug_bounds("workspace-pane-2").unwrap().center();
        visual.simulate_mouse_move(neighbour, MouseButton::Left, Default::default());
        visual.simulate_mouse_up(neighbour, MouseButton::Left, Default::default());
        assert!(sinks.iter().all(|sink| sink.borrow().is_empty()));
        hover_report(visual, neighbour + point(px(64.), px(32.)), &sinks[1]);
    });
}

#[gpui_kit::test]
fn split_resize_mid_grip_does_not_forward_held_motion_to_either_pty(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |root, visual, sinks| {
        let old = visual.debug_bounds("workspace-pane-2").unwrap();
        begin_grip(visual);
        visual.simulate_mouse_move(
            old.center() + point(px(24.), px(30.)),
            MouseButton::Left,
            Default::default(),
        );
        assert!(sinks.iter().all(|sink| sink.borrow().is_empty()));
        visual.update(|window, cx| {
            root.update(cx, |workspace, cx| {
                assert!(workspace.splits.resize(1, &[], &[0.65, 0.35]));
                cx.notify();
            });
            window.render_frame(cx);
        });
        let neighbour = visual.debug_bounds("workspace-pane-2").unwrap();
        assert!(neighbour.left() > old.left());
        let end = neighbour.center() + point(px(32.), px(32.));
        visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
        visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
        assert!(
            sinks.iter().all(|sink| sink.borrow().is_empty()),
            "resize leaked grip events to the terminal sinks: {sinks:?}"
        );
        hover_report(visual, neighbour.center(), &sinks[1]);
    });
}

#[gpui_kit::test]
fn grip_wheel_over_another_reporting_pane_never_reaches_its_pty(cx: &mut TestAppContext) {
    with_reporting_panes(cx, |_, visual, sinks| {
        let position =
            visual.debug_bounds("workspace-pane-2").unwrap().center() + point(px(32.), px(16.));
        let wheel = |visual: &mut VisualTestContext| {
            visual.simulate_event(ScrollEvent {
                position,
                delta: ScrollDelta::Lines(point(0., 1.)),
                ..Default::default()
            })
        };
        wheel(visual);
        assert!(sinks[1].take().starts_with(b"\x1b[<64;"));
        begin_grip(visual);
        visual.simulate_mouse_move(position, MouseButton::Left, Default::default());
        wheel(visual);
        visual.simulate_mouse_up(position, MouseButton::Left, Default::default());
        assert!(
            sinks.iter().all(|sink| sink.borrow().is_empty()),
            "grip leaked a wheel report to the terminal sinks: {sinks:?}"
        );
        wheel(visual);
        assert!(sinks[1].take().starts_with(b"\x1b[<64;"));
    });
}
