use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// User-configurable keyboard shortcut overrides. `bindings` maps action ids
/// to their chord serializations; `None` entries mean "unbound", missing
/// entries fall back to the tTerm defaults on the frontend.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct KeymapConfig {
    #[serde(default, deserialize_with = "deserialize_keymap_bindings")]
    pub bindings: BTreeMap<String, Option<Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum KeymapBindingValue {
    One(String),
    Many(Vec<String>),
}

fn deserialize_keymap_bindings<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, Option<Vec<String>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let bindings = BTreeMap::<String, Option<KeymapBindingValue>>::deserialize(deserializer)?;
    Ok(bindings
        .into_iter()
        .map(|(action, value)| {
            let chords = value.map(|binding| match binding {
                KeymapBindingValue::One(chord) => vec![chord],
                KeymapBindingValue::Many(chords) => chords,
            });
            (action, chords)
        })
        .collect())
}
