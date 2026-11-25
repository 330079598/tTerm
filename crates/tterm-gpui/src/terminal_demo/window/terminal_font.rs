//! Applies the shared config font to every pane, new and existing.
//!
//! The family, size and cell spacing live in the strict config file. A pane reads them
//! from the committed [`ConfigState`] snapshot when it is created, and an open
//! pane follows the [`TerminalFontChanged`] event, which is emitted only after
//! the file accepted the values and the snapshot was updated. A failed save
//! therefore leaves every PTY, grid and row cache alone, and an unsaved draft
//! never reaches a pane at all.
//!
//! The values reach a pane through [`tterm_terminal_view::view::TerminalView::set_font`], which
//! re-measures the cell, asks the session to resize its PTY and drops the
//! cached rows without touching the model, its scrollback or the session
//! identity. An unchanged pair is a no-op, so repeating a commit cannot
//! double-resize a PTY.
//!
//! [`ConfigState`]: crate::config_state::ConfigState

use gpui_kit::{App, Context, Subscription};
use tterm_terminal_view::view::font::TerminalFont;

use crate::config_state::TerminalFontChanged;

use super::{Prototype, TabFactory};

impl TabFactory {
    /// The font a pane opened now starts with.
    ///
    /// The chain is the config's CSS `font-family` list; the installed family
    /// is resolved when the first frame measures the grid, so a missing font
    /// falls back down the chain instead of loading the proportional UI font.
    pub(super) fn shared_font(&self, cx: &App) -> TerminalFont {
        let config = self.config.read(cx).config();
        TerminalFont {
            line_height: config.terminal_line_height as f32,
            letter_spacing: f32::from(config.terminal_letter_spacing),
            ..TerminalFont::from_shared(&config.font_family, config.font_size)
        }
    }
}

impl Prototype {
    /// Applies every committed font change to the panes that are already open.
    ///
    /// The event carries the values the config file now holds, so a save that
    /// fails, is refused while another save is pending, or never happens
    /// cannot change a pane. A new pane reads the same snapshot in
    /// [`TabFactory::shared_font`], which keeps the two paths in agreement.
    pub(super) fn subscribe_shared_font(&self, cx: &mut Context<Self>) -> Subscription {
        let config = self.factory.config.clone();
        cx.subscribe(&config, |prototype, _, _: &TerminalFontChanged, cx| {
            let font = prototype.factory.shared_font(cx);
            for (_, tab) in prototype.tabs.ordered() {
                tab.view
                    .update(cx, |view, cx| view.set_font(font.clone(), cx));
            }
        })
    }
}
