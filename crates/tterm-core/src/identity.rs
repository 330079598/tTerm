//! Application identity supplied by the client alongside its directories.

/// The identity of this running application, including isolated development builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIdentity {
    pub identifier: String,
    pub name: String,
    pub version: String,
}

/// Returns the operating system hostname with the same lossy conversion as Tauri.
pub fn hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}
