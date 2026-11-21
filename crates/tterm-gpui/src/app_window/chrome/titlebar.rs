//! The top bar: slot content, drag space, tools and platform controls.
//!
//! macOS keeps its traffic lights and reserves the same 80 logical pixels as
//! `layout.css`. Windows uses non-client hit testing so the OS provides
//! minimize, toggle-maximize, close, double-click and Snap Layout. Linux
//! draws and handles the buttons itself; when the compositor reports server
//! decorations, the bar drops its title and buttons so the OS title is not
//! duplicated.

use gpui_kit::{
    AnyElement, Decorations, Div, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, WindowControlArea, div, px,
};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::super::Platform;
use super::{ActionHandler, BAR_HEIGHT, buttons};

const TRAFFIC_LIGHT_INSET: f32 = 80.0;
const SIDE_PADDING: f32 = 12.0;

/// The title bar's content. The slot is the P3-07 tab-bar hand-off; until
/// then the current title is shown on Windows and Linux.
pub struct TitleBar {
    title: SharedString,
    theme: Theme,
    slot: Option<AnyElement>,
    tools: Option<AnyElement>,
    on_action: Option<ActionHandler>,
}

impl TitleBar {
    pub fn new(title: impl Into<SharedString>, theme: Theme) -> Self {
        Self {
            title: title.into(),
            theme,
            slot: None,
            tools: None,
            on_action: None,
        }
    }

    /// Replaces the placeholder title with the P3-07 tab strip.
    pub fn slot(mut self, slot: impl IntoElement) -> Self {
        self.slot = Some(slot.into_any_element());
        self
    }

    /// Adds the right-side tools (settings, tunnels, transfers) later.
    pub fn tools(mut self, tools: impl IntoElement) -> Self {
        self.tools = Some(tools.into_any_element());
        self
    }

    pub fn on_action(mut self, handler: ActionHandler) -> Self {
        self.on_action = Some(handler);
        self
    }

    pub fn render(self, window: &Window) -> Div {
        let Self {
            title,
            theme,
            slot,
            tools,
            on_action,
        } = self;
        let platform = Platform::current();
        let server_decorations = matches!(platform, Platform::Linux)
            && matches!(window.window_decorations(), Decorations::Server);
        let layout = layout(platform, server_decorations);

        let mut left = div()
            .min_w_0()
            .h(px(BAR_HEIGHT))
            .flex()
            .items_center()
            .overflow_hidden();
        left = if layout.traffic_light_inset > 0.0 {
            left.pl(px(layout.traffic_light_inset))
        } else {
            left.pl(px(SIDE_PADDING))
        };
        if let Some(slot) = slot {
            left = left.child(slot);
        } else if layout.show_title {
            left = left.child(
                div()
                    .max_w(px(480.0))
                    .text_sm()
                    .text_ellipsis()
                    .text_color(hsla(theme.colors.foreground, 1.0))
                    .child(title),
            );
        }

        let mut drag = div()
            .id("window-drag-region")
            .h(px(BAR_HEIGHT))
            .min_w(px(28.))
            .flex_1();
        if layout.drag {
            drag = match platform {
                Platform::Windows => drag.window_control_area(WindowControlArea::Drag),
                Platform::MacOs => drag
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .on_click(|event, window, _| {
                        if event.click_count() == 2 {
                            window.titlebar_double_click();
                        }
                    }),
                Platform::Linux => drag
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .on_click(|event, window, _| {
                        if event.click_count() == 2 {
                            window.zoom_window();
                        }
                    }),
            };
        }

        let mut right = div()
            .h(px(BAR_HEIGHT))
            .flex()
            .flex_shrink_0()
            .items_center();
        if let Some(tools) = tools {
            right = right.child(tools);
        }
        if layout.show_buttons {
            right = right.child(buttons::controls(
                theme,
                window.is_maximized(),
                layout.native_buttons,
                on_action,
            ));
        }

        div()
            .w_full()
            .h(px(BAR_HEIGHT))
            .flex()
            .items_center()
            .bg(hsla(theme.colors.titlebar, 1.0))
            .child(left)
            .child(drag)
            .child(right)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ChromeLayout {
    pub traffic_light_inset: f32,
    pub show_title: bool,
    pub show_buttons: bool,
    pub native_buttons: bool,
    pub drag: bool,
}

pub(crate) fn layout(platform: Platform, server_decorations: bool) -> ChromeLayout {
    match platform {
        Platform::MacOs => ChromeLayout {
            traffic_light_inset: TRAFFIC_LIGHT_INSET,
            show_title: false,
            show_buttons: false,
            native_buttons: false,
            drag: true,
        },
        Platform::Windows => ChromeLayout {
            traffic_light_inset: 0.0,
            show_title: true,
            show_buttons: true,
            native_buttons: true,
            drag: true,
        },
        Platform::Linux if server_decorations => ChromeLayout {
            traffic_light_inset: 0.0,
            show_title: false,
            show_buttons: false,
            native_buttons: false,
            drag: false,
        },
        Platform::Linux => ChromeLayout {
            traffic_light_inset: 0.0,
            show_title: true,
            show_buttons: true,
            native_buttons: false,
            drag: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{TRAFFIC_LIGHT_INSET, layout};
    use crate::app_window::Platform;
    use crate::app_window::chrome::BAR_HEIGHT;

    #[test]
    fn macos_reserves_the_traffic_lights_without_app_buttons() {
        let chrome = layout(Platform::MacOs, false);
        assert_eq!(chrome.traffic_light_inset, TRAFFIC_LIGHT_INSET);
        assert!(!chrome.show_title);
        assert!(!chrome.show_buttons);
        assert!(chrome.drag);
    }

    #[test]
    fn windows_shows_native_hit_test_buttons() {
        let chrome = layout(Platform::Windows, false);
        assert_eq!(chrome.traffic_light_inset, 0.0);
        assert!(chrome.show_title);
        assert!(chrome.show_buttons);
        assert!(chrome.native_buttons);
        assert!(chrome.drag);
    }

    #[test]
    fn linux_client_decorations_draw_app_buttons() {
        let chrome = layout(Platform::Linux, false);
        assert!(chrome.show_title);
        assert!(chrome.show_buttons);
        assert!(!chrome.native_buttons);
        assert!(chrome.drag);
    }

    #[test]
    fn linux_server_decorations_do_not_duplicate_the_title() {
        let chrome = layout(Platform::Linux, true);
        assert!(!chrome.show_title);
        assert!(!chrome.show_buttons);
        assert!(!chrome.drag);
    }

    #[test]
    fn the_bar_height_matches_the_stylesheet() {
        assert_eq!(BAR_HEIGHT, 36.0);
    }
}
