use super::*;
use gpui_kit::{MouseButton, point};

fn verify_sash(cx: &mut TestAppContext, edge: Edge) {
    let mut runtime = test_runtime();
    let events = std::mem::replace(
        &mut runtime.events,
        tokio::sync::mpsc::unbounded_channel().1,
    );
    cx.executor().allow_parking();
    cx.update(gpui_kit::init);
    let (root, visual) = cx.add_window_view(|_, cx| make_workspace(&runtime, events, cx));
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
    let snapshot = |visual: &mut VisualTestContext| {
        visual.update(|window, cx| {
            window.render_frame(cx);
            let workspace = root.read(cx);
            check_visible(workspace, &[1, 2], 2, window, cx);
            (
                workspace.splits.split_sizes(1, &[]).unwrap()[0],
                models.each_ref().map(|model| model.grid_size()),
            )
        })
    };
    let (initial, before) = snapshot(visual);
    let sash = visual
        .debug_bounds("workspace-sash")
        .expect("visible sash hit target");
    let start = sash.center();
    let horizontal = edge == Edge::Right;
    let end = if horizontal {
        point(start.x * 1.2, start.y)
    } else {
        point(start.x, start.y * 1.2)
    };
    visual.simulate_mouse_down(start, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(end, MouseButton::Left, Default::default());
    let (dragged, after) = snapshot(visual);
    assert!(
        dragged > initial && dragged < 1.,
        "ratio: {initial} -> {dragged}"
    );
    let dims = |grid: GridSize| {
        if horizontal {
            (grid.columns, grid.screen_lines)
        } else {
            (grid.screen_lines, grid.columns)
        }
    };
    let [(first, cross_first), (second, cross_second)] = before.map(dims);
    let [(larger, cross_larger), (smaller, cross_smaller)] = after.map(dims);
    assert!(larger > first && smaller < second);
    assert_eq!((cross_first, cross_second), (cross_larger, cross_smaller));
    let total = (larger + smaller) as f32;
    assert!((larger as f32 / total - dragged as f32).abs() < 2. / total);
    visual.simulate_mouse_up(end, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(start, None, Default::default());
    assert_eq!(snapshot(visual), (dragged, after));
    visual.update(|window, _| window.remove_window());
}

#[gpui_kit::test]
fn sash_mouse_drag_resizes_both_column_grids(cx: &mut TestAppContext) {
    verify_sash(cx, Edge::Right);
}

#[gpui_kit::test]
fn sash_mouse_drag_resizes_both_row_grids(cx: &mut TestAppContext) {
    verify_sash(cx, Edge::Bottom);
}
