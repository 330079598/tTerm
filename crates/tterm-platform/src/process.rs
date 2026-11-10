//! Final process requests, after the application's existing exit policy.
/// Dispatches exit or restart without deciding whether the user should be asked first.
pub trait ProcessControl: Send + Sync {
    /// Requests final exit after application confirmation and saving.
    fn exit(&self, code: i32) -> Result<(), String>;
    /// Requests the existing host restart operation.
    fn restart(&self) -> Result<(), String>;
}
