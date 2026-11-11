use super::*;
use serde_json::json;

fn set_with_rules(patterns: &[&str]) -> HighlightSet {
    HighlightSet {
        id: "set".into(),
        name: "Set".into(),
        rules: patterns
            .iter()
            .map(|pattern| HighlightRule {
                id: (*pattern).into(),
                pattern: (*pattern).into(),
                ..new_highlight_rule()
            })
            .collect(),
    }
}

#[test]
fn resolve_uses_builtin_until_user_sets() {
    assert_eq!(
        resolve_highlight_set(&default_keyword_highlight_config(), None)
            .unwrap()
            .id,
        BUILTIN_HIGHLIGHT_SET_ID
    );
}
#[test]
fn resolve_follows_default_or_choice_or_off() {
    let config = KeywordHighlightConfig {
        default_set: "servers".into(),
        sets: Some(vec![
            HighlightSet {
                id: "network".into(),
                ..Default::default()
            },
            HighlightSet {
                id: "servers".into(),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert_eq!(
        resolve_highlight_set(&config, Some(HIGHLIGHT_SET_FOLLOW_DEFAULT))
            .unwrap()
            .id,
        "servers"
    );
    assert_eq!(resolve_highlight_set(&config, None).unwrap().id, "servers");
    assert_eq!(
        resolve_highlight_set(&config, Some("network")).unwrap().id,
        "network"
    );
    assert!(resolve_highlight_set(&config, Some(HIGHLIGHT_SET_OFF)).is_none());
}
#[test]
fn resolve_falls_back_to_default_when_chosen_deleted() {
    let config = KeywordHighlightConfig {
        default_set: "servers".into(),
        sets: Some(vec![HighlightSet {
            id: "servers".into(),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert_eq!(
        resolve_highlight_set(&config, Some("network")).unwrap().id,
        "servers"
    );
}
#[test]
fn resolve_falls_back_to_first_when_default_deleted() {
    let config = KeywordHighlightConfig {
        default_set: "gone".into(),
        sets: Some(vec![HighlightSet {
            id: "network".into(),
            ..Default::default()
        }]),
        ..Default::default()
    };
    assert_eq!(resolve_highlight_set(&config, None).unwrap().id, "network");
}
#[test]
fn resolve_off_config_returns_none() {
    let config = KeywordHighlightConfig {
        enabled: false,
        ..Default::default()
    };
    assert!(resolve_highlight_set(&config, Some("network")).is_none());
}
#[test]
fn normalize_defaults() {
    assert_eq!(
        normalize_keyword_highlight_config(&Value::Null),
        default_keyword_highlight_config()
    );
    assert!(
        normalize_keyword_highlight_config(&json!({"sets":"nonsense"}))
            .sets
            .is_none()
    );
}
#[test]
fn normalize_keeps_valid_and_drops_invalid() {
    let config = normalize_keyword_highlight_config(
        &json!({"enabled":false,"default_set":"net","alternate_screen":true,"sets":[{"id":"net","name":"Network","rules":[{"id":"a","pattern":"down","kind":"regex","foreground":"brightRed","background":"#ABCDEF"},{"id":"b","pattern":"up","foreground":"rgb(1, 2, 3)"},{"pattern":"no id"},"nonsense"]},{"name":"no id"}]}),
    );
    assert!(!config.enabled);
    assert!(config.alternate_screen);
    assert_eq!(
        config.sets.unwrap(),
        vec![HighlightSet {
            id: "net".into(),
            name: "Network".into(),
            rules: vec![
                HighlightRule {
                    id: "a".into(),
                    enabled: true,
                    pattern: "down".into(),
                    kind: "regex".into(),
                    case_sensitive: false,
                    whole_word: false,
                    foreground: Some("brightRed".into()),
                    background: Some("#abcdef".into())
                },
                HighlightRule {
                    id: "b".into(),
                    enabled: true,
                    pattern: "up".into(),
                    kind: "keyword".into(),
                    case_sensitive: false,
                    whole_word: false,
                    foreground: None,
                    background: None
                }
            ],
        }]
    );
}
#[test]
fn normalize_caps_rules() {
    let rules = (0..MAX_HIGHLIGHT_RULES + 5)
        .map(|index| json!({"id":format!("r{index}"),"pattern":"x"}))
        .collect::<Vec<_>>();
    assert_eq!(
        normalize_keyword_highlight_config(&json!({"sets":[{"id":"s","rules":rules}]}))
            .sets
            .unwrap()[0]
            .rules
            .len(),
        MAX_HIGHLIGHT_RULES
    );
}
#[test]
fn edit_saves_builtin() {
    let config = update_highlight_set(
        &default_keyword_highlight_config(),
        BUILTIN_HIGHLIGHT_SET_ID,
        |set| HighlightSet {
            rules: vec![],
            ..set.clone()
        },
    );
    assert_eq!(
        config.sets.unwrap(),
        vec![HighlightSet {
            id: BUILTIN_HIGHLIGHT_SET_ID.into(),
            name: String::new(),
            rules: vec![]
        }]
    );
}
#[test]
fn edit_copies_set_with_new_rule_ids() {
    let builtin = builtin_highlight_set();
    let (config, id) = add_highlight_set(
        &default_keyword_highlight_config(),
        "Copy".into(),
        Some(&builtin),
    )
    .unwrap();
    let copy = config
        .sets
        .unwrap()
        .into_iter()
        .find(|set| set.id == id)
        .unwrap();
    assert_eq!(copy.name, "Copy");
    assert_eq!(
        copy.rules
            .iter()
            .map(|r| r.pattern.clone())
            .collect::<Vec<_>>(),
        builtin
            .rules
            .iter()
            .map(|r| r.pattern.clone())
            .collect::<Vec<_>>()
    );
    assert_ne!(copy.rules[0].id, builtin.rules[0].id);
}
#[test]
fn edit_caps_sets() {
    let sets = (0..MAX_HIGHLIGHT_SETS)
        .map(|index| HighlightSet {
            id: format!("s{index}"),
            ..Default::default()
        })
        .collect();
    assert!(
        add_highlight_set(
            &KeywordHighlightConfig {
                sets: Some(sets),
                ..Default::default()
            },
            "One more".into(),
            None
        )
        .is_none()
    );
}
#[test]
fn edit_deletes_default_and_last_guard() {
    let config = KeywordHighlightConfig {
        default_set: "b".into(),
        sets: Some(vec![
            HighlightSet {
                id: "a".into(),
                ..set_with_rules(&[])
            },
            HighlightSet {
                id: "b".into(),
                name: "B".into(),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    let deleted = delete_highlight_set(&config, "b");
    assert_eq!(deleted.default_set, "a");
    assert_eq!(
        deleted
            .sets
            .as_ref()
            .unwrap()
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>(),
        vec!["a"]
    );
    assert_eq!(delete_highlight_set(&deleted, "a"), deleted);
}
#[test]
fn edit_restores_only_builtin() {
    let emptied = HighlightSet {
        rules: vec![],
        ..builtin_highlight_set()
    };
    assert_eq!(
        restore_builtin_rules(&emptied).rules,
        builtin_highlight_set().rules
    );
    let other = set_with_rules(&["x"]);
    assert_eq!(restore_builtin_rules(&other), other);
}
#[test]
fn edit_add_update_remove_rules() {
    let mut set = add_highlight_rule(
        &set_with_rules(&["a"]),
        HighlightRule {
            id: "b".into(),
            ..new_highlight_rule()
        },
    );
    set = update_highlight_rule(
        &set,
        "b",
        HighlightRuleChanges {
            pattern: Some("down".into()),
            foreground: Some(Some("#ff0000".into())),
            ..Default::default()
        },
    );
    assert_eq!(set.rules[1].pattern, "down");
    assert_eq!(
        remove_highlight_rule(&set, "a")
            .rules
            .iter()
            .map(|r| r.id.clone())
            .collect::<Vec<_>>(),
        vec!["b"]
    );
}
#[test]
fn edit_moves_with_bounds() {
    let set = set_with_rules(&["a", "b", "c"]);
    let ids = |m: HighlightSet| m.rules.into_iter().map(|r| r.id).collect::<Vec<_>>();
    assert_eq!(ids(move_highlight_rule(&set, "c", -1)), vec!["a", "c", "b"]);
    assert_eq!(ids(move_highlight_rule(&set, "a", 1)), vec!["b", "a", "c"]);
    assert_eq!(move_highlight_rule(&set, "a", -1), set);
    assert_eq!(move_highlight_rule(&set, "c", 1), set);
}
#[test]
fn config_roundtrip_matches_backend_shape() {
    let config = KeywordHighlightConfig {
        sets: Some(vec![HighlightSet {
            id: "net".into(),
            name: "Network".into(),
            rules: vec![HighlightRule {
                id: "r1".into(),
                pattern: "down".into(),
                foreground: Some("#ff0000".into()),
                ..Default::default()
            }],
        }]),
        default_set: "net".into(),
        ..Default::default()
    };
    let serialized = serde_json::to_string(&config).unwrap();
    assert_eq!(
        serialized,
        r##"{"enabled":true,"default_set":"net","alternate_screen":false,"sets":[{"id":"net","name":"Network","rules":[{"id":"r1","enabled":true,"pattern":"down","kind":"keyword","case_sensitive":false,"whole_word":false,"foreground":"#ff0000","background":null}]}]}"##
    );
    assert_eq!(
        serde_json::from_str::<KeywordHighlightConfig>(&serialized).unwrap(),
        config
    );
}

#[test]
fn new_ids_are_uuid_and_do_not_collide_with_saved_ids() {
    let saved = HighlightRule {
        id: "rust-highlight-1".into(),
        ..Default::default()
    };
    let created = new_highlight_rule();
    assert_ne!(created.id, saved.id);
    assert_eq!(created.id.len(), 36);
}
#[test]
fn moving_more_than_128_rules_does_not_overflow() {
    let rules = (0..130)
        .map(|index| HighlightRule {
            id: format!("r{index}"),
            ..Default::default()
        })
        .collect();
    let set = HighlightSet {
        id: "s".into(),
        name: "S".into(),
        rules,
    };
    let moved = move_highlight_rule(&set, "r129", -1);
    assert_eq!(moved.rules[128].id, "r129");
}

#[test]
fn copied_and_restored_builtin_rules_use_the_terminal_source() {
    use tterm_terminal::keyword_highlight::{
        HighlightPalette, compile_highlight_set, match_highlights,
    };
    let builtin = builtin_highlight_set();
    let (config, id) = add_highlight_set(
        &KeywordHighlightConfig::default(),
        "Copy".into(),
        Some(&builtin),
    )
    .unwrap();
    let copy = config
        .sets
        .as_ref()
        .unwrap()
        .iter()
        .find(|set| set.id == id)
        .unwrap();
    let restored = restore_builtin_rules(&HighlightSet {
        rules: vec![],
        ..builtin.clone()
    });
    for set in [copy, &restored] {
        let compiled = compile_highlight_set(set, &HighlightPalette::new());
        for (text, expected) in [
            ("10.0.0.0/33", vec![(0, 8)]),
            ("Add::add", vec![]),
            ("x10.0.0.1", vec![]),
            ("1.2.3.4.5", vec![]),
        ] {
            assert_eq!(
                match_highlights(text, &compiled)
                    .iter()
                    .map(|span| (span.start, span.end))
                    .collect::<Vec<_>>(),
                expected,
                "{text}"
            );
        }
    }
}
