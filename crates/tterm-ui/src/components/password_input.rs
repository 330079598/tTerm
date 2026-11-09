//! Password input presentation: the editable state remains gpui-component's
//! InputState while controls and masking are local to tterm-ui.

use super::button::{ButtonSize, ButtonState, Variant, button};
use super::icons::{ARROW_BIG_UP_DASH, EYE, EYE_OFF, icon};
use super::input::{InputOptions, input};
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::component::input::InputState;
use gpui_kit::{Entity, IntoElement, ParentElement, Styled, div, px};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasswordState {
    pub visible: bool,
    pub focused: bool,
    pub caps_lock: bool,
    pub disabled: bool,
    pub revealable: bool,
}

pub fn password_input(
    state: &Entity<InputState>,
    options: PasswordState,
    theme: Theme,
) -> impl IntoElement {
    let warn_caps = options.focused && options.caps_lock && !options.visible && !options.disabled;
    let muted = hsla(theme.colors.muted_foreground, 1.0);
    let field = input(
        state,
        InputOptions {
            disabled: options.disabled,
            focused: options.focused,
            theme: Some(theme),
            ..InputOptions::default()
        },
    );
    let mut view = div().relative().child(field);
    if warn_caps {
        view = view.child(
            div()
                .absolute()
                .right(px(if options.revealable { 36.0 } else { 10.0 }))
                .top(px(10.0))
                .child(icon(
                    ARROW_BIG_UP_DASH,
                    hsla(theme.colors.warning, 1.0),
                    16.0,
                )),
        );
    }
    if options.revealable {
        let eye = if options.visible { EYE_OFF } else { EYE };
        view = view.child(
            div()
                .absolute()
                .right(px(4.0))
                .top(px(2.0))
                .child(
                    button(
                        "password-toggle",
                        "",
                        Variant::Ghost,
                        ButtonSize::IconSm,
                        ButtonState {
                            disabled: options.disabled,
                            ..ButtonState::default()
                        },
                        theme,
                    )
                    .into_any_element(),
                )
                .child(icon(eye, muted, 16.0)),
        );
    }
    view
}
