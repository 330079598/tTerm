//! Serialized pane trees and layouts, compared with the bytes the frontend
//! writes for a saved session.
//!
//! The sample strings come from `scripts/migration/P1-11d1-oracle.mjs`, which
//! runs the real TypeScript modules. `tabGroupsPersistence.ts` itself belongs
//! to the second P1-11d1 group; only the container it writes is mirrored here.

use serde::{Deserialize, Serialize};

use crate::workspace::pane_templates::{LayoutShape, PaneLayoutTemplate};
use crate::workspace::tab_groups::{PaneNode, normalize_pane_tree};

/// The current saved layout format, from `tabGroupsPersistence.ts`.
#[derive(Debug, Serialize, Deserialize)]
struct SavedTabGroups {
    version: u32,
    groups: Vec<SavedTabGroup>,
}

/// One saved group; the transient group id is left out.
#[derive(Debug, Serialize, Deserialize)]
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

#[test]
fn saved_session_layout_roundtrips_to_the_typescript_bytes() {
    let saved: SavedTabGroups = serde_json::from_str(SESSION_LAYOUT_JSON).unwrap();
    assert_eq!(saved.version, 2);
    assert_eq!(saved.groups.len(), 2);
    assert_eq!(saved.groups[0].active_tab_id, "c");
    assert_eq!(saved.groups[0].zoomed_tab_id.as_deref(), Some("b"));
    assert_eq!(saved.groups[1].zoomed_tab_id, None);
    assert_eq!(serde_json::to_string(&saved).unwrap(), SESSION_LAYOUT_JSON);
}

#[test]
fn normalized_pane_tree_roundtrips_to_the_typescript_bytes() {
    let tree: PaneNode = serde_json::from_str(TREE_JSON).unwrap();
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
fn reading_accepts_the_null_and_unknown_fields_the_frontend_reads() {
    let legacy = r#"{"version":2,"unknown":"kept by neither side","groups":[{"root":{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"a","extra":1},{"type":"pane","tabId":"b"}],"sizes":[0.5,0.5],"extra":true},"activeTabId":"a","zoomedTabId":null,"extra":[]}]}"#;
    let saved: SavedTabGroups = serde_json::from_str(legacy).unwrap();
    assert_eq!(saved.groups[0].active_tab_id, "a");
    assert_eq!(saved.groups[0].zoomed_tab_id, None);
    assert_eq!(
        serde_json::to_string(&saved).unwrap(),
        r#"{"version":2,"groups":[{"root":{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"a"},{"type":"pane","tabId":"b"}],"sizes":[0.5,0.5]},"activeTabId":"a"}]}"#
    );
}
