use super::*;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{Entity, MouseButton, TestAppContext, point, size};
use std::sync::Arc;
use tterm_terminal::{
    grid::{CellSize, GridSize},
    terminal::{Config, Terminal},
};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

#[test]
fn edges_use_the_panes_normalized_coordinates_and_vertical_ties() {
    let bounds = Bounds::new(point(px(10.), px(30.)), size(px(400.), px(100.)));
    for (x, y, expect) in [
        (11., 80., Some(Edge::Left)),
        (409., 80., Some(Edge::Right)),
        (215., 32., Some(Edge::Top)),
        (215., 128., Some(Edge::Bottom)),
        (210., 80., Some(Edge::Bottom)),
        (10., 30., Some(Edge::Top)),
        (10., 29., None),
    ] {
        assert_eq!(edge_at(bounds, point(px(x), px(y))), expect);
    }
}

struct SourcePreview;
impl Render for SourcePreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct DropProbe {
    view: Entity<TerminalView>,
    dropped: Rc<Cell<Option<(usize, Edge)>>>,
}
impl Render for DropProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity_id();
        let dropped = self.dropped.clone();
        div()
            .flex()
            .flex_col()
            .w(px(400.))
            .h(px(240.))
            .child(
                div()
                    .id("drag-source")
                    .h(px(40.))
                    .on_drag(TabDrag { owner, id: 6 }, |_, _, _, cx| {
                        cx.new(|_| SourcePreview)
                    }),
            )
            .child(
                div()
                    .debug_selector(|| "drop-probe-pane".into())
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.view.clone())
                    .when(cx.has_active_drag(), |target| {
                        target.child(leaf_drop_surface(
                            6,
                            gpui_kit::white(),
                            gpui_kit::white(),
                            true,
                            move |drag, edge, _, _| dropped.set(Some((drag.id, edge))),
                        ))
                    }),
            )
    }
}

#[gpui_kit::test]
fn native_tab_drop_reaches_surface_above_terminal_only_on_release(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dropped = Rc::new(Cell::new(None));
    let view = cx.update(|cx| {
        cx.new(|cx| {
            TerminalView::new(
                cx,
                Arc::new(Terminal::new(
                    Config::default(),
                    GridSize::new(50, 10),
                    CellSize::new(8, 16),
                )),
                TerminalViewConfig::default(),
            )
        })
    });
    let (probe, visual) = cx.add_window_view(|_, _| DropProbe {
        view,
        dropped: dropped.clone(),
    });
    visual.update(|window, cx| window.render_frame(cx));
    let source = point(px(50.), px(20.));
    visual.simulate_mouse_down(source, MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        point(px(65.), px(20.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.update(|window, cx| {
        probe.update(cx, |_, cx| cx.notify());
        window.render_frame(cx);
    });
    let right = point(px(390.), px(140.));
    visual.simulate_mouse_move(right, MouseButton::Left, Default::default());
    assert_eq!(dropped.get(), None);
    let pane = visual
        .debug_bounds("drop-probe-pane")
        .expect("probe pane is laid out");
    let surface = visual
        .debug_bounds("workspace-pane-drop-6")
        .expect("leaf surface is laid out");
    assert!(pane.contains(&surface.center()));
    visual.simulate_mouse_up(right, MouseButton::Left, Default::default());
    assert_eq!(dropped.get(), Some((6, Edge::Right)));
}
