//! Canonical session JSON and immutable typed projections.

use super::{ConnectionFields, Tab, TabConnection, TabFields, TabType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub(super) fn stringify(value: &Value) -> Result<String, serde_json::Error> {
    match value {
        Value::Number(number) => Ok(crate::workspace::js_number::format_f64(
            number.as_f64().unwrap_or(f64::NAN),
        )),
        Value::Array(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(stringify)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        )),
        Value::Object(fields) => {
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by_key(|(key, _)| {
                key.parse::<u32>()
                    .ok()
                    .filter(|index| *index != u32::MAX && index.to_string() == **key)
                    .map_or((1, 0), |index| (0, index))
            });
            let entries = entries
                .into_iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}:{}",
                        serde_json::to_string(key)?,
                        stringify(value)?
                    ))
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()?;
            Ok(format!("{{{}}}", entries.join(",")))
        }
        _ => serde_json::to_string(value),
    }
}

pub(super) fn read<T: serde::de::DeserializeOwned>(
    fields: &Map<String, Value>,
    key: &str,
) -> Option<T> {
    fields
        .get(key)
        .filter(|value| !value.is_null())
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
}

impl TabConnection {
    pub(super) fn from_fields(fields: Map<String, Value>) -> Self {
        let mut view = ConnectionFields::default();
        super::fields::read_connection(&mut view, &fields);
        Self {
            view,
            raw_fields: fields,
        }
    }
}

impl Tab {
    pub(super) fn from_fields(fields: Map<String, Value>) -> Self {
        let mut view = TabFields {
            id: read::<String>(&fields, "id").unwrap_or_default(),
            title: read::<String>(&fields, "title").unwrap_or_default(),
            tab_type: read::<TabType>(&fields, "type").unwrap_or(TabType::Unknown),
            is_active: read::<bool>(&fields, "isActive").unwrap_or(false),
            ..Self::new("", "", TabType::Unknown).view
        };
        super::fields::read_tab(&mut view, &fields);
        Self {
            view,
            raw_fields: fields,
        }
    }
}

macro_rules! canonical_serde {
    ($target:ty, $view:ty) => {
        impl std::ops::Deref for $target {
            type Target = $view;
            fn deref(&self) -> &Self::Target {
                &self.view
            }
        }
        impl<'de> Deserialize<'de> for $target {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = Value::deserialize(deserializer)?;
                Ok(Self::from_fields(
                    value.as_object().cloned().unwrap_or_default(),
                ))
            }
        }
        impl Serialize for $target {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let text = stringify(&Value::Object(self.raw_fields.clone()))
                    .map_err(serde::ser::Error::custom)?;
                serde_json::value::RawValue::from_string(text)
                    .map_err(serde::ser::Error::custom)?
                    .serialize(serializer)
            }
        }
        impl PartialEq for $target {
            fn eq(&self, other: &Self) -> bool {
                self.raw_fields == other.raw_fields
            }
        }
        impl $target {
            /// The current canonical JSON value, including unrecognized saved values.
            pub fn saved_field(&self, key: &str) -> Option<&Value> {
                self.raw_fields.get(key)
            }
            /// Assigns a field in place, or removes it for JavaScript `undefined`.
            pub fn set_saved_field(&mut self, key: &str, value: Option<Value>) {
                let mut fields = self.raw_fields.clone();
                if let Some(value) = value {
                    fields.insert(key.into(), value);
                } else {
                    fields.shift_remove(key);
                }
                *self = Self::from_fields(fields);
            }
        }
    };
}

canonical_serde!(Tab, TabFields);
canonical_serde!(TabConnection, ConnectionFields);
