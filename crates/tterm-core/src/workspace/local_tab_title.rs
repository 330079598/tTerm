//! Local tab titles, from `src/lib/localTabTitle.ts`.

use std::sync::LazyLock;

use indexmap::IndexMap;
use regress::{Flags, Regex};

use crate::workspace::pane_templates::js_trim;
use crate::workspace::tab::{Tab, TabConnection, TabType, TerminalShellType};

/// The shells' legacy automatic titles.
const LEGACY_LOCAL_TITLES: &[&str] = &[
    "OS terminal",
    "Terminal",
    "Local terminal",
    "终端",
    "本地终端",
];

/// A running command title longer than this shows as its program alone.
pub const MAX_COMMAND_TITLE_LENGTH: usize = 20;

/// Normalizes a restored local tab after the platform resolves its shell name.
pub fn normalize_saved_local_tab(tab: &Tab, shell_name: &str) -> Tab {
    if tab.tab_type != TabType::Terminal {
        return tab.clone();
    }
    let mut candidate = crate::workspace::tab_icons::normalize_saved_icon_fields(tab);
    let current = serde_json::to_value(tab).ok();
    let has_shell_name = current
        .as_ref()
        .and_then(|value| value.get("shellName"))
        .is_some_and(|value| match value {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(value) => *value,
            serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
            serde_json::Value::String(value) => !value.is_empty(),
            _ => true,
        });
    if !has_shell_name {
        candidate.set_shell_name(Some(shell_name.to_string()));
    }
    candidate
}

static GIT_BASH_DRIVE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^/([A-Za-z])(/|$)").ok());
static DRIVE_PREFIX: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[A-Za-z]:").ok());
static DRIVE_ROOT: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^[A-Za-z]:$").ok());
static COMMAND_RUNNER: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^(?:sudo|&|\w+=\S*)\s+").ok());
static QUOTED_PROGRAM: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r#"^(["'])(.*?)\1"#).ok());
static WHITESPACE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\s").ok());
static WHITESPACE_RUN: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"\s+").ok());
static PROGRAM_PATH: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^.*[\\/]").ok());
static PROGRAM_EXTENSION: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        r"\.(exe|cmd|bat|com|ps1)$",
        Flags {
            icase: true,
            ..Flags::default()
        },
    )
    .ok()
});
static LEGACY_SUFFIX: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"-\d+$").ok());

/// What a local tab shows for its shell, using the WSL distribution in place of
/// `WSL`.
pub fn local_shell_title(
    shell_name: Option<&str>,
    connection: Option<&TabConnection>,
) -> Option<String> {
    let distro = connection
        .filter(|connection| connection.terminal_shell == Some(TerminalShellType::Wsl))
        .and_then(|connection| connection.terminal_shell_wsl_distro.as_deref())
        .map(js_trim)
        .filter(|distro| !distro.is_empty());
    distro.or_else(|| shell_name.map(str::to_owned))
}

fn comparable_path(path: &str) -> String {
    let mut slashed = path.replace('\\', "/");
    if let Some(regex) = GIT_BASH_DRIVE.as_ref() {
        slashed = regex.replace(&slashed, "$1:/");
    }
    let trimmed = slashed.trim_end_matches('/');
    if DRIVE_PREFIX
        .as_ref()
        .is_some_and(|regex| regex.find(trimmed).is_some())
    {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    }
}

/// The title for a shell in `cwd`: the directory's name, `~` for the home
/// directory, or the root itself.
pub fn directory_title(cwd: &str, home_directory: Option<&str>) -> Option<String> {
    let trimmed = js_trim(cwd);
    if trimmed.is_empty() {
        return None;
    }
    let path = trimmed.trim_end_matches(['\\', '/']);
    if let Some(home) = home_directory.filter(|home| !home.is_empty())
        && comparable_path(path) == comparable_path(home)
    {
        return Some("~".to_string());
    }
    if path.is_empty() {
        return Some(if trimmed.starts_with('\\') {
            "\\".to_string()
        } else {
            "/".to_string()
        });
    }
    if DRIVE_ROOT
        .as_ref()
        .is_some_and(|regex| regex.find(path).is_some())
    {
        return Some(format!("{path}\\"));
    }
    let start = path.rfind(['/', '\\']).map_or(0, |index| index + 1);
    Some(path[start..].to_string())
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// The title for a running command: the whole command when short, otherwise
/// its program alone.
pub fn command_title(command: &str) -> Option<String> {
    let mut rest = js_trim(command);
    if let Some(regex) = COMMAND_RUNNER.as_ref() {
        loop {
            if regex.find(&rest).is_none() {
                break;
            }
            rest = regex.replace(&rest, "");
        }
    }
    let (first, arguments) = if let Some(regex) = QUOTED_PROGRAM.as_ref()
        && let Some(quoted) = regex.find(&rest)
        && let Some(group) = quoted.group(2)
    {
        (&rest[group.clone()], &rest[quoted.end()..])
    } else if let Some(regex) = WHITESPACE.as_ref()
        && let Some(whitespace) = regex.find(&rest)
    {
        (&rest[..whitespace.start()], &rest[whitespace.end()..])
    } else {
        (rest.as_str(), "")
    };
    let arguments = WHITESPACE_RUN.as_ref().map_or_else(
        || js_trim(arguments),
        |regex| regex.replace_all(&js_trim(arguments), " "),
    );
    let mut program = first.to_string();
    if let Some(regex) = PROGRAM_PATH.as_ref() {
        program = regex.replace(&program, "");
    }
    if let Some(regex) = PROGRAM_EXTENSION.as_ref() {
        program = regex.replace(&program, "");
    }
    if program.is_empty() {
        return None;
    }
    let whole = if arguments.is_empty() {
        program.clone()
    } else {
        format!("{program} {arguments}")
    };
    Some(if utf16_len(&whole) <= MAX_COMMAND_TITLE_LENGTH {
        whole
    } else {
        program
    })
}

/// What a local tab named automatically shows while its shell runs a command.
pub fn running_command_title(tab: &Tab, commands: &IndexMap<String, String>) -> Option<String> {
    if !tab.auto_title.unwrap_or(false) {
        return None;
    }
    let command = commands.get(&tab.id)?;
    let title = command_title(command)?;
    for (tab_id, other) in commands {
        if tab_id != &tab.id && command_title(other).as_deref() == Some(title.as_str()) {
            return Some(format!("{title} · {}", tab.title));
        }
    }
    Some(title)
}

fn cwd_title(cwd: Option<&str>, home_directory: Option<&str>) -> Option<String> {
    cwd.filter(|cwd| !cwd.is_empty())
        .and_then(|cwd| directory_title(cwd, home_directory))
}

/// The tab with the directory its local shell reported.
pub fn with_local_cwd(tab: &Tab, cwd: &str, home_directory: Option<&str>) -> Tab {
    let title = if tab.auto_title.unwrap_or(false) {
        cwd_title(Some(cwd), home_directory)
    } else {
        None
    }
    .unwrap_or_else(|| tab.title.clone());
    if tab
        .connection
        .as_ref()
        .and_then(|connection| connection.cwd.as_deref())
        == Some(cwd)
        && title == tab.title
    {
        return tab.clone();
    }
    let mut next = tab.clone();
    next.set_title(title);
    let mut connection = tab.connection.clone().unwrap_or_else(|| {
        let mut connection = TabConnection::default();
        connection.set_connection_type(Some(crate::workspace::tab::ConnectionType::Terminal));
        connection
    });
    connection.set_cwd(Some(cwd.to_string()));
    next.set_connection(Some(connection));
    next
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn distinct_suffix(cwd: &str, others: &[Vec<&str>]) -> String {
    let segments = path_segments(cwd);
    let comparable_path = comparable_path(cwd);
    let comparable = path_segments(&comparable_path);
    let separator = if cwd.contains('\\') { '\\' } else { '/' };
    for count in 2..=segments.len() {
        let suffix = comparable[comparable.len().saturating_sub(count)..].join("/");
        if others
            .iter()
            .all(|other| other[other.len().saturating_sub(count)..].join("/") != suffix)
        {
            return segments[segments.len().saturating_sub(count)..].join(&separator.to_string());
        }
    }
    js_trim(cwd)
}

/// Titles for automatically named local tabs, set apart where shells sit in
/// different directories of the same name.
pub fn distinct_local_titles(tabs: &[Tab], home_directory: Option<&str>) -> Vec<Tab> {
    let mut groups: IndexMap<String, Vec<(usize, String)>> = IndexMap::new();
    for (index, tab) in tabs.iter().enumerate() {
        let Some(cwd) = tab
            .connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref())
            .filter(|cwd| !cwd.is_empty())
        else {
            continue;
        };
        if !tab.auto_title.unwrap_or(false) {
            continue;
        }
        let Some(base) = directory_title(cwd, home_directory) else {
            continue;
        };
        groups
            .entry(base)
            .or_default()
            .push((index, cwd.to_string()));
    }

    let mut titles: IndexMap<String, String> = IndexMap::new();
    for (base, members) in &groups {
        let mut paths: Vec<String> = Vec::new();
        for (_, cwd) in members {
            let path = comparable_path(cwd);
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        for (index, cwd) in members {
            let own = comparable_path(cwd);
            let others: Vec<Vec<&str>> = paths
                .iter()
                .filter(|path| **path != own)
                .map(|path| path_segments(path))
                .collect();
            let title = if others.is_empty() {
                base.clone()
            } else {
                distinct_suffix(cwd, &others)
            };
            titles.insert(tabs[*index].id.clone(), title);
        }
    }

    let mut changed = false;
    let next: Vec<Tab> = tabs
        .iter()
        .map(|tab| {
            let Some(title) = titles.get(&tab.id) else {
                return tab.clone();
            };
            if title == &tab.title {
                return tab.clone();
            }
            changed = true;
            let mut next = tab.clone();
            next.set_title(title.clone());
            next
        })
        .collect();
    if changed { next } else { tabs.to_vec() }
}

/// [`distinct_local_titles`] with the user's home directory.
pub fn with_distinct_local_titles(tabs: &[Tab], home_directory: Option<&str>) -> Vec<Tab> {
    distinct_local_titles(tabs, home_directory)
}

/// A tab's tooltip: its title with the host or directory and shell below it.
pub fn tab_tooltip(tab: &Tab) -> String {
    let details: Vec<Option<String>> = if tab.tab_type == TabType::Terminal {
        vec![
            tab.connection
                .as_ref()
                .and_then(|connection| connection.cwd.clone()),
            local_shell_title(tab.shell_name.as_deref(), tab.connection.as_ref()),
        ]
    } else {
        vec![
            tab.connection
                .as_ref()
                .and_then(|connection| connection.host.clone()),
        ]
    };
    let detail = details
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty() && part != &tab.title)
        .collect::<Vec<String>>()
        .join(" · ");
    if detail.is_empty() {
        tab.title.clone()
    } else {
        format!("{} ({detail})", tab.title)
    }
}

/// Whether a local tab carries one of the old automatic titles.
pub fn has_legacy_local_title(tab: &Tab) -> bool {
    if tab.tab_type != TabType::Terminal {
        return false;
    }
    let base = LEGACY_SUFFIX
        .as_ref()
        .map_or_else(|| tab.title.clone(), |regex| regex.replace(&tab.title, ""));
    LEGACY_LOCAL_TITLES.contains(&base.as_str())
}
