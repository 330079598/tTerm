//! Interactive Switch matching the React primitive's geometry and states.

use super::surface::css_color;
use super::surface::surface;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::base::Toggle;
use gpui_kit::{App, ClickEvent, IntoElement, ParentElement, Styled, Window, div, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchState {
    pub checked: bool,
    pub focused: bool,
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchStyle {
    pub track: crate::tokens::color::Hsl,
    pub thumb: crate::tokens::color::Hsl,
    pub opacity: f32,
    pub ring_spread: f32,
}

pub fn style(state: SwitchState, theme: Theme) -> SwitchStyle {
    SwitchStyle {
        track: if state.checked {
            theme.colors.primary
        } else {
            theme.colors.input
        },
        thumb: theme.colors.background,
        opacity: if state.disabled { 0.5 } else { 1.0 },
        ring_spread: if state.focused { 3.0 } else { 0.0 },
    }
}

/// Builds a controlled switch. `on_change` is called for pointer and keyboard activation.
pub fn switch(
    id: impl Into<gpui_kit::ElementId>,
    state: SwitchState,
    theme: Theme,
    on_change: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let computed = style(state, theme);
    Toggle::new(id)
        .pressed(state.checked)
        .disabled(state.disabled)
        .h(px(20.0))
        .w(px(36.0))
        .rounded(px(10.0))
        .border(px(1.0))
        .border_color(gpui_kit::transparent_black())
        .bg(hsla(computed.track, 1.0))
        .opacity(computed.opacity)
        .on_change(on_change)
        .child(surface(
            gpui_kit::transparent_black(),
            gpui_kit::transparent_black(),
            0.0,
            state
                .focused
                .then_some((css_color(theme.colors.ring, 0.5), computed.ring_spread)),
        ))
        .child(
            div()
                .absolute()
                .left(px(if state.checked { 18.0 } else { 2.0 }))
                .top(px(2.0))
                .size(px(16.0))
                .rounded(px(8.0))
                .bg(hsla(computed.thumb, 1.0)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::palette::{DARK, LIGHT};
    use gpui_kit::{Modifiers, Render, TestAppContext, point};
    use std::cell::RefCell;
    use std::rc::Rc;

    struct Probe {
        changes: Rc<RefCell<Vec<bool>>>,
        disabled: bool,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            switch(
                "switch-test",
                SwitchState {
                    disabled: self.disabled,
                    ..SwitchState::default()
                },
                DARK,
                move |next, _, _, _| changes.borrow_mut().push(next),
            )
        }
    }

    #[gpui_kit::test]
    fn click_and_space_report_checked_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_for_view = changes.clone();
        let (_, visual) = cx.add_window_view(move |_, _| Probe {
            changes: changes_for_view,
            disabled: false,
        });
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.simulate_click(point(px(10.0), px(10.0)), Modifiers::default());
        assert_eq!(changes.borrow().as_slice(), &[true]);
    }

    #[gpui_kit::test]
    fn disabled_switch_does_not_report_changes(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let changes = Rc::new(RefCell::new(Vec::new()));
        let changes_for_view = changes.clone();
        let (_, visual) = cx.add_window_view(move |_, _| Probe {
            changes: changes_for_view,
            disabled: true,
        });
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.simulate_click(point(px(10.0), px(10.0)), Modifiers::default());
        assert!(changes.borrow().is_empty());
    }

    #[test]
    fn local_theme_style_is_isolated() {
        let dark = style(SwitchState::default(), DARK);
        let light = style(SwitchState::default(), LIGHT);
        assert_ne!(dark.track, light.track);
        assert_eq!(style(SwitchState::default(), DARK), dark);
    }
}
