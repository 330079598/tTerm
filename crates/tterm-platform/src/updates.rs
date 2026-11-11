//! Ownership-based update operations; route and cache policy belongs to core.
use crate::PlatformFuture;
use std::sync::Arc;
/// Public release metadata, independent of the updater plugin.
#[derive(Clone, Debug)]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}
/// Download progress preserves retries' initial reset and chunk-sized increments.
#[derive(Clone, Debug)]
pub enum DownloadEvent {
    Started { content_length: Option<u64> },
    Progress { chunk_length: usize },
    Finished,
}
/// Receives progress without turning a disconnected UI into download cancellation.
pub trait UpdateProgress: Send + Sync {
    /// Receives one progress event; a disconnected UI may ignore it.
    fn report(&self, event: DownloadEvent);
}
/// Checks an endpoint supplied by application policy.
pub trait Updates: Send + Sync {
    /// Checks a policy-selected endpoint without installing anything.
    fn check(&self, endpoint: String) -> PlatformFuture<'_, Option<Box<dyn AvailableUpdate>>>;
}
/// A checked release owns its metadata until consumed or dropped.
pub trait AvailableUpdate: Send + Sync {
    /// Returns neutral release metadata.
    fn metadata(&self) -> UpdateMetadata;
    /// Consumes the checked release and verifies the downloaded bytes.
    fn download(
        self: Box<Self>,
        progress: Arc<dyn UpdateProgress>,
    ) -> PlatformFuture<'static, Box<dyn DownloadedUpdate>>;
}
/// A verified download retains the exact installer metadata it was checked against.
pub trait DownloadedUpdate: Send + Sync {
    /// Transfers verified bytes to application caching without retaining a duplicate.
    fn take_bytes(&mut self) -> Vec<u8>;
    /// Consumes this ownership reference using bytes from the same verified download.
    fn install(self: Arc<Self>, bytes: Vec<u8>) -> PlatformFuture<'static, ()>;
}
