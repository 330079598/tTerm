//! Specification tests transcribed from
//! `src/components/SftpDrawer/sftpArchive.test.ts`.

use crate::sftp::entry::SftpDirectoryEntry;
use crate::sftp::internal::api::archive::ArchiveFormat;
use crate::sftp::view::archive::{
    archive_format_of, common_parent, default_archive_name, default_extract_destination,
    with_archive_extension,
};

fn file(name: &str) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        name: name.to_owned(),
        path: format!("/srv/{name}"),
        is_dir: false,
        is_symlink: false,
        size: None,
        modified_at: None,
        permissions: None,
        mode: None,
        owner: None,
        group: None,
    }
}

fn directory(name: &str) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        is_dir: true,
        ..file(name)
    }
}

#[test]
fn prefers_the_longer_archive_suffix() {
    assert_eq!(
        archive_format_of(&file("site.TAR.GZ")),
        Some(ArchiveFormat::TarGz)
    );
    assert_eq!(
        archive_format_of(&file("app.log.gz")),
        Some(ArchiveFormat::Gzip)
    );
    assert_eq!(
        archive_format_of(&file("a.tgz")),
        Some(ArchiveFormat::TarGz)
    );
    assert_eq!(
        archive_format_of(&file("a.7z")),
        Some(ArchiveFormat::SevenZip)
    );
}

#[test]
fn ignores_folders_other_files_and_bare_suffixes() {
    assert_eq!(archive_format_of(&directory("backup.zip")), None);
    assert_eq!(archive_format_of(&file("notes.txt")), None);
    assert_eq!(archive_format_of(&file(".zip")), None);
}

#[test]
fn unpacks_an_archive_into_a_folder_named_after_it() {
    assert_eq!(
        default_extract_destination("/srv/site.tar.gz", ArchiveFormat::TarGz),
        "/srv/site"
    );
    assert_eq!(
        default_extract_destination("/site.zip", ArchiveFormat::Zip),
        "/site"
    );
}

#[test]
fn unpacks_a_single_compressed_file_beside_it() {
    assert_eq!(
        default_extract_destination("/var/log/app.log.gz", ArchiveFormat::Gzip),
        "/var/log"
    );
    assert_eq!(
        default_extract_destination("/dump.xz", ArchiveFormat::Xz),
        "/"
    );
}

#[test]
fn names_one_entrys_archive_after_it_and_several_after_their_folder() {
    assert_eq!(
        default_archive_name(&[file("logs")], "/srv", ArchiveFormat::TarGz).as_deref(),
        Some("logs.tar.gz")
    );
    assert_eq!(
        default_archive_name(&[file("a"), file("b")], "/srv/app", ArchiveFormat::Zip).as_deref(),
        Some("app.zip")
    );
    assert_eq!(
        default_archive_name(&[file("a"), file("b")], "/", ArchiveFormat::Tar).as_deref(),
        Some("archive.tar")
    );
}

#[test]
fn swaps_the_extension_the_old_format_gave() {
    assert_eq!(
        with_archive_extension("logs.tar.gz", ArchiveFormat::TarGz, ArchiveFormat::Zip),
        "logs.zip"
    );
    assert_eq!(
        with_archive_extension("custom.bin", ArchiveFormat::TarGz, ArchiveFormat::Zip),
        "custom.bin"
    );
}

#[test]
fn finds_the_folder_shared_by_every_entry() {
    assert_eq!(
        common_parent(&[file("a"), file("b")]).as_deref(),
        Some("/srv")
    );
    let mut root = file("a");
    root.path = "/a".to_owned();
    assert_eq!(common_parent(&[root]).as_deref(), Some("/"));
    let mut nested = file("b");
    nested.path = "/srv/x/b".to_owned();
    assert_eq!(common_parent(&[file("a"), nested]), None);
    assert_eq!(common_parent(&[]), None);
}
