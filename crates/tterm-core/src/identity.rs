//! Application identity supplied by the client alongside its directories.

use std::sync::OnceLock;

static APP_VERSION: OnceLock<String> = OnceLock::new();

/// The identity of this running application, including isolated development builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppIdentity {
    pub identifier: String,
    pub name: String,
    pub version: String,
}

/// Installs the version of the running application for adapters in this process.
///
/// The first installed version wins; later calls are ignored. The client must
/// install the version before it loads configuration for the first time.
pub fn install_app_version(version: &str) {
    let _ = APP_VERSION.set(version.to_string());
}

/// Returns the version installed for this process.
///
/// This is `None` until the client calls [`install_app_version`].
pub fn app_version() -> Option<&'static str> {
    APP_VERSION.get().map(String::as_str)
}

/// Returns the operating system hostname with the same lossy conversion as Tauri.
pub fn hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}
