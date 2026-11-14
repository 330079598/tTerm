use super::include::read_ssh_config_in;
use super::tests::entries;
use crate::profiles::types::SshConfigImportHost;
use std::fs;
use std::path::{Path, PathBuf};

/// A folder of config files that is removed when the test ends.
struct ConfigDir(PathBuf);

impl ConfigDir {
    fn new(files: &[(&str, &str)]) -> Self {
        let root =
            std::env::temp_dir().join(format!("tterm-ssh-config-include-{}", uuid::Uuid::new_v4()));
        for (name, content) in files {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap_or(&root)).expect("folder should be created");
            fs::write(&path, content).expect("config file should be written");
        }
        Self(root)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Imports `config`, looking relative includes up in this folder.
    fn import(&self) -> (Vec<SshConfigImportHost>, Vec<String>) {
        let (config, warnings) = read_ssh_config_in(&self.0.join("config"), self.0.clone())
            .expect("the config should be read");
        (entries(&config), warnings)
    }
}

impl Drop for ConfigDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn host<'a>(hosts: &'a [SshConfigImportHost], name: &str) -> &'a SshConfigImportHost {
    hosts
        .iter()
        .find(|host| host.host_pattern == name)
        .unwrap_or_else(|| panic!("{name} should be imported"))
}

#[test]
fn included_files_are_read_in_place() {
    let dir = ConfigDir::new(&[
        ("config", "User default-user\nInclude config.d/*.conf\n"),
        (
            "config.d/prod.conf",
            "Host prod\n  HostName prod.example.com\n",
        ),
    ]);

    let (hosts, warnings) = dir.import();
    assert!(warnings.is_empty(), "{warnings:?}");
    let prod = host(&hosts, "prod");
    assert_eq!(prod.username.as_deref(), Some("default-user"));
    assert_eq!(prod.host.as_deref(), Some("prod.example.com"));
}

#[test]
fn a_relative_include_is_looked_up_in_the_ssh_folder_from_any_file() {
    let dir = ConfigDir::new(&[
        ("config", "Include config.d/first.conf\n"),
        ("config.d/first.conf", "Include config.d/second.conf\n"),
        ("config.d/second.conf", "Host deep\n  Port 2200\n"),
    ]);

    let (hosts, warnings) = dir.import();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(host(&hosts, "deep").port, 2200);
}

#[test]
fn an_include_under_a_host_line_applies_to_that_host_only() {
    let dir = ConfigDir::new(&[
        (
            "config",
            "Host web\n  Include web.conf\n  Port 2222\nHost db\n  HostName db.internal\n",
        ),
        ("web.conf", "User deploy\nHost inner\n  Port 2200\n"),
    ]);

    let (hosts, warnings) = dir.import();
    assert!(warnings.is_empty(), "{warnings:?}");
    let web = host(&hosts, "web");
    assert_eq!(web.username.as_deref(), Some("deploy"));
    assert_eq!(
        web.port, 2222,
        "the lines after the Include are web's again"
    );
    assert!(host(&hosts, "db").username.is_none());
    assert_eq!(
        host(&hosts, "inner").port,
        22,
        "a Host line in the included file holds only where web does"
    );
}

#[test]
fn the_lines_after_an_include_apply_as_they_did_before_it() {
    let dir = ConfigDir::new(&[
        ("config", "Include hosts.conf\nUser everyone\n"),
        ("hosts.conf", "Host app\n  HostName app.example.com\n"),
    ]);

    let (hosts, _) = dir.import();
    assert_eq!(host(&hosts, "app").username.as_deref(), Some("everyone"));
}

#[test]
fn a_warning_names_the_included_file_it_is_about() {
    let dir = ConfigDir::new(&[
        ("config", "Include hosts.conf\nInclude missing.conf\n"),
        ("hosts.conf", "Host app\nMatch exec true\n  User x\n"),
    ]);

    let (_, warnings) = dir.import();
    let included = dir.path().join("hosts.conf").display().to_string();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with(&format!("{included}:2: Match exec"))),
        "{warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("Line 2: Include 'missing.conf'")),
        "{warnings:?}"
    );
}

#[test]
fn an_include_loop_is_stopped() {
    let dir = ConfigDir::new(&[("config", "Include config\nHost app\n")]);

    let (hosts, warnings) = dir.import();
    assert_eq!(hosts.len(), 1);
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("Include loop"))
    );
}
