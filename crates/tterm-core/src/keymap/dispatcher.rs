use super::actions::KeymapActionId;
use super::chord::{Chord, EditableTarget, KeyEventData, event_to_chord, serialize_chord};
use super::config::build_chord_index;
use std::collections::BTreeMap;

/// Mutable keydown state passed to handlers and updated when a handler accepts.
pub struct DispatchEvent<'a> {
    pub data: KeyEventData<'a>,
    pub target: EditableTarget,
    pub prevented: bool,
    pub stopped: bool,
}
/// Result returned by a keymap handler. `true` consumes the key.
pub type HandlerResult = bool;
/// A mounted action handler.
pub type KeymapHandler = Box<dyn FnMut(&mut DispatchEvent<'_>, &Chord) -> HandlerResult>;
/// Runtime state used by the pure dispatcher.
pub struct DispatchEnvironment<'a> {
    pub suppressed: bool,
    pub layer_open: bool,
    pub mac: bool,
    pub index: &'a BTreeMap<String, KeymapActionId>,
    pub handlers: &'a mut BTreeMap<KeymapActionId, Vec<KeymapHandler>>,
}
fn terminal_ctrl(key: &str) -> bool {
    (key.len() == 1
        && key
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase()))
        || matches!(key, "[" | "]" | "\\")
}
/// Resolves and dispatches one keydown event.
pub fn dispatch_keydown_event(
    event: &mut DispatchEvent<'_>,
    env: &mut DispatchEnvironment<'_>,
) -> Option<KeymapActionId> {
    if env.suppressed || env.layer_open || event.data.key == "Escape" {
        return None;
    }
    let chord = event_to_chord(event.data, env.mac)?;
    let serialized = serialize_chord(&chord)?;
    let mut action_id = env.index.get(&serialized).copied();
    if action_id.is_none() && env.mac && chord.ctrl && !chord.mod_key && !terminal_ctrl(&chord.key)
    {
        let mut fallback = chord.clone();
        fallback.ctrl = false;
        fallback.mod_key = true;
        action_id = serialize_chord(&fallback).and_then(|value| env.index.get(&value).copied());
    }
    if action_id.is_none() && !env.mac && chord.mod_key {
        let mut fallback = chord.clone();
        fallback.mod_key = false;
        fallback.ctrl = true;
        action_id = serialize_chord(&fallback).and_then(|value| env.index.get(&value).copied());
    }
    let action_id = action_id?;
    let (_, allow_in_editable) = action_id.definition();
    if !allow_in_editable && super::chord::is_editable_target(event.target) {
        return None;
    }
    let handlers = env.handlers.get_mut(&action_id)?;
    let mut accepted = false;
    for handler in handlers.iter_mut() {
        if handler(event, &chord) {
            accepted = true;
        }
    }
    if !accepted {
        return None;
    }
    event.prevented = true;
    event.stopped = true;
    Some(action_id)
}
/// Builds an index from the platform-specific defaults.
pub fn index_for_defaults(mac: bool) -> BTreeMap<String, KeymapActionId> {
    build_chord_index(&super::config::resolve_effective_keymap(
        &super::config::default_keymap_config(),
        mac,
    ))
}
