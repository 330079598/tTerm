//! Splits the arguments typed for a custom shell into the words it gets.

/// Splits `text` at whitespace outside quotes; double quotes group a word on
/// every system. With `posix`, as on macOS and Linux, single quotes group one
/// too and a backslash keeps the character after it, as a POSIX shell reads
/// them. Without it, as on Windows, a backslash is a path separator and stays.
/// A quote left open runs to the end.
pub(super) fn split_args(text: &str, posix: bool) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    // A word has started, even an empty one written as `""`.
    let mut started = false;
    let mut quote: Option<char> = None;
    let mut chars = text.chars();

    while let Some(c) = chars.next() {
        match quote {
            Some(open) if c == open => quote = None,
            // Inside double quotes a POSIX shell lets a backslash escape only these.
            Some('"') if posix && c == '\\' => match chars.next() {
                Some(next @ ('"' | '\\' | '$' | '`')) => word.push(next),
                Some(next) => {
                    word.push('\\');
                    word.push(next);
                }
                None => word.push('\\'),
            },
            Some(_) => word.push(c),
            None if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            None if c == '"' || (posix && c == '\'') => {
                quote = Some(c);
                started = true;
            }
            None if posix && c == '\\' => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
                started = true;
            }
            None => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::split_args;

    #[test]
    fn splits_plain_words_at_any_whitespace() {
        assert_eq!(split_args("  --login\t-i  ", true), ["--login", "-i"]);
        assert!(split_args("   ", false).is_empty());
    }

    #[test]
    fn keeps_a_quoted_windows_path_whole() {
        assert_eq!(
            split_args(r#"-NoExit -File "C:\my scripts\a.ps1""#, false),
            ["-NoExit", "-File", r"C:\my scripts\a.ps1"]
        );
        assert_eq!(
            split_args(r"C:\tools\x 'a b'", false),
            [r"C:\tools\x", "'a", "b'"]
        );
    }

    #[test]
    fn reads_quotes_and_backslashes_as_a_posix_shell_does() {
        assert_eq!(
            split_args(r#"-c 'echo "hi there"' --rc=a\ b"#, true),
            ["-c", r#"echo "hi there""#, "--rc=a b"]
        );
        assert_eq!(split_args(r#""say \"hi\" \n""#, true), [r#"say "hi" \n"#]);
        assert_eq!(split_args(r#"-a "" -b"#, true), ["-a", "", "-b"]);
    }

    #[test]
    fn runs_an_open_quote_to_the_end() {
        assert_eq!(split_args(r#"-c "echo hi"#, false), ["-c", "echo hi"]);
    }
}
