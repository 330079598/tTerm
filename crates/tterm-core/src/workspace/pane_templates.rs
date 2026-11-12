//! Split layout templates, from `src/lib/paneTemplates.ts`.

use std::collections::HashSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::js_number::serialize_sizes;
use super::tab_groups::{
    PaneNode, SplitOrientation, TabGroup, find_group_of_tab, get_pane_ids, normalize_pane_tree,
    pane_leaf,
};

/// The shape of a split tree without the tabs its panes show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutShape {
    /// One pane.
    Pane,
    /// A split whose children divide it.
    Split {
        /// The axis the children divide.
        orientation: SplitOrientation,
        /// The panes or splits that make up the shape.
        children: Vec<LayoutShape>,
        /// Each child's share of the split, summing to one.
        #[serde(serialize_with = "serialize_sizes")]
        sizes: Vec<f64>,
    },
}

/// A saved split layout with the name the user gave it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneLayoutTemplate {
    /// The template's unique id.
    pub id: String,
    /// The name the user gave the template.
    pub name: String,
    /// The shape the template applies.
    pub root: LayoutShape,
}

/// A built-in layout and the translation key of its name.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinPaneTemplate {
    /// The id stored when the template is chosen.
    pub id: &'static str,
    /// The translation key of the template's label.
    pub label_key: &'static str,
    /// The built-in's empty name; the picker translates `label_key` instead.
    pub name: &'static str,
    /// The shape the template applies.
    pub root: LayoutShape,
}

/// No template holds more panes than this.
pub const MAX_TEMPLATE_PANES: usize = 9;
const MAX_TEMPLATE_DEPTH: usize = 4;
const MAX_SAVED_TEMPLATES: usize = 30;

fn pane() -> LayoutShape {
    LayoutShape::Pane
}

fn split(
    orientation: SplitOrientation,
    children: Vec<LayoutShape>,
    sizes: Option<Vec<f64>>,
) -> LayoutShape {
    let sizes = sizes.unwrap_or_else(|| vec![1.0 / children.len() as f64; children.len()]);
    LayoutShape::Split {
        orientation,
        children,
        sizes,
    }
}

/// The templates tTerm comes with, in picker order.
pub static BUILTIN_PANE_TEMPLATES: LazyLock<Vec<BuiltinPaneTemplate>> = LazyLock::new(|| {
    vec![
        BuiltinPaneTemplate {
            id: "builtin:columns-2",
            label_key: "columns2",
            name: "",
            root: split(SplitOrientation::Horizontal, vec![pane(), pane()], None),
        },
        BuiltinPaneTemplate {
            id: "builtin:rows-2",
            label_key: "rows2",
            name: "",
            root: split(SplitOrientation::Vertical, vec![pane(), pane()], None),
        },
        BuiltinPaneTemplate {
            id: "builtin:columns-3",
            label_key: "columns3",
            name: "",
            root: split(
                SplitOrientation::Horizontal,
                vec![pane(), pane(), pane()],
                None,
            ),
        },
        BuiltinPaneTemplate {
            id: "builtin:main-right-2",
            label_key: "mainRight2",
            name: "",
            root: split(
                SplitOrientation::Horizontal,
                vec![
                    pane(),
                    split(SplitOrientation::Vertical, vec![pane(), pane()], None),
                ],
                Some(vec![0.6, 0.4]),
            ),
        },
        BuiltinPaneTemplate {
            id: "builtin:main-bottom-2",
            label_key: "mainBottom2",
            name: "",
            root: split(
                SplitOrientation::Vertical,
                vec![
                    pane(),
                    split(SplitOrientation::Horizontal, vec![pane(), pane()], None),
                ],
                Some(vec![0.6, 0.4]),
            ),
        },
        BuiltinPaneTemplate {
            id: "builtin:grid-4",
            label_key: "grid4",
            name: "",
            root: split(
                SplitOrientation::Horizontal,
                vec![
                    split(SplitOrientation::Vertical, vec![pane(), pane()], None),
                    split(SplitOrientation::Vertical, vec![pane(), pane()], None),
                ],
                None,
            ),
        },
    ]
});

/// Whether `id` names one of the built-in templates.
pub fn is_builtin_template_id(id: &str) -> bool {
    id.starts_with("builtin:")
}

/// The number of panes a shape holds.
pub fn count_template_panes(shape: &LayoutShape) -> usize {
    match shape {
        LayoutShape::Pane => 1,
        LayoutShape::Split { children, .. } => children.iter().map(count_template_panes).sum(),
    }
}

/// The shape of a tab's split tree, its panes left out.
pub fn shape_of_tree(node: &PaneNode) -> LayoutShape {
    match node {
        PaneNode::Pane { .. } => LayoutShape::Pane,
        PaneNode::Split {
            orientation,
            children,
            sizes,
        } => LayoutShape::Split {
            orientation: *orientation,
            children: children.iter().map(shape_of_tree).collect(),
            sizes: sizes.clone(),
        },
    }
}

/// Puts `tab_ids` into the shape's panes left to right and top to bottom.
pub fn fill_shape(shape: &LayoutShape, tab_ids: &[String]) -> PaneNode {
    let mut next = 0;
    normalize_pane_tree(&fill(shape, tab_ids, &mut next))
}

fn fill(shape: &LayoutShape, tab_ids: &[String], next: &mut usize) -> PaneNode {
    match shape {
        LayoutShape::Pane => {
            let tab_id = tab_ids.get(*next).cloned().unwrap_or_default();
            *next += 1;
            pane_leaf(tab_id)
        }
        LayoutShape::Split {
            orientation,
            children,
            sizes,
        } => PaneNode::Split {
            orientation: *orientation,
            children: children
                .iter()
                .map(|child| fill(child, tab_ids, next))
                .collect(),
            sizes: sizes.clone(),
        },
    }
}

fn read_shape(value: &Value, depth: usize) -> Option<LayoutShape> {
    let node = value.as_object()?;
    match node.get("type").and_then(Value::as_str) {
        Some("pane") => Some(LayoutShape::Pane),
        Some("split") => {
            if depth >= MAX_TEMPLATE_DEPTH {
                return None;
            }
            let orientation = match node.get("orientation").and_then(Value::as_str) {
                Some("horizontal") => SplitOrientation::Horizontal,
                Some("vertical") => SplitOrientation::Vertical,
                _ => return None,
            };
            let raw_children = node.get("children")?.as_array()?;
            if raw_children.len() < 2 {
                return None;
            }
            let children = raw_children
                .iter()
                .map(|child| read_shape(child, depth + 1))
                .collect::<Option<Vec<_>>>()?;
            let raw_sizes = node
                .get("sizes")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let sizes: Vec<f64> = children
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    raw_sizes
                        .get(index)
                        .and_then(Value::as_f64)
                        .filter(|size| size.is_finite() && *size > 0.0)
                        .unwrap_or(0.0)
                })
                .collect();
            let total = sizes.iter().sum::<f64>();
            let sizes = if total > 0.0 {
                sizes.iter().map(|size| size / total).collect()
            } else {
                vec![1.0 / sizes.len() as f64; sizes.len()]
            };
            Some(LayoutShape::Split {
                orientation,
                children,
                sizes,
            })
        }
        _ => None,
    }
}

/// A saved shape, or `None` when it is malformed, too deep or has too many
/// panes.
pub fn normalize_layout_shape(value: &Value) -> Option<LayoutShape> {
    let shape = read_shape(value, 0)?;
    if shape == LayoutShape::Pane {
        return None;
    }
    if count_template_panes(&shape) > MAX_TEMPLATE_PANES {
        return None;
    }
    Some(shape)
}

/// Trims the whitespace ECMAScript's `trim` removes.
pub(crate) fn js_trim(value: &str) -> String {
    value
        .trim_matches(|character: char| {
            matches!(character,
                '\u{0009}'..='\u{000D}' | '\u{0020}' | '\u{00A0}' | '\u{1680}' |
                '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' |
                '\u{205F}' | '\u{3000}' | '\u{FEFF}')
        })
        .to_owned()
}

/// The saved templates from a config, dropping broken ones, repeated ids and
/// ids reserved for built-ins.
pub fn normalize_pane_templates(value: &Value) -> Vec<PaneLayoutTemplate> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut templates = Vec::new();
    for entry in entries {
        let Some(object) = entry.as_object() else {
            continue;
        };
        let Some(id) = object.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id.is_empty() || is_builtin_template_id(id) || seen.contains(id) {
            continue;
        }
        let Some(root) = object.get("root").and_then(normalize_layout_shape) else {
            continue;
        };
        seen.insert(id.to_owned());
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .map(js_trim)
            .unwrap_or_default();
        templates.push(PaneLayoutTemplate {
            id: id.to_owned(),
            name,
            root,
        });
        if templates.len() >= MAX_SAVED_TEMPLATES {
            break;
        }
    }
    templates
}

/// Lays the group holding `tab_id` out as `shape`. Its panes fill the shape
/// in their order, `new_tab_ids` fill the panes left over, and panes the shape
/// has no room for move out into tabs of their own right after it.
pub fn apply_template_to_groups(
    groups: &[TabGroup],
    tab_id: &str,
    shape: &LayoutShape,
    new_tab_ids: &[String],
    create_group_id: &mut dyn FnMut() -> String,
) -> Vec<TabGroup> {
    let Some(group) = find_group_of_tab(groups, tab_id) else {
        return groups.to_vec();
    };
    let slots = count_template_panes(shape);
    let existing = get_pane_ids(&group.root);
    let mut kept: Vec<String> = if existing.iter().any(|id| id == tab_id) {
        let mut ordered = vec![tab_id.to_owned()];
        ordered.extend(existing.iter().filter(|id| id.as_str() != tab_id).cloned());
        ordered.truncate(slots);
        ordered.sort_by_key(|id| {
            existing
                .iter()
                .position(|existing_id| existing_id == id)
                .unwrap_or(usize::MAX)
        });
        ordered
    } else {
        existing.iter().take(slots).cloned().collect()
    };
    for new_id in new_tab_ids {
        if kept.len() >= slots {
            break;
        }
        kept.push(new_id.clone());
    }
    if kept.len() < slots {
        return groups.to_vec();
    }
    let left_over: Vec<String> = existing
        .iter()
        .filter(|id| !kept.iter().any(|kept_id| kept_id == *id))
        .cloned()
        .collect();
    let index = groups
        .iter()
        .position(|candidate| std::ptr::eq(candidate, group))
        .unwrap_or(0);
    let laid_out = TabGroup {
        id: group.id.clone(),
        root: fill_shape(shape, &kept),
        active_tab_id: tab_id.to_owned(),
        zoomed_tab_id: None,
    };
    let singles: Vec<TabGroup> = left_over
        .into_iter()
        .map(|id| TabGroup {
            id: create_group_id(),
            root: pane_leaf(id.clone()),
            active_tab_id: id,
            zoomed_tab_id: None,
        })
        .collect();
    let mut result = Vec::with_capacity(groups.len() + singles.len());
    result.extend_from_slice(&groups[..index]);
    result.push(laid_out);
    result.extend(singles);
    result.extend_from_slice(&groups[index + 1..]);
    result
}
