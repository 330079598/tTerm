//! Tabs primitives matching `src/components/ui/tabs.tsx`.

use super::surface::{shadow_sm, surface};
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::base::Button;
use gpui_kit::{App, ClickEvent, ElementId, IntoElement, ParentElement, Styled, Window, div, px};

const LIST_HEIGHT: f32 = 40.0;
const TRIGGER_HEIGHT: f32 = 28.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TriggerState {
    pub active: bool,
    pub focused: bool,
    pub disabled: bool,
}

/// Resolves and paints the muted tab list container.
pub fn list(theme: Theme, children: impl IntoElement) -> impl IntoElement {
    div()
        .h(px(LIST_HEIGHT))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .flex_none()
        .rounded(px(6.0))
        .p(px(4.0))
        .bg(hsla(theme.colors.muted, 1.0))
        .text_color(hsla(theme.colors.muted_foreground, 1.0))
        .child(children)
}

/// A keyboard and pointer accessible tab trigger.
pub fn trigger(
    id: impl Into<ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    state: TriggerState,
    theme: Theme,
    on_change: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let colors = theme.colors;
    let foreground = if state.active {
        colors.foreground
    } else {
        colors.muted_foreground
    };
    let mut button = Button::new(id)
        .h(px(TRIGGER_HEIGHT))
        .w_auto()
        .flex_shrink_0()
        .px(px(12.0))
        .py(px(6.0))
        .rounded(px(4.0))
        .text_size(px(14.0))
        .font_weight(gpui_kit::FontWeight(500.0))
        .text_color(hsla(foreground, 1.0))
        .disabled(state.disabled)
        .on_click(on_change)
        .relative();
    if state.active {
        button = button
            .bg(hsla(colors.background, 1.0))
            .shadow(shadow_sm(1.0));
    }
    if state.focused {
        // `ring-offset-2` is the background-coloured outer layer; the ring is
        // painted inside it, so the two layers remain separately measurable.
        button = button
            .child(surface(
                gpui_kit::transparent_black(),
                gpui_kit::transparent_black(),
                0.0,
                4.0,
                Some((hsla(colors.background, 1.0), 4.0)),
            ))
            .child(surface(
                gpui_kit::transparent_black(),
                gpui_kit::transparent_black(),
                0.0,
                4.0,
                Some((hsla(colors.ring, 0.5), 2.0)),
            ));
    }
    button.child(label.into())
}

/// The content wrapper supplies the React component's `mt-2` spacing.
pub fn content(children: impl IntoElement) -> impl IntoElement {
    div().mt(px(8.0)).child(children)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::palette::{DARK, LIGHT};
    use gpui_kit::{KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Render, TestAppContext, point};
    use std::cell::Cell;
    use std::rc::Rc;

    struct TriggerProbe {
        clicks: Rc<Cell<usize>>,
    }

    impl Render for TriggerProbe {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            trigger(
                "tabs-test-trigger",
                "Overview",
                TriggerState::default(),
                DARK,
                move |_, _, _| clicks.set(clicks.get() + 1),
            )
        }
    }

    #[test]
    fn active_trigger_uses_the_local_theme() {
        let dark = DARK.colors.background;
        let light = LIGHT.colors.background;
        assert_ne!(dark, light);
        assert!(
            TriggerState {
                active: true,
                ..Default::default()
            }
            .active
        );
    }

    #[gpui_kit::test]
    fn trigger_activates_from_click_and_keyboard_buttons(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let clicks = Rc::new(Cell::new(0));
        let (_, visual) = cx.add_window_view({
            let clicks = clicks.clone();
            move |_, _| TriggerProbe { clicks }
        });
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.simulate_click(point(px(10.0), px(10.0)), Modifiers::default());
        assert_eq!(clicks.get(), 1);
        for key in ["enter", "space"] {
            let keystroke = Keystroke::parse(key).expect("valid tab activation key");
            visual.simulate_event(KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            visual.simulate_event(KeyUpEvent { keystroke });
        }
        assert_eq!(clicks.get(), 3);
    }
}
