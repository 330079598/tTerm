//! Archive names and destinations for the SFTP drawer, from
//! `src/components/SftpDrawer/sftpArchive.ts`.

use std::collections::HashSet;

use crate::sftp::entry::SftpDirectoryEntry;
use crate::sftp::internal::api::archive::ArchiveFormat;
use crate::sftp::view::paths::parent_remote_path;

#[cfg(test)]
mod tests;

/// The formats an archive can be made in, the most common first.
pub const COMPRESS_FORMATS: [ArchiveFormat; 5] = [
    ArchiveFormat::TarGz,
    ArchiveFormat::Zip,
    ArchiveFormat::TarXz,
    ArchiveFormat::TarBz2,
    ArchiveFormat::Tar,
];

/// The suffix a new archive of `format` carries.
pub fn compress_extension(format: ArchiveFormat) -> Option<&'static str> {
    match format {
        ArchiveFormat::TarGz => Some(".tar.gz"),
        ArchiveFormat::Zip => Some(".zip"),
        ArchiveFormat::TarXz => Some(".tar.xz"),
        ArchiveFormat::TarBz2 => Some(".tar.bz2"),
        ArchiveFormat::Tar => Some(".tar"),
        ArchiveFormat::TarZst
        | ArchiveFormat::SevenZip
        | ArchiveFormat::Gzip
        | ArchiveFormat::Bzip2
        | ArchiveFormat::Xz => None,
    }
}

// Longer suffixes come first so `.tar.gz` wins over `.gz`.
const EXTRACT_SUFFIXES: [(&str, ArchiveFormat); 15] = [
    (".tar.gz", ArchiveFormat::TarGz),
    (".tgz", ArchiveFormat::TarGz),
    (".tar.bz2", ArchiveFormat::TarBz2),
    (".tbz2", ArchiveFormat::TarBz2),
    (".tbz", ArchiveFormat::TarBz2),
    (".tar.xz", ArchiveFormat::TarXz),
    (".txz", ArchiveFormat::TarXz),
    (".tar.zst", ArchiveFormat::TarZst),
    (".tzst", ArchiveFormat::TarZst),
    (".tar", ArchiveFormat::Tar),
    (".zip", ArchiveFormat::Zip),
    (".7z", ArchiveFormat::SevenZip),
    (".gz", ArchiveFormat::Gzip),
    (".bz2", ArchiveFormat::Bzip2),
    (".xz", ArchiveFormat::Xz),
];

/// Formats that hold one compressed file rather than a tree of entries.
const SINGLE_FILE_FORMATS: [ArchiveFormat; 3] =
    [ArchiveFormat::Gzip, ArchiveFormat::Bzip2, ArchiveFormat::Xz];

fn match_suffix(name: &str) -> Option<(&'static str, ArchiveFormat)> {
    let lower = name.to_lowercase();
    EXTRACT_SUFFIXES
        .into_iter()
        .find(|(suffix, _)| lower.ends_with(suffix))
}

/// The format of an archive file, judged by its name; `None` for other files.
pub fn archive_format_of(entry: &SftpDirectoryEntry) -> Option<ArchiveFormat> {
    if entry.is_dir {
        return None;
    }
    match_suffix(&entry.name)
        .filter(|(suffix, _)| suffix.len() < entry.name.len())
        .map(|(_, format)| format)
}

/// Where an archive unpacks by default: a folder named after it beside it, or
/// the folder it is in for a single compressed file.
pub fn default_extract_destination(archive_path: &str, format: ArchiveFormat) -> String {
    let parent = parent_remote_path(archive_path);
    if SINGLE_FILE_FORMATS.contains(&format) {
        return parent;
    }

    let name = archive_path
        .rfind('/')
        .map(|slash| &archive_path[slash + 1..])
        .unwrap_or(archive_path);
    let stem = match match_suffix(name) {
        Some((suffix, _)) => &name[..name.len() - suffix.len()],
        None => name,
    };
    if parent == "/" {
        format!("/{stem}")
    } else {
        format!("{parent}/{stem}")
    }
}

/// The name offered for a new archive: one entry's own name, or the folder's
/// name when several entries are selected.
pub fn default_archive_name(
    entries: &[SftpDirectoryEntry],
    folder_path: &str,
    format: ArchiveFormat,
) -> Option<String> {
    let extension = compress_extension(format)?;
    let folder_name = folder_path
        .split('/')
        .filter(|name| !name.is_empty())
        .next_back();
    let base = if entries.len() == 1 {
        entries[0].name.as_str()
    } else {
        folder_name.unwrap_or("archive")
    };
    Some(format!("{base}{extension}"))
}

/// Swaps the extension of `name` when it still carries `from`'s suffix.
pub fn with_archive_extension(name: &str, from: ArchiveFormat, to: ArchiveFormat) -> String {
    let (Some(old_extension), Some(new_extension)) =
        (compress_extension(from), compress_extension(to))
    else {
        return name.to_owned();
    };
    if !name.to_lowercase().ends_with(old_extension) {
        return name.to_owned();
    }
    format!(
        "{}{new_extension}",
        &name[..name.len() - old_extension.len()]
    )
}

/// The folder holding all of `entries`, or `None` when they are in several.
pub fn common_parent(entries: &[SftpDirectoryEntry]) -> Option<String> {
    let parents: HashSet<String> = entries
        .iter()
        .map(|entry| parent_remote_path(&entry.path))
        .collect();
    (parents.len() == 1)
        .then(|| parents.into_iter().next())
        .flatten()
}
