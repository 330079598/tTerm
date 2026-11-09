//! Input primitive backed by gpui-component's editable input state.

use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size};
use gpui_kit::{Entity, IntoElement, ParentElement, Styled, div, px};

/// Presentation options shared by parity fixtures and the future app shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputOptions {
    pub disabled: bool,
    pub focused: bool,
    pub size: Size,
    pub theme: Option<Theme>,
}

impl Default for InputOptions {
    fn default() -> Self {
        Self {
            disabled: false,
            focused: false,
            size: Size::Medium,
            theme: None,
        }
    }
}

/// Creates an editable single-line input from a gpui-component state entity.
pub fn input(state: &Entity<InputState>, options: InputOptions) -> impl IntoElement {
    let theme = options.theme.unwrap_or(crate::tokens::palette::DARK);
    let colors = theme.colors;
    let background_alpha = if theme.uses_dark_variant() { 0.3 } else { 0.0 };
    let mut view = div()
        .h(px(36.0))
        .w_full()
        .px(px(12.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .border(px(1.0))
        .border_color(hsla(
            if options.focused {
                colors.ring
            } else {
                colors.input
            },
            1.0,
        ))
        .bg(hsla(colors.input, background_alpha))
        .child(
            Input::new(state)
                .disabled(false)
                .with_size(options.size)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .text_color(hsla(colors.foreground, 1.0)),
        );
    if options.disabled {
        view = view.opacity(0.5);
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::{ButtonSize, ButtonState, Variant, button};
    use crate::tokens::palette::{DARK, LIGHT};
    use gpui_kit::{AppContext as _, Render, TestAppContext, Window};

    struct Probe {
        dark: Entity<InputState>,
        light: Entity<InputState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut gpui_kit::Context<Self>,
        ) -> impl IntoElement {
            gpui_kit::div()
                .child(input(
                    &self.dark,
                    InputOptions {
                        theme: Some(DARK),
                        ..InputOptions::default()
                    },
                ))
                .child(input(
                    &self.light,
                    InputOptions {
                        theme: Some(LIGHT),
                        ..InputOptions::default()
                    },
                ))
                .child(button(
                    "theme-isolation-button",
                    "button",
                    Variant::Default,
                    ButtonSize::Default,
                    ButtonState::default(),
                    DARK,
                ))
        }
    }

    #[gpui_kit::test]
    fn simulated_keystrokes_update_the_editable_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx));
            state.update(cx, |state, cx| state.focus(window, cx));
            let dark = state;
            let light = cx.new(|cx| InputState::new(window, cx));
            Probe { dark, light }
        });
        cx.simulate_input("hello");
        let value = cx.read(|cx| view.read(cx).dark.read(cx).value());
        assert_eq!(value.as_ref(), "hello");
    }

    #[gpui_kit::test]
    fn rendering_local_themes_does_not_mutate_global_theme(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let before = cx.read(|cx| {
            serde_json::to_string(gpui_kit::component::theme::Theme::global(cx))
                .expect("global theme serializes")
        });
        let _view = cx.add_window_view(|window, cx| {
            let dark = cx.new(|cx| InputState::new(window, cx));
            let light = cx.new(|cx| InputState::new(window, cx));
            Probe { dark, light }
        });
        let after = cx.read(|cx| {
            serde_json::to_string(gpui_kit::component::theme::Theme::global(cx))
                .expect("global theme serializes")
        });
        assert_eq!(before, after);
    }
}
