//! Applies the shared config terminal options to every pane, new and existing.
//!
//! The font, cursor style, page padding and clipboard rules live in the strict
//! config file. A pane reads them from the committed [`ConfigState`] snapshot
//! when it is created, and an open pane follows the [`TerminalFontChanged`] event,
//! which is emitted only after the file accepted the values and the snapshot was
//! updated. A failed save therefore leaves every PTY, grid and row cache alone,
//! and an unsaved draft never reaches a pane or a paste already waiting for an
//! answer.
//!
//! The font reaches a pane through
//! [`tterm_terminal_view::view::TerminalView::set_font`], which re-measures the cell,
//! asks the session to resize its PTY and drops the cached rows without
//! touching the model, its scrollback or the session identity. Padding goes
//! through [`tterm_terminal_view::view::TerminalView::set_padding`], which refits only
//! the content rectangle. Clipboard rules go through
//! [`tterm_terminal_view::view::TerminalView::set_clipboard_options`] and the paste
//! controller's confirmation field; neither drops a pending paste or moves its
//! delivery to whichever pane has focus. The cursor shape is the model's
//! default, so a live DECSCUSR override and a hidden cursor still win; resetting
//! the cursor returns to the configured shape. An unchanged value is a no-op, so
//! repeating a commit cannot double-resize a PTY.
//!
//! [`ConfigState`]: crate::config_state::ConfigState

use gpui_kit::{App, Context, Subscription};
use tterm_terminal::snapshot::CursorShape;
use tterm_terminal_view::view::font::TerminalFont;
use tterm_terminal_view::view::layout::TerminalPadding;

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

    /// The page padding a pane opened now starts with.
    ///
    /// The top edge has no setting: the reference client has no top padding.
    pub(super) fn shared_padding(&self, cx: &App) -> TerminalPadding {
        let config = self.config.read(cx).config();
        TerminalPadding::new(
            config.terminal_padding_left_px,
            config.terminal_padding_right_px,
            config.terminal_padding_bottom_px,
        )
    }

    /// The shape a reset cursor returns to in a pane opened now.
    pub(super) fn shared_cursor_shape(&self, cx: &App) -> CursorShape {
        CursorShape::from_config(&self.config.read(cx).config().cursor_style)
    }
}

impl Prototype {
    /// Applies every committed terminal option change to the panes already open.
    ///
    /// The event carries the values the config file now holds, so a save that
    /// fails, is refused while another save is pending, or never happens
    /// cannot change a pane. New panes read the same snapshot in
    /// [`TabFactory::shared_font`], [`TabFactory::shared_padding`] and
    /// [`TabFactory::shared_cursor_shape`]; their open path also reads the
    /// clipboard fields from it, which keeps the two paths in agreement.
    pub(super) fn subscribe_shared_font(&self, cx: &mut Context<Self>) -> Subscription {
        let config = self.factory.config.clone();
        cx.subscribe(
            &config,
            |prototype, _, changed: &TerminalFontChanged, cx| {
                let font = prototype.factory.shared_font(cx);
                let padding = prototype.factory.shared_padding(cx);
                let cursor = prototype.factory.shared_cursor_shape(cx);
                prototype.factory.sessions.set_default_cursor_shape(cursor);
                for (_, tab) in prototype.tabs.ordered() {
                    tab.view.update(cx, |view, cx| {
                        view.set_font(font.clone(), cx);
                        view.set_padding(padding, cx);
                        view.set_clipboard_options(
                            changed.copy_on_select,
                            changed.right_click_paste,
                        );
                        view.notify_settings_changed(cx);
                    });
                    if let Some(controller) = &tab.paste_controller {
                        controller.update(cx, |controller, _| {
                            controller.set_confirm_multiline(changed.confirm_multiline_paste);
                        });
                    }
                }
            },
        )
    }
}
