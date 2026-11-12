//! Theme values shared by the preset, catalog and custom-theme modules.

use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value};
use std::sync::OnceLock;

use tterm_terminal::theme::{OptionalText, PaletteText, TERMINAL_PALETTE_KEYS, ThemeColors};

/// The ids of the six built-in themes, in display order.
pub const PRESET_THEME_IDS: [&str; 6] = ["default", "light", "ocean", "forest", "sunset", "ubuntu"];

/// A built-in theme id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetThemeId {
    /// The dark theme tTerm opens with.
    Default,
    /// The light theme.
    Light,
    /// The blue ocean theme.
    Ocean,
    /// The green forest theme.
    Forest,
    /// The warm sunset theme.
    Sunset,
    /// The Ubuntu-inspired theme.
    Ubuntu,
}

impl PresetThemeId {
    /// The persisted id.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Light => "light",
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Sunset => "sunset",
            Self::Ubuntu => "ubuntu",
        }
    }

    /// Reads a persisted id, or `None` when it names something else.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "default" => Some(Self::Default),
            "light" => Some(Self::Light),
            "ocean" => Some(Self::Ocean),
            "forest" => Some(Self::Forest),
            "sunset" => Some(Self::Sunset),
            "ubuntu" => Some(Self::Ubuntu),
            _ => None,
        }
    }
}

/// Which system appearance a theme is shown in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSlot {
    /// The light appearance.
    Light,
    /// The dark appearance.
    Dark,
}

/// A built-in theme.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetTheme {
    /// The theme id.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The one-line description.
    pub description: OptionalText,
    /// The interface colors.
    pub colors: ThemeColors,
    /// The terminal palette.
    pub terminal: PaletteText,
    /// Always false for a preset.
    pub is_custom: bool,
}

/// Which creation path gave a custom theme its property order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CustomThemeOrder {
    /// `normalizeCustomTheme` puts the id first.
    Normalized,
    /// `createCustomThemeFromPreset` spreads the new theme and appends the id.
    FromPreset,
    /// The import path omits `baseTheme`.
    Imported,
}

/// A JSON object whose property order is written by the theme code.
///
/// `serde_json::Map` sorts its keys, while the web view writes object keys in
/// creation order. Keeping an explicit entry list lets a saved custom theme
/// serialize the fields in the same order as the TypeScript object.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RawObject {
    entries: Vec<(String, Value)>,
}

impl RawObject {
    /// Builds an object from entries whose string keys are in insertion
    /// order. Integer-index keys move first, as JavaScript enumerates them.
    pub(crate) fn from_entries(entries: Vec<(String, Value)>) -> Self {
        // JavaScript enumerates integer-index properties before string keys,
        // in ascending order, even when the string keys were inserted first.
        // A colors or terminal value that came from spreading a string or an
        // array gets such numeric keys.
        let (mut indices, mut named): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|(key, _)| array_index(key).is_some());
        indices.sort_by_key(|(key, _)| array_index(key));
        indices.append(&mut named);
        Self { entries: indices }
    }

    /// Reorders the keys a map has in the frontend's canonical order.
    ///
    /// Keys the canonical order does not name follow, as an object spread
    /// appends unknown properties; integer-index keys still move first. The
    /// map itself comes from `serde_json`, so the order of the remaining
    /// extra keys is the backend's sorted order rather than the original text
    /// order.
    pub(crate) fn canonical(map: &Map<String, Value>, keys: &[&str]) -> Self {
        let mut entries = Vec::with_capacity(map.len());
        for key in keys {
            if let Some(value) = map.get(*key) {
                entries.push(((*key).to_string(), value.clone()));
            }
        }
        for (key, value) in map {
            if !keys.contains(&key.as_str()) {
                entries.push((key.clone(), value.clone()));
            }
        }
        Self::from_entries(entries)
    }

    /// Reorders terminal colors with the known keys first.
    pub(crate) fn canonical_terminal(map: &Map<String, Value>) -> Self {
        Self::canonical(map, &TERMINAL_PALETTE_KEYS)
    }
}

fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse::<u32>().ok().filter(|index| *index != u32::MAX)
}

impl Serialize for RawObject {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_map(Some(self.entries.len()))?;
        for (key, value) in &self.entries {
            state.serialize_entry(key, value)?;
        }
        state.end()
    }
}

/// A theme the user created or imported.
///
/// The three frontend creation paths order the properties differently. The
/// serializer reproduces the path that made the value, which matters because
/// the backend stores the JSON text it receives.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomTheme {
    /// The theme id.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The one-line description.
    pub description: OptionalText,
    /// The interface colors.
    pub colors: ThemeColors,
    /// The terminal palette.
    pub terminal: PaletteText,
    /// The preset or catalog theme this one started from.
    pub base_theme: OptionalText,
    /// Always true for a custom theme.
    pub is_custom: bool,
    /// Creation time in milliseconds.
    pub created_at: i64,
    /// Last edit time in milliseconds.
    pub updated_at: i64,
    raw_colors: Option<RawObject>,
    raw_terminal: Option<RawObject>,
    order: CustomThemeOrder,
}

impl CustomTheme {
    /// Builds a theme the preset editor created. The id is written last,
    /// matching `{ ...createCustomThemeFromPreset(...), id }`.
    pub fn from_preset(new_theme: NewCustomTheme, id: String) -> Self {
        Self {
            id,
            name: new_theme.name,
            description: new_theme.description,
            colors: new_theme.colors,
            terminal: new_theme.terminal,
            base_theme: new_theme.base_theme,
            is_custom: true,
            created_at: new_theme.created_at,
            updated_at: new_theme.updated_at,
            raw_colors: None,
            raw_terminal: None,
            order: CustomThemeOrder::FromPreset,
        }
    }

    /// Builds a theme imported from another terminal. The import path has no
    /// `baseTheme` property.
    pub fn imported(
        id: String,
        name: String,
        description: OptionalText,
        colors: ThemeColors,
        terminal: PaletteText,
        created_at: i64,
        updated_at: i64,
    ) -> Self {
        Self {
            id,
            name,
            description,
            colors,
            terminal,
            base_theme: OptionalText::Missing,
            is_custom: true,
            created_at,
            updated_at,
            raw_colors: None,
            raw_terminal: None,
            order: CustomThemeOrder::Imported,
        }
    }

    /// The normalized representation of a theme read from the backend.
    pub(crate) fn normalized(parts: NormalizedThemeParts) -> Self {
        Self {
            id: parts.id,
            name: parts.name,
            description: parts.description,
            colors: parts.colors,
            terminal: parts.terminal,
            base_theme: parts.base_theme,
            is_custom: true,
            created_at: parts.created_at,
            updated_at: parts.updated_at,
            raw_colors: Some(parts.raw_colors),
            raw_terminal: Some(parts.raw_terminal),
            order: CustomThemeOrder::Normalized,
        }
    }

    /// The object a normalize pass should write back for the colors.
    pub(crate) fn raw_colors(&self) -> Option<&RawObject> {
        self.raw_colors.as_ref()
    }

    /// The object a normalize pass should write back for the terminal.
    pub(crate) fn raw_terminal(&self) -> Option<&RawObject> {
        self.raw_terminal.as_ref()
    }
}

/// The values `normalizeCustomTheme` assembles after validating a saved
/// object.
pub(crate) struct NormalizedThemeParts {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: OptionalText,
    pub(crate) colors: ThemeColors,
    pub(crate) terminal: PaletteText,
    pub(crate) base_theme: OptionalText,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) raw_colors: RawObject,
    pub(crate) raw_terminal: RawObject,
}

impl Serialize for CustomTheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CustomTheme", 9)?;
        match self.order {
            CustomThemeOrder::Normalized => {
                state.serialize_field("id", &self.id)?;
                state.serialize_field("name", &self.name)?;
                serialize_optional(&mut state, "description", &self.description)?;
                serialize_colors(&mut state, self)?;
                serialize_terminal(&mut state, self)?;
                serialize_optional(&mut state, "baseTheme", &self.base_theme)?;
                state.serialize_field("isCustom", &self.is_custom)?;
                state.serialize_field("createdAt", &self.created_at)?;
                state.serialize_field("updatedAt", &self.updated_at)?;
            }
            CustomThemeOrder::FromPreset | CustomThemeOrder::Imported => {
                state.serialize_field("name", &self.name)?;
                serialize_optional(&mut state, "description", &self.description)?;
                serialize_colors(&mut state, self)?;
                serialize_terminal(&mut state, self)?;
                if self.order == CustomThemeOrder::FromPreset {
                    serialize_optional(&mut state, "baseTheme", &self.base_theme)?;
                }
                state.serialize_field("isCustom", &self.is_custom)?;
                state.serialize_field("createdAt", &self.created_at)?;
                state.serialize_field("updatedAt", &self.updated_at)?;
                state.serialize_field("id", &self.id)?;
            }
        }
        state.end()
    }
}

fn serialize_optional<S>(
    state: &mut S,
    key: &'static str,
    value: &OptionalText,
) -> Result<(), S::Error>
where
    S: SerializeStruct,
{
    if !value.is_missing() {
        state.serialize_field(key, value)?;
    }
    Ok(())
}

fn serialize_colors<S>(state: &mut S, theme: &CustomTheme) -> Result<(), S::Error>
where
    S: SerializeStruct,
{
    match theme.raw_colors() {
        Some(colors) => state.serialize_field("colors", colors),
        None => state.serialize_field("colors", &theme.colors),
    }
}

fn serialize_terminal<S>(state: &mut S, theme: &CustomTheme) -> Result<(), S::Error>
where
    S: SerializeStruct,
{
    match theme.raw_terminal() {
        Some(terminal) => state.serialize_field("terminal", terminal),
        None => state.serialize_field("terminal", &theme.terminal),
    }
}

/// A custom theme before an id has been assigned.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCustomTheme {
    /// The display name.
    pub name: String,
    /// The one-line description.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub description: OptionalText,
    /// The interface colors.
    pub colors: ThemeColors,
    /// The terminal palette.
    pub terminal: PaletteText,
    /// The preset or catalog theme this one started from.
    #[serde(skip_serializing_if = "OptionalText::is_missing")]
    pub base_theme: OptionalText,
    /// Always true for a custom theme.
    pub is_custom: bool,
    /// Creation time in milliseconds.
    pub created_at: i64,
    /// Last edit time in milliseconds.
    pub updated_at: i64,
}

impl NewCustomTheme {
    /// Adds the id the store assigns and produces the persisted shape.
    pub fn with_id(self, id: String) -> CustomTheme {
        CustomTheme::from_preset(self, id)
    }
}

/// A theme from the bundled library.
#[derive(Clone, Debug, PartialEq)]
pub struct CatalogTheme {
    /// The `catalog:`-prefixed id.
    pub id: String,
    /// The library name, also the part after the prefix.
    pub name: String,
    terminal: PaletteText,
    colors: OnceLock<ThemeColors>,
    is_dark: bool,
}

impl CatalogTheme {
    /// Builds a library theme. Its interface colors are derived on first use,
    /// like the getter `createCatalogTheme` installs.
    pub fn new(theme_id: String, terminal: PaletteText) -> Self {
        let colors = OnceLock::new();
        let is_dark = crate::theme::derivation::is_dark_palette(&terminal);
        Self {
            name: crate::theme::catalog::catalog_theme_name(&theme_id),
            id: theme_id,
            terminal,
            colors,
            is_dark,
        }
    }

    /// The terminal palette.
    pub fn terminal(&self) -> &PaletteText {
        &self.terminal
    }

    /// The interface colors, derived once and then reused.
    pub fn colors(&self) -> &ThemeColors {
        self.colors
            .get_or_init(|| crate::theme::derivation::derive_theme_colors(&self.terminal))
    }

    /// Whether the background is darker than the foreground.
    pub fn is_dark(&self) -> bool {
        self.is_dark
    }
}

impl Serialize for CatalogTheme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CatalogTheme", 7)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("colors", self.colors())?;
        state.serialize_field("terminal", &self.terminal)?;
        state.serialize_field("isCustom", &false)?;
        state.serialize_field("isCatalog", &true)?;
        state.serialize_field("isDark", &self.is_dark)?;
        state.end()
    }
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Preset(theme) => theme.serialize(serializer),
            Self::Custom(theme) => theme.serialize(serializer),
            Self::Catalog(theme) => theme.serialize(serializer),
        }
    }
}

/// Any theme the interface can show.
#[derive(Clone, Debug, PartialEq)]
pub enum Theme {
    /// A built-in theme.
    Preset(PresetTheme),
    /// A user theme.
    Custom(CustomTheme),
    /// A library theme.
    Catalog(CatalogTheme),
}

impl Theme {
    /// The interface colors.
    pub fn colors(&self) -> &ThemeColors {
        match self {
            Self::Preset(theme) => &theme.colors,
            Self::Custom(theme) => &theme.colors,
            Self::Catalog(theme) => theme.colors(),
        }
    }

    /// The terminal palette.
    pub fn terminal(&self) -> &PaletteText {
        match self {
            Self::Preset(theme) => &theme.terminal,
            Self::Custom(theme) => &theme.terminal,
            Self::Catalog(theme) => theme.terminal(),
        }
    }

    /// Whether this is a library theme.
    pub fn is_catalog(&self) -> bool {
        matches!(self, Self::Catalog(_))
    }
}

/// The two values a theme editor preview needs.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorsAndPalette {
    /// The interface colors.
    pub colors: ThemeColors,
    /// The terminal palette.
    pub terminal: PaletteText,
}
