//! Launch forwarding data, independent of a particular single-instance plugin.
use std::path::PathBuf;
/// Arguments and working directory received from a second instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub args: Vec<String>,
    pub cwd: PathBuf,
}
/// Lifetime of a native primary-instance lock/listener.
pub trait InstanceGuard: Send + Sync {}
/// Startup must stop before opening storage when the request was forwarded.
pub enum InstanceClaim {
    Primary(Box<dyn InstanceGuard>),
    Forwarded,
}
/// Native-client startup capability. Tauri's plugin owns its own lock instead.
pub trait SingleInstance: Send + Sync {
    /// Claims primary ownership or forwards this request before storage opens.
    fn claim_or_forward(
        &self,
        identifier: &str,
        request: LaunchRequest,
    ) -> Result<InstanceClaim, String>;
}
