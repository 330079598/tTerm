//! Opens an SSH config file and the files its `Include` lines name.

use super::parse::{SshConfig, parse_ssh_config_with};
use super::paths::{expand_home_path, home_dir};
use super::pattern::{has_glob_pattern, wildcard_matches};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// How deep includes may nest, as `READCONF_MAX_DEPTH` in OpenSSH.
const MAX_INCLUDE_DEPTH: usize = 16;

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

/// Finds and opens the files `Include` lines name while a config is parsed.
pub(super) struct IncludeReader {
    /// Where a relative `Include` path is looked up. `ssh` uses `~/.ssh` for
    /// every user config, whichever file the `Include` is in.
    base_dir: PathBuf,
    /// The files being read, outermost first, so a loop is stopped.
    open: Vec<PathBuf>,
}

impl IncludeReader {
    fn new(base_dir: PathBuf, source: &Path) -> Self {
        Self {
            base_dir,
            open: vec![canonical(source)],
        }
    }

    /// Where a relative `Include` path is looked up.
    pub(super) fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// The readable files an `Include` pattern names, in name order.
    pub(super) fn files(&self, pattern: &str) -> Vec<PathBuf> {
        expand_include_source(pattern, &self.base_dir)
            .into_iter()
            .filter(|path| path.is_file())
            .collect()
    }

    /// Starts reading an included file, or gives the warning about why it is
    /// skipped. A file that is read must be closed with [`Self::leave`].
    pub(super) fn enter(&mut self, path: &Path) -> Result<String, String> {
        let canonical_path = canonical(path);
        if self.open.contains(&canonical_path) {
            return Err(format!(
                "Include loop skipped for SSH config file: {}",
                path.display()
            ));
        }
        if self.open.len() > MAX_INCLUDE_DEPTH {
            return Err(format!(
                "Include depth limit reached while reading SSH config file: {}",
                path.display()
            ));
        }
        let content =
            fs::read_to_string(path).map_err(|e| describe_ssh_config_read_error(path, &e))?;
        self.open.push(canonical_path);
        Ok(content)
    }

    /// Finishes the file [`Self::enter`] opened last.
    pub(super) fn leave(&mut self) {
        self.open.pop();
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Reads a user's SSH config file with its includes, looking relative
/// `Include` paths up in `~/.ssh` as `ssh` does.
pub(super) fn read_ssh_config(source: &Path) -> Result<(SshConfig, Vec<String>), String> {
    let base_dir = home_dir()
        .map(|home| home.join(".ssh"))
        .or_else(|| source.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    read_ssh_config_in(source, base_dir)
}

/// Reads an SSH config file with its includes, looking relative `Include`
/// paths up in `base_dir`.
pub(super) fn read_ssh_config_in(
    source: &Path,
    base_dir: PathBuf,
) -> Result<(SshConfig, Vec<String>), String> {
    let content =
        fs::read_to_string(source).map_err(|e| describe_ssh_config_read_error(source, &e))?;
    let mut reader = IncludeReader::new(base_dir, source);
    Ok(parse_ssh_config_with(&content, Some(&mut reader)))
}
