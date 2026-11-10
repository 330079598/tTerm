//! OS verification delegates native prompts to the shared platform crate.
pub(crate) mod platform {
    pub fn available() -> bool {
        tterm_platform::native::verification::platform::available()
    }
}
