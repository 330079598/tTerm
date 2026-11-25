//! Shared numbered-instance paths, the one-shot P2-12 SSH profile seed, and
//! the preflight checks an SSH field run performs before it connects.

use std::path::{Path, PathBuf};

#[cfg(debug_assertions)]
use std::ffi::OsString;
#[cfg(debug_assertions)]
use std::fs;
#[cfg(debug_assertions)]
use tterm_core::db::Database;
use tterm_core::paths::AppPaths;
#[cfg(debug_assertions)]
use tterm_core::profiles::storage::upsert_profile;
#[cfg(all(debug_assertions, feature = "automation"))]
use tterm_core::profiles::storage::{get_profile, normalize_profile};
#[cfg(debug_assertions)]
use tterm_core::profiles::types::SavedProfile;
#[cfg(debug_assertions)]
use tterm_core::proxy::ProxyChoice;
#[cfg(all(debug_assertions, feature = "automation"))]
use tterm_core::ssh::store::KnownHostRecord;

/// Keeps the GPUI development data apart from the Tauri application's files.
pub(crate) const IDENTIFIER: &str = "com.stone.tTerm.gpui-dev";

#[cfg(debug_assertions)]
const INSTANCE_ENV: &str = "TTERM_GPUI_CHECK_INSTANCE";

#[cfg(debug_assertions)]
pub(crate) const PROFILE_ID: &str = "p2-12-remote";

#[cfg(debug_assertions)]
pub(crate) const PROFILE_NAME: &str = "p2-12-remote";

/// A numbered debug instance keeps manual acceptance runs out of the developer's
/// database, configuration and OS credential namespace, including across restarts.
#[cfg(debug_assertions)]
pub(crate) fn debug_identifier(instance: Option<OsString>) -> Result<String, String> {
    match instance {
        None => Ok(IDENTIFIER.to_owned()),
        Some(value) => value
            .to_str()
            .and_then(|text| text.parse::<u32>().ok())
            .map(|number| format!("{IDENTIFIER}.check-{number}"))
            .ok_or_else(|| {
                "Set TTERM_GPUI_CHECK_INSTANCE to an unsigned integer or unset it.".into()
            }),
    }
}

/// Resolves the paths the GPUI runtime uses for an identifier.
pub(crate) fn runtime_paths(identifier: &str) -> Result<AppPaths, String> {
    if identifier != IDENTIFIER {
        return Ok(isolated_paths(instance_root_in(
            &std::env::temp_dir(),
            identifier,
        )));
    }
    AppPaths::native(identifier)
}

/// All application directories live under one disposable instance root.
pub(crate) fn isolated_paths(root: PathBuf) -> AppPaths {
    AppPaths {
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        local_data_dir: root.join("local-data"),
        cache_dir: root.join("cache"),
        log_dir: root.join("logs"),
        download_dir: None,
        temp_dir: root.join("temp"),
    }
}

/// The only path layout shared by the runtime, tests and the seed example.
pub(crate) fn instance_root_in(base: &Path, identifier: &str) -> PathBuf {
    base.join(identifier)
}

/// The directory created by a successful seed, for the operator to record.
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct SeededInstance {
    pub identifier: String,
    pub root: PathBuf,
}

/// Seeds the fixed P2-12 SSH profile into a brand-new numbered debug instance.
///
/// The instance must be named explicitly through
/// `TTERM_GPUI_CHECK_INSTANCE`. This function only writes profile metadata; it does
/// not store a password or key, open a network connection, or start a client.
#[cfg(debug_assertions)]
pub fn seed_ssh_check(host: &str, port: u16, username: &str) -> Result<SeededInstance, String> {
    let instance = std::env::var_os(INSTANCE_ENV).ok_or_else(|| {
        format!("Set {INSTANCE_ENV} to a numbered debug instance before seeding.")
    })?;
    seed_in(&std::env::temp_dir(), Some(instance), host, port, username)
}

#[cfg(debug_assertions)]
fn seed_in(
    base: &Path,
    instance: Option<OsString>,
    host: &str,
    port: u16,
    username: &str,
) -> Result<SeededInstance, String> {
    let host = clean_field(host, "host")?;
    let username = clean_field(username, "username")?;
    if port == 0 {
        return Err("port must be between 1 and 65535.".to_string());
    }
    let identifier = required_numbered_identifier(instance)?;
    let root = instance_root_in(base, &identifier);
    create_instance_root(&root).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => format!(
            "Instance root '{}' already exists. Use a new numbered instance; existing data is never changed.",
            root.display()
        ),
        _ => format!("Failed to create instance root '{}': {error}", root.display()),
    })?;

    if let Err(error) = seed_profile(&root, &host, port, &username) {
        return Err(format!(
            "Seeding instance '{identifier}' failed after creating its root '{}': {error} \
             The root may be incomplete; use a new numbered instance and leave this one in place.",
            root.display()
        ));
    }

    Ok(SeededInstance { identifier, root })
}

#[cfg(debug_assertions)]
#[cfg(unix)]
fn create_instance_root(root: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder.create(root)
}

#[cfg(debug_assertions)]
#[cfg(not(unix))]
fn create_instance_root(root: &Path) -> std::io::Result<()> {
    let builder = fs::DirBuilder::new();
    builder.create(root)
}

#[cfg(debug_assertions)]
pub(crate) fn required_numbered_identifier(instance: Option<OsString>) -> Result<String, String> {
    let identifier = debug_identifier(instance).map_err(|error| {
        format!("{error} The seed example requires an explicit numbered instance.")
    })?;
    if identifier == IDENTIFIER {
        return Err(format!(
            "Set {INSTANCE_ENV} to a numbered debug instance; the unnumbered development root is not allowed."
        ));
    }
    Ok(identifier)
}

/// Rejects an unset or malformed instance before the runtime can fall back to
/// the shared development root.
#[cfg(all(debug_assertions, feature = "automation"))]
pub(crate) fn require_numbered_instance() -> Result<String, String> {
    required_numbered_identifier(std::env::var_os(INSTANCE_ENV))
}

/// Reads the fixed field profile and checks it cannot use a credential other
/// than the one password supplied for this run.
#[cfg(all(debug_assertions, feature = "automation"))]
pub(crate) fn field_profile() -> Result<SavedProfile, String> {
    let profile = tterm_core::db::read(|connection| get_profile(connection, PROFILE_ID))?
        .ok_or_else(|| {
            format!(
                "The SSH field profile '{PROFILE_ID}' is missing from this instance; seed a new \
                 numbered instance."
            )
        })?;
    let mut profile = profile;
    normalize_profile(&mut profile);
    validate_field_profile(&profile)?;
    Ok(profile)
}

/// Requires an application-store pin for the profile's exact host and port.
///
/// The field run never accepts a key for the first time: the operator confirms
/// it once in a normal connection, and this run only reuses that record.
#[cfg(all(debug_assertions, feature = "automation"))]
pub(crate) fn require_pinned_host(profile: &SavedProfile) -> Result<KnownHostRecord, String> {
    let host = profile
        .host
        .as_deref()
        .ok_or_else(|| "The SSH field profile has no host.".to_string())?;
    let port = profile
        .port
        .ok_or_else(|| "The SSH field profile has no port.".to_string())?;
    let pin = tterm_core::ssh::store::load_known_host(
        &profile.name,
        Some(&profile.id),
        host,
        port,
    )?
    .ok_or_else(|| {
        format!(
            "No pinned host key for '{}' in this instance. Confirm the fingerprint once in a \
             normal connection, then run the field entry again.",
            profile.name
        )
    })?;
    if pin.fingerprint.trim().is_empty() || pin.algorithm.trim().is_empty() {
        return Err(format!(
            "The pinned host key for '{}' is incomplete; confirm the fingerprint again.",
            profile.name
        ));
    }
    Ok(pin)
}

#[cfg(all(debug_assertions, feature = "automation"))]
fn validate_field_profile(profile: &SavedProfile) -> Result<(), String> {
    let reject = |reason: &str| {
        Err(format!(
            "The SSH field profile '{}' {reason}; seed a new numbered instance.",
            profile.id
        ))
    };
    if profile.id != PROFILE_ID || profile.name != PROFILE_NAME {
        return reject("is not the fixed check profile");
    }
    if profile.connection_type != "ssh" {
        return reject("is not an SSH connection");
    }
    if profile
        .host
        .as_deref()
        .is_none_or(|host| host.trim().is_empty())
    {
        return reject("has no host");
    }
    if profile.port.unwrap_or(0) == 0 {
        return reject("has no port");
    }
    if profile
        .username
        .as_deref()
        .is_none_or(|username| username.trim().is_empty())
    {
        return reject("has no username");
    }
    if profile.auth_method.as_deref() != Some("password") {
        return reject("must use password authentication");
    }
    if profile.password.is_some()
        || profile.private_key_path.is_some()
        || profile.private_key_passphrase.is_some()
        || profile.certificate_path.is_some()
    {
        return reject("must not carry a saved password, key or certificate");
    }
    if profile.remember_password || profile.ignore_saved_password {
        return reject("must not read or write a saved password");
    }
    if profile.agent_forward {
        return reject("must not forward or use an SSH agent");
    }
    if profile.use_jump_host == Some(true) || !profile.jump_hosts.is_empty() {
        return reject("must not use jump hosts");
    }
    if profile.proxy != ProxyChoice::Direct {
        return reject("must connect without a proxy");
    }
    if profile.session_log.as_deref() != Some("never") {
        return reject("must not log the session");
    }
    if profile.sudo_autofill || profile.sudo_password.is_some() {
        return reject("must not carry a sudo password");
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn clean_field(value: &str, name: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{name} must not be empty."));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{name} must not contain control characters."));
    }
    Ok(value.to_string())
}

#[cfg(debug_assertions)]
fn seed_profile(root: &Path, host: &str, port: u16, username: &str) -> Result<(), String> {
    let paths = isolated_paths(root.to_path_buf());
    fs::create_dir_all(&paths.config_dir).map_err(|error| {
        format!(
            "Failed to create the instance config directory '{}': {error}",
            paths.config_dir.display()
        )
    })?;
    let database = Database::open_in_config_dir(&paths.config_dir)?;
    database
        .write(|transaction| upsert_profile(transaction, &check_profile(host, port, username)))?;
    Ok(())
}

#[cfg(debug_assertions)]
fn check_profile(host: &str, port: u16, username: &str) -> SavedProfile {
    SavedProfile {
        id: PROFILE_ID.to_string(),
        name: PROFILE_NAME.to_string(),
        group_id: None,
        group_path: None,
        legacy_group: String::new(),
        connection_type: "ssh".to_string(),
        host: Some(host.to_string()),
        port: Some(port),
        username: Some(username.to_string()),
        password: None,
        ignore_saved_password: false,
        remember_password: false,
        auth_method: Some("password".to_string()),
        agent_forward: false,
        private_key_path: None,
        private_key_passphrase: None,
        certificate_path: None,
        keepalive_interval_secs: 15,
        keepalive_count_max: 3,
        server_monitor_visible: false,
        use_jump_host: Some(false),
        legacy_jump_host: None,
        jump_hosts: Vec::new(),
        sudo_autofill: false,
        sudo_password: None,
        clear_sudo_password: false,
        encoding: None,
        terminal_theme: None,
        login_script: None,
        ssh_shell_integration: Some(false),
        legacy_shell_integration: false,
        session_log: Some("never".to_string()),
        sftp_start_directory: None,
        highlight_set: None,
        proxy: ProxyChoice::Direct,
        tab_icon: None,
        distro: None,
        serial: None,
        local_terminal: None,
    }
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tterm_core::profiles::launch::launch_from_saved_profile;
    use tterm_core::profiles::launch::list_saved_ssh_profile_summaries;
    use tterm_core::profiles::storage::get_profile;
    #[cfg(feature = "automation")]
    use tterm_core::profiles::types::SavedJumpHost;

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch() -> Scratch {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        let path = std::env::temp_dir().join(format!(
            "tterm-gpui-seed-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create scratch base");
        Scratch(path)
    }

    fn identifier(number: u32) -> String {
        debug_identifier(Some(number.to_string().into())).expect("numbered identifier")
    }

    #[test]
    fn numbered_instances_and_runtime_paths_share_one_root() {
        assert_eq!(debug_identifier(None).unwrap(), IDENTIFIER);
        assert_eq!(
            debug_identifier(Some("42".into())).unwrap(),
            format!("{IDENTIFIER}.check-42")
        );
        for invalid in ["", "../tterm", IDENTIFIER, "4294967296"] {
            assert!(debug_identifier(Some(invalid.into())).is_err());
        }

        let identifier = identifier(42);
        let paths = runtime_paths(&identifier).expect("numbered runtime paths");
        let root = instance_root_in(&std::env::temp_dir(), &identifier);
        assert_eq!(paths.config_dir, root.join("config"));
        assert_eq!(paths.data_dir, root.join("data"));
        assert_eq!(paths.temp_dir, root.join("temp"));
    }

    #[test]
    fn seed_writes_the_fixed_profile_without_credentials() {
        let scratch = scratch();
        let seeded = seed_in(&scratch.0, Some("7".into()), "example.test", 2222, "tester")
            .expect("seed a fresh instance");

        assert_eq!(seeded.identifier, identifier(7));
        let paths = isolated_paths(seeded.root);
        let database =
            Database::open_in_config_dir(&paths.config_dir).expect("open the seeded database");
        let profile = database
            .read(|connection| get_profile(connection, PROFILE_ID))
            .expect("read the seeded profile")
            .expect("the profile exists");

        assert_eq!(profile.id, PROFILE_ID);
        assert_eq!(profile.name, PROFILE_NAME);
        assert_eq!(profile.connection_type, "ssh");
        assert_eq!(profile.host.as_deref(), Some("example.test"));
        assert_eq!(profile.port, Some(2222));
        assert_eq!(profile.username.as_deref(), Some("tester"));
        assert_eq!(profile.auth_method.as_deref(), Some("password"));
        assert!(!profile.remember_password);
        assert!(!profile.ignore_saved_password);
        assert_eq!(profile.proxy, ProxyChoice::Direct);
        assert!(!profile.sudo_autofill);
        assert_eq!(profile.session_log.as_deref(), Some("never"));
        assert_eq!(profile.ssh_shell_integration, Some(false));
        assert_eq!(profile.use_jump_host, Some(false));
        assert!(profile.password.is_none());
        assert!(profile.private_key_path.is_none());
        assert!(profile.private_key_passphrase.is_none());
        assert!(profile.certificate_path.is_none());
        assert!(profile.jump_hosts.is_empty());

        let summaries = database
            .read(list_saved_ssh_profile_summaries)
            .expect("read the saved-connection picker data");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, PROFILE_ID);
        assert_eq!(summaries[0].name, PROFILE_NAME);
        assert_eq!(summaries[0].endpoint(), "tester@example.test:2222");

        let launch = launch_from_saved_profile(&profile).expect("build the saved launch");
        assert_eq!(launch.summary.id, PROFILE_ID);
        assert_eq!(launch.connection.auth_method.as_deref(), Some("password"));
        assert_eq!(launch.connection.host.as_deref(), Some("example.test"));
        assert_eq!(launch.connection.port, Some(2222));
        assert_eq!(launch.connection.username.as_deref(), Some("tester"));
        assert!(launch.connection.password.is_none());
    }

    #[test]
    fn seed_refuses_an_existing_root_without_changing_it() {
        let scratch = scratch();
        let identifier = identifier(8);
        let root = instance_root_in(&scratch.0, &identifier);
        fs::create_dir_all(&root).expect("create the existing root");
        let marker = root.join("keep");
        fs::write(&marker, b"unchanged").expect("write the marker");

        let error = seed_in(&scratch.0, Some("8".into()), "example.test", 22, "tester")
            .expect_err("an existing root must be rejected");

        assert!(error.contains("already exists"), "{error}");
        assert_eq!(fs::read(&marker).unwrap(), b"unchanged");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn seed_refuses_an_existing_file_at_the_instance_root() {
        let scratch = scratch();
        let identifier = identifier(9);
        let root = instance_root_in(&scratch.0, &identifier);
        fs::write(&root, b"unchanged").expect("write a file at the root");

        let error = seed_in(&scratch.0, Some("9".into()), "example.test", 22, "tester")
            .expect_err("an existing file must be rejected");

        assert!(error.contains("already exists"), "{error}");
        assert_eq!(fs::read(&root).unwrap(), b"unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn seed_refuses_an_existing_symlink_at_the_instance_root() {
        use std::os::unix::fs::symlink;

        let scratch = scratch();
        let identifier = identifier(10);
        let root = instance_root_in(&scratch.0, &identifier);
        let target = scratch.0.join("target");
        fs::create_dir(&target).expect("create the symlink target");
        symlink(&target, &root).expect("create the root symlink");

        let error = seed_in(&scratch.0, Some("10".into()), "example.test", 22, "tester")
            .expect_err("an existing symlink must be rejected");

        assert!(error.contains("already exists"), "{error}");
        assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn seeded_instance_root_is_private_to_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = scratch();
        let seeded = seed_in(&scratch.0, Some("11".into()), "example.test", 22, "tester")
            .expect("seed a fresh instance");
        let mode = fs::metadata(seeded.root)
            .expect("read the instance root metadata")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(mode, 0o700);
    }

    #[test]
    fn seed_rejects_invalid_fields_before_creating_the_root() {
        let scratch = scratch();
        for (index, (host, port, username)) in [
            ("", 22, "tester"),
            ("example.test", 0, "tester"),
            ("example.test", 22, ""),
        ]
        .into_iter()
        .enumerate()
        {
            let number = u32::try_from(index).unwrap() + 20;
            let error = seed_in(
                &scratch.0,
                Some(number.to_string().into()),
                host,
                port,
                username,
            )
            .expect_err("invalid fields must fail");
            assert!(!error.is_empty());
            assert!(!instance_root_in(&scratch.0, &identifier(number)).exists());
        }
    }

    #[test]
    fn seed_requires_an_explicit_numbered_instance() {
        let scratch = scratch();
        for instance in [
            None,
            Some("".into()),
            Some("abc".into()),
            Some(IDENTIFIER.into()),
            Some("4294967296".into()),
        ] {
            assert!(seed_in(&scratch.0, instance, "example.test", 22, "tester").is_err());
        }
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[cfg(feature = "automation")]
    #[test]
    fn field_profile_rejects_credentials_and_routes_the_run_must_not_use() {
        fn rejected(mutate: impl FnOnce(&mut SavedProfile)) {
            let mut profile = check_profile("example.test", 2222, "tester");
            mutate(&mut profile);
            assert!(validate_field_profile(&profile).is_err());
        }

        let profile = check_profile("example.test", 2222, "tester");
        validate_field_profile(&profile).expect("the seed profile is usable");

        rejected(|profile| profile.id = "other".to_string());
        rejected(|profile| profile.name = "other".to_string());
        rejected(|profile| profile.connection_type = "local".to_string());
        rejected(|profile| profile.host = None);
        rejected(|profile| profile.port = Some(0));
        rejected(|profile| profile.username = None);
        rejected(|profile| profile.auth_method = Some("interactive".to_string()));
        rejected(|profile| profile.password = Some("saved".to_string()));
        rejected(|profile| profile.private_key_path = Some("id_ed25519".to_string()));
        rejected(|profile| profile.private_key_passphrase = Some("secret".to_string()));
        rejected(|profile| profile.certificate_path = Some("id-cert.pub".to_string()));
        rejected(|profile| profile.remember_password = true);
        rejected(|profile| profile.ignore_saved_password = true);
        rejected(|profile| profile.agent_forward = true);
        rejected(|profile| profile.use_jump_host = Some(true));
        rejected(|profile| profile.jump_hosts.push(SavedJumpHost::default()));
        rejected(|profile| {
            profile.proxy = ProxyChoice::Server {
                id: "proxy".to_string(),
            }
        });
        rejected(|profile| profile.session_log = Some("always".to_string()));
        rejected(|profile| profile.sudo_autofill = true);
        rejected(|profile| profile.sudo_password = Some("sudo".to_string()));
    }
}
