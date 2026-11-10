//! OS verification delegates native prompts to the shared platform crate.
pub(crate) const CANCELED: &str = tterm_platform::verification::CANCELED;
pub(crate) mod platform {
    pub fn available() -> bool {
        tterm_platform::native::verification::platform::available()
    }
}
