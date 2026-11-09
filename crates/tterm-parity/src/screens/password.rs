//! GPUI PasswordInput parity screen, matching `src/parity/screens/P024Primitives.tsx`.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::parity_child;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use tterm_ui::components::input::{InputOptions, PADDING_X, style as input_style};
use tterm_ui::components::password_input::{PasswordOptions, layout, password_input};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const SCREEN_ID: &str = "ui.password.default";

/// The three fields of the React screen: id, placeholder or value, disabled.
const FIELDS: [(&str, Field, bool); 3] = [
    ("ui.password.default", Field::Placeholder("Password"), false),
    ("ui.password.masked", Field::Value("secret-value"), false),
    ("ui.password.disabled", Field::Value("Disabled"), true),
];

#[derive(Clone, Copy)]
enum Field {
    Placeholder(&'static str),
    Value(&'static str),
}

pub struct PasswordScreen {
    layout: LayoutSink,
    theme: Theme,
    states: Vec<Entity<InputState>>,
}

impl PasswordScreen {
    pub fn load(manifest: &ScreensManifest, layout: LayoutSink) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            states: Vec::new(),
        })
    }

    fn ensure_states(&mut self, window: &mut Window, cx: &mut impl AppContext) {
        if !self.states.is_empty() {
            return;
        }
        self.states = FIELDS
            .iter()
            .map(|(_, field, _)| {
                cx.new(|cx| {
                    let state = InputState::new(window, cx).masked(true);
                    match field {
                        Field::Placeholder(text) => state.placeholder(*text),
                        Field::Value(text) => state.default_value(*text),
                    }
                })
            })
            .collect();
    }

    /// The input's snapshot, from the same rules the field paints with.
    fn style(&self, options: PasswordOptions) -> StyleSnapshot {
        // The fixture never focuses a field and Caps Lock is off.
        let resolved = layout(options, true, false, false);
        let computed = input_style(InputOptions {
            disabled: options.disabled,
            theme: Some(self.theme),
            padding_right: resolved.padding_right,
            ..InputOptions::default()
        });
        let mut background = hsl_to_rgba(computed.background);
        background[3] = (computed.background_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        StyleSnapshot {
            background_color: background,
            border_color: hsl_to_rgba(computed.border),
            border_width: 1.0,
            border_radius: 6.0,
            ring_spread: 0.0,
            ring_spreads: Vec::new(),
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(computed.foreground),
            padding: PaddingSnapshot {
                top: 4.0,
                right: computed.padding_right,
                bottom: 4.0,
                left: PADDING_X,
            },
            declared_background_color: None,
            declared_border_color: None,
            opacity: computed.opacity,
        }
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<super::ScreenView>,
    ) -> impl IntoElement {
        self.ensure_states(window, cx);
        let children = FIELDS
            .iter()
            .zip(&self.states)
            .map(|((id, _, disabled), state)| {
                let options = PasswordOptions {
                    disabled: *disabled,
                    theme: Some(self.theme),
                    ..PasswordOptions::default()
                };
                // The marker records the input, as `data-parity` sits on the
                // React `<input>`; the toggle is its sibling in both trees.
                let field = password_input(*id, state, options, window, cx);
                parity_child(
                    &self.layout,
                    *id,
                    ElementSizing::Fixed,
                    self.style(options),
                    field,
                )
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .p(px(32.0))
            .bg(to_gpui(hsl_to_rgba(self.theme.colors.background)))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .children(children)
    }
}
