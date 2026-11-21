//! The preview scrolls the tab viewport only for the lifetime of a native drag.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, EntityId, Pixels, ScrollHandle, Stateful, Window, div, px};
use std::{cell::Cell, rc::Rc};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::Prototype;
use super::motion::{Motion, Placement, destination};

#[derive(Default)]
pub(super) struct DragLayout {
    pub(super) gesture: Rc<Cell<Option<(usize, Pixels)>>>,
    press_x: Rc<Cell<Pixels>>,
    pub(super) motion: Motion,
}

pub(super) struct TabDrag {
    pub(super) owner: EntityId,
    pub(super) id: usize,
}

struct Preview {
    title: String,
    theme: Theme,
}

fn edge_step(position: Pixels, width: Pixels) -> Pixels {
    let edge = px(40.).min(width * 0.5);
    if edge <= px(0.) || position < px(0.) || position > width {
        return px(0.);
    }
    let near_left = (edge - position).max(px(0.));
    let near_right = (edge - (width - position)).max(px(0.));
    px(14. * f32::from(near_left - near_right) / f32::from(edge))
}

impl Prototype {
    pub(super) fn placement(&self, window: &Window) -> Option<Placement> {
        let (id, grab) = self.drag_layout.gesture.get()?;
        let source = self.tabs.ordered().position(|(tab, _)| tab == id)?;
        let scroll = &self.overflow.handle;
        if !scroll.bounds().contains(&window.mouse_position()) {
            return None;
        }
        let slots = (0..self.tabs.ordered().len())
            .map(|index| scroll.bounds_for_item(index))
            .collect::<Option<Vec<_>>>()?;
        let width = slots[source].size.width;
        let gap = slots
            .get(1)
            .map_or(px(0.), |second| second.left() - slots[0].right());
        let first = slots.first()?.left();
        let left = (window.mouse_position().x - scroll.offset().x - grab)
            .clamp(first, (slots.last()?.right() - width).max(first));
        let centres = slots
            .iter()
            .map(|slot| f32::from(slot.center().x))
            .collect::<Vec<_>>();
        Some(Placement {
            source,
            target: destination(&centres, source, left.into(), (left + width).into()),
            span: (width + gap).into(),
        })
    }
}

pub(super) fn advance_scroll(scroll: &ScrollHandle, window: &Window) {
    window.request_animation_frame();
    let bounds = scroll.bounds();
    let pointer = window.mouse_position();
    if bounds.contains(&pointer) {
        let mut offset = scroll.offset();
        let step = edge_step(pointer.x - bounds.left(), bounds.size.width);
        let next = (offset.x + step).clamp(-scroll.max_offset().x, px(0.));
        if offset.x != next {
            offset.x = next;
            scroll.set_offset(offset);
        }
    }
}

impl Render for Preview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .h(px(30.))
            .max_w(px(220.))
            .px_2()
            .rounded_md()
            .bg(hsla(self.theme.colors.tab_active, 0.92))
            .text_color(hsla(self.theme.colors.foreground, 1.))
            .text_sm()
            .truncate()
            .child(self.title.clone())
    }
}

pub(super) fn reorderable(
    item: Stateful<Div>,
    id: usize,
    title: String,
    theme: Theme,
    layout: &DragLayout,
    cx: &Context<Prototype>,
) -> Stateful<Div> {
    let owner = cx.entity_id();
    let gesture = layout.gesture.clone();
    let press = layout.press_x.clone();
    let start = press.clone();
    item.on_mouse_down(gpui_kit::MouseButton::Left, move |event, window, _| {
        press.set(event.position.x);
        window.prevent_default()
    })
    .on_drag(TabDrag { owner, id }, move |_, grab, window, cx| {
        gesture.set(Some((id, grab.x + start.get() - window.mouse_position().x)));
        cx.notify(owner);
        cx.new(|_| Preview {
            title: title.clone(),
            theme,
        })
    })
}

pub(super) fn drop_target(strip: Div, cx: &Context<Prototype>) -> Div {
    strip.on_drop(cx.listener(|this, drag: &TabDrag, window, cx| {
        if drag.owner == cx.entity_id()
            && let Some(placement) = this.placement(window)
        {
            let target = this.tabs.ordered().nth(placement.target).map(|(id, _)| id);
            if let Some(target) = target {
                this.tabs.move_to(drag.id, target);
            }
        }
        this.drag_layout.gesture.set(None);
        this.drag_layout.motion = Motion::default();
        cx.notify();
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_steps_are_bounded_and_only_apply_inside_the_edges() {
        let step = |position| edge_step(px(position), px(200.));
        assert_eq!(step(0.), px(14.));
        assert_eq!(step(20.), px(7.));
        assert_eq!(step(190.), px(-10.5));
        for stationary in [-1., 40., 110., 160., 201.] {
            assert_eq!(step(stationary), px(0.));
        }
    }
}

#[cfg(test)]
mod window_tests;
