//! The shell scripts that make and unpack archives on the host. Paths go in
//! quoted, and each script checks for its tool first, so a missing tool
//! exits 127 rather than failing somewhere in the middle.

use super::ArchiveFormat;
use crate::sftp::internal::exec::{shell_path, shell_quote};

/// The exit status a script ends with when the host lacks its tool.
pub const MISSING_TOOL_STATUS: u32 = 127;

pub struct ArchiveScript {
    pub text: String,
    /// The command the host must have, named when it has not.
    pub tool: &'static str,
}

/// Checks that `name` is one entry of a folder rather than a path.
pub fn check_entry_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        Err(format!("'{name}' is not a valid file name"))
    } else {
        Ok(())
    }
}

fn tool_check(tool: &str) -> String {
    format!("command -v {tool} >/dev/null 2>&1 || exit {MISSING_TOOL_STATUS}\n")
}

/// Packs `names`, entries of `directory`, into `archive_name` in the same
/// folder. The members are stored relative to `directory`.
pub fn compress_script(
    directory: &str,
    names: &[String],
    archive_name: &str,
    format: ArchiveFormat,
) -> Result<ArchiveScript, String> {
    check_entry_name(archive_name)?;
    if names.is_empty() {
        return Err("Choose at least one item to compress".to_string());
    }
    for name in names {
        check_entry_name(name)?;
    }
    let members = names
        .iter()
        .map(|name| shell_path(name))
        .collect::<Vec<_>>()
        .join(" ");
    let archive = shell_path(archive_name);
    let (tool, command) = match format {
        // -y stores symbolic links as links, as tar does.
        ArchiveFormat::Zip => ("zip", format!("zip -qry {archive} {members}")),
        ArchiveFormat::Tar => ("tar", format!("tar -cf {archive} {members}")),
        ArchiveFormat::TarGz => ("tar", format!("tar -czf {archive} {members}")),
        ArchiveFormat::TarBz2 => ("tar", format!("tar -cjf {archive} {members}")),
        ArchiveFormat::TarXz => ("tar", format!("tar -cJf {archive} {members}")),
        _ => return Err("This archive format can only be extracted".to_string()),
    };
    Ok(ArchiveScript {
        text: format!(
            "cd {} || exit 1\n{}exec {command} </dev/null\n",
            shell_path(directory),
            tool_check(tool),
        ),
        tool,
    })
}

/// The file a single-file compressed archive unpacks to: its name without
/// the compression suffix.
pub fn decompressed_name(archive_name: &str, format: ArchiveFormat) -> String {
    let suffix = match format {
        ArchiveFormat::Gzip => ".gz",
        ArchiveFormat::Bzip2 => ".bz2",
        ArchiveFormat::Xz => ".xz",
        _ => "",
    };
    let lower = archive_name.to_ascii_lowercase();
    match lower.strip_suffix(suffix) {
        Some(stem) if !suffix.is_empty() && !stem.is_empty() => {
            archive_name[..stem.len()].to_string()
        }
        _ => format!("{archive_name}.out"),
    }
}

/// Unpacks `archive_path` into `destination`, creating the folder when it
/// does not exist. Existing files are replaced only when `overwrite` is set.
pub fn extract_script(
    archive_path: &str,
    destination: &str,
    format: ArchiveFormat,
    overwrite: bool,
) -> Result<ArchiveScript, String> {
    if destination.trim().is_empty() {
        return Err("Choose a folder to extract into".to_string());
    }
    let archive = shell_path(archive_path);
    let keep = if overwrite { "" } else { " -k" };
    let tar = |flag: &str| ("tar", format!("tar -x{keep} {flag}-f {archive}"));
    let (tool, command) = match format {
        ArchiveFormat::Tar => tar(""),
        ArchiveFormat::TarGz => tar("-z "),
        ArchiveFormat::TarBz2 => tar("-j "),
        ArchiveFormat::TarXz => tar("-J "),
        ArchiveFormat::TarZst => tar("--zstd "),
        ArchiveFormat::Zip => {
            let mode = if overwrite { "-o" } else { "-n" };
            ("unzip", format!("unzip -q {mode} {archive} -d ."))
        }
        ArchiveFormat::SevenZip => return Ok(seven_zip_script(&archive, destination, overwrite)),
        ArchiveFormat::Gzip | ArchiveFormat::Bzip2 | ArchiveFormat::Xz => {
            return Ok(decompress_script(
                archive_path,
                destination,
                format,
                overwrite,
            ));
        }
    };
    Ok(ArchiveScript {
        text: format!(
            "{}{}exec {command} </dev/null\n",
            enter_destination(destination),
            tool_check(tool),
        ),
        tool,
    })
}

fn enter_destination(destination: &str) -> String {
    let destination = shell_path(destination);
    format!("mkdir -p {destination} || exit 1\ncd {destination} || exit 1\n")
}

/// 7-Zip installs under one of three names depending on the package.
fn seven_zip_script(archive: &str, destination: &str, overwrite: bool) -> ArchiveScript {
    let mode = if overwrite { "-aoa" } else { "-aos" };
    ArchiveScript {
        text: format!(
            "{}for tool in 7zz 7z 7za; do\n  \
             if command -v \"$tool\" >/dev/null 2>&1; then\n    \
             exec \"$tool\" x -y {mode} -o. {archive} </dev/null >/dev/null\n  \
             fi\ndone\nexit {MISSING_TOOL_STATUS}\n",
            enter_destination(destination),
        ),
        tool: "7z",
    }
}

fn decompress_script(
    archive_path: &str,
    destination: &str,
    format: ArchiveFormat,
    overwrite: bool,
) -> ArchiveScript {
    let tool = match format {
        ArchiveFormat::Bzip2 => "bzip2",
        ArchiveFormat::Xz => "xz",
        _ => "gzip",
    };
    let archive_name = archive_path.rsplit('/').next().unwrap_or(archive_path);
    let output_name = decompressed_name(archive_name, format);
    let output = shell_path(&output_name);
    let refuse_existing = if overwrite {
        String::new()
    } else {
        format!(
            "if [ -e {output} ]; then echo {} >&2; exit 1; fi\n",
            shell_quote(&format!("{output_name} already exists"))
        )
    };
    ArchiveScript {
        text: format!(
            "{}{}{refuse_existing}exec {tool} -dc {} >{output} </dev/null\n",
            enter_destination(destination),
            tool_check(tool),
            shell_path(archive_path),
        ),
        tool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compresses_entries_relative_to_their_folder() {
        let script = compress_script(
            "/srv/app",
            &["logs".to_string(), "-odd name".to_string()],
            "backup.tar.gz",
            ArchiveFormat::TarGz,
        )
        .unwrap();
        assert_eq!(script.tool, "tar");
        assert_eq!(
            script.text,
            "cd '/srv/app' || exit 1\n\
             command -v tar >/dev/null 2>&1 || exit 127\n\
             exec tar -czf 'backup.tar.gz' 'logs' './-odd name' </dev/null\n"
        );
    }

    #[test]
    fn zips_with_symbolic_links_kept() {
        let script =
            compress_script("/srv", &["a".to_string()], "a.zip", ArchiveFormat::Zip).unwrap();
        assert!(script.text.contains("exec zip -qry 'a.zip' 'a' </dev/null"));
    }

    #[test]
    fn refuses_paths_as_names() {
        for name in ["", ".", "..", "a/b"] {
            assert!(
                compress_script("/srv", &[name.to_string()], "a.tar", ArchiveFormat::Tar).is_err()
            );
            assert!(compress_script("/srv", &["a".to_string()], name, ArchiveFormat::Tar).is_err());
        }
        assert!(compress_script("/srv", &[], "a.tar", ArchiveFormat::Tar).is_err());
        assert!(
            compress_script("/srv", &["a".to_string()], "a.7z", ArchiveFormat::SevenZip).is_err()
        );
    }

    #[test]
    fn extracts_tar_keeping_existing_files_unless_overwriting() {
        let keep = extract_script("/srv/a.tgz", "/srv/a", ArchiveFormat::TarGz, false).unwrap();
        assert_eq!(
            keep.text,
            "mkdir -p '/srv/a' || exit 1\n\
             cd '/srv/a' || exit 1\n\
             command -v tar >/dev/null 2>&1 || exit 127\n\
             exec tar -x -k -z -f '/srv/a.tgz' </dev/null\n"
        );
        let replace = extract_script("/srv/a.tar", "/srv", ArchiveFormat::Tar, true).unwrap();
        assert!(replace
            .text
            .contains("exec tar -x -f '/srv/a.tar' </dev/null"));
    }

    #[test]
    fn extracts_zip_and_7z_with_their_overwrite_modes() {
        let zip = extract_script("/a.zip", "/out", ArchiveFormat::Zip, false).unwrap();
        assert_eq!(zip.tool, "unzip");
        assert!(zip
            .text
            .contains("exec unzip -q -n '/a.zip' -d . </dev/null"));
        let seven = extract_script("/a.7z", "/out", ArchiveFormat::SevenZip, true).unwrap();
        assert_eq!(seven.tool, "7z");
        assert!(seven.text.contains("x -y -aoa -o. '/a.7z'"));
        assert!(seven.text.ends_with("exit 127\n"));
    }

    #[test]
    fn decompresses_a_single_file_beside_it() {
        let script =
            extract_script("/logs/app.log.gz", "/logs", ArchiveFormat::Gzip, false).unwrap();
        assert_eq!(script.tool, "gzip");
        assert!(script.text.contains("if [ -e 'app.log' ]; then"));
        assert!(script
            .text
            .ends_with("exec gzip -dc '/logs/app.log.gz' >'app.log' </dev/null\n"));
        let replace = extract_script("/a.xz", "/", ArchiveFormat::Xz, true).unwrap();
        assert!(!replace.text.contains("if [ -e"));
    }

    #[test]
    fn names_the_decompressed_file() {
        assert_eq!(
            decompressed_name("app.log.GZ", ArchiveFormat::Gzip),
            "app.log"
        );
        assert_eq!(decompressed_name("dump.bz2", ArchiveFormat::Bzip2), "dump");
        assert_eq!(decompressed_name(".gz", ArchiveFormat::Gzip), ".gz.out");
        assert_eq!(decompressed_name("data", ArchiveFormat::Xz), "data.out");
    }

    #[test]
    fn needs_a_destination() {
        assert!(extract_script("/a.zip", " ", ArchiveFormat::Zip, true).is_err());
    }
}
