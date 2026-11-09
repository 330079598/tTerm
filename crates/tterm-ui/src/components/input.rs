//! Input primitive backed by gpui-component's editable input state.

use crate::components::surface::{css_color, css_opacity, surface};
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size};
use gpui_kit::{Entity, ParentElement, Styled, div, px};

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

/// Presentation resolved once for both painting and parity recording.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputStyle {
    pub background: crate::tokens::color::Hsl,
    pub background_alpha: f32,
    pub border: crate::tokens::color::Hsl,
    pub foreground: crate::tokens::color::Hsl,
    pub opacity: f32,
}

/// Resolves the production Input rules without changing any global theme.
pub fn style(options: InputOptions) -> InputStyle {
    let theme = options.theme.unwrap_or(crate::tokens::palette::DARK);
    let colors = theme.colors;
    InputStyle {
        background: colors.input,
        background_alpha: if theme.uses_dark_variant() { 0.3 } else { 0.0 },
        border: if options.focused {
            colors.ring
        } else {
            colors.input
        },
        foreground: colors.foreground,
        opacity: if options.disabled { 0.5 } else { 1.0 },
    }
}

/// Creates an editable single-line input from a gpui-component state entity.
pub fn input(state: &Entity<InputState>, options: InputOptions) -> gpui_kit::Div {
    let computed = style(options);
    div()
        .h(px(36.0))
        .w_full()
        .px(px(12.0))
        .py(px(4.0))
        .rounded(px(6.0))
        .border(px(1.0))
        .border_color(gpui_kit::transparent_black())
        .relative()
        .text_color(hsla(computed.foreground, 1.0))
        .child(surface(
            css_color(computed.background, computed.background_alpha),
            css_color(computed.border, 1.0),
            1.0,
            None,
        ))
        .child(
            Input::new(state)
                .disabled(false)
                .with_size(options.size)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .text_color(hsla(computed.foreground, 1.0)),
        )
        .opacity(css_opacity(computed.opacity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::{ButtonSize, ButtonState, Variant, button};
    use crate::tokens::palette::{DARK, LIGHT};
    use gpui_kit::{AppContext as _, IntoElement, Render, TestAppContext, Window};

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
        let (view, visual) = cx.add_window_view(|window, cx| {
            let dark = cx.new(|cx| InputState::new(window, cx));
            let light = cx.new(|cx| InputState::new(window, cx));
            Probe { dark, light }
        });
        visual.update(|window, cx| {
            use gpui_kit::test::TestWindowExt as _;
            window.render_frame(cx);
            let probe = view.read(cx);
            let mut dark = input(
                &probe.dark,
                InputOptions {
                    theme: Some(DARK),
                    ..InputOptions::default()
                },
            );
            let mut dark_style = dark.style().clone();
            let mut light = input(
                &probe.light,
                InputOptions {
                    theme: Some(LIGHT),
                    ..InputOptions::default()
                },
            );
            assert_ne!(dark_style.text_style(), light.style().text_style());
            assert_eq!(dark_style, dark.style().clone());
        });
        let after = visual.read(|cx| {
            serde_json::to_string(gpui_kit::component::theme::Theme::global(cx))
                .expect("global theme serializes")
        });
        assert_eq!(before, after);
    }
}
