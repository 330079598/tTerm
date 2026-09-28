//! Name conflicts between a transfer's source and an existing destination.
//!
//! A transfer never asks per file. The UI runs a pre-flight check (the
//! `sftp_check_*_conflicts` commands), lets the user pick one
//! [`ConflictPolicy`] for the whole batch, and the transfer applies it to each
//! destination at the moment the file is about to be written. Resolving at
//! write time rather than trusting the pre-flight report keeps the policy
//! honest when the destination changes in between.

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

/// What the destination currently holds, as far as a stat can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetStat {
    pub size: u64,
    pub mtime: Option<i64>,
    pub is_dir: bool,
}

/// The source side of the comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceStat {
    pub size: u64,
    pub mtime: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateDecision {
    /// Write to this candidate (it is free, or the policy overwrites it).
    Use,
    Skip,
    /// The candidate is taken and the policy keeps both: try the next name.
    NextName,
}

/// Upper bound on `name (n)` candidates, so a directory full of copies ends
/// in an error instead of an unbounded stat loop.
pub const MAX_RENAME_ATTEMPTS: u32 = 999;

/// Most conflicts listed in a pre-flight report; `total` still counts all.
pub const MAX_REPORTED_CONFLICTS: usize = 50;

/// Whether the destination provably already holds the source's bytes: an
/// identical length and a destination mtime at least as new as the source.
///
/// A size match alone is not evidence: an edited file that kept its length
/// would be reported as transferred while the destination still holds stale
/// bytes. Clock skew between client and server can only make this stricter.
/// The known blind spot of every mtime+size check (rsync has it too): a
/// rewrite that keeps the length while carrying an older mtime compares as
/// already transferred.
pub fn target_holds_source(source: SourceStat, target: TargetStat) -> bool {
    if target.is_dir || target.size != source.size {
        return false;
    }
    match (source.mtime, target.mtime) {
        (Some(source_mtime), Some(target_mtime)) => target_mtime >= source_mtime,
        _ => false,
    }
}

/// Decides what to do with one destination candidate.
///
/// `skip_existing` is set by a batch retry: files an earlier attempt already
/// finished are skipped before the policy is consulted. Under `Rename` that
/// includes candidates the earlier attempt renamed to, so a retry never piles
/// up further copies of files that already made it.
pub fn decide_candidate(
    policy: ConflictPolicy,
    source: SourceStat,
    target: Option<TargetStat>,
    skip_existing: bool,
) -> CandidateDecision {
    let Some(target) = target else {
        return CandidateDecision::Use;
    };
    if skip_existing && target_holds_source(source, target) {
        return CandidateDecision::Skip;
    }
    match policy {
        ConflictPolicy::Overwrite => CandidateDecision::Use,
        ConflictPolicy::Skip => CandidateDecision::Skip,
        ConflictPolicy::OverwriteIfNewer => match (source.mtime, target.mtime) {
            (Some(source_mtime), Some(target_mtime))
                if !target.is_dir && source_mtime > target_mtime =>
            {
                CandidateDecision::Use
            }
            // Without both mtimes "newer" cannot be shown, and a directory
            // cannot be replaced by a file: keep what is there.
            _ => CandidateDecision::Skip,
        },
        ConflictPolicy::Rename => CandidateDecision::NextName,
    }
}

/// Multi-part extensions kept intact when a copy number is inserted, so
/// `backup.tar.gz` becomes `backup (1).tar.gz` rather than `backup.tar (1).gz`.
const COMPOUND_EXTENSIONS: &[&str] = &[".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst"];

/// The `attempt`-th candidate name: the original name for 0, otherwise
/// `stem (attempt).ext`. Dotfiles without a further extension keep the whole
/// name as stem (`.bashrc (1)`).
pub fn candidate_file_name(name: &str, attempt: u32) -> String {
    if attempt == 0 {
        return name.to_string();
    }

    let lower = name.to_ascii_lowercase();
    let split_at = COMPOUND_EXTENSIONS
        .iter()
        .find(|ext| lower.len() > ext.len() && lower.ends_with(*ext))
        .map(|ext| name.len() - ext.len())
        .or_else(|| name.rfind('.').filter(|&index| index > 0));

    match split_at {
        Some(index) => format!("{} ({attempt}){}", &name[..index], &name[index..]),
        None => format!("{name} ({attempt})"),
    }
}

/// One entry of a pre-flight report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferConflict {
    pub source_path: String,
    pub target_path: String,
    pub source_size: u64,
    /// Seconds since the Unix epoch.
    pub source_mtime: Option<i64>,
    pub target_size: u64,
    /// Seconds since the Unix epoch.
    pub target_mtime: Option<i64>,
    pub target_is_dir: bool,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictReport {
    /// Every conflicting file, including those beyond `conflicts`.
    pub total: usize,
    /// Files the whole batch would transfer; lets the UI tell "all of them
    /// exist" apart from "a few of them exist".
    pub file_count: usize,
    /// The first [`MAX_REPORTED_CONFLICTS`] conflicts in transfer order.
    pub conflicts: Vec<TransferConflict>,
}

impl ConflictReport {
    pub fn push(&mut self, conflict: TransferConflict) {
        self.total += 1;
        if self.conflicts.len() < MAX_REPORTED_CONFLICTS {
            self.conflicts.push(conflict);
        }
    }
}

/// Seconds since the Unix epoch for local metadata's mtime.
pub fn local_mtime_secs(metadata: &std::fs::Metadata) -> Option<i64> {
    let modified = metadata.modified().ok()?;
    Some(
        modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64,
    )
}

/// Stat of a local destination; `None` when nothing is there (or it cannot
/// be read, in which case the transfer surfaces the real error).
pub async fn local_target_stat(path: &std::path::Path) -> Option<TargetStat> {
    let metadata = tokio::fs::metadata(path).await.ok()?;
    Some(TargetStat {
        size: metadata.len(),
        mtime: local_mtime_secs(&metadata),
        is_dir: metadata.is_dir(),
    })
}

/// Stat of a remote destination; `None` when nothing is there (or it cannot
/// be read, in which case the transfer surfaces the real error).
pub async fn remote_target_stat(
    sftp: &russh_sftp::client::SftpSession,
    path: &str,
) -> Option<TargetStat> {
    let attrs = sftp.metadata(path).await.ok()?;
    Some(TargetStat {
        size: attrs.size.unwrap_or(0),
        mtime: attrs.mtime.map(i64::from),
        is_dir: attrs.is_dir(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(size: u64, mtime: i64) -> SourceStat {
        SourceStat {
            size,
            mtime: Some(mtime),
        }
    }

    fn file(size: u64, mtime: i64) -> Option<TargetStat> {
        Some(TargetStat {
            size,
            mtime: Some(mtime),
            is_dir: false,
        })
    }

    #[test]
    fn free_destination_is_used_under_every_policy() {
        for policy in [
            ConflictPolicy::Overwrite,
            ConflictPolicy::Skip,
            ConflictPolicy::OverwriteIfNewer,
            ConflictPolicy::Rename,
        ] {
            assert_eq!(
                decide_candidate(policy, source(1, 1), None, false),
                CandidateDecision::Use
            );
        }
    }

    #[test]
    fn policies_resolve_an_existing_file() {
        let src = source(10, 200);
        let target = file(20, 100);
        assert_eq!(
            decide_candidate(ConflictPolicy::Overwrite, src, target, false),
            CandidateDecision::Use
        );
        assert_eq!(
            decide_candidate(ConflictPolicy::Skip, src, target, false),
            CandidateDecision::Skip
        );
        assert_eq!(
            decide_candidate(ConflictPolicy::Rename, src, target, false),
            CandidateDecision::NextName
        );
    }

    #[test]
    fn overwrite_if_newer_needs_a_strictly_newer_source() {
        let policy = ConflictPolicy::OverwriteIfNewer;
        assert_eq!(
            decide_candidate(policy, source(1, 200), file(1, 100), false),
            CandidateDecision::Use
        );
        assert_eq!(
            decide_candidate(policy, source(1, 100), file(1, 100), false),
            CandidateDecision::Skip
        );
        assert_eq!(
            decide_candidate(policy, source(1, 100), file(1, 200), false),
            CandidateDecision::Skip
        );
        let unknown = SourceStat {
            size: 1,
            mtime: None,
        };
        assert_eq!(
            decide_candidate(policy, unknown, file(1, 100), false),
            CandidateDecision::Skip
        );
        let dir = Some(TargetStat {
            size: 0,
            mtime: Some(0),
            is_dir: true,
        });
        assert_eq!(
            decide_candidate(policy, source(1, 200), dir, false),
            CandidateDecision::Skip
        );
    }

    #[test]
    fn retry_skips_finished_files_before_the_policy() {
        // Same size, destination written after the source: already transferred.
        let finished = file(10, 300);
        assert_eq!(
            decide_candidate(ConflictPolicy::Rename, source(10, 200), finished, true),
            CandidateDecision::Skip
        );
        assert_eq!(
            decide_candidate(ConflictPolicy::Overwrite, source(10, 200), finished, true),
            CandidateDecision::Skip
        );
        // A different file under the same name still follows the policy.
        assert_eq!(
            decide_candidate(ConflictPolicy::Rename, source(10, 200), file(11, 300), true),
            CandidateDecision::NextName
        );
        assert_eq!(
            decide_candidate(
                ConflictPolicy::Overwrite,
                source(10, 200),
                file(11, 300),
                true
            ),
            CandidateDecision::Use
        );
    }

    #[test]
    fn candidate_names_insert_the_copy_number_before_the_extension() {
        assert_eq!(candidate_file_name("nginx.conf", 0), "nginx.conf");
        assert_eq!(candidate_file_name("nginx.conf", 1), "nginx (1).conf");
        assert_eq!(candidate_file_name("Makefile", 2), "Makefile (2)");
        assert_eq!(candidate_file_name(".bashrc", 1), ".bashrc (1)");
        assert_eq!(candidate_file_name(".env.local", 1), ".env (1).local");
        assert_eq!(candidate_file_name("backup.tar.gz", 1), "backup (1).tar.gz");
        assert_eq!(candidate_file_name("BACKUP.TAR.XZ", 3), "BACKUP (3).TAR.XZ");
        assert_eq!(candidate_file_name(".tar.gz", 1), ".tar (1).gz");
    }

    #[test]
    fn report_counts_everything_but_lists_a_bounded_prefix() {
        let mut report = ConflictReport::default();
        for index in 0..(MAX_REPORTED_CONFLICTS + 5) {
            report.push(TransferConflict {
                source_path: format!("s{index}"),
                target_path: format!("t{index}"),
                source_size: 0,
                source_mtime: None,
                target_size: 0,
                target_mtime: None,
                target_is_dir: false,
            });
        }
        assert_eq!(report.total, MAX_REPORTED_CONFLICTS + 5);
        assert_eq!(report.conflicts.len(), MAX_REPORTED_CONFLICTS);
    }
}
