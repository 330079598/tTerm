use super::actions::KeymapActionId;
use super::chord::{format_chord, parse_chord, serialize_chord};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct KeymapConfig {
    #[serde(default)]
    pub bindings: BTreeMap<String, Option<Vec<String>>>,
}
pub type EffectiveKeymap = BTreeMap<KeymapActionId, Option<Vec<String>>>;
pub fn default_keymap_config() -> KeymapConfig {
    KeymapConfig::default()
}
fn digits(m: &str) -> Vec<String> {
    (1..=9).map(|i| format!("{m}+{i}")).collect()
}
fn defaults(mac: bool) -> BTreeMap<KeymapActionId, Option<Vec<String>>> {
    let mut m = BTreeMap::new();
    macro_rules! s {
        ($a:ident,$v:expr) => {
            m.insert(KeymapActionId::$a, Some(vec![$v.into()]));
        };
    }
    macro_rules! n {
        ($a:ident) => {
            m.insert(KeymapActionId::$a, None);
        };
    }
    s!(WorkspaceNewTab, if mac { "mod+t" } else { "ctrl+shift+t" });
    s!(WorkspaceSplitRight, "mod+\\");
    s!(WorkspaceSplitBelow, "mod+shift+\\");
    s!(
        WorkspaceFocusNextPane,
        if mac { "mod+]" } else { "mod+shift+]" }
    );
    s!(
        WorkspaceFocusPrevPane,
        if mac { "mod+[" } else { "mod+shift+[" }
    );
    s!(WorkspaceFocusPaneLeft, "mod+alt+arrowleft");
    s!(WorkspaceFocusPaneRight, "mod+alt+arrowright");
    s!(WorkspaceFocusPaneUp, "mod+alt+arrowup");
    s!(WorkspaceFocusPaneDown, "mod+alt+arrowdown");
    s!(WorkspaceRestoreSingleGroup, "mod+alt+\\");
    s!(WorkspaceTogglePaneZoom, "mod+alt+enter");
    n!(WorkspaceMovePaneToNewTab);
    n!(WorkspaceChooseLayout);
    s!(CommandLibraryOpen, "mod+shift+p");
    s!(BroadcastTogglePanel, "mod+shift+b");
    s!(TabsNext, "ctrl+tab");
    s!(TabsPrev, "ctrl+shift+tab");
    m.insert(
        KeymapActionId::TabsSwitchToNth,
        Some(if mac { digits("mod") } else { digits("alt") }),
    );
    s!(TabsRenameActive, "mod+shift+r");
    s!(TabsDuplicateActive, "mod+shift+d");
    s!(TabsCloseActive, "mod+shift+w");
    m.insert(
        KeymapActionId::TabsReopenClosed,
        if mac {
            Some(vec!["mod+shift+t".into()])
        } else {
            None
        },
    );
    s!(TabsSaveConnection, "mod+shift+o");
    s!(TerminalFind, if mac { "mod+f" } else { "mod+shift+f" });
    s!(TerminalClear, "mod+shift+k");
    n!(TerminalReset);
    s!(TerminalSaveSelection, "mod+shift+s");
    s!(TerminalFillSavedPassword, "mod+shift+enter");
    m.insert(
        KeymapActionId::TerminalZoomIn,
        Some(vec![
            "mod+=".into(),
            "mod+shift+=".into(),
            "mod+numpadadd".into(),
        ]),
    );
    m.insert(
        KeymapActionId::TerminalZoomOut,
        Some(vec!["mod+-".into(), "mod+numpadsubtract".into()]),
    );
    m.insert(
        KeymapActionId::TerminalZoomReset,
        Some(vec!["mod+0".into(), "mod+numpad0".into()]),
    );
    s!(TerminalPreviousCommand, "mod+arrowup");
    s!(TerminalNextCommand, "mod+arrowdown");
    n!(TerminalCopyLastCommandOutput);
    m.insert(
        KeymapActionId::TerminalCopy,
        if mac {
            None
        } else {
            Some(vec!["mod+shift+c".into()])
        },
    );
    m.insert(
        KeymapActionId::TerminalPaste,
        if mac {
            None
        } else {
            Some(vec!["mod+shift+v".into()])
        },
    );
    s!(TerminalToggleComposer, "mod+shift+.");
    s!(SftpToggle, "mod+shift+e");
    s!(SftpSelectAll, "mod+a");
    s!(SftpFocusPath, "mod+l");
    s!(SftpPasteUpload, "mod+v");
    s!(SftpBack, "alt+arrowleft");
    s!(SftpForward, "alt+arrowright");
    s!(SftpUp, "alt+arrowup");
    n!(ZmodemSendFiles);
    n!(ZmodemReceiveFiles);
    s!(EditorSave, "mod+s");
    m
}
fn normalize_binding(v: &[String]) -> Option<Vec<String>> {
    let out: Vec<_> = v
        .iter()
        .filter_map(|s| parse_chord(s).and_then(|c| serialize_chord(&c)))
        .collect();
    if out.is_empty() { None } else { Some(out) }
}
pub fn resolve_effective_keymap(config: &KeymapConfig, mac: bool) -> EffectiveKeymap {
    let d = defaults(mac);
    KeymapActionId::ALL
        .iter()
        .map(|id| {
            let v = config.bindings.get(id.as_str());
            let e = match v {
                Some(None) => None,
                Some(Some(x)) => normalize_binding(x).or_else(|| d.get(id).cloned().flatten()),
                None => d.get(id).cloned().unwrap_or(None),
            };
            (*id, e)
        })
        .collect()
}
pub fn get_shortcut_label(e: &EffectiveKeymap, id: KeymapActionId, mac: bool) -> Option<String> {
    e.get(&id)
        .and_then(|v| v.as_ref())
        .and_then(|v| v.first())
        .map(|s| format_chord(s, mac))
}
pub fn with_shortcut(label: &str, s: Option<&str>) -> String {
    match s {
        Some(x) => format!("{label} ({x})"),
        None => label.into(),
    }
}
pub fn find_keymap_conflicts(e: &EffectiveKeymap) -> BTreeMap<String, Vec<KeymapActionId>> {
    let mut owners: BTreeMap<String, Vec<KeymapActionId>> = BTreeMap::new();
    for id in KeymapActionId::ALL {
        for c in e.get(id).and_then(|v| v.as_ref()).into_iter().flatten() {
            owners.entry(c.clone()).or_default().push(*id)
        }
    }
    owners.retain(|_, v| v.len() > 1);
    owners
}
pub fn build_chord_index(e: &EffectiveKeymap) -> BTreeMap<String, KeymapActionId> {
    let mut out = BTreeMap::new();
    for id in KeymapActionId::ALL {
        for c in e.get(id).and_then(|v| v.as_ref()).into_iter().flatten() {
            out.entry(c.clone()).or_insert(*id);
        }
    }
    out
}
pub const KNOWN_CLI_CONFLICT_HINTS: &[(&str, &str)] = &[
    ("mod+s", "xoff"),
    ("mod+q", "xon"),
    ("mod+a", "readlineHome"),
    ("mod+e", "readlineEnd"),
    ("mod+u", "readlineKillLineStart"),
    ("mod+k", "readlineKillLineEnd"),
    ("mod+w", "readlineDeleteWord"),
    ("mod+l", "clearScreen"),
    ("mod+r", "historySearch"),
    ("mod+t", "readlineTransposeChars"),
    ("mod+f", "readlineForwardChar"),
    ("mod+b", "tmuxPrefix"),
    ("mod+z", "suspend"),
    ("mod+d", "eof"),
    ("mod+v", "quotedInsert"),
    ("mod+n", "historyNext"),
    ("mod+p", "historyPrevious"),
    ("mod+o", "operateAndGetNext"),
    ("alt+b", "wordBack"),
    ("alt+f", "wordForward"),
    ("alt+d", "wordKill"),
];
pub fn get_cli_conflict_hint(chord: &str, mac: bool) -> Option<&'static str> {
    if mac && chord.starts_with("mod+") {
        return None;
    }
    let normalized = chord.strip_prefix("ctrl+").map(|x| format!("mod+{x}"));
    let key = normalized.as_deref().unwrap_or(chord);
    KNOWN_CLI_CONFLICT_HINTS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
pub fn normalize_keymap(value: Option<&serde_json::Value>) -> KeymapConfig {
    let Some(v) = value.and_then(|v| v.as_object()) else {
        return default_keymap_config();
    };
    let Some(b) = v.get("bindings").and_then(|x| x.as_object()) else {
        return default_keymap_config();
    };
    let mut out = BTreeMap::new();
    for id in KeymapActionId::ALL {
        if let Some(raw) = b.get(id.as_str()) {
            if raw.is_null() {
                out.insert(id.as_str().into(), None);
                continue;
            }
            let arr = raw
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_owned))
                        .collect()
                })
                .or_else(|| raw.as_str().map(|s| vec![s.into()]));
            if let Some(a) = arr.and_then(|a| normalize_binding(&a)) {
                out.insert(id.as_str().into(), Some(a));
            }
        }
    }
    KeymapConfig { bindings: out }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_cover_all_actions_and_platform_overrides() {
        let other = resolve_effective_keymap(&default_keymap_config(), false);
        assert_eq!(other.len(), KeymapActionId::ALL.len());
        assert_eq!(
            other[&KeymapActionId::WorkspaceNewTab],
            Some(vec!["ctrl+shift+t".into()])
        );
        assert_eq!(
            other[&KeymapActionId::TerminalZoomIn]
                .as_ref()
                .unwrap()
                .len(),
            3
        );
        let mac = resolve_effective_keymap(&default_keymap_config(), true);
        assert_eq!(
            mac[&KeymapActionId::WorkspaceNewTab],
            Some(vec!["mod+t".into()])
        );
        assert_eq!(
            mac[&KeymapActionId::TabsSwitchToNth].as_ref().unwrap()[0],
            "mod+1"
        );
        assert_eq!(mac[&KeymapActionId::TerminalCopy], None);
    }
    #[test]
    fn override_null_invalid_and_conflicts_match_ts() {
        let mut c = default_keymap_config();
        c.bindings
            .insert("terminal.find".into(), Some(vec!["SHIFT+mod+J".into()]));
        c.bindings.insert("terminal.clear".into(), None);
        let e = resolve_effective_keymap(&c, false);
        assert_eq!(
            e[&KeymapActionId::TerminalFind],
            Some(vec!["mod+shift+j".into()])
        );
        assert_eq!(e[&KeymapActionId::TerminalClear], None);
        let mut conflict = e.clone();
        conflict.insert(
            KeymapActionId::TerminalReset,
            Some(vec!["mod+shift+j".into()]),
        );
        assert_eq!(find_keymap_conflicts(&conflict)["mod+shift+j"].len(), 2);
        assert_eq!(
            build_chord_index(&e)["mod+shift+j"],
            KeymapActionId::TerminalFind
        );
    }
    #[test]
    fn cli_hints_and_json_normalization_are_stable() {
        assert_eq!(get_cli_conflict_hint("mod+s", false), Some("xoff"));
        assert_eq!(
            get_cli_conflict_hint("ctrl+f", true),
            Some("readlineForwardChar")
        );
        assert_eq!(get_cli_conflict_hint("mod+s", true), None);
        let value = serde_json::json!({"bindings":{"terminal.find":["SHIFT+mod+J"],"terminal.clear":"bad chord","editor.save":null,"unknown.action":["mod+x"]}});
        let normalized = normalize_keymap(Some(&value));
        assert_eq!(
            normalized.bindings["terminal.find"],
            Some(vec!["mod+shift+j".into()])
        );
        assert_eq!(normalized.bindings["editor.save"], None);
        assert!(!normalized.bindings.contains_key("unknown.action"));
    }
}
