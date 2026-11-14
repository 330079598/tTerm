//! The external editor's display name, from `src/lib/externalEditor.ts`.
//!
//! Choosing the program needs the platform's file dialog and stays with the
//! clients.

use crate::js::trim_js;

/// The name to show for the external editor: the program's file name without
/// its folder or its application suffix, as in "Visual Studio Code" for
/// `/Applications/Visual Studio Code.app`.
pub fn editor_display_name(program: &str) -> String {
    let trimmed = trim_js(program);
    let without_separators = trimmed.trim_end_matches(['\\', '/']);
    let name = without_separators
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default();
    let lower = name.to_lowercase();
    let display = [".app", ".exe", ".cmd", ".bat", ".com"]
        .into_iter()
        .find_map(|suffix| {
            lower
                .strip_suffix(suffix)
                .map(|_| &name[..name.len() - suffix.len()])
        })
        .unwrap_or(name);
    if display.is_empty() {
        trimmed.to_owned()
    } else {
        display.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::editor_display_name;

    #[test]
    fn shows_a_macos_application_by_its_name() {
        assert_eq!(
            editor_display_name("/Applications/Visual Studio Code.app"),
            "Visual Studio Code"
        );
        assert_eq!(
            editor_display_name("/Applications/Sublime Text.app/"),
            "Sublime Text"
        );
        assert_eq!(editor_display_name("TextEdit"), "TextEdit");
    }

    #[test]
    fn drops_the_folder_and_suffix_of_a_windows_program() {
        assert_eq!(
            editor_display_name("C:\\Program Files\\Notepad++\\notepad++.exe"),
            "notepad++"
        );
        assert_eq!(editor_display_name("C:\\Users\\me\\bin\\code.CMD"), "code");
    }

    #[test]
    fn keeps_a_command_name_as_it_is() {
        assert_eq!(editor_display_name("/usr/bin/gedit"), "gedit");
        assert_eq!(editor_display_name(" kate "), "kate");
        assert_eq!(editor_display_name("C:\\"), "C:");
    }
}
