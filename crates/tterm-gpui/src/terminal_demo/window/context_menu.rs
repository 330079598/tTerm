//! The terminal pane's Copy/Paste context menu.
//!
//! The view only reports a right-click. This controller owns the menu entity,
//! its localized labels and the activated actions, so every menu acts on the
//! pane that was clicked. Paste goes through the view's shared clipboard path
//! and therefore through the same confirmation gate as the keyboard, while
//! Copy refuses to replace the clipboard with an empty selection.

use std::rc::Rc;

use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    WeakEntity, Window, div,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{TranslationKey, TranslationRequest};
use tterm_terminal_view::view::TerminalView;
use tterm_terminal_view::view::context_menu::{ContextMenuHandler, ContextMenuRequest};
use tterm_ui::components::context_menu::{ContextMenu, MenuAction};
use tterm_ui::tokens::theme::Theme;

use crate::locale::LocaleGlobal;

#[cfg(test)]
mod tests;

/// The action id of the Copy row.
const COPY: &str = "copy";
/// The action id of the Paste row.
const PASTE: &str = "paste";

/// One pane's open menu and the view its rows act on.
pub(super) struct PaneContextMenu {
    theme: Theme,
    menu: Option<Entity<ContextMenu>>,
}

impl PaneContextMenu {
    /// Creates a closed menu for one pane.
    pub(super) fn new(theme: Theme) -> Self {
        Self { theme, menu: None }
    }

    /// Opens the menu at the click and replaces one this pane already shows.
    ///
    /// A missing target means the pane closed between the click and the
    /// request, so there is nothing to act on and no menu to show.
    pub(super) fn open(
        &mut self,
        request: ContextMenuRequest,
        target: WeakEntity<TerminalView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = target.upgrade() else {
            return;
        };
        if let Some(previous) = self.menu.take() {
            previous.update(cx, |menu, cx| menu.dismiss(window, cx));
        }
        let actions = actions(request.has_selection, cx);
        let origin = (request.position.x.as_f32(), request.position.y.as_f32());
        let theme = self.theme;
        let id = format!("terminal-context-menu-{}", target.entity_id());
        let controller = cx.entity().downgrade();
        let target = target.downgrade();
        let menu = cx.new(|cx| {
            ContextMenu::new(id, origin, actions, theme, window, cx)
                .on_action(move |action, window, cx| activate(action, &target, window, cx))
                .on_close(move |_, cx| {
                    let _ = controller.update(cx, |controller, cx| {
                        // A replacement menu may already be open; only clear
                        // the slot when the menu that closed is still ours.
                        if controller
                            .menu
                            .as_ref()
                            .is_some_and(|menu| !menu.read(cx).is_open())
                        {
                            controller.menu = None;
                            cx.notify();
                        }
                    });
                })
        });
        self.menu = Some(menu);
        cx.notify();
    }

    /// Whether this pane currently shows its menu.
    #[cfg(test)]
    pub(super) fn is_open(&self, cx: &App) -> bool {
        self.menu
            .as_ref()
            .is_some_and(|menu| menu.read(cx).is_open())
    }
}

impl Render for PaneContextMenu {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().children(self.menu.clone())
    }
}

/// Connects one view's right-click to its pane's menu.
pub(super) fn view_handler(menu: Entity<PaneContextMenu>) -> ContextMenuHandler {
    Rc::new(move |request, window, cx| {
        let target = cx.entity().downgrade();
        menu.update(cx, |menu, cx| menu.open(request, target, window, cx));
    })
}

/// Runs one enabled row against the pane that opened the menu.
fn activate(action: &str, target: &WeakEntity<TerminalView>, window: &mut Window, cx: &mut App) {
    let Some(target) = target.upgrade() else {
        return;
    };
    match action {
        COPY => {
            target.update(cx, |view, cx| {
                view.copy_selection(cx);
            });
        }
        PASTE => {
            target.update(cx, |view, cx| view.paste_from_clipboard(window, cx));
        }
        _ => {}
    }
}

/// Builds the two rows this feature promises, in a stable order.
fn actions(has_selection: bool, cx: &App) -> Vec<MenuAction> {
    vec![
        MenuAction {
            label: translated(keys::TERMINAL_CONTEXT_COPY, cx),
            action: COPY.into(),
            icon: None,
            separator: false,
            disabled: !has_selection,
            shortcut: None,
        },
        MenuAction {
            label: translated(keys::TERMINAL_CONTEXT_PASTE, cx),
            action: PASTE.into(),
            icon: None,
            separator: false,
            disabled: false,
            shortcut: None,
        },
    ]
}

/// Reads one label in the window's language, falling back to the key name so
/// a missing catalogue entry is visible instead of blank.
fn translated(key: TranslationKey, cx: &App) -> SharedString {
    LocaleGlobal::get(cx)
        .and_then(|locale| locale.read(cx).translate(TranslationRequest::new(key)).ok())
        .unwrap_or_else(|| key.as_str().to_string())
        .into()
}
