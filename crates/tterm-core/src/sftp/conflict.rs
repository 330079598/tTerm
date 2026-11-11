use serde::{Deserialize, Serialize};

/// How an existing destination file is treated. `Overwrite` is the default so
/// that callers which predate conflict handling keep their behaviour.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConflictPolicy {
    #[default]
    Overwrite,
    Skip,
    /// Overwrite only when the source is strictly newer than the destination.
    /// Transfers do not carry mtimes across, so a destination written by an
    /// earlier transfer carries its transfer time: re-sending an unchanged
    /// source is skipped, while a source edited after that transfer wins.
    OverwriteIfNewer,
    /// Keep both: write to the first free `name (n).ext`.
    Rename,
}
