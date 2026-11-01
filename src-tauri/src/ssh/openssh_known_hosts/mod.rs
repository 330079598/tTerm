//! The host keys and certificate authorities OpenSSH trusts, read from its
//! `known_hosts` files. A host `ssh` already trusts is not asked about again,
//! a revoked key is refused as `ssh` refuses it, and a host that proves itself
//! with a certificate from an `@cert-authority` can be reached without a
//! fingerprint prompt. `hosts` matches the host names of an entry and `verify`
//! decides about the key a host presents.

mod hosts;
pub(crate) mod verify;

#[cfg(test)]
mod handshake_tests;
#[cfg(test)]
mod tests;

use crate::profiles::parser::config_known_hosts_files;
use crate::profiles::parser::paths::home_dir;
use crate::ssh::store::KnownHostRecord;
use hosts::{host_key_name, patterns_match};
use russh::keys::ssh_key::known_hosts::{Entry, HostPatterns, Marker};
use russh::keys::ssh_key::public::KeyData;
use russh::keys::ssh_key::Algorithm;
use russh::keys::PublicKeyOrCertificate;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use verify::KnownHostVerdict;

/// One entry of a `known_hosts` file that could be read.
struct KnownHostLine {
    marker: Option<Marker>,
    patterns: HostPatterns,
    key: KeyData,
    /// The file and line, as messages name it.
    place: String,
}

/// The entries of the `known_hosts` files `ssh` reads by default.
#[derive(Default)]
pub(crate) struct OpenSshKnownHosts {
    lines: Vec<KnownHostLine>,
}

/// The files `ssh` reads when the config names no `UserKnownHostsFile` or
/// `GlobalKnownHostsFile`.
fn default_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(home) = home_dir() {
        files.push(home.join(".ssh").join("known_hosts"));
        files.push(home.join(".ssh").join("known_hosts2"));
    }
    #[cfg(windows)]
    if let Some(program_data) = std::env::var_os("ProgramData") {
        let ssh = PathBuf::from(program_data).join("ssh");
        files.push(ssh.join("ssh_known_hosts"));
        files.push(ssh.join("ssh_known_hosts2"));
    }
    #[cfg(not(windows))]
    {
        files.push(PathBuf::from("/etc/ssh/ssh_known_hosts"));
        files.push(PathBuf::from("/etc/ssh/ssh_known_hosts2"));
    }
    files
}

impl OpenSshKnownHosts {
    /// Reads the default `known_hosts` files and those `~/.ssh/config` names
    /// for `host` or for the saved connection `profile_name`. A file that is
    /// missing or cannot be read adds nothing, as for `ssh`.
    ///
    /// `ssh` reads only the files a config names in place of the defaults;
    /// tTerm reads both, which trusts nothing the user's own files do not
    /// list, so a key `ssh` was told about in either place is not asked about.
    pub(crate) fn load(host: &str, port: u16, profile_name: Option<&str>) -> Self {
        let names: Vec<&str> = profile_name.into_iter().chain([host]).collect();
        let mut files = default_files();
        for file in config_known_hosts_files(&names, port) {
            if !files.contains(&file) {
                files.push(file);
            }
        }
        let mut known = Self::default();
        for file in files {
            if let Ok(text) = std::fs::read_to_string(&file) {
                known.add(&text, &file);
            }
        }
        known
    }

    /// Adds the entries of one file. Lines that cannot be read are passed
    /// over, as `ssh` passes over them.
    fn add(&mut self, text: &str, file: &Path) {
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // `ssh` separates the fields with any run of blanks.
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            let Ok(entry) = line.parse::<Entry>() else {
                continue;
            };
            self.lines.push(KnownHostLine {
                marker: entry.marker().copied(),
                patterns: entry.host_patterns().clone(),
                key: entry.public_key().key_data().clone(),
                place: format!("{}:{}", file.display(), index + 1),
            });
        }
    }

    /// The entries whose host names match `host` at `port`.
    fn lines_for(&self, host: &str, port: u16) -> impl Iterator<Item = &KnownHostLine> {
        let name = host_key_name(host, port);
        self.lines
            .iter()
            .filter(move |line| patterns_match(&line.patterns, &name))
    }

    /// Whether an `@cert-authority` is trusted for `host` at `port`, which
    /// makes it worth asking the host for a certificate.
    pub(crate) fn has_authority_for(&self, host: &str, port: u16) -> bool {
        self.lines_for(host, port)
            .any(|line| line.marker == Some(Marker::CertAuthority))
    }
}

/// The host certificate types tTerm can verify, best first.
static HOST_CERTIFICATE_ALGORITHMS: [Algorithm; 6] = [
    Algorithm::Ed25519,
    Algorithm::Ecdsa {
        curve: russh::keys::ssh_key::EcdsaCurve::NistP256,
    },
    Algorithm::Ecdsa {
        curve: russh::keys::ssh_key::EcdsaCurve::NistP384,
    },
    Algorithm::Ecdsa {
        curve: russh::keys::ssh_key::EcdsaCurve::NistP521,
    },
    Algorithm::Rsa {
        hash: Some(russh::keys::HashAlg::Sha512),
    },
    Algorithm::Rsa {
        hash: Some(russh::keys::HashAlg::Sha256),
    },
];

/// Asks `host` for a host certificate when `known_hosts` trusts an authority
/// for it. Without one a certificate could not be checked, so a host that
/// has both would present the certificate and fail where its plain key works.
pub(crate) fn request_host_certificates(
    config: &mut russh::client::Config,
    host: &str,
    port: u16,
    profile_name: Option<&str>,
) {
    OpenSshKnownHosts::load(host, port, profile_name).request_host_certificates(config, host, port);
}

impl OpenSshKnownHosts {
    fn request_host_certificates(&self, config: &mut russh::client::Config, host: &str, port: u16) {
        if self.has_authority_for(host, port) {
            config.preferred.host_key_certificates = Cow::Borrowed(&HOST_CERTIFICATE_ALGORITHMS);
        }
    }
}

/// What the `known_hosts` files say now about what `host` at `port`
/// presented. They are read on every connection, as `ssh` reads them, so an
/// entry added or removed with `ssh` or `ssh-keygen -R` counts at once.
pub(crate) fn check_presented_key(
    host: &str,
    port: u16,
    profile_name: Option<&str>,
    presented: &PublicKeyOrCertificate,
) -> KnownHostVerdict {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    OpenSshKnownHosts::load(host, port, profile_name).verify(host, port, presented, now)
}

/// Stands in for tTerm's own record of a host it has none for when
/// `known_hosts` lists a different key for it, so the prompt shows that the
/// key changed instead of a host seen for the first time.
pub(crate) fn listed_record(
    host: &str,
    port: u16,
    algorithm: &str,
    listed_fingerprint: String,
) -> KnownHostRecord {
    KnownHostRecord {
        profile_id: None,
        profile_name: "~/.ssh/known_hosts".to_string(),
        host: host.to_string(),
        port,
        algorithm: algorithm.to_string(),
        fingerprint: listed_fingerprint,
        trusted_at: 0,
    }
}
