//! The effective interface theme and the terminal palette.
//!
//! The Tauri client follows the system when `theme_follow_system` is on,
//! otherwise it shows `theme`; an empty or unknown id falls back to `default`
//! (`ThemeContext` and `resolveThemeCache`). A saved id names either one of the
//! six built-in themes or a theme in the bundled library, which the shared core
//! catalog loads once for the whole process. The interface colors of a preset
//! come from the generated `tterm-ui` tokens, while a library theme uses the
//! colors the shared catalog derives once; the ANSI colors always come from the
//! shared core theme definitions, so the two palettes stay separate.

use gpui_kit::Rgba;
use tterm_core::config::AppConfig;
use tterm_core::theme::catalog::load_theme_catalog;
use tterm_core::theme::definitions::get_preset_theme;
use tterm_core::theme::types::{CatalogTheme, PresetTheme};
use tterm_core::workspace::tab::Tab;
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::theme::{PaletteText, ThemeColors, parse_hsl};
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;
use tterm_ui::tokens::palette::{Colors, DARK, FOREST, LIGHT, OCEAN, SUNSET, UBUNTU};
use tterm_ui::tokens::theme::Theme;

/// The built-in theme Tauri uses when a saved id is missing or unknown.
pub const DEFAULT_THEME_ID: &str = "default";

/// A resolved theme: interface colors for the window and ANSI colors for its
/// terminals.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTheme {
    /// The resolved theme id, borrowed from the shared presets or catalog so it
    /// is the id the config names rather than an alias.
    pub id: &'static str,
    /// The interface palette. It carries the same id and its own darkness.
    pub ui: Theme,
    /// The terminal palette from the shared core definitions.
    pub terminal: PaletteText,
}

impl Default for ResolvedTheme {
    fn default() -> Self {
        resolve(&AppConfig::default(), true)
    }
}

/// The dark built-in theme, for callers without a saved configuration such as
/// headless captures and focused tests.
pub fn default_theme() -> ResolvedTheme {
    ResolvedTheme::default()
}

impl ResolvedTheme {
    /// The opaque interface background, painted behind the title bar and the
    /// workspace. The terminal views paint their own background from the
    /// ANSI palette.
    pub fn background(&self) -> Rgba {
        hsla(self.ui.colors.background, 1.0).to_rgb()
    }

    /// The terminal palette for the effective application theme.
    pub fn terminal_palette(&self) -> TerminalPalette {
        self.terminal.to_terminal_palette()
    }

    /// The palette for a tab. A connection theme wins when it names a built-in
    /// or bundled library theme; an unknown value falls back to the application
    /// theme, as the Tauri client does for a connection whose theme no longer
    /// exists.
    pub fn terminal_palette_for(&self, theme_id: Option<&str>) -> TerminalPalette {
        theme_id
            .and_then(theme_palette)
            .unwrap_or_else(|| self.terminal_palette())
    }

    /// The palette for a saved tab, including the theme of the connection it
    /// was opened from.
    pub fn palette_for_tab(&self, tab: &Tab) -> TerminalPalette {
        self.terminal_palette_for(
            tab.connection
                .as_ref()
                .and_then(|connection| connection.terminal_theme.as_deref()),
        )
    }
}

/// Resolves the theme the Tauri client would show for `config`.
///
/// `system_dark` is the window's current appearance. It only matters when the
/// saved configuration follows the system.
pub fn resolve(config: &AppConfig, system_dark: bool) -> ResolvedTheme {
    let configured = if config.theme_follow_system {
        if system_dark {
            &config.theme_dark
        } else {
            &config.theme_light
        }
    } else {
        &config.theme
    };
    let configured = if configured.is_empty() {
        DEFAULT_THEME_ID
    } else {
        configured.as_str()
    };
    resolve_theme(configured).unwrap_or_else(default_resolved)
}

/// Resolves a theme id against the presets and the bundled library, without
/// consulting the saved follow-system slots.
///
/// The settings page uses this for a transient preview: the id is shown while
/// the user browses and is never written by this call.
pub fn resolve_theme_id(theme_id: &str) -> Option<ResolvedTheme> {
    resolve_theme(theme_id)
}

/// The built-in id behind a saved value. `dark` is an alias used by the
/// generated UI tokens for the default theme; Tauri also resolves it to the
/// default by falling back.
fn preset_id(theme_id: &str) -> Option<&'static str> {
    match theme_id {
        "default" | "dark" => Some("default"),
        "light" => Some("light"),
        "ocean" => Some("ocean"),
        "forest" => Some("forest"),
        "sunset" => Some("sunset"),
        "ubuntu" => Some("ubuntu"),
        _ => None,
    }
}

/// The interface palette for a built-in preset id.
///
/// The settings page previews every preset while the window keeps the theme
/// it started with, so a card needs the palette of a theme it is not showing.
pub fn preset_ui_theme(id: &str) -> Theme {
    ui_theme(preset_id(id).unwrap_or(DEFAULT_THEME_ID))
}

fn ui_theme(id: &str) -> Theme {
    match id {
        "light" => LIGHT,
        "ocean" => OCEAN,
        "forest" => FOREST,
        "sunset" => SUNSET,
        "ubuntu" => UBUNTU,
        _ => DARK,
    }
}

/// Resolves an id against the six presets (and the `dark` alias), then the
/// bundled library. A library id is used exactly as saved.
fn resolve_theme(theme_id: &str) -> Option<ResolvedTheme> {
    if let Some(preset) = preset_id(theme_id).and_then(get_preset_theme) {
        return Some(from_preset(preset));
    }
    load_theme_catalog()
        .ok()?
        .iter()
        .find(|theme| theme.id == theme_id)
        .map(from_catalog)
}

fn from_preset(preset: &'static PresetTheme) -> ResolvedTheme {
    let id = preset.id.as_str();
    ResolvedTheme {
        id,
        ui: ui_theme(id),
        terminal: preset.terminal.clone(),
    }
}

fn from_catalog(theme: &'static CatalogTheme) -> ResolvedTheme {
    let id = theme.id.as_str();
    ResolvedTheme {
        id,
        ui: Theme::library(id, ui_colors(theme.colors()), theme.is_dark()),
        terminal: theme.terminal().clone(),
    }
}

/// The fallback for an empty or unknown id. The default preset is part of the
/// shared definitions; the empty palette only covers a broken preset table.
fn default_resolved() -> ResolvedTheme {
    get_preset_theme(DEFAULT_THEME_ID).map_or_else(
        || ResolvedTheme {
            id: DEFAULT_THEME_ID,
            ui: DARK,
            terminal: PaletteText::default(),
        },
        from_preset,
    )
}

/// The ANSI palette of a preset or library theme, or `None` for an unknown id.
fn theme_palette(theme_id: &str) -> Option<TerminalPalette> {
    if let Some(preset) = preset_id(theme_id).and_then(get_preset_theme) {
        return Some(preset.terminal.to_terminal_palette());
    }
    load_theme_catalog()
        .ok()?
        .iter()
        .find(|theme| theme.id == theme_id)
        .map(|theme| theme.terminal().to_terminal_palette())
}

/// Converts the derived interface colors of a library theme to the shared
/// interface tokens. The shared catalog derives these once per theme.
fn ui_colors(colors: &ThemeColors) -> Colors {
    Colors {
        background: ui_hsl(&colors.background),
        foreground: ui_hsl(&colors.foreground),
        card: ui_hsl(&colors.card),
        card_foreground: ui_hsl(&colors.card_foreground),
        popover: ui_hsl(&colors.popover),
        popover_foreground: ui_hsl(&colors.popover_foreground),
        primary: ui_hsl(&colors.primary),
        primary_foreground: ui_hsl(&colors.primary_foreground),
        secondary: ui_hsl(&colors.secondary),
        secondary_foreground: ui_hsl(&colors.secondary_foreground),
        muted: ui_hsl(&colors.muted),
        muted_foreground: ui_hsl(&colors.muted_foreground),
        accent: ui_hsl(&colors.accent),
        accent_foreground: ui_hsl(&colors.accent_foreground),
        destructive: ui_hsl(&colors.destructive),
        destructive_foreground: ui_hsl(&colors.destructive_foreground),
        success: ui_hsl(&colors.success),
        success_foreground: ui_hsl(&colors.success_foreground),
        warning: ui_hsl(&colors.warning),
        warning_foreground: ui_hsl(&colors.warning_foreground),
        border: ui_hsl(&colors.border),
        input: ui_hsl(&colors.input),
        ring: ui_hsl(&colors.ring),
        tab_background: ui_hsl(&colors.tab_background),
        tab_active: ui_hsl(&colors.tab_active),
        tab_active_border: ui_hsl(&colors.tab_active_border),
        tab_hover: ui_hsl(&colors.tab_hover),
        titlebar: ui_hsl(&colors.titlebar),
    }
}

/// One HSL token in the token layer's own precision.
fn ui_hsl(value: &str) -> Hsl {
    parse_hsl(value).map_or(Hsl::new(0.0, 0.0, 0.0), |color| {
        Hsl::new(color.h as f32, color.s as f32, color.l as f32)
    })
}

#[cfg(test)]
mod tests;
