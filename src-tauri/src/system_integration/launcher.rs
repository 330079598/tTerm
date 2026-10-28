//! Writing the scripts that start tTerm as `tterm`.

use std::path::Path;

/// Writes `content` to `path` unless it holds that already. Returns whether
/// it wrote.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    if std::fs::read_to_string(path).is_ok_and(|current| current == content) {
        return Ok(false);
    }
    std::fs::write(path, content)
        .map_err(|error| format!("Failed to write '{}': {error}", path.display()))?;
    Ok(true)
}

/// A sh script that starts `program` with the script's arguments, in the
/// directory it was run in, and returns to the prompt at once.
pub fn sh_launcher(program: &str) -> String {
    let program = sh_quote(program);
    format!("#!/bin/sh\n{program} \"$@\" </dev/null >/dev/null 2>&1 &\n")
}

/// `text` as one word that sh takes literally.
pub fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sh_launcher_quotes_the_program_for_the_shell() {
        assert_eq!(
            sh_launcher("/Users/o'neil/tTerm"),
            "#!/bin/sh\n'/Users/o'\\''neil/tTerm' \"$@\" </dev/null >/dev/null 2>&1 &\n"
        );
    }

    #[test]
    fn writes_only_what_changed() {
        let path = std::env::temp_dir().join(format!("tterm-launcher-{}", uuid::Uuid::new_v4()));
        assert_eq!(write_if_changed(&path, "a"), Ok(true));
        assert_eq!(write_if_changed(&path, "a"), Ok(false));
        assert_eq!(write_if_changed(&path, "b"), Ok(true));
        let _ = std::fs::remove_file(&path);
    }
}
