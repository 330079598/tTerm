use super::actions::KeymapActionId;
use super::chord::{EditableTarget, KeyEventData, event_to_chord, serialize_chord};
use super::config::build_chord_index;
use std::collections::BTreeMap;
pub struct DispatchEvent<'a> {
    pub data: KeyEventData<'a>,
    pub target: EditableTarget,
    pub prevented: bool,
    pub stopped: bool,
}
pub struct DispatchEnvironment<'a> {
    pub suppressed: bool,
    pub layer_open: bool,
    pub mac: bool,
    pub index: &'a BTreeMap<String, KeymapActionId>,
    pub handlers: &'a BTreeMap<KeymapActionId, usize>,
}
fn terminal_ctrl(k: &str) -> bool {
    (k.len() == 1 && k.chars().next().is_some_and(|c| c.is_ascii_lowercase()))
        || matches!(k, "[" | "]" | "\\")
}
pub fn dispatch_keydown_event(
    event: &mut DispatchEvent<'_>,
    env: &DispatchEnvironment<'_>,
) -> Option<KeymapActionId> {
    if env.suppressed || env.layer_open || event.data.key == "Escape" {
        return None;
    }
    let c = event_to_chord(event.data, env.mac)?;
    let s = serialize_chord(&c)?;
    let mut id = env.index.get(&s).copied();
    if id.is_none() && env.mac && c.ctrl && !c.mod_key && !terminal_ctrl(&c.key) {
        let mut f = c.clone();
        f.ctrl = false;
        f.mod_key = true;
        id = serialize_chord(&f).and_then(|x| env.index.get(&x).copied())
    }
    if id.is_none() && !env.mac && c.mod_key {
        let mut f = c.clone();
        f.mod_key = false;
        f.ctrl = true;
        id = serialize_chord(&f).and_then(|x| env.index.get(&x).copied())
    }
    let id = id?;
    let (_, allow) = id.definition();
    if !allow && event.target.editable {
        return None;
    }
    if !env.handlers.get(&id).is_some_and(|n| *n > 0) {
        return None;
    }
    event.prevented = true;
    event.stopped = true;
    Some(id)
}
pub fn index_for_defaults(mac: bool) -> BTreeMap<String, KeymapActionId> {
    build_chord_index(&super::config::resolve_effective_keymap(
        &super::config::default_keymap_config(),
        mac,
    ))
}
