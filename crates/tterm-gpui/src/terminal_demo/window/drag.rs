//! The preview scrolls the tab viewport only for the lifetime of a native drag.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, EntityId, Pixels, ScrollHandle, Stateful, Window, div, px};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::Prototype;

struct TabDrag {
    owner: EntityId,
    id: usize,
}

struct Preview {
    title: String,
    theme: Theme,
    scroll: ScrollHandle,
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

impl Render for Preview {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.scroll.bounds();
        let pointer = window.mouse_position();
        if bounds.contains(&pointer) {
            let mut offset = self.scroll.offset();
            let step = edge_step(pointer.x - bounds.left(), bounds.size.width);
            let next = (offset.x + step).clamp(-self.scroll.max_offset().x, px(0.));
            if offset.x != next {
                offset.x = next;
                self.scroll.set_offset(offset);
                window.request_animation_frame();
            }
        }
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
    scroll: ScrollHandle,
    cx: &Context<Prototype>,
) -> Stateful<Div> {
    let owner = cx.entity_id();
    item.on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
        window.prevent_default()
    })
    .on_drag(TabDrag { owner, id }, move |_, _, _, cx| {
        cx.new(|_| Preview {
            title: title.clone(),
            theme,
            scroll: scroll.clone(),
        })
    })
    .drag_over::<TabDrag>(move |style, drag, _, _| {
        if drag.owner == owner && drag.id != id {
            style
                .bg(hsla(theme.colors.tab_hover, 1.))
                .border_b_2()
                .border_color(hsla(theme.colors.tab_active_border, 1.))
        } else {
            style
        }
    })
    .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| {
        if owner == drag.owner && this.tabs.move_to(drag.id, id) {
            cx.notify();
        }
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
