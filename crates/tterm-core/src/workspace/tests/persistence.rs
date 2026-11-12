//! Serialized pane trees and layouts, compared with the bytes the frontend
//! writes for a saved session.
//!
//! The sample strings come from `scripts/migration/P1-11d1-oracle.mjs`, which
//! runs the real TypeScript modules. `tabGroupsPersistence.ts` itself belongs
//! to the second P1-11d1 group; only the container it writes is mirrored here.

use serde::Serialize;

use crate::workspace::pane_templates::{LayoutShape, PaneLayoutTemplate};
use crate::workspace::tab_groups::{PaneNode, SplitOrientation, normalize_pane_tree, pane_leaf};

/// The current saved layout format, from `tabGroupsPersistence.ts`.
#[derive(Debug, Serialize)]
struct SavedTabGroups {
    version: u32,
    groups: Vec<SavedTabGroup>,
}

/// One saved group; the transient group id is left out.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SavedTabGroup {
    root: PaneNode,
    active_tab_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    zoomed_tab_id: Option<String>,
}

const SESSION_LAYOUT_JSON: &str = r#"{"version":2,"groups":[{"root":{"type":"split","orientation":"horizontal","children":[{"type":"pane","tabId":"a"},{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"b"},{"type":"pane","tabId":"c"}],"sizes":[0.25,0.75]}],"sizes":[0.5,0.5]},"activeTabId":"c","zoomedTabId":"b"},{"root":{"type":"pane","tabId":"d"},"activeTabId":"d"}]}"#;
const TREE_JSON: &str = r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane","tabId":"a"},{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"b"},{"type":"pane","tabId":"c"}],"sizes":[0.25,0.75]}],"sizes":[0.5,0.5]}"#;
const SHAPE_JSON: &str = r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.25,0.75]}],"sizes":[0.5,0.5]}"#;
const NESTED_SHAPE_JSON: &str = r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.5,0.5]}"#;
const TEMPLATE_JSON: &str = r#"[{"id":"t1","name":"Ops","root":{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.5,0.5]}}]"#;

fn saved_tree() -> PaneNode {
    PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        children: vec![
            pane_leaf("a"),
            PaneNode::Split {
                orientation: SplitOrientation::Vertical,
                children: vec![pane_leaf("b"), pane_leaf("c")],
                sizes: vec![0.25, 0.75],
            },
        ],
        sizes: vec![0.5, 0.5],
    }
}

#[test]
fn saved_session_layout_serializes_to_the_typescript_bytes() {
    let saved = SavedTabGroups {
        version: 2,
        groups: vec![
            SavedTabGroup {
                root: saved_tree(),
                active_tab_id: "c".to_owned(),
                zoomed_tab_id: Some("b".to_owned()),
            },
            SavedTabGroup {
                root: pane_leaf("d"),
                active_tab_id: "d".to_owned(),
                zoomed_tab_id: None,
            },
        ],
    };
    assert_eq!(saved.version, 2);
    assert_eq!(saved.groups.len(), 2);
    assert_eq!(saved.groups[0].active_tab_id, "c");
    assert_eq!(saved.groups[0].zoomed_tab_id.as_deref(), Some("b"));
    assert_eq!(saved.groups[1].zoomed_tab_id, None);
    assert_eq!(serde_json::to_string(&saved).unwrap(), SESSION_LAYOUT_JSON);
}

#[test]
fn normalized_pane_tree_serializes_to_the_typescript_bytes() {
    let tree = saved_tree();
    assert_eq!(
        serde_json::to_string(&normalize_pane_tree(&tree)).unwrap(),
        TREE_JSON
    );
}

#[test]
fn layout_shapes_roundtrip_to_the_typescript_bytes() {
    let shape: LayoutShape = serde_json::from_str(SHAPE_JSON).unwrap();
    assert_eq!(serde_json::to_string(&shape).unwrap(), SHAPE_JSON);
    let nested: LayoutShape = serde_json::from_str(NESTED_SHAPE_JSON).unwrap();
    assert_eq!(serde_json::to_string(&nested).unwrap(), NESTED_SHAPE_JSON);
}

#[test]
fn saved_templates_roundtrip_to_the_typescript_bytes() {
    let templates: Vec<PaneLayoutTemplate> = serde_json::from_str(TEMPLATE_JSON).unwrap();
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].id, "t1");
    assert_eq!(templates[0].name, "Ops");
    assert_eq!(serde_json::to_string(&templates).unwrap(), TEMPLATE_JSON);
}

#[test]
fn omitted_zoomed_tab_id_stays_out_of_the_saved_bytes() {
    let saved = SavedTabGroups {
        version: 2,
        groups: vec![SavedTabGroup {
            root: pane_leaf("a"),
            active_tab_id: "a".to_owned(),
            zoomed_tab_id: None,
        }],
    };
    assert_eq!(
        serde_json::to_string(&saved).unwrap(),
        r#"{"version":2,"groups":[{"root":{"type":"pane","tabId":"a"},"activeTabId":"a"}]}"#
    );
}

#[test]
fn non_finite_pane_sizes_are_written_as_null() {
    let tree = PaneNode::Split {
        orientation: SplitOrientation::Vertical,
        children: vec![pane_leaf("a"), pane_leaf("b")],
        sizes: vec![f64::NAN, 0.5],
    };
    assert_eq!(
        serde_json::to_string(&tree).unwrap(),
        r#"{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"a"},{"type":"pane","tabId":"b"}],"sizes":[null,0.5]}"#
    );
}
