//! P0-24 parity screens for PasswordInput, Select and Switch.

use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::parity_element;
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, Window};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use tterm_ui::components::password_input::{PasswordState, password_input};
use tterm_ui::components::select::{SelectState, select};
use tterm_ui::components::switch::{SwitchState, switch};
use tterm_ui::tokens::palette::{DARK, LIGHT};

pub const PASSWORD_ID: &str = "ui.password.default";
pub const SELECT_ID: &str = "ui.select.default";
pub const SWITCH_ID: &str = "ui.switch.default";

pub struct P024Screen {
    layout: LayoutSink,
    theme: tterm_ui::tokens::theme::Theme,
    kind: Kind,
    password_state: Option<Entity<InputState>>,
}
#[derive(Clone, Copy)]
pub enum Kind {
    Password,
    Select,
    Switch,
}

impl P024Screen {
    pub fn load(
        manifest: &ScreensManifest,
        layout: LayoutSink,
        kind: Kind,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            kind,
            password_state: None,
        })
    }
    pub fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<crate::screens::ScreenView>,
    ) -> impl IntoElement {
        let colors = self.theme.colors;
        let style = StyleSnapshot {
            background_color: crate::color::hsl_to_rgba(colors.background),
            border_color: crate::color::hsl_to_rgba(colors.border),
            border_width: 0.0,
            border_radius: 0.0,
            ring_spread: 0.0,
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.0,
            color: crate::color::hsl_to_rgba(colors.foreground),
            padding: PaddingSnapshot::all(32.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let child = match self.kind {
            Kind::Password => {
                if self.password_state.is_none() {
                    self.password_state = Some(cx.new(|cx| InputState::new(window, cx)));
                }
                let state = self
                    .password_state
                    .as_ref()
                    .expect("password state initialized");
                div().w(px(576.0)).child(password_input(
                    state,
                    PasswordState {
                        revealable: true,
                        ..PasswordState::default()
                    },
                    self.theme,
                ))
            }
            Kind::Select => div().h(px(36.0)).w(px(576.0)).child(select(
                &SelectState::new(vec!["One".into(), "Two".into()]),
                self.theme,
            )),
            Kind::Switch => div().child(switch(SwitchState::default(), self.theme)),
        };
        parity_element(
            &self.layout,
            match self.kind {
                Kind::Password => "ui.password.screen",
                Kind::Select => "ui.select.screen",
                Kind::Switch => "ui.switch.screen",
            },
            ElementSizing::Fixed,
            style,
            move |view| view.size_full().p(px(32.0)).child(child),
        )
        .size_full()
    }
}
