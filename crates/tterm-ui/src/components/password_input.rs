//! Password field matching `src/components/ui/password-input.tsx`.
//!
//! The text, editing and masking stay in gpui-component's `InputState`; create
//! it with `.masked(true)`. This module adds the reveal toggle and the Caps Lock
//! warning on top of the shared [`input`] primitive.

use super::button::{ButtonSize, ButtonState, Variant, button};
use super::icons::{ARROW_BIG_UP_DASH, EYE, EYE_OFF, icon};
use super::input::{InputOptions, PADDING_X, input};
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::component::input::InputState;
use gpui_kit::{
    App, Div, ElementId, Entity, Focusable as _, ParentElement, StatefulInteractiveElement, Styled,
    Window, div, px,
};

/// Presentation options for [`password_input`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PasswordOptions {
    pub disabled: bool,
    /// Draws the focus border even when the field does not hold focus, for
    /// parity fixtures; real focus is read from the input state.
    pub focused: bool,
    /// Hide the toggle when the field holds a placeholder mask rather than a
    /// real secret.
    pub revealable: bool,
    pub theme: Option<Theme>,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            disabled: false,
            focused: false,
            revealable: true,
            theme: None,
        }
    }
}

/// `pr-9`: room for the 32px toggle at `right-1`.
const TOGGLE_PADDING: f32 = 36.0;
/// `pr-16`: room for the toggle and the Caps Lock icon.
const TOGGLE_AND_WARNING_PADDING: f32 = 64.0;
/// The field height (`h-9`), the toggle size (`icon-sm`) and the icon size.
const FIELD_HEIGHT: f32 = 36.0;
const TOGGLE: f32 = 32.0;
const ICON: f32 = 16.0;

/// What the field shows, resolved once for painting and parity recording.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PasswordLayout {
    /// Whether the text is currently revealed.
    pub shown: bool,
    /// Whether the Caps Lock warning icon is drawn.
    pub warn_caps_lock: bool,
    /// The input's right padding.
    pub padding_right: f32,
}

/// Applies React's rules: a revealed password needs no Caps Lock warning, and
/// the padding grows with the controls drawn over the field.
pub fn layout(
    options: PasswordOptions,
    masked: bool,
    focused: bool,
    caps_lock: bool,
) -> PasswordLayout {
    let shown = options.revealable && !masked;
    let warn_caps_lock = focused && caps_lock && !shown && !options.disabled;
    let padding_right = match (options.revealable, warn_caps_lock) {
        (true, true) => TOGGLE_AND_WARNING_PADDING,
        (true, false) | (false, true) => TOGGLE_PADDING,
        (false, false) => PADDING_X,
    };
    PasswordLayout {
        shown,
        warn_caps_lock,
        padding_right,
    }
}

/// Builds a password field over `state`.
pub fn password_input(
    id: impl Into<ElementId>,
    state: &Entity<InputState>,
    options: PasswordOptions,
    window: &Window,
    cx: &App,
) -> Div {
    let theme = options.theme.unwrap_or(crate::tokens::palette::DARK);
    let read = state.read(cx);
    let focused = options.focused || read.focus_handle(cx).is_focused(window);
    let masked = read.presentation().is_masked();
    let resolved = layout(options, masked, focused, window.capslock().on);
    let muted = hsla(theme.colors.muted_foreground, 1.0);
    let field = input(
        state,
        InputOptions {
            disabled: options.disabled,
            focused: options.focused,
            theme: Some(theme),
            padding_right: resolved.padding_right,
            ..InputOptions::default()
        },
    );
    let mut view = div().relative().w_full().child(field);
    if resolved.warn_caps_lock {
        // `right-9` beside the toggle, `right-2.5` without it.
        let right = if options.revealable { 36.0 } else { 10.0 };
        view = view.child(
            div()
                .absolute()
                .right(px(right))
                .top(px((FIELD_HEIGHT - ICON) / 2.0))
                .child(icon(
                    ARROW_BIG_UP_DASH,
                    hsla(theme.colors.warning, 1.0),
                    ICON,
                )),
        );
    }
    if options.revealable {
        let toggle = button(
            id,
            "",
            Variant::Ghost,
            ButtonSize::IconSm,
            ButtonState {
                disabled: options.disabled,
                ..ButtonState::default()
            },
            theme,
        )
        // `text-muted-foreground` overrides the ghost variant's text colour.
        .text_color(muted)
        .child(icon(
            if resolved.shown { EYE_OFF } else { EYE },
            muted,
            ICON,
        ));
        let toggle = if options.disabled {
            toggle
        } else {
            let state = state.clone();
            toggle.on_click(move |_, window, cx| {
                state.update(cx, |state, cx| {
                    let masked = state.presentation().is_masked();
                    state.set_masked(!masked, window, cx);
                });
            })
        };
        // `right-1 top-1/2 -translate-y-1/2`.
        view = view.child(
            div()
                .absolute()
                .right(px(4.0))
                .top(px((FIELD_HEIGHT - TOGGLE) / 2.0))
                .child(toggle),
        );
    }
    view
}

#[cfg(test)]
mod tests;
