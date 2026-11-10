//! Launch forwarding data, independent of a particular single-instance plugin.
use std::path::PathBuf;
/// Arguments and working directory received from a second instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub args: Vec<String>,
    pub cwd: PathBuf,
}
