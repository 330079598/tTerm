//! Recognizes password prompts on the terminal's cursor line.

use std::sync::LazyLock;

use regress::{Match, Regex};

use crate::js_text::{from_utf16_units_lossy, js_trim, utf16_len, utf16_units};
use crate::snapshot::Snapshot;

/// A recognized password prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasswordPromptMatch {
    /// The prompt as displayed, trimmed; the backend checks it is still
    /// waiting.
    pub prompt: String,
    /// The account whose password is requested, or `None` when the prompt
    /// does not say.
    pub user: Option<String>,
}

/// A long line cannot be a password prompt; skip matching it.
pub const MAX_PROMPT_CHARS: usize = 256;

static NAMED_USER_PROMPTS: LazyLock<[Option<Regex>; 3]> = LazyLock::new(|| {
    [
        Regex::new(r"^\[sudo\] password for '?(\S+?)'?:$").ok(),
        Regex::new(r"^\[sudo\] (\S+) 的密[码碼][:：]$").ok(),
        Regex::new(r"^doas \((\S+?)@[^)]*\) password:$").ok(),
    ]
});
static LOCALIZED_SUDO_PROMPT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\[sudo\] .+[:：]$").ok());
static SUDO_RS_PROMPT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\[sudo: authenticate\] (.+)[:：]$").ok());
static PAM_PASSWORD_WORD: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"password|passwort|passwd|mot de passe|contraseña|senha|пароль|密码|密碼|パスワード|비밀번호|암호",
        "i",
    )
    .ok()
});

/// Compiles user-configured prompt patterns, dropping invalid ones.
///
/// A named group `user` identifies the account; without it the prompt is
/// matched for any user. The patterns follow ECMAScript syntax through
/// `regress`, which does not yet implement Unicode property escapes.
pub fn compile_prompt_patterns(patterns: &[String]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|pattern| {
            let source = js_trim(pattern);
            if source.is_empty() {
                return None;
            }
            Regex::new(source).ok()
        })
        .collect()
}

/// Whether `pattern` is a regular expression the frontend would accept.
pub fn is_valid_prompt_pattern(pattern: &str) -> bool {
    Regex::new(pattern).is_ok()
}

/// Matches a terminal line against the built-in and custom password prompts.
///
/// `known_user` is the session's login name, used to recognize localized
/// sudo prompts whose wording is not known ahead of time.
pub fn match_password_prompt(
    line: &str,
    known_user: Option<&str>,
    custom_patterns: &[Regex],
) -> Option<PasswordPromptMatch> {
    let prompt = js_trim(line);
    if prompt.is_empty() || utf16_len(prompt) > MAX_PROMPT_CHARS {
        return None;
    }
    if !prompt.ends_with(':') && !prompt.ends_with('：') {
        return None;
    }
    let units = utf16_units(prompt);

    for pattern in NAMED_USER_PROMPTS.iter().flatten() {
        if let Some(found) = pattern.find_from_ucs2(&units, 0).next() {
            return Some(PasswordPromptMatch {
                prompt: prompt.to_owned(),
                user: group_text(&units, &found, 1),
            });
        }
    }

    if let Some(pattern) = SUDO_RS_PROMPT.as_ref()
        && let Some(found) = pattern.find_from_ucs2(&units, 0).next()
        && let Some(description) = group_text(&units, &found, 1)
        && PAM_PASSWORD_WORD
            .as_ref()
            .and_then(|word| word.find_from_ucs2(&utf16_units(&description), 0).next())
            .is_some()
    {
        return Some(PasswordPromptMatch {
            prompt: prompt.to_owned(),
            user: None,
        });
    }

    if let Some(known_user) = known_user.filter(|known_user| !known_user.is_empty())
        && LOCALIZED_SUDO_PROMPT
            .as_ref()
            .and_then(|pattern| pattern.find_from_ucs2(&units, 0).next())
            .is_some()
    {
        let rest = prompt.strip_prefix("[sudo] ").unwrap_or(prompt);
        let token = format!(r"(^|[\s(]){}($|[\s):：'’])", escape_reg_exp(known_user));
        if let Ok(pattern) = Regex::new(&token)
            && pattern
                .find_from_ucs2(&utf16_units(rest), 0)
                .next()
                .is_some()
        {
            return Some(PasswordPromptMatch {
                prompt: prompt.to_owned(),
                user: Some(known_user.to_owned()),
            });
        }
    }

    for pattern in custom_patterns {
        if let Some(found) = pattern.find_from_ucs2(&units, 0).next() {
            return Some(PasswordPromptMatch {
                prompt: prompt.to_owned(),
                user: found
                    .named_group("user")
                    .map(|range| from_utf16_units_lossy(&units[range])),
            });
        }
    }
    None
}

/// Reads the visible text before the cursor, including earlier wrapped rows.
///
/// The frontend reads xterm's buffer here. In the native model the snapshot
/// has the visible rows and the cursor; a prompt that began above the
/// viewport is not in the frame, which the shell integration that starts
/// sessions does not produce.
pub fn read_cursor_line(snapshot: &Snapshot) -> String {
    let cursor = snapshot.cursor().point;
    let Some(row) = snapshot.screen_row(cursor.line) else {
        return String::new();
    };
    let mut text = row_text(snapshot, row, cursor.column, true);
    let mut current_row = row;
    while current_row > 0
        && utf16_len(&text) <= MAX_PROMPT_CHARS
        && row_is_wrapped(snapshot, current_row)
    {
        current_row -= 1;
        text = format!(
            "{}{}",
            row_text(snapshot, current_row, usize::MAX, false),
            text
        );
    }
    text
}

fn row_text(snapshot: &Snapshot, row: usize, end: usize, trim_right: bool) -> String {
    let Some(line) = snapshot.row(row) else {
        return String::new();
    };
    let end = end.min(line.len());
    let mut text = String::new();
    for cell in &line.cells()[..end] {
        if cell.attributes.wide_spacer || cell.attributes.leading_wide_spacer {
            continue;
        }
        text.push(cell.ch);
        if let Some(zerowidth) = &cell.zerowidth {
            text.extend(zerowidth.iter().copied());
        }
    }
    if trim_right {
        text = text.trim_end_matches([' ', '\t']).to_owned();
    }
    text
}

fn row_is_wrapped(snapshot: &Snapshot, row: usize) -> bool {
    let Some(previous) = row.checked_sub(1).and_then(|line| snapshot.row(line)) else {
        return false;
    };
    previous
        .cells()
        .last()
        .is_some_and(|cell| cell.attributes.wrapline)
}

fn group_text(units: &[u16], found: &Match, index: usize) -> Option<String> {
    found
        .group(index)
        .map(|range| from_utf16_units_lossy(&units[range]))
}

fn escape_reg_exp(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '.' | '*' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests;
