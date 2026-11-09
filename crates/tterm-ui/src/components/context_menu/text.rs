//! Inline CSS line boxes without GPUI's whole-pixel line-height rounding.
use super::{PartObserver, PartStyle};
use gpui_kit::{
    App, AvailableSpace, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, SharedString, Window, WrappedLine, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(super) struct MenuText {
    text: SharedString,
    lines: Rc<RefCell<Vec<WrappedLine>>>,
    observer: Option<(PartObserver, String, PartStyle)>,
}
impl MenuText {
    pub fn new(text: SharedString, observer: Option<(PartObserver, String, PartStyle)>) -> Self {
        Self {
            text,
            observer,
            lines: Rc::new(RefCell::new(Vec::new())),
        }
    }
}
impl IntoElement for MenuText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for MenuText {
    type RequestLayoutState = ();
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
        _: &mut App,
    ) -> (LayoutId, ()) {
        let text = self.text.clone();
        let lines = self.lines.clone();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let run = style.to_run(text.len());
        // WebKit truncates this inline line box to whole CSS pixels: the measured
        // 16.8px declaration yields 16px per line (32px for two wrapped lines).
        // Use that derived height for every line, including the first and its text region.
        let line_height = px(style
            .line_height
            .to_pixels(font_size.into(), window.rem_size())
            .as_f32()
            .floor());
        let layout = window.request_measured_layout(
            Default::default(),
            move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                });
                let shaped = window
                    .text_system()
                    .shape_text(
                        text.clone(),
                        font_size,
                        std::slice::from_ref(&run),
                        width,
                        None,
                    )
                    .unwrap_or_default();
                let mut measured = size(px(0.0), px(0.0));
                let mut count = 0;
                for line in &shaped {
                    let line_size = line.size(line_height);
                    count += line.wrap_boundaries.len() + 1;
                    measured.width = measured.width.max(line_size.width.ceil());
                }
                if count > 0 {
                    measured.height = line_height * count as f32;
                }
                *lines.borrow_mut() = shaped.into_iter().collect();
                measured
            },
        );
        (layout, ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let Some((observer, slot, style)) = &self.observer else {
            return;
        };
        let height = px(style.line_height.floor());
        let mut y = bounds.origin.y;
        let mut index = 0;
        for line in self.lines.borrow().iter() {
            let layout = &line.unwrapped_layout;
            let mut start = px(0.0);
            let ends = line
                .wrap_boundaries
                .iter()
                .map(|b| layout.runs[b.run_ix].glyphs[b.glyph_ix].position.x)
                .chain(std::iter::once(layout.width));
            for end in ends {
                observer(
                    &format!("text-{slot}-{index}"),
                    Bounds::new(
                        gpui_kit::point(bounds.origin.x, y),
                        size(end - start, height),
                    ),
                    *style,
                );
                index += 1;
                y += height;
                start = end;
            }
        }
        let _ = window;
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let height = px(style
            .line_height
            .to_pixels(font_size.into(), window.rem_size())
            .as_f32()
            .floor());
        let mut origin = bounds.origin;
        for line in self.lines.borrow().iter() {
            let _ = line.paint(origin, height, style.text_align, Some(bounds), window, cx);
            origin.y += line.size(height).height;
        }
    }
}
