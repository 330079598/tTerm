//! The icon-only actions at the right end of the title bar.
//!
//! React draws these with `tab-action` and `settings-button`; `layout.css`
//! gives them a 28 by 28 hit area, the `--radius` corners, a muted 16 px
//! Lucide icon and a muted background under the pointer. The localized label
//! stays in the accessible name, so no action widens with its translation.

use gpui_kit::{
    Div, Hsla, InteractiveElement, ParentElement, Role, SharedString, Stateful,
    StatefulInteractiveElement, Styled, div, px, svg,
};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

/// The hit area React gives one title-bar action in `layout.css`.
pub(crate) const ACTION_SIZE: f32 = 28.0;
/// The size React passes to the Lucide component inside the action.
pub(crate) const ACTION_ICON_SIZE: f32 = 16.0;
/// React's `--radius`, shared by `.tab-action` and `.settings-button`.
const ACTION_RADIUS: f32 = 4.0;

/// Builds one title-bar action. The caller attaches its click handler, and
/// the localized `label` becomes the accessible name and description because
/// GPUI has no hover tooltip for a plain element.
pub(crate) fn action_button(
    id: &'static str,
    label: impl Into<SharedString>,
    icon_data: &'static [u8],
    theme: Theme,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let muted: Hsla = hsla(theme.colors.muted_foreground, 1.0);
    let foreground: Hsla = hsla(theme.colors.foreground, 1.0);
    let group = SharedString::from(id);
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .group(group.clone())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(ACTION_SIZE))
        .rounded(px(ACTION_RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(hsla(theme.colors.muted, 1.0)))
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_description(label)
        .child(
            svg()
                .data(icon_data)
                .size(px(ACTION_ICON_SIZE))
                .text_color(muted)
                .group_hover(group, move |style| style.text_color(foreground)),
        )
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        Context, IntoElement, ParentElement, Pixels, Render, Size, Styled, TestAppContext,
        VisualTestContext, Window, div, px,
    };
    use tterm_ui::components::icons::{BOOK_MARKED, LAYOUT_TEMPLATE, SETTINGS, WAYPOINTS};

    use super::{ACTION_SIZE, action_button};
    use crate::app_window::config::MIN_WIDTH;
    use crate::app_window::theme::default_theme;

    const ACTIONS: [(&str, &[u8]); 4] = [
        ("titlebar-action-tunnels", WAYPOINTS),
        ("titlebar-action-layouts", LAYOUT_TEMPLATE),
        ("titlebar-action-saved", BOOK_MARKED),
        ("titlebar-action-settings", SETTINGS),
    ];
    /// The gap `Prototype::render` puts between the title-bar tools.
    const GAP: f32 = 4.0;

    struct Row;

    impl Render for Row {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let theme = default_theme().ui;
            div()
                .w_full()
                .flex()
                .items_center()
                .child(div().flex_1())
                .child(
                    div().flex().items_center().gap(px(GAP)).children(
                        ACTIONS.map(|(id, icon)| action_button(id, "action", icon, theme)),
                    ),
                )
        }
    }

    fn layout_at(visual: &mut VisualTestContext, width: Pixels) {
        visual.simulate_resize(Size::new(width, px(240.0)));
        visual.update(|window, cx| window.render_frame(cx));
        let mut previous_right = None;
        for (id, _) in ACTIONS {
            let bounds = visual
                .debug_bounds(id)
                .unwrap_or_else(|| panic!("{id} should be laid out"));
            assert_eq!(bounds.size.width, px(ACTION_SIZE), "{id} width");
            assert_eq!(bounds.size.height, px(ACTION_SIZE), "{id} height");
            if let Some(right) = previous_right {
                assert_eq!(bounds.left(), right + px(GAP), "{id} follows the row");
            }
            previous_right = Some(bounds.right());
        }
        let last = visual
            .debug_bounds(ACTIONS[3].0)
            .expect("the last action is laid out");
        assert!(
            (last.right() - width).abs() <= px(0.5),
            "the row ends at the window edge: width {width:?}, right {:?}",
            last.right()
        );
    }

    #[gpui_kit::test]
    fn the_actions_keep_the_layout_css_hit_size_at_the_minimum_width(cx: &mut TestAppContext) {
        let (_, visual) = cx.add_window_view(|_, _| Row);
        visual.update(|window, cx| window.render_frame(cx));
        let window_width = visual.update(|window, _| window.bounds().size.width);
        layout_at(visual, window_width);
        layout_at(visual, px(MIN_WIDTH));
    }
}
