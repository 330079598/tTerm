//! The bundled theme library, following `src/lib/themeCatalog.ts`.

use std::sync::LazyLock;

use tterm_terminal::theme::PaletteText;
use tterm_terminal::theme::js::js_slice_from;

use crate::theme::derivation::{TerminalScheme, terminal_palette_from_scheme};
use crate::theme::types::{CatalogTheme, Theme};

/// Where the bundled schemes come from.
pub const THEME_CATALOG_SOURCE: &str = "https://github.com/mbadolato/iTerm2-Color-Schemes";

/// The prefix every library theme id carries.
pub const CATALOG_ID_PREFIX: &str = "catalog:";

/// The JSON the web view imports as `src/assets/themes/catalog.json`.
const CATALOG_JSON: &str = include_str!("../../../../src/assets/themes/catalog.json");

#[derive(serde::Deserialize)]
struct CatalogFile {
    commit: String,
    themes: Vec<TerminalScheme>,
}

static THEME_CATALOG: LazyLock<Result<Vec<CatalogTheme>, String>> = LazyLock::new(|| {
    let file: CatalogFile = serde_json::from_str(CATALOG_JSON)
        .map_err(|error| format!("Invalid theme catalog: {error}"))?;
    let _commit = file.commit;
    Ok(file.themes.iter().map(catalog_theme_from_scheme).collect())
});

/// Whether an id names a library theme.
pub fn is_catalog_theme_id(theme_id: &str) -> bool {
    theme_id.starts_with(CATALOG_ID_PREFIX)
}

/// Whether a theme came from the bundled library.
pub fn is_catalog_theme(theme: Option<&Theme>) -> bool {
    theme.is_some_and(Theme::is_catalog)
}

/// The library name carried by a `catalog:` id.
///
/// This is `themeId.slice(CATALOG_ID_PREFIX.length)`, including its behaviour
/// for ids that do not carry the prefix and for inputs shorter than it.
pub fn catalog_theme_name(theme_id: &str) -> String {
    js_slice_from(theme_id, CATALOG_ID_PREFIX.len())
}

/// Builds a library theme from a palette.
pub fn create_catalog_theme(theme_id: String, terminal: PaletteText) -> CatalogTheme {
    CatalogTheme::new(theme_id, terminal)
}

/// Builds a library theme from a scheme.
pub fn catalog_theme_from_scheme(scheme: &TerminalScheme) -> CatalogTheme {
    let name = match &scheme.name {
        tterm_terminal::theme::OptionalText::Missing => "undefined".to_string(),
        tterm_terminal::theme::OptionalText::Null => "null".to_string(),
        tterm_terminal::theme::OptionalText::Value(name) => name.clone(),
        tterm_terminal::theme::OptionalText::Json(value) => value.to_string(),
    };
    create_catalog_theme(
        format!("{CATALOG_ID_PREFIX}{name}"),
        terminal_palette_from_scheme(scheme),
    )
}

/// Loads the bundled library, once for the whole process.
pub fn load_theme_catalog() -> Result<&'static [CatalogTheme], &'static str> {
    THEME_CATALOG.as_deref().map_err(String::as_str)
}

#[cfg(test)]
mod tests;
