//! Routes a keydown through the core application keymap before terminal encoding.
//!
//! The view asks this router once per keydown. A mounted handler must accept
//! the action for the key to be swallowed; otherwise the chord continues to
//! the terminal encoder. P3-04 will mount the real action handlers; the
//! prototype already uses the persisted bindings so the priority is fixed
//! now.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::{KeyDownEvent, Keystroke};
use tterm_core::config::keymap::KeymapConfig as PersistedKeymapConfig;
use tterm_core::keymap::actions::KeymapActionId;
use tterm_core::keymap::chord::{EditableTarget, KeyEventData};
use tterm_core::keymap::config::{KeymapConfig, build_chord_index, resolve_effective_keymap};
use tterm_core::keymap::dispatcher::{
    DispatchEnvironment, DispatchEvent, KeymapHandler, dispatch_keydown_event,
};
use tterm_terminal_view::input::keyboard::EncoderConfig;

/// The mounted keymap of one window.
pub(crate) struct KeymapRouter {
    mac: bool,
    index: BTreeMap<String, KeymapActionId>,
    handlers: RefCell<BTreeMap<KeymapActionId, Vec<KeymapHandler>>>,
}

impl KeymapRouter {
    /// Builds the router from the persisted overrides and platform defaults.
    pub(crate) fn new(config: &PersistedKeymapConfig, mac: bool) -> Self {
        let bindings = KeymapConfig {
            bindings: config.bindings.clone(),
        };
        let index = build_chord_index(&resolve_effective_keymap(&bindings, mac));
        Self {
            mac,
            index,
            handlers: RefCell::new(BTreeMap::new()),
        }
    }

    /// Mounts a handler for an action. Every mounted handler gets the event;
    /// the key is consumed only when at least one accepts it.
    #[cfg(test)]
    pub(crate) fn register(&self, action: KeymapActionId, handler: KeymapHandler) {
        self.handlers
            .borrow_mut()
            .entry(action)
            .or_default()
            .push(handler);
    }

    /// Returns the action when a mounted handler accepted the keydown.
    pub(crate) fn dispatch(&self, event: &KeyDownEvent) -> Option<KeymapActionId> {
        if event.prefer_character_input {
            return None;
        }
        let mut event = DispatchEvent {
            data: key_event_data(&event.keystroke),
            target: EditableTarget {
                form_control: false,
                content_editable: false,
                editable_ancestor: false,
            },
            prevented: false,
            stopped: false,
        };
        let mut handlers = self.handlers.borrow_mut();
        let mut environment = DispatchEnvironment {
            suppressed: false,
            layer_open: false,
            mac: self.mac,
            index: &self.index,
            handlers: &mut handlers,
        };
        dispatch_keydown_event(&mut event, &mut environment)
    }

    /// Whether the application keymap consumes this keydown.
    pub(crate) fn route(&self, event: &KeyDownEvent) -> bool {
        self.dispatch(event).is_some()
    }
}

/// A keydown that the view can hand to the router.
pub(crate) fn route_keymap(router: &Rc<KeymapRouter>, event: &KeyDownEvent) -> bool {
    router.route(event)
}

/// Builds the encoder policy from the persisted macOS Option setting.
///
/// Only macOS reads the setting. Every other platform keeps the encoder's
/// existing non-macOS policy, where Alt is always a Meta prefix.
pub(crate) const fn encoder_config(mac: bool, mac_option_is_meta: bool) -> EncoderConfig {
    if mac {
        EncoderConfig::macos(mac_option_is_meta)
    } else {
        EncoderConfig {
            alt_sends_escape: true,
        }
    }
}

/// The keydown fields the core chord mapper can use.
pub(crate) fn key_event_data(keystroke: &Keystroke) -> KeyEventData<'_> {
    KeyEventData {
        key: keystroke.key.as_str(),
        code: None,
        meta: keystroke.modifiers.platform,
        ctrl: keystroke.modifiers.control,
        alt: keystroke.modifiers.alt,
        shift: keystroke.modifiers.shift,
        composing: false,
    }
}

#[cfg(test)]
mod tests;
