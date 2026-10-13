//! Reads an SSH config file with its `Include` directives expanded in place.

use super::paths::expand_home_path;
use super::words::{split_ssh_words, strip_inline_comment};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

fn describe_ssh_config_read_error(path: &Path, error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            format!("SSH config file not found: {}", path.display())
        }
        std::io::ErrorKind::PermissionDenied => {
            format!(
                "Permission denied while reading SSH config file: {}",
                path.display()
            )
        }
        _ => format!(
            "Failed to read SSH config file '{}': {}",
            path.display(),
            error
        ),
    }
}

fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?')
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    fn matches(pattern: &[char], value: &[char]) -> bool {
        if pattern.is_empty() {
            return value.is_empty();
        }

        match pattern[0] {
            '*' => {
                matches(&pattern[1..], value)
                    || (!value.is_empty() && matches(pattern, &value[1..]))
            }
            '?' => !value.is_empty() && matches(&pattern[1..], &value[1..]),
            ch => !value.is_empty() && ch == value[0] && matches(&pattern[1..], &value[1..]),
        }
    }

    matches(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
    )
}

fn expand_include_source(pattern: &str, base_dir: &Path) -> Vec<PathBuf> {
    let source = if pattern.starts_with('~') {
        expand_home_path(pattern)
    } else {
        let path = PathBuf::from(pattern);
        if path.is_absolute() {
            path
        } else {
            base_dir.join(path)
        }
    };

    if !has_glob_pattern(&source.to_string_lossy()) {
        return vec![source];
    }

    let mut candidates = vec![PathBuf::new()];
    for component in source.components() {
        match component {
            Component::Prefix(prefix) => {
                candidates = vec![PathBuf::from(prefix.as_os_str())];
            }
            Component::RootDir => {
                for candidate in &mut candidates {
                    candidate.push(component.as_os_str());
                }
            }
            Component::CurDir => {}
            Component::ParentDir => {
                for candidate in &mut candidates {
                    candidate.push("..");
                }
            }
            Component::Normal(part) => {
                let part = part.to_string_lossy().to_string();
                let mut next = Vec::new();
                if has_glob_pattern(&part) {
                    for candidate in &candidates {
                        if let Ok(entries) = fs::read_dir(candidate) {
                            for entry in entries.flatten() {
                                let name = entry.file_name().to_string_lossy().to_string();
                                if wildcard_matches(&part, &name) {
                                    next.push(entry.path());
                                }
                            }
                        }
                    }
                } else {
                    for mut candidate in candidates {
                        candidate.push(&part);
                        next.push(candidate);
                    }
                }
                candidates = next;
            }
        }
    }

    candidates.sort();
    candidates
}

pub(super) fn read_ssh_config_with_includes(
    source: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: usize,
) -> Result<(String, Vec<String>), String> {
    const MAX_INCLUDE_DEPTH: usize = 16;

    let canonical_source = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf());
    if !visited.insert(canonical_source.clone()) {
        return Ok((
            String::new(),
            vec![format!(
                "Include loop skipped for SSH config file: {}",
                source.display()
            )],
        ));
    }

    if depth > MAX_INCLUDE_DEPTH {
        visited.remove(&canonical_source);
        return Ok((
            String::new(),
            vec![format!(
                "Include depth limit reached while reading SSH config file: {}",
                source.display()
            )],
        ));
    }

    let content =
        fs::read_to_string(source).map_err(|e| describe_ssh_config_read_error(source, &e))?;
    let base_dir = source.parent().unwrap_or_else(|| Path::new("."));
    let mut expanded = String::new();
    let mut warnings = Vec::new();

    for (line_index, raw_line) in content.lines().enumerate() {
        let line = strip_inline_comment(raw_line);
        let mut parts = split_ssh_words(&line);
        let is_include = parts
            .first()
            .map(|key| key.eq_ignore_ascii_case("include"))
            .unwrap_or(false);

        if !is_include {
            expanded.push_str(raw_line);
            expanded.push('\n');
            continue;
        }

        parts.remove(0);
        if parts.is_empty() {
            warnings.push(format!(
                "{}:{}: Include has no path and was ignored.",
                source.display(),
                line_index + 1
            ));
            continue;
        }

        for include_pattern in parts {
            let include_paths = expand_include_source(&include_pattern, base_dir);
            let readable_paths: Vec<PathBuf> = include_paths
                .into_iter()
                .filter(|path| path.is_file())
                .collect();

            if readable_paths.is_empty() {
                warnings.push(format!(
                    "{}:{}: Include '{}' did not match any readable files.",
                    source.display(),
                    line_index + 1,
                    include_pattern
                ));
                continue;
            }

            for include_path in readable_paths {
                match read_ssh_config_with_includes(&include_path, visited, depth + 1) {
                    Ok((included_content, mut include_warnings)) => {
                        expanded.push_str(&included_content);
                        warnings.append(&mut include_warnings);
                    }
                    Err(error) => warnings.push(error),
                }
            }
        }
    }

    visited.remove(&canonical_source);
    Ok((expanded, warnings))
}
