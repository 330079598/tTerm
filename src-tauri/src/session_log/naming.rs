//! Log file names: the name template filled in for a session, and the paths
//! of a log's parts.

use super::{library, LogFiles, SessionMetadata};
use chrono::Local;
use std::path::{Path, PathBuf};

/// The template's parts: folders, then the file name. `/` and `\\` both
/// separate them; empty parts are dropped.
pub(super) fn template_parts(template: &str) -> impl Iterator<Item = &str> {
    template
        .split(['/', '\\'])
        .filter(|part| !part.trim().is_empty())
}

/// The base name of a session's files, relative to the log directory: the
/// template's folders and file name with their variables filled in, joined
/// by `/`. A folder that comes out empty is left out.
pub(super) fn render_name(template: &str, metadata: &SessionMetadata) -> String {
    let now = Local::now();
    let render = |part: &str| {
        part.replace("{profile}", &metadata.profile)
            .replace("{host}", &metadata.host)
            .replace("{port}", &metadata.port.to_string())
            .replace("{username}", &metadata.username)
            .replace("{type}", &metadata.session_type)
            .replace("{year}", &now.format("%Y").to_string())
            .replace("{month}", &now.format("%m").to_string())
            .replace("{day}", &now.format("%d").to_string())
            .replace("{date}", &now.format("%Y%m%d").to_string())
            .replace("{time}", &now.format("%H%M%S").to_string())
            .replace(
                "{yyyyMMdd-HHmmss}",
                &now.format("%Y%m%d-%H%M%S").to_string(),
            )
            .replace("{sessionId}", &metadata.tab_id)
    };
    let mut parts: Vec<&str> = template_parts(template).collect();
    let file = parts.pop().unwrap_or_default();
    let mut rendered: Vec<String> = parts
        .into_iter()
        .filter_map(|folder| {
            let name = sanitize_name_part(&render(folder));
            (!name.is_empty()).then_some(name)
        })
        .collect();
    rendered.push(sanitize_file_name(&render(file)));
    rendered.join("/")
}

pub(super) fn sanitize_file_name(value: &str) -> String {
    let name = sanitize_name_part(value);
    if name.is_empty() {
        "terminal-session".to_string()
    } else {
        name
    }
}

/// `value` as one file or folder name on every platform; empty if nothing is
/// left of it.
fn sanitize_name_part(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    sanitized
        .trim()
        .trim_matches('.')
        .trim()
        .chars()
        .take(180)
        .collect()
}

pub(super) fn unique_base_name(directory: &Path, requested: &str, files: &LogFiles) -> String {
    for index in 0u32.. {
        let candidate = if index == 0 {
            requested.to_string()
        } else {
            format!("{requested}-{index}")
        };
        let raw_exists = files.wants_raw()
            && path_or_compressed_exists(&part_path(directory, &candidate, "tlog", 0));
        let plain_exists = files.wants_plain()
            && path_or_compressed_exists(&part_path(directory, &candidate, "log", 0));
        if !raw_exists && !plain_exists {
            return candidate;
        }
    }
    unreachable!()
}

pub(super) fn path_or_compressed_exists(path: &Path) -> bool {
    path.exists() || compressed_path(path).exists()
}

pub(super) fn compressed_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.gz", path.to_string_lossy()))
}

/// A log file's path; `base_name` may start with folders, joined by `/`.
pub(super) fn part_path(directory: &Path, base_name: &str, extension: &str, part: u32) -> PathBuf {
    if part == 0 {
        library::join_relative(directory, &format!("{base_name}.{extension}"))
    } else {
        library::join_relative(directory, &format!("{base_name}.{part:03}.{extension}"))
    }
}
