//! The terminal font a first launch uses, from `src/lib/terminalFont.ts`.

use std::collections::HashSet;

use tterm_terminal::js_text::js_trim;

/// The default font chain on macOS.
pub const MACOS_FONT_FAMILY: &str = "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", Menlo, Monaco, monospace";

/// The default font chain on Windows.
pub const WINDOWS_FONT_FAMILY: &str = "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", \"Cascadia Code\", Consolas, monospace";

/// The default font chain on Linux and every other platform.
pub const LINUX_FONT_FAMILY: &str = "\"JetBrains Mono Nerd Font\", \"JetBrainsMono Nerd Font\", \"JetBrains Mono\", \"DejaVu Sans Mono\", monospace";

/// The default terminal font chain for a platform; any other platform gets
/// Linux's.
pub fn default_font_family(platform: &str) -> &'static str {
    match platform {
        "macos" => MACOS_FONT_FAMILY,
        "windows" => WINDOWS_FONT_FAMILY,
        _ => LINUX_FONT_FAMILY,
    }
}

/// The default font chain of the platform this build runs on.
pub fn detected_font_family() -> &'static str {
    if cfg!(target_os = "macos") {
        MACOS_FONT_FAMILY
    } else if cfg!(target_os = "windows") {
        WINDOWS_FONT_FAMILY
    } else {
        LINUX_FONT_FAMILY
    }
}

/// The first font in a CSS font chain that the system has, which is the one
/// the terminal draws with; the whole chain when none of it is installed.
pub fn first_installed_font(font_family: &str, system_fonts: &[String]) -> String {
    let installed: HashSet<String> = system_fonts
        .iter()
        .map(|font| font.to_lowercase())
        .collect();
    font_family
        .split(',')
        .map(clean_font_name)
        .find(|name| installed.contains(&name.to_lowercase()))
        .map(str::to_owned)
        .unwrap_or_else(|| font_family.to_owned())
}

/// Trims a chain entry and drops one quote from each end, as the frontend's
/// `replace(/^["']|["']$/g, "")` does.
fn clean_font_name(name: &str) -> &str {
    let name = js_trim(name);
    let name = name.strip_prefix(['"', '\'']).unwrap_or(name);
    name.strip_suffix(['"', '\'']).unwrap_or(name)
}

#[cfg(test)]
mod tests;
