//! Shell command and file-manager integration.
use crate::PlatformFuture;
/// Whether an integration is supported and currently installed.
#[derive(Clone, Debug)]
pub struct IntegrationStatus {
    pub supported: bool,
    pub installed: bool,
}
/// Native installation operations run away from the UI thread.
pub trait SystemIntegration: Send + Sync {
    /// Queries whether the shell command is supported and installed.
    fn shell_status(&self) -> PlatformFuture<'_, IntegrationStatus>;
    /// Installs or removes the shell command, preserving administrator cancellation.
    fn set_shell(
        &self,
        installed: bool,
        admin_prompt: String,
    ) -> PlatformFuture<'_, IntegrationStatus>;
    /// Queries whether the file-manager integration is installed.
    fn folder_status(&self) -> PlatformFuture<'_, IntegrationStatus>;
    /// Installs or removes the file-manager entry using a language code (unknown codes use English).
    fn set_folder(&self, installed: bool, language: String) -> PlatformFuture<'_, IntegrationStatus>;
    /// Refreshes existing integration using a language code (unknown codes use English).
    fn refresh(&self, language: String) -> PlatformFuture<'_, ()>;
}
