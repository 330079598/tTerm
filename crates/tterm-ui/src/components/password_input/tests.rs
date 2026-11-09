use super::*;
use crate::tokens::palette::DARK;
use gpui_kit::{AppContext as _, IntoElement, Modifiers, Render, TestAppContext, point};

#[test]
fn caps_lock_warning_follows_the_react_rules() {
    let options = PasswordOptions::default();
    let warned = layout(options, true, true, true);
    assert!(warned.warn_caps_lock);
    assert_eq!(warned.padding_right, TOGGLE_AND_WARNING_PADDING);
    // Revealed text shows its case, so it needs no warning.
    assert!(!layout(options, false, true, true).warn_caps_lock);
    assert!(!layout(options, true, false, true).warn_caps_lock);
    let disabled = PasswordOptions {
        disabled: true,
        ..options
    };
    assert!(!layout(disabled, true, true, true).warn_caps_lock);
    assert_eq!(layout(options, true, false, false).padding_right, 36.0);
    let fixed = PasswordOptions {
        revealable: false,
        ..options
    };
    assert_eq!(layout(fixed, true, true, true).padding_right, 36.0);
    assert_eq!(layout(fixed, true, false, false).padding_right, PADDING_X);
    // Without a toggle the text can never be revealed.
    assert!(!layout(fixed, false, false, false).shown);
}

struct Probe {
    state: Entity<InputState>,
    disabled: bool,
}

impl Render for Probe {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> impl IntoElement {
        div().w(px(300.0)).child(password_input(
            "password",
            &self.state,
            PasswordOptions {
                disabled: self.disabled,
                theme: Some(DARK),
                ..PasswordOptions::default()
            },
            window,
            cx,
        ))
    }
}

fn masked_after_clicking_the_toggle(cx: &mut TestAppContext, disabled: bool) -> bool {
    cx.update(gpui_kit::init);
    let (view, visual) = cx.add_window_view(move |window, cx| Probe {
        state: cx.new(|cx| InputState::new(window, cx).masked(true)),
        disabled,
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
    // The 32px toggle sits at `right-1` of the 300px field, 2px from the top.
    visual.simulate_click(point(px(280.0), px(18.0)), Modifiers::default());
    visual.read(|cx| view.read(cx).state.read(cx).presentation().is_masked())
}

#[gpui_kit::test]
fn the_toggle_reveals_the_text(cx: &mut TestAppContext) {
    assert!(!masked_after_clicking_the_toggle(cx, false));
}

#[gpui_kit::test]
fn a_disabled_toggle_keeps_the_text_masked(cx: &mut TestAppContext) {
    assert!(masked_after_clicking_the_toggle(cx, true));
}
