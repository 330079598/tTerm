//! Searching with the host's `find`, which walks the tree where it lives
//! instead of one SFTP round trip per folder. `find` lists names only; the
//! matches are then looked up over SFTP for the details a listing shows.

use super::matcher::NameMatcher;
use super::ResultSink;
use crate::sftp::internal::api::base::directory_entry;
use crate::sftp::internal::exec::{cancelled, shell_path, shell_quote, RemoteScript};
use crate::sftp::internal::paths::{join_remote_path, parent_remote_path};
use crate::sftp::internal::types::SftpDirectoryEntry;
use russh::client::{Handle, Handler};
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::task::JoinSet;

/// Printed as the first record once the script knows `find` works, so
/// output from anything else, such as a Windows shell, is never read as
/// results.
const MARKER: &[u8] = b"tterm-find";

/// The script's exit status when the host cannot search this way.
const UNAVAILABLE_STATUS: u32 = 125;

/// How many matches are looked up at once.
const STAT_CONCURRENCY: usize = 32;

pub enum FindOutcome {
    /// The host cannot search with `find`; nothing was reported.
    Unavailable,
    Finished {
        cancelled: bool,
    },
}

/// Lists every path under `root` whose name may match, NUL-separated and
/// relative to `root`, after the marker. `find . -prune` checks that this
/// `find` takes the options used, as BusyBox builds may not.
pub(super) fn find_script(root: &str, matcher: &NameMatcher, include_hidden: bool) -> String {
    let mut expression = String::new();
    if !include_hidden {
        expression.push_str(" -path '*/.*' -prune -o");
    }
    if let Some(pattern) = matcher.find_name_pattern() {
        expression.push_str(&format!(" -iname {}", shell_quote(&pattern)));
    }
    format!(
        "cd {} 2>/dev/null || exit {UNAVAILABLE_STATUS}\n\
         command -v find >/dev/null 2>&1 || exit {UNAVAILABLE_STATUS}\n\
         find . -prune -print0 >/dev/null 2>&1 </dev/null || exit {UNAVAILABLE_STATUS}\n\
         printf 'tterm-find\\0'\n\
         exec find .{expression} -print0 2>/dev/null </dev/null\n",
        shell_path(root),
    )
}

/// Splits `find -print0` output into paths, checking the marker first.
#[derive(Default)]
pub(super) struct RecordParser {
    pending: Vec<u8>,
    /// None until the first record arrived.
    marked: Option<bool>,
}

impl RecordParser {
    /// The paths completed by `chunk`, relative to the root, without `.`.
    pub(super) fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);
        let mut paths = Vec::new();
        let mut start = 0;
        while let Some(offset) = self.pending[start..].iter().position(|&byte| byte == 0) {
            let record = &self.pending[start..start + offset];
            start += offset + 1;
            match self.marked {
                None => self.marked = Some(record == MARKER),
                Some(true) => {
                    let path = String::from_utf8_lossy(record);
                    let path = path.strip_prefix("./").unwrap_or(&path);
                    if path != "." && !path.is_empty() {
                        paths.push(path.to_string());
                    }
                }
                Some(false) => break,
            }
        }
        self.pending.drain(..start);
        paths
    }

    pub(super) fn is_find_output(&self) -> Option<bool> {
        self.marked
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Looks up `paths` for the details a listing shows. A path gone since
/// `find` listed it is left out.
async fn look_up(sftp: &Arc<SftpSession>, paths: Vec<String>) -> Vec<SftpDirectoryEntry> {
    let mut entries = Vec::with_capacity(paths.len());
    for batch in paths.chunks(STAT_CONCURRENCY) {
        let mut lookups = JoinSet::new();
        for path in batch {
            let sftp = sftp.clone();
            let path = path.clone();
            lookups.spawn(async move {
                let metadata = sftp.symlink_metadata(path.clone()).await;
                (path, metadata)
            });
        }
        while let Some(joined) = lookups.join_next().await {
            if let Ok((path, Ok(metadata))) = joined {
                let parent = parent_remote_path(&path).unwrap_or_else(|| "/".to_string());
                entries.push(directory_entry(
                    &parent,
                    file_name(&path).to_string(),
                    &metadata,
                ));
            }
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

pub(super) struct FindSearch<'a, H: Handler> {
    pub ssh: &'a Handle<H>,
    pub sftp: &'a Arc<SftpSession>,
    pub root: &'a str,
    pub matcher: &'a NameMatcher,
    pub include_hidden: bool,
}

impl<H: Handler> FindSearch<'_, H> {
    pub(super) async fn run(
        &self,
        sink: &mut ResultSink,
        cancel: &mut watch::Receiver<bool>,
    ) -> FindOutcome {
        let script = find_script(self.root, self.matcher, self.include_hidden);
        // A server that refuses a command channel, or runs no shell for
        // one, gets the SFTP search instead.
        let Ok(mut running) = RemoteScript::start(self.ssh, &script).await else {
            return FindOutcome::Unavailable;
        };
        let mut parser = RecordParser::default();
        loop {
            let output = tokio::select! {
                () = cancelled(cancel) => {
                    running.stop().await;
                    return FindOutcome::Finished { cancelled: true };
                }
                output = running.next_output() => output,
            };
            let Some(chunk) = output else { break };
            let paths = parser.push(&chunk);
            if parser.is_find_output() == Some(false) {
                running.stop().await;
                return FindOutcome::Unavailable;
            }
            let matches = paths
                .into_iter()
                .filter(|path| self.matcher.matches(file_name(path)))
                .map(|path| join_remote_path(self.root, &path))
                // One past the room is enough to know the results are cut.
                .take(sink.room() + 1)
                .collect::<Vec<_>>();
            if !matches.is_empty() && !sink.send(look_up(self.sftp, matches).await) {
                running.stop().await;
                return FindOutcome::Finished { cancelled: false };
            }
        }
        let outcome = running.finish().await;
        // `find` itself exits 1 after unreadable folders, which still
        // leaves the results it found.
        if parser.is_find_output() != Some(true) || outcome.exit_status == Some(UNAVAILABLE_STATUS)
        {
            return FindOutcome::Unavailable;
        }
        FindOutcome::Finished { cancelled: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_prunes_hidden_entries_and_prefilters_names() {
        let matcher = NameMatcher::new("log", false).unwrap();
        let script = find_script("/var/log", &matcher, false);
        assert!(script.starts_with("cd '/var/log' 2>/dev/null || exit 125\n"));
        assert!(script.ends_with(
            "exec find . -path '*/.*' -prune -o -iname '*log*' -print0 2>/dev/null </dev/null\n"
        ));

        let regex = NameMatcher::new("^a$", true).unwrap();
        let script = find_script("/", &regex, true);
        assert!(script.ends_with("exec find . -print0 2>/dev/null </dev/null\n"));
    }

    #[test]
    fn parser_reads_records_across_chunks_after_the_marker() {
        let mut parser = RecordParser::default();
        assert!(parser.push(b"tterm-fi").is_empty());
        assert_eq!(parser.is_find_output(), None);
        assert_eq!(parser.push(b"nd\0.\0./a/b"), Vec::<String>::new());
        assert_eq!(parser.is_find_output(), Some(true));
        assert_eq!(parser.push(b".txt\0./c\0"), vec!["a/b.txt", "c"]);
    }

    #[test]
    fn parser_rejects_output_without_the_marker() {
        let mut parser = RecordParser::default();
        assert!(parser
            .push(b"'exec' is not recognized\r\n\0./a\0")
            .is_empty());
        assert_eq!(parser.is_find_output(), Some(false));
    }

    #[test]
    fn names_come_from_the_last_component() {
        assert_eq!(file_name("a/b/c.txt"), "c.txt");
        assert_eq!(file_name("c.txt"), "c.txt");
    }
}
