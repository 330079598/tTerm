//! Creating an editable theme from a preset, following
//! `src/lib/themeUtils.ts`.

use tterm_terminal::theme::OptionalText;

use crate::theme::definitions::resolve_theme_definition;
use crate::theme::types::NewCustomTheme;

/// Builds an unsaved custom theme from a preset.
///
/// `now` is the caller's `Date.now()` value. An unknown `preset_id` resolves
/// to the `default` colors and terminal palette, while the returned
/// `baseTheme` keeps the id that was passed in, as the TypeScript function
/// does.
pub fn create_custom_theme_from_preset(
    preset_id: &str,
    name: &str,
    description: Option<&str>,
    now: i64,
) -> NewCustomTheme {
    let base_theme = resolve_theme_definition(preset_id, &[]);
    NewCustomTheme {
        name: name.to_string(),
        description: description.map_or(OptionalText::Missing, OptionalText::from),
        colors: base_theme.colors().clone(),
        terminal: base_theme.terminal().clone(),
        base_theme: OptionalText::from(preset_id),
        is_custom: true,
        created_at: now,
        updated_at: now,
    }
}

#[cfg(test)]
mod tests;
