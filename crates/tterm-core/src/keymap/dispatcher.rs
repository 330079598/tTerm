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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatches_and_preserves_terminal_control_keys() {
        let index = index_for_defaults(false);
        let mut handlers = BTreeMap::new();
        handlers.insert(KeymapActionId::CommandLibraryOpen, 1);
        let env = DispatchEnvironment {
            suppressed: false,
            layer_open: false,
            mac: false,
            index: &index,
            handlers: &handlers,
        };
        let mut event = DispatchEvent {
            data: KeyEventData {
                key: "p",
                code: Some("KeyP"),
                meta: false,
                ctrl: true,
                alt: false,
                shift: true,
                composing: false,
            },
            target: EditableTarget { editable: false },
            prevented: false,
            stopped: false,
        };
        assert_eq!(
            dispatch_keydown_event(&mut event, &env),
            Some(KeymapActionId::CommandLibraryOpen)
        );
        assert!(event.prevented);
        let mac_index = index_for_defaults(true);
        let mac_env = DispatchEnvironment {
            mac: true,
            index: &mac_index,
            ..env
        };
        let mut ctrl = DispatchEvent {
            data: KeyEventData {
                key: "f",
                code: Some("KeyF"),
                meta: false,
                ctrl: true,
                alt: false,
                shift: false,
                composing: false,
            },
            target: EditableTarget { editable: false },
            prevented: false,
            stopped: false,
        };
        assert_eq!(dispatch_keydown_event(&mut ctrl, &mac_env), None);
        assert!(!ctrl.prevented);
    }
}
