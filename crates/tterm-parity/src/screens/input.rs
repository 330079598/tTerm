//! GPUI Input parity screen using tterm-ui's editable primitive.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::{parity_child, parity_element};
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use tterm_ui::components::input::{InputOptions, input, style as input_style};
use tterm_ui::tokens::palette::{DARK, LIGHT};

pub const SCREEN_ID: &str = "ui.input.default";

pub struct InputScreen {
    layout: LayoutSink,
    theme: tterm_ui::tokens::theme::Theme,
    states: Option<Vec<Entity<InputState>>>,
}

impl InputScreen {
    pub fn load(manifest: &ScreensManifest, layout: LayoutSink) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            states: None,
        })
    }

    fn ensure_states(&mut self, window: &mut Window, cx: &mut impl AppContext) {
        if self.states.is_some() {
            return;
        }
        let empty = cx.new(|cx| InputState::new(window, cx).placeholder("Placeholder"));
        let long = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("A long input value used to verify clipping and horizontal layout")
        });
        let focused = cx.new(|cx| InputState::new(window, cx).placeholder("Focus me"));
        let disabled = cx.new(|cx| InputState::new(window, cx).default_value("Disabled"));
        self.states = Some(vec![empty, long, focused, disabled]);
    }

    fn style(&self, focused: bool, disabled: bool) -> StyleSnapshot {
        let computed = input_style(InputOptions {
            focused,
            disabled,
            theme: Some(self.theme),
            ..InputOptions::default()
        });
        StyleSnapshot {
            background_color: with_alpha(
                hsl_to_rgba(computed.background),
                computed.background_alpha,
            ),
            border_color: hsl_to_rgba(computed.border),
            border_width: 1.0,
            border_radius: 6.0,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(computed.foreground),
            padding: PaddingSnapshot::symmetric(4.0, 12.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: computed.opacity,
        }
    }

    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<crate::screens::ScreenView>,
    ) -> impl IntoElement {
        self.ensure_states(window, cx);
        let states = self.states.as_ref().expect("input states initialized");
        let screen_style = StyleSnapshot {
            background_color: hsl_to_rgba(self.theme.colors.background),
            border_color: hsl_to_rgba(self.theme.colors.border),
            border_width: 0.0,
            border_radius: 0.0,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(self.theme.colors.foreground),
            padding: PaddingSnapshot::all(32.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let field_style = self.style(false, false);
        let focused_style = self.style(true, false);
        let disabled_style = self.style(false, true);
        let children = [
            parity_child(
                &self.layout,
                "ui.input.empty",
                ElementSizing::Fixed,
                field_style.clone(),
                input(
                    &states[0],
                    InputOptions {
                        theme: Some(self.theme),
                        ..InputOptions::default()
                    },
                ),
            ),
            parity_child(
                &self.layout,
                "ui.input.long",
                ElementSizing::Fixed,
                field_style.clone(),
                input(
                    &states[1],
                    InputOptions {
                        theme: Some(self.theme),
                        ..InputOptions::default()
                    },
                ),
            ),
            parity_child(
                &self.layout,
                "ui.input.focus",
                ElementSizing::Fixed,
                focused_style,
                input(
                    &states[2],
                    InputOptions {
                        focused: true,
                        theme: Some(self.theme),
                        ..InputOptions::default()
                    },
                ),
            ),
            parity_child(
                &self.layout,
                "ui.input.disabled",
                ElementSizing::Fixed,
                disabled_style,
                input(
                    &states[3],
                    InputOptions {
                        disabled: true,
                        theme: Some(self.theme),
                        ..InputOptions::default()
                    },
                ),
            ),
        ];
        let bg = to_gpui(hsl_to_rgba(self.theme.colors.background));
        parity_element(
            &self.layout,
            "ui.input.screen",
            ElementSizing::Fixed,
            screen_style,
            move |view| {
                view.size_full().p(px(32.0)).bg(bg).child(
                    div()
                        .w(px(576.0))
                        .flex()
                        .flex_col()
                        .gap(px(16.0))
                        .children(children),
                )
            },
        )
        .size_full()
    }
}

fn with_alpha(mut color: [u8; 4], alpha: f32) -> [u8; 4] {
    color[3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    color
}
