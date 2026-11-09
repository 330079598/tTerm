//! Measure before prepaint, then translate into window coordinates.
use gpui_kit::{
    AnyElement, App, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    Pixels, Point, Size, Style, Window, point, px,
};

use std::{cell::Cell, rc::Rc};

/// Matches React's measure-then-reposition rule, including its strict `>` check.
pub fn fit_in_viewport(
    origin: Point<Pixels>,
    menu: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    let x = if origin.x + menu.width > viewport.width {
        viewport.width - menu.width - px(10.0)
    } else {
        origin.x
    };
    let y = if origin.y + menu.height > viewport.height {
        viewport.height - menu.height - px(10.0)
    } else {
        origin.y
    };
    point(x.max(px(10.0)), y.max(px(10.0)))
}

pub(super) struct PlacedMenu {
    pub child: AnyElement,
    pub origin: Point<Pixels>,
    pub position: Rc<Cell<Option<Point<Pixels>>>>,
}
impl IntoElement for PlacedMenu {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PlacedMenu {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<gpui_kit::ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        let style = Style {
            position: gpui_kit::Position::Absolute,
            ..Style::default()
        };
        (window.request_layout(style, [child], cx), child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*child).size;
        let position = self.position.get().unwrap_or_else(|| {
            let position = fit_in_viewport(self.origin, size, window.viewport_size());
            self.position.set(Some(position));
            position
        });
        window.with_element_offset(position - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
