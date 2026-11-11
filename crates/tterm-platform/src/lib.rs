//! Runtime-neutral platform capabilities and reusable native implementations.

pub mod clipboard;
pub mod dialogs;
pub mod drag;
pub mod integration;
pub mod launch;
pub mod native;
pub mod notifications;
pub mod open;
pub mod process;
pub mod updates;
pub mod verification;
pub mod window;

/// An object-safe asynchronous operation whose failures remain actionable strings.
pub type PlatformFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, String>> + Send + 'a>>;
pub mod tray;
