//! Splits SSH config lines into words the way OpenSSH quotes them.

/// The keyword of a line and the rest after it, which OpenSSH separates with
/// spaces, an `=`, or both: `Port 22`, `Port=22` and `Port = 22`.
pub(super) fn split_keyword(line: &str) -> (&str, &str) {
    let line = line.trim_start();
    let end = line
        .find(|ch: char| ch.is_whitespace() || ch == '=')
        .unwrap_or(line.len());
    let rest = line[end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim_start();
    (&line[..end], rest)
}

pub(super) fn strip_inline_comment(line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut output = String::new();

    for ch in line.chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            output.push(ch);
            escaped = true;
            continue;
        }

        if ch == '\'' && !in_double {
            in_single = !in_single;
            output.push(ch);
            continue;
        }

        if ch == '"' && !in_single {
            in_double = !in_double;
            output.push(ch);
            continue;
        }

        if ch == '#' && !in_single && !in_double {
            break;
        }

        output.push(ch);
    }

    output.trim().to_string()
}

pub(super) fn split_ssh_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in value.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if ch == '\'' && !in_double {
            in_single = !in_single;
            continue;
        }

        if ch == '"' && !in_single {
            in_double = !in_double;
            continue;
        }

        if ch.is_whitespace() && !in_single && !in_double {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}
