//! Horizontal navigation uses the laid-out tab widths, not estimated title lengths.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, Entity, MouseButton, Pixels, ScrollHandle, div, px};
use tterm_core::i18n::keys;

use super::{Locale, Prototype};

const ARROW_WIDTH: f32 = 26.;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Edges {
    overflow: bool,
    left: bool,
    right: bool,
}

impl Edges {
    fn measured(max: Pixels, offset: Pixels, showing: bool) -> Self {
        let reserved = if showing { 3. * ARROW_WIDTH } else { 0. };
        Self {
            overflow: max > px(reserved + 0.5),
            left: offset < px(-0.5),
            right: max + offset > px(0.5),
        }
    }
}

#[derive(Default)]
pub(super) struct TabViewport {
    handle: ScrollHandle,
    edges: Rc<Cell<Edges>>,
    last: Rc<Cell<Option<(usize, usize, Pixels)>>>,
}

impl TabViewport {
    pub(super) fn wrap(
        &self,
        strip: Div,
        active: usize,
        index: usize,
        locale: &Entity<Locale>,
        cx: &mut Context<Prototype>,
    ) -> Div {
        let showing = self.edges.get();
        let handle = self.handle.clone();
        let edges = self.edges.clone();
        let last = self.last.clone();
        let list = strip
            .min_w_0()
            .on_children_prepainted(move |_, window, _| {
                let width = handle.bounds().size.width;
                let geometry = (active, index, width);
                let changed = last.replace(Some(geometry)) != Some(geometry);
                if changed {
                    handle.scroll_to_item(index);
                }
                let next =
                    Edges::measured(handle.max_offset().x, handle.offset().x, showing.overflow);
                if edges.replace(next) != next || changed {
                    window.request_animation_frame();
                }
            })
            .id("workspace-tab-viewport")
            .overflow_x_scroll()
            .track_scroll(&self.handle);
        let mut bar = div().flex().min_w_0().items_center();
        if showing.overflow {
            bar = bar.child(self.arrow(false, showing.left, locale, cx));
        }
        bar = bar.child(list);
        if showing.overflow {
            let label = locale
                .read(cx)
                .text(keys::TAB_SHOW_ALL)
                .unwrap_or_else(|_| keys::TAB_SHOW_ALL.as_str().into());
            bar = bar
                .child(self.arrow(true, showing.right, locale, cx))
                .child(
                    div()
                        .id("tabs-show-all")
                        .aria_label(label)
                        .w(px(ARROW_WIDTH))
                        .h(px(28.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child("⌄")
                        .on_click(cx.listener(|this, _, window, cx| this.open_picker(window, cx))),
                );
        }
        bar
    }

    fn arrow(
        &self,
        right: bool,
        enabled: bool,
        locale: &Entity<Locale>,
        cx: &mut Context<Prototype>,
    ) -> impl IntoElement + use<> {
        let (id, key, glyph) = if right {
            ("tabs-scroll-right", keys::TAB_SCROLL_RIGHT, "›")
        } else {
            ("tabs-scroll-left", keys::TAB_SCROLL_LEFT, "‹")
        };
        let handle = self.handle.clone();
        let label = locale
            .read(cx)
            .text(key)
            .unwrap_or_else(|_| key.as_str().to_owned());
        div()
            .id(id)
            .aria_label(label)
            .w(px(ARROW_WIDTH))
            .h(px(28.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .opacity(if enabled { 1. } else { 0.35 })
            .child(glyph)
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let distance = px((f32::from(handle.bounds().size.width) * 0.72)
                            .floor()
                            .max(140.));
                        let mut offset = handle.offset();
                        offset.x = (offset.x + if right { -distance } else { distance })
                            .clamp(-handle.max_offset().x, px(0.));
                        handle.set_offset(offset);
                        cx.notify();
                    }))
            })
    }

    pub(super) fn trigger_right(&self) -> Pixels {
        self.handle.bounds().right() + px(2. * ARROW_WIDTH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_do_not_create_their_own_overflow() {
        assert!(!Edges::measured(px(78.), px(-20.), true).overflow);
        assert!(Edges::measured(px(80.), px(-20.), true).overflow);
        assert!(!Edges::measured(px(0.), px(0.), false).overflow);
    }

    #[test]
    fn arrows_follow_both_scroll_limits() {
        let edges = |position| Edges::measured(px(320.), px(position), true);
        assert!(!edges(0.).left && edges(0.).right);
        assert!(edges(-160.).left && edges(-160.).right);
        assert!(edges(-320.).left && !edges(-320.).right);
    }
}
