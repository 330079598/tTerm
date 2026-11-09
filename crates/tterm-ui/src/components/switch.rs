//! Interactive Switch matching the React primitive's geometry and states.

use super::surface::{css_color, css_group_color, shadow_xs, surface};
use crate::tokens::color::Hsl;
use crate::tokens::theme::Theme;
use gpui_kit::base::Toggle;
use gpui_kit::{App, ClickEvent, IntoElement, ParentElement, Styled, Window, div, px};

/// The controlled state of a [`switch`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwitchState {
    pub checked: bool,
    pub focused: bool,
    pub disabled: bool,
    /// The opaque colour behind the switch, used to flatten the disabled
    /// opacity (see [`css_group_color`]). Defaults to the theme background.
    pub backdrop: Option<Hsl>,
}

/// Resolved colours and geometry of a switch, shared by painting and parity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchStyle {
    pub track: Hsl,
    pub thumb: Hsl,
    /// `border-transparent`: the border takes space but paints nothing.
    pub border: [u8; 4],
    pub opacity: f32,
    pub ring_spread: f32,
    /// Thumb origin relative to the border box: `translate-x-4` when checked.
    pub thumb_offset: (f32, f32),
    /// Thumb diameter (`size-4`).
    pub thumb_size: f32,
}

/// Track size (`w-9 h-5`), border width, thumb size (`size-4`) and the
/// `translate-x-4` distance of the React primitive, in logical pixels.
const WIDTH: f32 = 36.0;
const HEIGHT: f32 = 20.0;
const BORDER: f32 = 1.0;
const THUMB: f32 = 16.0;
const TRAVEL: f32 = 16.0;
/// `rounded-full` clamps to half the height.
const RADIUS: f32 = HEIGHT / 2.0;

pub fn style(state: SwitchState, theme: Theme) -> SwitchStyle {
    // `inline-flex items-center` places the thumb at the start of the padding
    // box, centred vertically; `translate-x-4` moves it without layout.
    let inner_height = HEIGHT - 2.0 * BORDER;
    SwitchStyle {
        track: if state.checked {
            theme.colors.primary
        } else {
            theme.colors.input
        },
        thumb: theme.colors.background,
        border: [0, 0, 0, 0],
        opacity: if state.disabled { 0.5 } else { 1.0 },
        ring_spread: if state.focused { 3.0 } else { 0.0 },
        thumb_offset: (
            BORDER + if state.checked { TRAVEL } else { 0.0 },
            BORDER + (inner_height - THUMB) / 2.0,
        ),
        thumb_size: THUMB,
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
    let backdrop = state.backdrop.unwrap_or(theme.colors.background);
    // Both layers are opaque, so the CSS group opacity is reproduced exactly by
    // flattening each one over the backdrop and painting at full opacity.
    let paint = |color: Hsl| css_group_color(color, computed.opacity, backdrop);
    Toggle::new(id)
        .pressed(state.checked)
        .disabled(state.disabled)
        .h(px(HEIGHT))
        .w(px(WIDTH))
        .rounded(px(RADIUS))
        .border(px(BORDER))
        .border_color(gpui_kit::transparent_black())
        // Toggle centres its content by default; the React track starts it.
        .justify_start()
        .bg(paint(computed.track))
        .shadow(vec![shadow_xs(computed.opacity)])
        .on_change(on_change)
        .child(surface(
            gpui_kit::transparent_black(),
            gpui_kit::transparent_black(),
            BORDER,
            RADIUS,
            state
                .focused
                .then_some((css_color(theme.colors.ring, 0.5), computed.ring_spread)),
        ))
        .child(
            div()
                .relative()
                .left(px(computed.thumb_offset.0 - BORDER))
                .size(px(computed.thumb_size))
                .rounded(px(computed.thumb_size / 2.0))
                .bg(paint(computed.thumb)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::palette::{DARK, LIGHT};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{KeyDownEvent, KeyUpEvent, Keystroke, Modifiers, Render, TestAppContext, point};
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

        // The click focused the switch; Space must activate it once more.
        visual.update(|window, cx| window.draw(cx).clear(cx));
        let space = Keystroke::parse("space").expect("space keystroke");
        visual.simulate_event(KeyDownEvent {
            keystroke: space.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        visual.simulate_event(KeyUpEvent { keystroke: space });
        assert_eq!(changes.borrow().as_slice(), &[true, true]);
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

    struct Placed(SwitchState);

    impl Render for Placed {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div()
                .p(px(20.0))
                .child(switch("placed", self.0, DARK, |_, _, _, _| {}))
        }
    }

    /// Finds the painted thumb quad: the only one sized like the thumb.
    fn thumb_quad(cx: &mut TestAppContext, state: SwitchState) -> (f32, f32, gpui_kit::Background) {
        let (_, visual) = cx.add_window_view(move |_, _| Placed(state));
        visual.update(|window, cx| {
            window.render_frame(cx);
            let scale = window.scale_factor();
            let quads = window.painted_quads();
            let thumb = quads
                .iter()
                .find(|quad| (quad.bounds.size.width.as_f32() - THUMB * scale).abs() < 0.01)
                .expect("painted thumb");
            (
                thumb.bounds.origin.x.as_f32() / scale - 20.0,
                thumb.bounds.origin.y.as_f32() / scale - 20.0,
                thumb.background,
            )
        })
    }

    #[gpui_kit::test]
    fn thumb_sits_inside_the_border_and_travels_sixteen_pixels(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for (checked, expected) in [(false, (1.0, 2.0)), (true, (17.0, 2.0))] {
            let state = SwitchState {
                checked,
                ..SwitchState::default()
            };
            let (x, y, _) = thumb_quad(cx, state);
            assert_eq!((x, y), expected, "checked = {checked}");
            assert_eq!(style(state, DARK).thumb_offset, expected);
        }
    }

    #[gpui_kit::test]
    fn disabled_thumb_does_not_show_the_track_through(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        // CSS flattens the group first: the thumb is the background colour at
        // half opacity over the same background, so it stays the background.
        let (_, _, thumb) = thumb_quad(
            cx,
            SwitchState {
                checked: true,
                disabled: true,
                ..SwitchState::default()
            },
        );
        assert_eq!(thumb, css_color(DARK.colors.background, 1.0).into());
    }
}
