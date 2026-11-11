//! Verification of the user at the keyboard.
/// Blocking OS prompts; callers must use their background blocking executor.
pub trait UserVerification: Send + Sync {
    /// Checks whether the OS can verify the current user.
    fn available(&self) -> bool;
    /// Blocks on the OS prompt; call only from a blocking worker.
    fn verify(&self, message: &str) -> Result<(), String>;
}
/// The cancellation message used by the existing password verification policy.
pub const CANCELED: &str = "Verification was canceled.";
