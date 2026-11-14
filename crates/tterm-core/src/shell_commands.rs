//! Splitting the compose bar's text into the commands a shell reads, from
//! `src/lib/shellCommands.ts`.
//!
//! Running the text one command at a time must send a command that goes on
//! over several lines as one: a line ending in a backslash, an open quote, a
//! here-document, or a block such as `if ... fi`, `for ... done`, `case ...
//! esac` or a braced one (functions, PowerShell). Sent a line at a time, such a
//! command leaves the shell at its continuation prompt, which marks no prompt,
//! and the run would wait there for good. Only as much of the shell's grammar
//! is read as that takes.

use std::sync::LazyLock;

use regress::Regex;
use tterm_terminal::js_text::js_trim;

/// Words that start a block once `then`, `do` or a brace follows.
const LEADS: &[&str] = &["if", "elif", "while", "until", "for", "select"];

/// Words after which the next word is a command, so a keyword can follow.
const PREFIXES: &[&str] = &[
    "then", "do", "else", "elif", "if", "while", "until", "!", "time", "{",
];

static BREAKS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r#"[\s;&|()<>'"`\\]"#, "").ok());
static COMMENT_BREAKS: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"[\s;&|()]", "").ok());
static WHITESPACE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::with_flags(r"\s", "").ok());
static CASE_IN: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"\sin(\s|$)", "").ok());
static HEREDOC: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r#"^<<(-?)[ \t]*\\?(['"]?)([^\s'"\\;&|<>()]+)\2"#, "").ok());

#[derive(Debug)]
struct ScanState {
    quote: Option<char>,
    /// Open blocks.
    blocks: u32,
    /// `if`, `while` and the like still waiting for their `then`, `do` or
    /// brace.
    leads: u32,
    /// Open parentheses; a case pattern's lone `)` closes none.
    parens: u32,
    /// Here-documents whose bodies follow, with the line that ends each.
    heredocs: Vec<HereDoc>,
}

#[derive(Debug, Clone)]
struct HereDoc {
    word: String,
    strip_tabs: bool,
}

/// Notes a keyword at the start of a command.
fn note_keyword(word: &str, rest: &str, state: &mut ScanState) {
    if LEADS.contains(&word) {
        state.leads = state.leads.saturating_add(1);
        // `elif` closes the branch `then` opened, and opens one of its own.
        if word == "elif" {
            state.blocks = state.blocks.saturating_sub(1);
        }
    } else if (word == "then" || word == "do") && state.leads > 0 {
        state.leads -= 1;
        state.blocks = state.blocks.saturating_add(1);
    } else if word == "case" && regex_finds(&CASE_IN, rest) {
        state.blocks = state.blocks.saturating_add(1);
    } else if matches!(word, "fi" | "done" | "esac") {
        state.blocks = state.blocks.saturating_sub(1);
    } else if word == "end" && state.leads > 0 {
        state.leads -= 1;
    }
}

/// Reads one line; true when it ends in a backslash, so the next goes on with
/// it.
fn scan_line(line: &str, state: &mut ScanState) -> bool {
    let mut command_start = true;
    let mut i = 0;
    while i < line.len() {
        let rest = &line[i..];
        let Some(ch) = rest.chars().next() else {
            break;
        };
        if let Some(quote) = state.quote {
            if ch == '\\' && quote != '\'' {
                i += 1;
                i += char_len(rest);
            } else {
                if ch == quote {
                    state.quote = None;
                }
                i += ch.len_utf8();
            }
            continue;
        }

        if ch == '\\' {
            if i + ch.len_utf8() >= line.len() {
                return true;
            }
            i += ch.len_utf8();
            i += char_len(&line[i..]);
            command_start = false;
        } else if matches!(ch, '\'' | '"' | '`') {
            state.quote = Some(ch);
            command_start = false;
            i += ch.len_utf8();
        } else if ch == '#' && (i == 0 || comment_break_before(line, i)) {
            return false;
        } else if matches!(ch, ';' | '&' | '|' | '(' | ')') {
            if ch == '(' {
                state.parens = state.parens.saturating_add(1);
            } else if ch == ')' && state.parens > 0 {
                state.parens -= 1;
            }
            command_start = true;
            i += 1;
        } else if rest.starts_with("<<<") {
            // A here-string, on its own line.
            i += 3;
        } else if rest.starts_with("<<") {
            if let Some((heredoc, length)) = read_heredoc(rest) {
                state.heredocs.push(heredoc);
                i += length;
            } else {
                i += 2;
            }
            command_start = false;
        } else if char_is_whitespace(rest) || ch == '<' || ch == '>' {
            i += ch.len_utf8();
        } else {
            let start = i;
            while i < line.len() && !char_is_break(&line[i..]) {
                i += char_len(&line[i..]);
            }
            let word = &line[start..i];
            // Glued to a quote, as in `if"x"`, it is no keyword.
            let whole = i == line.len()
                || !matches!(line[i..].chars().next(), Some('\'' | '"' | '`' | '\\'));
            if word == "{" {
                if state.leads > 0 {
                    state.leads -= 1;
                }
                state.blocks = state.blocks.saturating_add(1);
            } else if word == "}" {
                state.blocks = state.blocks.saturating_sub(1);
            } else if command_start && whole {
                note_keyword(word, &line[i..], state);
            }
            command_start = command_start && PREFIXES.contains(&word);
        }
    }
    false
}

/// The commands in `text`, each a line or the lines that go on with it.
/// Blank lines between commands are dropped, as they run nothing.
pub fn split_shell_commands(text: &str) -> Vec<String> {
    let mut state = ScanState {
        quote: None,
        blocks: 0,
        leads: 0,
        parens: 0,
        heredocs: Vec::new(),
    };
    let mut commands = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in split_lines(text) {
        if current.is_empty() && js_trim(line).is_empty() {
            continue;
        }
        current.push(line);
        let body = state
            .heredocs
            .first()
            .map(|heredoc| (heredoc.word.clone(), heredoc.strip_tabs));
        let mut continues = false;
        if let Some((word, strip_tabs)) = body {
            // Opened on an earlier line: this one belongs to the document.
            let compared = if strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if compared == word {
                state.heredocs.remove(0);
            }
        } else {
            continues = scan_line(line, &mut state);
        }
        let open = continues
            || state.quote.is_some()
            || state.blocks > 0
            || state.leads > 0
            || state.parens > 0
            || !state.heredocs.is_empty();
        if !open {
            commands.push(current.join("\n"));
            current.clear();
        }
    }
    // A command left open still goes, for the shell to say what it lacks.
    if !current.is_empty() {
        commands.push(current.join("\n"));
    }
    commands
}

/// Splits on CRLF, CR or LF, as `/\r\n?|\n/` does.
fn split_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                lines.push(&text[start..i]);
                i += 1;
                if i < bytes.len() && bytes[i] == b'\n' {
                    i += 1;
                }
                start = i;
            }
            b'\n' => {
                lines.push(&text[start..i]);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    lines.push(&text[start..]);
    lines
}

fn char_len(text: &str) -> usize {
    text.chars().next().map_or(0, char::len_utf8)
}

fn regex_finds(regex: &LazyLock<Option<Regex>>, text: &str) -> bool {
    regex
        .as_ref()
        .is_some_and(|regex| regex.find(text).is_some())
}

fn regex_matches_start(regex: &LazyLock<Option<Regex>>, text: &str) -> bool {
    regex
        .as_ref()
        .is_some_and(|regex| regex.find(text).is_some_and(|found| found.start() == 0))
}

fn char_is_break(text: &str) -> bool {
    regex_matches_start(&BREAKS, text)
}

fn char_is_whitespace(text: &str) -> bool {
    regex_matches_start(&WHITESPACE, text)
}

fn comment_break_before(line: &str, index: usize) -> bool {
    line[..index]
        .chars()
        .next_back()
        .is_some_and(|previous| regex_finds(&COMMENT_BREAKS, &previous.to_string()))
}

/// Reads a here-document opener at the start of `text`, returning the document
/// it opens and the bytes the opener takes.
fn read_heredoc(text: &str) -> Option<(HereDoc, usize)> {
    let found = HEREDOC.as_ref()?.find(text)?;
    if found.start() != 0 {
        return None;
    }
    let strip_tabs = found.group(1).is_some_and(|group| &text[group] == "-");
    let word = found.group(3).map(|group| text[group].to_owned())?;
    Some((HereDoc { word, strip_tabs }, found.end()))
}

#[cfg(test)]
mod tests;
