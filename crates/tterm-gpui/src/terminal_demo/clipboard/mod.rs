//! Paste confirmation and OSC 52 delivery for the GPUI window.
//!
//! Every paste gesture reaches [`PasteController::request`]. Text that may run
//! commands waits in one pending slot while the modal asks the user; no bytes
//! reach the session before confirmation. OSC 52 stores arrive through a
//! bounded slot and are written to the ordinary clipboard on the UI thread
//! only.

pub(super) mod store;

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    AppContext as _, ClipboardItem, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, Task, Window, div,
};
use tterm_core::config::AppConfig;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::dropped_paths::{drop_quoting_for, dropped_paths_text};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{TranslationKey, TranslationRequest};
use tterm_core::paste_guard::{paste_needs_confirmation, summarize_paste};
use tterm_core::terminal::pty::{LocalTerminalShellNameInput, local_terminal_shell_name_service};
use tterm_terminal_view::view::paste::DropText;
use tterm_ui::components::dialog::{Dialog, DialogAction, DialogOptions};
use tterm_ui::tokens::palette::DARK;

use super::binding::TabBinding;
use crate::locale::Locale;
use store::ClipboardWrites;

/// Everything the controller needs to send one approved paste.
pub(super) struct PasteTarget {
    /// The session generation the request came from.
    pub session_nonce: u32,
    /// Whether that generation is still the tab's live session.
    pub is_current: Rc<dyn Fn(u32) -> bool>,
    /// Sends the text through the view's current bracketed-paste mode.
    pub delivery: tterm_terminal_view::view::paste::PasteDelivery,
}

/// One paste waiting for the dialog's answer.
struct PendingPaste {
    /// The text the dialog describes; it is never sent before confirmation.
    text: String,
    target: PasteTarget,
}

/// The window's paste dialog and OSC 52 clipboard writer.
pub(super) struct PasteController {
    locale: Entity<Locale>,
    _locale_subscription: Subscription,
    confirm_multiline: bool,
    pending: Option<PendingPaste>,
    dialog: Option<Entity<Dialog>>,
    dialog_subscription: Option<Subscription>,
    clipboard_task: Option<Task<()>>,
}

impl PasteController {
    /// Creates the controller for one session generation.
    pub(super) fn new(
        cx: &mut Context<Self>,
        locale: Entity<Locale>,
        config: &AppConfig,
        binding: Rc<TabBinding>,
        session_nonce: u32,
    ) -> Self {
        let mut controller = Self::empty(cx, locale, config.confirm_multiline_paste);
        if let Some(session) = binding.session() {
            let binding = Rc::clone(&binding);
            let is_current = Rc::new(move || is_current_session(&binding, session_nonce));
            controller.clipboard_task =
                spawn_clipboard_writes(cx, is_current, session.clipboard_writes());
        }
        controller
    }

    #[cfg(test)]
    pub(super) fn for_test(
        cx: &mut Context<Self>,
        locale: Entity<Locale>,
        confirm_multiline: bool,
    ) -> Self {
        Self::empty(cx, locale, confirm_multiline)
    }

    #[cfg(test)]
    pub(super) fn for_test_with_clipboard(
        cx: &mut Context<Self>,
        locale: Entity<Locale>,
        confirm_multiline: bool,
        writes: Arc<ClipboardWrites>,
    ) -> Self {
        let mut controller = Self::empty(cx, locale, confirm_multiline);
        controller.clipboard_task =
            spawn_clipboard_writes(cx, Rc::new(|| true), Arc::clone(&writes));
        controller
    }

    /// Applies a committed confirmation setting without dropping a paste that
    /// is already waiting for its answer.
    pub(super) fn set_confirm_multiline(&mut self, enabled: bool) {
        self.confirm_multiline = enabled;
    }

    /// The confirmation setting currently in force.
    #[cfg(test)]
    pub(super) fn confirm_multiline(&self) -> bool {
        self.confirm_multiline
    }

    fn empty(cx: &mut Context<Self>, locale: Entity<Locale>, confirm_multiline: bool) -> Self {
        let locale_subscription = cx.observe(&locale, |this, _, cx| this.locale_changed(cx));
        Self {
            locale,
            _locale_subscription: locale_subscription,
            confirm_multiline,
            pending: None,
            dialog: None,
            dialog_subscription: None,
            clipboard_task: None,
        }
    }

    /// Handles one paste request from any gesture.
    ///
    /// `true` means the controller consumed the request, either by opening a
    /// modal or by rejecting a stale target. `false` means the view may deliver
    /// immediately after its mutable borrow ends.
    pub(super) fn request(
        &mut self,
        text: String,
        bracketed: bool,
        target: PasteTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if text.is_empty() {
            return true;
        }
        // A modal locks out every later request, including a single-line one.
        if self.pending.is_some() {
            return true;
        }
        if !paste_needs_confirmation(&text, self.confirm_multiline, bracketed) {
            return !(target.is_current)(target.session_nonce);
        }
        let (title, description, cancel, confirm) = dialog_texts(&self.locale, cx, &text);
        self.pending = Some(PendingPaste { text, target });
        let mut options = DialogOptions::confirm(title, description);
        options.default_action = Some(DialogAction::Confirm);
        options.cancel_text = cancel;
        options.confirm_text = confirm;
        let dialog = cx.new(|cx| Dialog::new(options, DARK, window, cx));
        self.dialog_subscription = Some(cx.observe(&dialog, |this, dialog, cx| {
            this.dialog_changed(dialog, cx);
        }));
        self.dialog = Some(dialog);
        cx.notify();
        true
    }

    fn dialog_changed(&mut self, dialog: Entity<Dialog>, cx: &mut Context<Self>) {
        if dialog.read(cx).is_open() {
            return;
        }
        let action = dialog.read(cx).last_action;
        let pending = self.pending.take();
        self.dialog = None;
        self.dialog_subscription = None;
        if let Some(pending) = pending
            && action == Some(DialogAction::Confirm)
            && (pending.target.is_current)(pending.target.session_nonce)
        {
            (pending.target.delivery)(cx, &pending.text);
        }
        cx.notify();
    }

    fn locale_changed(&mut self, cx: &mut Context<Self>) {
        let (Some(pending), Some(dialog)) = (&self.pending, &self.dialog) else {
            return;
        };
        let (title, description, cancel, confirm) = dialog_texts(&self.locale, cx, &pending.text);
        dialog.update(cx, |dialog, cx| {
            if !dialog.is_open() {
                return;
            }
            dialog.options.title = title;
            dialog.options.description = Some(description);
            dialog.options.cancel_text = cancel;
            dialog.options.confirm_text = confirm;
            cx.notify();
        });
    }
}

impl Render for PasteController {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(self.dialog.clone())
    }
}

fn spawn_clipboard_writes(
    cx: &mut Context<PasteController>,
    is_current: Rc<dyn Fn() -> bool>,
    writes: Arc<ClipboardWrites>,
) -> Option<Task<()>> {
    // The watcher starts even while the setting is off: a committed change can
    // enable this generation later, and the slot itself rejects a store and
    // revokes a pending write while it is disabled.
    let mut watch = writes.subscribe();
    Some(cx.spawn(async move |controller, cx| {
        loop {
            // Check entity lifetime before taking the pending value. A wake-up
            // queued just before disposal must not consume another store.
            let consumed = controller.update(cx, |_, cx| {
                let Some(text) = writes.take() else {
                    return false;
                };
                if writes.enabled() && is_current() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
                true
            });
            match consumed {
                Err(_) => break,
                Ok(true) => continue,
                Ok(false) => {}
            }
            if watch.changed().await.is_err() {
                break;
            }
        }
    }))
}

fn is_current_session(binding: &TabBinding, session_nonce: u32) -> bool {
    binding
        .session()
        .is_some_and(|session| session.session_nonce() == session_nonce && session.is_live())
}

fn dialog_texts(
    locale: &Entity<Locale>,
    cx: &gpui_kit::App,
    text: &str,
) -> (String, String, String, String) {
    let locale = locale.read(cx);
    let summary = summarize_paste(text);
    let title = translated(locale, TranslationRequest::new(keys::PASTE_MULTILINE_TITLE));
    let description = if summary.line_count > 1 {
        translated(
            locale,
            TranslationRequest::new(keys::PASTE_MULTILINE_DESCRIPTION)
                .count(summary.line_count as f64),
        )
    } else {
        translated(
            locale,
            TranslationRequest::new(keys::PASTE_TRAILING_LINE_BREAK_DESCRIPTION),
        )
    };
    (
        title,
        format!("{description}\n\n{}", summary.preview),
        translated(locale, TranslationRequest::new(keys::COMMON_CANCEL)),
        translated(locale, TranslationRequest::new(keys::PASTE_PASTE_ANYWAY)),
    )
}

fn translated(locale: &Locale, request: TranslationRequest<'_>) -> String {
    let key: TranslationKey = request.key;
    locale.translate(request).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate a paste dialog label: {error}");
        key.as_str().to_string()
    })
}

/// Builds the local shell's drop formatter, or `None` for remote sessions.
pub(super) fn local_drop_text(connection: &PtyConnectionOptions) -> Option<DropText> {
    if !matches!(
        connection.connection_type.as_deref(),
        None | Some("terminal")
    ) {
        return None;
    }
    let shell_name = local_terminal_shell_name_service(LocalTerminalShellNameInput {
        shell: connection.terminal_shell.clone(),
        custom_path: connection.terminal_shell_custom_path.clone(),
    });
    let quoting = drop_quoting_for(std::env::consts::OS, shell_name.as_deref());
    Some(Rc::new(move |paths| dropped_paths_text(paths, quoting)))
}

/// Binds every view paste to this controller and session generation.
pub(super) fn view_gate(
    controller: &Entity<PasteController>,
    binding: &Rc<TabBinding>,
    session_nonce: u32,
) -> tterm_terminal_view::view::paste::PasteGate {
    let gate_controller = controller.downgrade();
    let gate_binding = Rc::clone(binding);
    Rc::new(move |request, window, cx| {
        let Some(controller) = gate_controller.upgrade() else {
            return true;
        };
        let tterm_terminal_view::view::paste::PasteRequest {
            text,
            bracketed,
            delivery,
        } = request;
        let binding = Rc::clone(&gate_binding);
        let is_current = Rc::new(move |nonce| {
            binding
                .session()
                .is_some_and(|session| session.session_nonce() == nonce && session.is_live())
        });
        controller.update(cx, |controller, cx| {
            controller.request(
                text,
                bracketed,
                PasteTarget {
                    session_nonce,
                    is_current,
                    delivery,
                },
                window,
                cx,
            )
        })
    })
}
