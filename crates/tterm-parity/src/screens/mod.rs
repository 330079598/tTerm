//! The GPUI views the parity capture can render.
//!
//! There is one entry per screen id in `parity/screens.json`. The reference
//! side keeps its own registry in `src/parity/screens.ts`; a screen missing
//! from either side fails the manifest test or the capture, not silently.

pub mod button;
pub mod dialog;
pub mod example;
pub mod input;
pub mod password;
pub mod select;
pub mod switch;

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
    Dialog(dialog::DialogScreen),
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
            dialog::CONFIRM_SCREEN_ID => Ok(Self::Dialog(dialog::DialogScreen::load(
                manifest,
                sink,
                tterm_ui::components::dialog::DialogKind::Confirm,
                false,
                false,
            )?)),
            dialog::DESTRUCTIVE_SCREEN_ID => Ok(Self::Dialog(dialog::DialogScreen::load(
                manifest,
                sink,
                tterm_ui::components::dialog::DialogKind::Confirm,
                true,
                false,
            )?)),
            dialog::PROMPT_SCREEN_ID => Ok(Self::Dialog(dialog::DialogScreen::load(
                manifest,
                sink,
                tterm_ui::components::dialog::DialogKind::Prompt,
                false,
                false,
            )?)),
            dialog::MULTILINE_SCREEN_ID => Ok(Self::Dialog(dialog::DialogScreen::load(
                manifest,
                sink,
                tterm_ui::components::dialog::DialogKind::Confirm,
                false,
                true,
            )?)),
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
            Self::Dialog(view) => view.render(_window, _cx).into_any_element(),
        }
    }
}
