//! The GPUI views the parity capture can render.
//!
//! There is one entry per screen id in `parity/screens.json`. The reference
//! side keeps its own registry in `src/parity/screens.ts`; a screen missing
//! from either side fails the manifest test or the capture, not silently.

pub mod button;
pub mod context_menu;
pub mod example;
pub mod input;
pub mod password;
pub mod select;
pub mod switch;
pub mod tabs;

use crate::layout::LayoutSink;
use crate::manifest::{ScreenSpec, ScreensManifest};
use example::ExampleScreen;
use gpui_kit::{Context, IntoElement, Render, Window};
use std::path::Path;

/// A screen's GPUI view.
pub enum ScreenView {
    /// The W0-04 example: one button and one input.
    Example(ExampleScreen),
    ButtonVariants(button::ButtonScreen),
    ButtonSizes(button::ButtonScreen),
    Input(input::InputScreen),
    Password(password::PasswordScreen),
    Select(select::SelectScreen),
    Switch(switch::SwitchScreen),
    TabsDefault(tabs::TabsScreen),
    TabsServer(tabs::TabsScreen),
    ContextMenu(context_menu::ContextMenuScreen),
}

impl ScreenView {
    /// Builds the view for `screen`, loading its fixture.
    pub fn build(
        root: &Path,
        screen: &ScreenSpec,
        manifest: &ScreensManifest,
        sink: LayoutSink,
    ) -> Result<Self, String> {
        match screen.id.as_str() {
            example::SCREEN_ID => Ok(Self::Example(ExampleScreen::load(
                root, screen, manifest, sink,
            )?)),
            button::VARIANTS_SCREEN_ID => Ok(Self::ButtonVariants(button::ButtonScreen::load(
                manifest, sink, true,
            )?)),
            button::SIZES_SCREEN_ID => Ok(Self::ButtonSizes(button::ButtonScreen::load(
                manifest, sink, false,
            )?)),
            input::SCREEN_ID => Ok(Self::Input(input::InputScreen::load(manifest, sink)?)),
            password::SCREEN_ID => Ok(Self::Password(password::PasswordScreen::load(
                manifest, sink,
            )?)),
            select::SCREEN_ID => Ok(Self::Select(select::SelectScreen::load(manifest, sink)?)),
            switch::SCREEN_ID => Ok(Self::Switch(switch::SwitchScreen::load(manifest, sink)?)),
            tabs::DEFAULT_SCREEN_ID => Ok(Self::TabsDefault(tabs::TabsScreen::load(
                manifest, sink, false,
            )?)),
            tabs::SERVER_SCREEN_ID => Ok(Self::TabsServer(tabs::TabsScreen::load(
                manifest, sink, true,
            )?)),
            context_menu::DEFAULT_ID | context_menu::HIGHLIGHTED_ID | context_menu::WRAPPED_ID => {
                Ok(Self::ContextMenu(context_menu::ContextMenuScreen::load(
                    root, screen, manifest, sink,
                )?))
            }
            other => Err(format!(
                "No GPUI view is registered for parity screen \"{other}\""
            )),
        }
    }
}

impl Render for ScreenView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        match self {
            Self::Example(view) => view.render().into_any_element(),
            Self::ButtonVariants(view) | Self::ButtonSizes(view) => {
                view.render().into_any_element()
            }
            Self::Input(view) => view.render(_window, _cx).into_any_element(),
            Self::Password(view) => view.render(_window, _cx).into_any_element(),
            Self::Select(view) => view.render(_cx).into_any_element(),
            Self::Switch(view) => view.render().into_any_element(),
            Self::TabsDefault(view) | Self::TabsServer(view) => view.render(_cx).into_any_element(),
            Self::ContextMenu(view) => view.render(_window, _cx).into_any_element(),
        }
    }
}
