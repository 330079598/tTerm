//! How a shell wants a dropped path written, from `src/lib/droppedPaths.ts`.

use crate::js::is_js_whitespace;

/// How a shell wants a dropped path written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropQuoting {
    /// A POSIX shell: backslash-escaped specials.
    Posix,
    /// PowerShell: single quotes when needed.
    PowerShell,
    /// The Windows command prompt: double quotes when needed.
    Cmd,
    /// A shell inside WSL, which sees Windows paths as mount paths.
    Wsl,
    /// Git Bash, which sees Windows paths under the drive root.
    GitBash,
}

/// The quoting for a local shell, from the platform and the shell's name as
/// `local_terminal_shell_name` reports it (`pwsh`, `cmd`, `WSL`, `zsh`).
///
/// A Windows shell tTerm cannot place gets double quotes, as Windows
/// Terminal gives every shell that is not WSL.
pub fn drop_quoting_for(platform: &str, shell_name: Option<&str>) -> DropQuoting {
    if platform != "windows" {
        return DropQuoting::Posix;
    }
    match shell_name.map(|name| name.trim().to_lowercase()).as_deref() {
        Some("powershell" | "pwsh") => DropQuoting::PowerShell,
        Some("wsl") => DropQuoting::Wsl,
        Some("git bash") => DropQuoting::GitBash,
        _ => DropQuoting::Cmd,
    }
}

fn is_posix_special(character: char) -> bool {
    is_js_whitespace(character)
        || matches!(
            character,
            '!' | '"'
                | '#'
                | '$'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | ';'
                | '<'
                | '>'
                | '?'
                | '['
                | '\\'
                | ']'
                | '^'
                | '`'
                | '{'
                | '|'
                | '}'
                | '~'
                | '='
        )
}

fn is_cmd_special(character: char) -> bool {
    is_js_whitespace(character)
        || matches!(
            character,
            '&' | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '^'
                | '='
                | ';'
                | '!'
                | '\''
                | '+'
                | ','
                | '`'
                | '~'
                | '%'
        )
}

fn is_powershell_special(character: char) -> bool {
    is_js_whitespace(character)
        || matches!(
            character,
            '\'' | '"'
                | '`'
                | '$'
                | '&'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | ';'
                | ','
                | '@'
                | '#'
                | '|'
                | '<'
                | '>'
        )
}

/// Escapes each special character with a backslash, as macOS Terminal does.
pub fn quote_posix(path: &str) -> String {
    let mut quoted = String::with_capacity(path.len());
    for character in path.chars() {
        if character == '\n' {
            // A backslash before a line break joins the lines, so a break gets
            // quoted.
            quoted.push_str("'\n'");
        } else if is_posix_special(character) {
            quoted.push('\\');
            quoted.push(character);
        } else {
            quoted.push(character);
        }
    }
    quoted
}

fn quote_powershell(path: &str) -> String {
    if path.chars().any(is_powershell_special) {
        // Single quotes keep `$` and backticks literal, unlike double quotes.
        format!("'{}'", path.replace('\'', "''"))
    } else {
        path.to_owned()
    }
}

fn quote_cmd(path: &str) -> String {
    if path.chars().any(is_cmd_special) {
        format!("\"{path}\"")
    } else {
        path.to_owned()
    }
}

/// A Windows path as a Linux or MSYS shell sees it: `C:\a` becomes
/// `<drive_root>c/a`, and a path inside a WSL distribution its own path.
/// Any other network path stays as it is, in single quotes.
fn to_unix_path(path: &str, drive_root: &str) -> String {
    if let Some((drive, rest)) = drive_path(path) {
        let rest = rest.replace('\\', "/");
        let suffix = if rest.is_empty() {
            String::new()
        } else {
            format!("/{rest}")
        };
        return quote_posix(&format!(
            "{drive_root}{}{suffix}",
            drive.to_ascii_lowercase()
        ));
    }
    if let Some(distro_path) = wsl_distro_path(path) {
        return quote_posix(&distro_path.replace('\\', "/"));
    }
    format!("'{}'", path.replace('\'', "'\\''"))
}

/// Splits a drive path like `C:\a` or `D:/` into its drive letter and rest.
fn drive_path(path: &str) -> Option<(char, &str)> {
    let mut characters = path.chars();
    let drive = characters.next()?;
    if !drive.is_ascii_alphabetic() || characters.next()? != ':' {
        return None;
    }
    let rest = &path[2..];
    let rest = rest
        .strip_prefix('\\')
        .or_else(|| rest.strip_prefix('/'))
        .unwrap_or(rest);
    Some((drive, rest))
}

/// The path inside a WSL distribution, for `\\wsl$\Ubuntu\home\me`.
fn wsl_distro_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("\\\\")?;
    let separator = rest.find('\\')?;
    let server = &rest[..separator];
    if !(server.eq_ignore_ascii_case("wsl$") || server.eq_ignore_ascii_case("wsl.localhost")) {
        return None;
    }
    let after_server = &rest[separator + 1..];
    let end = after_server.find('\\').unwrap_or(after_server.len());
    if end == 0 {
        return None;
    }
    let inside = &after_server[end..];
    Some(if inside.is_empty() {
        "\\".to_owned()
    } else {
        inside.to_owned()
    })
}

/// One dropped path, written so the shell reads it back as that path.
pub fn quote_dropped_path(path: &str, quoting: DropQuoting) -> String {
    match quoting {
        DropQuoting::Posix => quote_posix(path),
        DropQuoting::PowerShell => quote_powershell(path),
        DropQuoting::Cmd => quote_cmd(path),
        DropQuoting::Wsl => to_unix_path(path, "/mnt/"),
        DropQuoting::GitBash => to_unix_path(path, "/"),
    }
}

/// The text a drop of `paths` types into the terminal: each path quoted for
/// the shell, separated by spaces, with a space after the last so the next
/// argument can follow, as macOS Terminal and Windows Terminal do.
pub fn dropped_paths_text(paths: &[String], quoting: DropQuoting) -> String {
    let quoted: Vec<String> = paths
        .iter()
        .filter(|path| !path.is_empty())
        .map(|path| quote_dropped_path(path, quoting))
        .collect();
    if quoted.is_empty() {
        String::new()
    } else {
        format!("{} ", quoted.join(" "))
    }
}

/// The line that takes a remote shell to `path`, Enter included.
///
/// The path is escaped as for a dropped file, which bash, zsh and fish all
/// read.
pub fn cd_command(path: &str) -> String {
    format!("cd {}\r", quote_posix(path))
}

#[cfg(test)]
mod tests {
    use super::{
        DropQuoting, cd_command, drop_quoting_for, dropped_paths_text, quote_dropped_path,
    };

    #[test]
    fn escapes_posix_style_outside_windows_whatever_the_shell() {
        assert_eq!(drop_quoting_for("macos", Some("zsh")), DropQuoting::Posix);
        assert_eq!(drop_quoting_for("linux", Some("fish")), DropQuoting::Posix);
        assert_eq!(drop_quoting_for("linux", None), DropQuoting::Posix);
    }

    #[test]
    fn follows_the_shell_on_windows() {
        assert_eq!(
            drop_quoting_for("windows", Some("pwsh")),
            DropQuoting::PowerShell
        );
        assert_eq!(
            drop_quoting_for("windows", Some("PowerShell")),
            DropQuoting::PowerShell
        );
        assert_eq!(drop_quoting_for("windows", Some("cmd")), DropQuoting::Cmd);
        assert_eq!(drop_quoting_for("windows", Some("WSL")), DropQuoting::Wsl);
        assert_eq!(
            drop_quoting_for("windows", Some("Git Bash")),
            DropQuoting::GitBash
        );
    }

    #[test]
    fn gives_an_unknown_windows_shell_double_quotes_like_windows_terminal() {
        assert_eq!(drop_quoting_for("windows", Some("nu")), DropQuoting::Cmd);
        assert_eq!(drop_quoting_for("windows", None), DropQuoting::Cmd);
    }

    #[test]
    fn escapes_posix_specials_with_backslashes_and_leaves_other_text_alone() {
        assert_eq!(
            quote_dropped_path("/Users/me/notes.txt", DropQuoting::Posix),
            "/Users/me/notes.txt"
        );
        assert_eq!(
            quote_dropped_path("/Users/me/My File (1).txt", DropQuoting::Posix),
            "/Users/me/My\\ File\\ \\(1\\).txt"
        );
        assert_eq!(
            quote_dropped_path("/tmp/it's $HOME", DropQuoting::Posix),
            "/tmp/it\\'s\\ \\$HOME"
        );
        assert_eq!(
            quote_dropped_path("/Users/me/文档/报告.pdf", DropQuoting::Posix),
            "/Users/me/文档/报告.pdf"
        );
    }

    #[test]
    fn quotes_a_line_break_instead_of_escaping_it_into_a_continuation() {
        assert_eq!(
            quote_dropped_path("/tmp/a\nb", DropQuoting::Posix),
            "/tmp/a'\n'b"
        );
    }

    #[test]
    fn puts_a_powershell_path_in_single_quotes_only_when_it_needs_them() {
        assert_eq!(
            quote_dropped_path("C:\\Users\\me\\a.txt", DropQuoting::PowerShell),
            "C:\\Users\\me\\a.txt"
        );
        assert_eq!(
            quote_dropped_path("C:\\Program Files\\app", DropQuoting::PowerShell),
            "'C:\\Program Files\\app'"
        );
        assert_eq!(
            quote_dropped_path("C:\\tmp\\$it's", DropQuoting::PowerShell),
            "'C:\\tmp\\$it''s'"
        );
    }

    #[test]
    fn puts_a_cmd_path_in_double_quotes_only_when_it_needs_them() {
        assert_eq!(
            quote_dropped_path("C:\\Users\\me\\a.txt", DropQuoting::Cmd),
            "C:\\Users\\me\\a.txt"
        );
        assert_eq!(
            quote_dropped_path("C:\\Program Files\\app", DropQuoting::Cmd),
            "\"C:\\Program Files\\app\""
        );
        assert_eq!(
            quote_dropped_path("C:\\a&b", DropQuoting::Cmd),
            "\"C:\\a&b\""
        );
    }

    #[test]
    fn turns_a_drive_path_into_a_mount_path_for_wsl_and_git_bash() {
        assert_eq!(
            quote_dropped_path("C:\\Users\\me\\a b.txt", DropQuoting::Wsl),
            "/mnt/c/Users/me/a\\ b.txt"
        );
        assert_eq!(quote_dropped_path("D:\\", DropQuoting::Wsl), "/mnt/d");
        assert_eq!(
            quote_dropped_path("C:\\Users\\me\\a b.txt", DropQuoting::GitBash),
            "/c/Users/me/a\\ b.txt"
        );
    }

    #[test]
    fn turns_a_path_inside_a_wsl_distribution_into_the_distributions_own_path() {
        assert_eq!(
            quote_dropped_path("\\\\wsl.localhost\\Ubuntu\\home\\me\\x", DropQuoting::Wsl),
            "/home/me/x"
        );
        assert_eq!(
            quote_dropped_path("\\\\wsl$\\Ubuntu", DropQuoting::Wsl),
            "/"
        );
    }

    #[test]
    fn single_quotes_any_other_network_path_for_wsl() {
        assert_eq!(
            quote_dropped_path("\\\\server\\share\\it's", DropQuoting::Wsl),
            "'\\\\server\\share\\it'\\''s'"
        );
    }

    #[test]
    fn joins_the_paths_with_spaces_and_ends_with_one() {
        assert_eq!(
            dropped_paths_text(&["/a b".to_owned(), "/c".to_owned()], DropQuoting::Posix),
            "/a\\ b /c "
        );
    }

    #[test]
    fn types_nothing_for_no_paths() {
        assert_eq!(dropped_paths_text(&[], DropQuoting::Posix), "");
        assert_eq!(dropped_paths_text(&[String::new()], DropQuoting::Cmd), "");
    }

    #[test]
    fn escapes_what_a_shell_would_read_as_syntax_and_presses_enter() {
        assert_eq!(cd_command("/var/log"), "cd /var/log\r");
        assert_eq!(
            cd_command("/srv/my app/$HOME"),
            "cd /srv/my\\ app/\\$HOME\r"
        );
        assert_eq!(cd_command("/tmp/it's"), "cd /tmp/it\\'s\r");
    }
}
