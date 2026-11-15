//! Window-control buttons and their platform hit-test areas.
//!
//! Windows receives non-client areas, so the OS owns minimize, maximize /
//! restore, close and the Windows 11 Snap Layout flyout. Linux calls the
//! handler directly. The glyphs are simple shapes drawn here rather than a
//! copied icon set.

use gpui_kit::{
    Div, Hsla, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
    WindowControlArea, div, px, svg,
};
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::theme::Theme;

use super::BAR_HEIGHT;
use super::{ActionHandler, TitleBarAction};

const BUTTON_WIDTH: f32 = 46.0;
const CLOSE_HOVER: u32 = 0x00c4_2b1c;

/// The three buttons, in their Windows and Linux order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Control {
    Minimize,
    Maximize,
    Close,
}

/// Windows' non-client area for a button. `None` means the button is drawn
/// and handled by the app (Linux).
pub(crate) fn control_area(control: Control) -> Option<WindowControlArea> {
    match control {
        Control::Minimize => Some(WindowControlArea::Min),
        Control::Maximize => Some(WindowControlArea::Max),
        Control::Close => Some(WindowControlArea::Close),
    }
}

fn action(control: Control) -> TitleBarAction {
    match control {
        Control::Minimize => TitleBarAction::Minimize,
        Control::Maximize => TitleBarAction::ToggleMaximize,
        Control::Close => TitleBarAction::Close,
    }
}

/// A row of window buttons for the current platform.
pub(crate) fn controls(
    theme: Theme,
    maximized: bool,
    native: bool,
    handler: Option<ActionHandler>,
) -> Div {
    let mut row = div().h(px(BAR_HEIGHT)).flex().items_center();
    for control in [Control::Minimize, Control::Maximize, Control::Close] {
        row = row.child(button(control, theme, maximized, native, handler.clone()));
    }
    row
}

fn button(
    control: Control,
    theme: Theme,
    maximized: bool,
    native: bool,
    handler: Option<ActionHandler>,
) -> Stateful<Div> {
    let close = control == Control::Close;
    let hover: Hsla = if close {
        gpui_kit::rgb(CLOSE_HOVER).into()
    } else {
        hsla(theme.colors.muted, 1.0)
    };
    let mut element = div()
        .id(match control {
            Control::Minimize => "window-minimize",
            Control::Maximize => "window-maximize",
            Control::Close => "window-close",
        })
        .w(px(BUTTON_WIDTH))
        .h(px(BAR_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .hover(move |style| style.bg(hover));

    if native {
        if let Some(area) = control_area(control) {
            element = element.window_control_area(area);
        }
    } else if let Some(handler) = handler {
        let action = action(control);
        element = element.on_click(move |_, window, cx| handler(action, window, cx));
    }

    element.child(
        svg()
            .data(icon(control, maximized))
            .size(px(16.0))
            .text_color(icon_color(theme, close)),
    )
}

fn icon_color(theme: Theme, close: bool) -> Hsla {
    if close {
        hsla(theme.colors.foreground, 1.0)
    } else {
        hsla(theme.colors.muted_foreground, 1.0)
    }
}

fn icon(control: Control, maximized: bool) -> &'static [u8] {
    match (control, maximized) {
        (Control::Minimize, _) => MINIMIZE,
        (Control::Maximize, false) => MAXIMIZE,
        (Control::Maximize, true) => RESTORE,
        (Control::Close, _) => CLOSE,
    }
}

// Simple geometry, not copied from another icon package.
const MINIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 12h16"/></svg>"#;
const MAXIMIZE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="5" y="5" width="14" height="14"/></svg>"#;
const RESTORE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="8" y="4" width="12" height="12"/><path d="M16 16v4H4V8h4"/></svg>"#;
const CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M6 6l12 12"/><path d="M18 6L6 18"/></svg>"#;

#[cfg(test)]
mod tests {
    use gpui_kit::WindowControlArea;

    use super::{Control, action, control_area};
    use crate::app_window::chrome::TitleBarAction;

    #[test]
    fn each_button_maps_to_the_windows_hit_test_area() {
        assert_eq!(
            control_area(Control::Minimize),
            Some(WindowControlArea::Min)
        );
        assert_eq!(
            control_area(Control::Maximize),
            Some(WindowControlArea::Max)
        );
        assert_eq!(control_area(Control::Close), Some(WindowControlArea::Close));
    }

    #[test]
    fn each_button_maps_to_the_shared_action() {
        assert_eq!(action(Control::Minimize), TitleBarAction::Minimize);
        assert_eq!(action(Control::Maximize), TitleBarAction::ToggleMaximize);
        assert_eq!(action(Control::Close), TitleBarAction::Close);
    }
}
