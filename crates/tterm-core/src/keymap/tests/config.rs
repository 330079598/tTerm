use crate::keymap::actions::KeymapActionId as A;
use crate::keymap::chord::parse_chord;
use crate::keymap::config::*;
use serde_json::json;

fn defaults(mac: bool) -> EffectiveKeymap {
    resolve_effective_keymap(&default_keymap_config(), mac)
}
#[test]
fn get_shortcut_label_shows_first_chord_in_effect() {
    let d = defaults(false);
    assert_eq!(
        get_shortcut_label(&d, A::TabsRenameActive, true).as_deref(),
        Some("⌘⇧R")
    );
    assert_eq!(
        get_shortcut_label(&d, A::TabsRenameActive, false).as_deref(),
        Some("Ctrl+Shift+R")
    );
    assert_eq!(
        get_shortcut_label(&d, A::TerminalZoomIn, false).as_deref(),
        Some("Ctrl+=")
    );
    let mut c = default_keymap_config();
    c.bindings
        .insert("tabs.renameActive".into(), Some(vec!["mod+e".into()]));
    assert_eq!(
        get_shortcut_label(&defaults_from(&c, false), A::TabsRenameActive, true).as_deref(),
        Some("⌘E")
    );
}
fn defaults_from(c: &KeymapConfig, mac: bool) -> EffectiveKeymap {
    resolve_effective_keymap(c, mac)
}
#[test]
fn get_shortcut_label_shows_nothing_for_unbound_action() {
    let mut c = default_keymap_config();
    c.bindings.insert("tabs.closeActive".into(), None);
    let e = defaults_from(&c, true);
    assert_eq!(get_shortcut_label(&e, A::TabsCloseActive, true), None);
    assert_eq!(get_shortcut_label(&e, A::TerminalReset, true), None);
}
#[test]
fn with_shortcut_appends_only_when_there_is_one() {
    assert_eq!(with_shortcut("New tab", Some("⌃⇧T")), "New tab (⌃⇧T)");
    assert_eq!(with_shortcut("New tab", None), "New tab");
}
#[test]
fn resolve_effective_keymap_provides_a_default_for_every_action() {
    let e = defaults(false);
    assert_eq!(e.len(), A::ALL.len());
    assert_eq!(e[&A::WorkspaceSplitRight], Some(vec!["mod+\\".into()]));
    assert_eq!(e[&A::WorkspaceNewTab], Some(vec!["ctrl+shift+t".into()]));
    assert_eq!(e[&A::SftpToggle], Some(vec!["mod+shift+e".into()]));
    assert_eq!(e[&A::TerminalFind], Some(vec!["mod+shift+f".into()]));
    assert_eq!(e[&A::CommandLibraryOpen], Some(vec!["mod+shift+p".into()]));
    assert_eq!(e[&A::TabsNext], Some(vec!["ctrl+tab".into()]));
}
#[test]
fn resolve_effective_keymap_binds_font_zoom_actions() {
    let e = defaults(false);
    assert_eq!(
        e[&A::TerminalZoomIn],
        Some(vec![
            "mod+=".into(),
            "mod+shift+=".into(),
            "mod+numpadadd".into()
        ])
    );
    assert_eq!(
        e[&A::TerminalZoomOut],
        Some(vec!["mod+-".into(), "mod+numpadsubtract".into()])
    );
    assert_eq!(
        e[&A::TerminalZoomReset],
        Some(vec!["mod+0".into(), "mod+numpad0".into()])
    );
}
#[test]
fn resolve_effective_keymap_binds_command_jumps_and_leaves_copy_output_unbound() {
    let e = defaults(false);
    assert_eq!(
        e[&A::TerminalPreviousCommand],
        Some(vec!["mod+arrowup".into()])
    );
    assert_eq!(
        e[&A::TerminalNextCommand],
        Some(vec!["mod+arrowdown".into()])
    );
    assert_eq!(e[&A::TerminalCopyLastCommandOutput], None);
}
#[test]
fn resolve_effective_keymap_copies_and_pastes_with_shift_elsewhere() {
    let o = defaults(false);
    assert_eq!(o[&A::TerminalCopy], Some(vec!["mod+shift+c".into()]));
    assert_eq!(o[&A::TerminalPaste], Some(vec!["mod+shift+v".into()]));
    let m = defaults(true);
    assert_eq!(m[&A::TerminalCopy], None);
    assert_eq!(m[&A::TerminalPaste], None);
}
#[test]
fn resolve_effective_keymap_supports_multiple_chords_for_nth_tab_switching() {
    let e = defaults(false);
    assert_eq!(e[&A::TabsSwitchToNth].as_ref().unwrap().len(), 9);
    assert_eq!(e[&A::TabsSwitchToNth].as_ref().unwrap()[0], "alt+1");
}
#[test]
fn resolve_effective_keymap_switches_panes_by_platform() {
    let m = defaults(true);
    assert_eq!(m[&A::WorkspaceFocusPrevPane], Some(vec!["mod+[".into()]));
    assert_eq!(m[&A::WorkspaceFocusNextPane], Some(vec!["mod+]".into()]));
    assert_eq!(
        m[&A::WorkspaceFocusPaneLeft],
        Some(vec!["mod+alt+arrowleft".into()])
    );
    let o = defaults(false);
    assert_eq!(
        o[&A::WorkspaceFocusPrevPane],
        Some(vec!["mod+shift+[".into()])
    );
    assert_eq!(
        o[&A::WorkspaceFocusNextPane],
        Some(vec!["mod+shift+]".into()])
    );
    assert_eq!(
        o[&A::WorkspaceFocusPaneDown],
        Some(vec!["mod+alt+arrowdown".into()])
    );
    assert_eq!(o[&A::SftpBack], Some(vec!["alt+arrowleft".into()]));
}
#[test]
fn resolve_effective_keymap_restores_single_group_with_mod_alt_backslash() {
    assert_eq!(
        defaults(false)[&A::WorkspaceRestoreSingleGroup],
        Some(vec!["mod+alt+\\".into()])
    );
}
#[test]
fn resolve_effective_keymap_switches_tabs_with_cmd_digits_on_mac() {
    assert_eq!(
        defaults(true)[&A::TabsSwitchToNth].as_ref().unwrap(),
        &vec![
            "mod+1", "mod+2", "mod+3", "mod+4", "mod+5", "mod+6", "mod+7", "mod+8", "mod+9"
        ]
    );
}
#[test]
fn resolve_effective_keymap_opens_tabs_and_searches_on_mac() {
    let e = defaults(true);
    assert_eq!(e[&A::WorkspaceNewTab], Some(vec!["mod+t".into()]));
    assert_eq!(e[&A::TabsCloseActive], Some(vec!["mod+shift+w".into()]));
    assert_eq!(e[&A::TerminalFind], Some(vec!["mod+f".into()]));
    assert!(find_keymap_conflicts(&e).is_empty());
}
#[test]
fn resolve_effective_keymap_allows_explicit_override() {
    let mut c = default_keymap_config();
    c.bindings
        .insert("terminal.find".into(), Some(vec!["mod+j".into()]));
    assert_eq!(
        defaults_from(&c, false)[&A::TerminalFind],
        Some(vec!["mod+j".into()])
    );
}
#[test]
fn resolve_effective_keymap_allows_explicit_null_unbinding() {
    let mut c = default_keymap_config();
    c.bindings.insert("terminal.find".into(), None);
    assert_eq!(defaults_from(&c, false)[&A::TerminalFind], None);
}
#[test]
fn resolve_effective_keymap_falls_back_for_unparseable_override() {
    let mut c = default_keymap_config();
    c.bindings
        .insert("terminal.find".into(), Some(vec!["not a chord".into()]));
    assert_eq!(
        defaults_from(&c, false)[&A::TerminalFind],
        Some(vec!["mod+shift+f".into()])
    );
}
#[test]
fn find_keymap_conflicts_reports_chords_bound_to_multiple_actions() {
    let mut e = defaults(false);
    e.insert(A::TerminalClear, Some(vec!["mod+shift+f".into()]));
    assert_eq!(
        find_keymap_conflicts(&e)["mod+shift+f"],
        vec![A::TerminalFind, A::TerminalClear]
    );
}
#[test]
fn find_keymap_conflicts_returns_empty_for_defaults() {
    assert!(find_keymap_conflicts(&defaults(false)).is_empty());
    assert!(find_keymap_conflicts(&defaults(true)).is_empty());
}
#[test]
fn find_keymap_conflicts_does_not_flag_multiple_chords_on_one_action() {
    assert!(find_keymap_conflicts(&defaults(false)).is_empty());
}
#[test]
fn build_chord_index_maps_each_chord_to_its_action() {
    let i = build_chord_index(&defaults(false));
    assert_eq!(i["mod+shift+f"], A::TerminalFind);
    assert_eq!(i["ctrl+shift+t"], A::WorkspaceNewTab);
    assert_eq!(i["mod+shift+e"], A::SftpToggle);
    assert!(!i.contains_key("ctrl+t"));
    assert_eq!(i["alt+1"], A::TabsSwitchToNth);
    assert_eq!(i["mod+shift+p"], A::CommandLibraryOpen);
}
#[test]
fn normalize_keymap_returns_defaults_for_non_object_input() {
    assert_eq!(normalize_keymap(None), default_keymap_config());
    assert_eq!(
        normalize_keymap(Some(&json!(null))),
        default_keymap_config()
    );
}
#[test]
fn normalize_keymap_keeps_valid_and_drops_invalid_bindings() {
    let v = json!({"bindings":{"terminal.find":["mod+j"],"terminal.clear":"bad chord","editor.save":null,"unknown.action":["mod+x"]}});
    let n = normalize_keymap(Some(&v));
    assert_eq!(n.bindings["terminal.find"], Some(vec!["mod+j".into()]));
    assert!(!n.bindings.contains_key("terminal.clear"));
    assert_eq!(n.bindings["editor.save"], None);
    assert!(!n.bindings.contains_key("unknown.action"));
}
#[test]
fn normalize_keymap_drops_empty_invalid_overrides_and_preserves_unbinding() {
    let v = json!({"bindings":{"terminal.find":[],"terminal.clear":["not a chord"],"editor.save":null}});
    let n = normalize_keymap(Some(&v));
    assert!(!n.bindings.contains_key("terminal.find"));
    assert!(!n.bindings.contains_key("terminal.clear"));
    assert_eq!(n.bindings["editor.save"], None);
}
#[test]
fn normalize_keymap_canonicalizes_chord_serializations() {
    let v = json!({"bindings":{"terminal.find":["SHIFT+mod+J"]}});
    assert_eq!(
        normalize_keymap(Some(&v)).bindings["terminal.find"],
        Some(vec!["mod+shift+j".into()])
    );
}
#[test]
fn get_cli_conflict_hint_returns_known_readline_and_tmux_ids() {
    assert_eq!(get_cli_conflict_hint("mod+s", false), Some("xoff"));
    assert_eq!(get_cli_conflict_hint("mod+b", false), Some("tmuxPrefix"));
    assert_eq!(get_cli_conflict_hint("mod+a", false), Some("readlineHome"));
    assert_eq!(get_cli_conflict_hint("ctrl+s", true), Some("xoff"));
    assert_eq!(get_cli_conflict_hint("mod+s", true), None);
}
#[test]
fn get_cli_conflict_hint_warns_for_ctrl_t_and_ctrl_f_readline_chords() {
    assert_eq!(
        get_cli_conflict_hint("mod+t", false),
        Some("readlineTransposeChars")
    );
    assert_eq!(
        get_cli_conflict_hint("ctrl+f", true),
        Some("readlineForwardChar")
    );
}
#[test]
fn get_cli_conflict_hint_returns_none_for_unknown_chords() {
    assert_eq!(get_cli_conflict_hint("mod+shift+k", false), None);
}

#[test]
fn find_keymap_conflicts_deduplicates_repeated_chords_on_one_action() {
    let mut config = default_keymap_config();
    config.bindings.insert(
        "terminal.find".into(),
        Some(vec!["mod+j".into(), "MOD+J".into()]),
    );
    let effective = resolve_effective_keymap(&config, false);
    assert!(!find_keymap_conflicts(&effective).contains_key("mod+j"));
}

#[test]
fn keymap_config_deserializes_and_serializes_legacy_single_string_bindings_byte_for_byte() {
    let source = r#"{"bindings":{"terminal.find":"mod+j","editor.save":["mod+s"]}}"#;
    let config: KeymapConfig = serde_json::from_str(source).expect("legacy keymap JSON parses");
    let encoded = serde_json::to_string(&config).expect("keymap JSON serializes");
    assert_eq!(
        encoded,
        r#"{"bindings":{"editor.save":["mod+s"],"terminal.find":["mod+j"]}}"#
    );
}

#[test]
fn parse_chord_uses_ecmascript_whitespace_trim() {
    assert_eq!(
        parse_chord("\u{FEFF}mod+a"),
        Some(crate::keymap::chord::Chord {
            key: "a".into(),
            mod_key: true,
            ..Default::default()
        })
    );
    assert_eq!(parse_chord("mod+\u{85}a"), None);
}

#[test]
fn with_shortcut_treats_empty_shortcut_as_missing() {
    assert_eq!(with_shortcut("L", Some("")), "L");
}

#[test]
fn action_groups_preserve_order_strings_and_members() {
    use crate::keymap::actions::{
        KEYMAP_ACTION_GROUPS, KeymapActionGroup, action_group_id, get_actions_by_group,
    };
    assert_eq!(
        KEYMAP_ACTION_GROUPS,
        &[
            KeymapActionGroup::Workspace,
            KeymapActionGroup::Tabs,
            KeymapActionGroup::Terminal,
            KeymapActionGroup::Sftp,
            KeymapActionGroup::Zmodem,
            KeymapActionGroup::Editor
        ]
    );
    assert_eq!(action_group_id(KeymapActionGroup::Workspace), "workspace");
    assert_eq!(
        get_actions_by_group(KeymapActionGroup::Sftp),
        vec![
            A::SftpToggle,
            A::SftpSelectAll,
            A::SftpFocusPath,
            A::SftpPasteUpload,
            A::SftpBack,
            A::SftpForward,
            A::SftpUp
        ]
    );
}
