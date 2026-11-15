//! The application's top bar and the platform-specific window controls.

use std::rc::Rc;

use gpui_kit::{App, Window};

/// The title bar height, matching `--title-bar-height` in `layout.css`.
pub const BAR_HEIGHT: f32 = 36.0;

pub mod buttons;
pub mod shell;
pub mod titlebar;

/// What a title-bar button asks the window to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TitleBarAction {
    Minimize,
    ToggleMaximize,
    Close,
}

/// The callbacks the Linux title bar uses. Windows uses native non-client hit
/// testing instead, and macOS keeps its traffic lights.
pub type ActionHandler = Rc<dyn Fn(TitleBarAction, &mut Window, &mut App)>;
