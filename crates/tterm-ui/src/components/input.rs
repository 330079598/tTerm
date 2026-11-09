//! Input primitive backed by gpui-component's editable input state.

use crate::tokens::gpui::hsla;
use crate::tokens::palette::Colors;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size};
use gpui_kit::{Entity, IntoElement, ParentElement, Styled, div, px};

/// Presentation options shared by parity fixtures and the future app shell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputOptions {
    pub disabled: bool,
    pub focused: bool,
    pub size: Size,
    pub colors: Option<Colors>,
}

impl Default for InputOptions {
    fn default() -> Self {
        Self {
            disabled: false,
            focused: false,
            size: Size::Medium,
            colors: None,
        }
    }
}

/// Creates an editable single-line input from a gpui-component state entity.
pub fn input(state: &Entity<InputState>, options: InputOptions) -> impl IntoElement {
    let colors = options
        .colors
        .unwrap_or(crate::tokens::palette::DARK.colors);
    let background_alpha = if colors.background.lightness < 50.0 {
        0.3
    } else {
        0.0
    };
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
    use crate::tokens::palette::THEMES;
    use gpui_kit::{AppContext as _, Render, TestAppContext, Window};

    struct Probe {
        state: Entity<InputState>,
    }

    impl Render for Probe {
        fn render(
            &mut self,
            _window: &mut Window,
            _cx: &mut gpui_kit::Context<Self>,
        ) -> impl IntoElement {
            input(&self.state, InputOptions::default())
        }
    }

    #[gpui_kit::test]
    fn simulated_keystrokes_update_the_editable_state(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let state = cx.new(|cx| InputState::new(window, cx));
            state.update(cx, |state, cx| state.focus(window, cx));
            Probe { state }
        });
        cx.simulate_input("hello");
        let value = cx.read(|cx| view.read(cx).state.read(cx).value());
        assert_eq!(value.as_ref(), "hello");
    }

    #[test]
    fn options_keep_theme_selection_local() {
        let dark = InputOptions {
            colors: Some(THEMES[0].colors),
            ..InputOptions::default()
        };
        let light = InputOptions {
            colors: Some(THEMES[1].colors),
            ..InputOptions::default()
        };
        assert_ne!(dark.colors, light.colors);
        assert_eq!(dark.colors, Some(THEMES[0].colors));
        assert_eq!(light.colors, Some(THEMES[1].colors));
    }
}
