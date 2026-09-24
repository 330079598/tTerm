use russh::keys::{self, PrivateKey};
use std::io::ErrorKind;
use std::path::Path;

/// Loads a private key file, with an error that names the file. `subject`
/// says whose key it is, e.g. "SSH key" or "Jump host SSH key".
pub fn load_private_key(
    path: &str,
    passphrase: Option<&str>,
    subject: &str,
) -> Result<PrivateKey, String> {
    keys::load_secret_key(Path::new(path), passphrase)
        .map_err(|error| describe_key_error(subject, path, &error))
}

pub fn describe_key_error(subject: &str, path: &str, error: &keys::Error) -> String {
    match error {
        keys::Error::IO(io) if io.kind() == ErrorKind::NotFound => {
            format!("{subject} file not found: {path}")
        }
        keys::Error::IO(io) if io.kind() == ErrorKind::PermissionDenied => {
            format!("Permission denied reading {subject} file: {path}")
        }
        keys::Error::IO(io) => format!("Cannot read {subject} file {path}: {io}"),
        keys::Error::KeyIsEncrypted => {
            format!("{subject} {path} is encrypted; enter its passphrase")
        }
        other => format!("Failed to load {subject} {path}: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_file_names_the_path() {
        let path = "/nonexistent/tterm/id_ed25519";
        let error = load_private_key(path, None, "SSH key").expect_err("missing file");
        assert_eq!(error, format!("SSH key file not found: {path}"));
    }

    #[test]
    fn unparseable_key_file_names_the_path() {
        let path = std::env::temp_dir().join(format!("tterm-bad-key-{}", std::process::id()));
        std::fs::write(&path, "not a key").expect("write fixture");
        let path_str = path.to_string_lossy().into_owned();
        let error = load_private_key(&path_str, None, "Jump host SSH key").expect_err("bad key");
        assert!(
            error.starts_with(&format!("Failed to load Jump host SSH key {path_str}:")),
            "{error}"
        );
        std::fs::remove_file(path).ok();
    }
}
