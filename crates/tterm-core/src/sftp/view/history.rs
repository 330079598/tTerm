//! Back and forward history of the remote directories an SFTP drawer has
//! shown, from `src/components/SftpDrawer/sftpHistory.ts`.

/// The number of directories one drawer's history keeps.
pub const SFTP_HISTORY_LIMIT: usize = 50;

/// Back/forward history of the remote directories one SFTP drawer has shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpHistory {
    /// The directories, oldest first.
    pub paths: Vec<String>,
    /// Position of the directory on screen; -1 before the first listing.
    pub index: i32,
}

/// A history with no directory shown yet.
pub const EMPTY_SFTP_HISTORY: SftpHistory = SftpHistory {
    paths: Vec::new(),
    index: -1,
};

/// One step back or forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpHistoryStep {
    /// The directory before the current one.
    Back,
    /// The directory after the current one.
    Forward,
}

impl SftpHistoryStep {
    fn offset(self) -> i32 {
        match self {
            SftpHistoryStep::Back => -1,
            SftpHistoryStep::Forward => 1,
        }
    }
}

/// Records a regular navigation.
///
/// Reloading the current directory changes nothing; a new directory drops the
/// forward entries, like a browser.
pub fn record_sftp_visit(history: &SftpHistory, path: &str) -> SftpHistory {
    if history.index >= 0
        && history
            .paths
            .get(history.index as usize)
            .is_some_and(|current| current == path)
    {
        return history.clone();
    }

    let keep = if history.index < 0 {
        0
    } else {
        (history.index + 1) as usize
    };
    let mut paths: Vec<String> = history.paths.iter().take(keep).cloned().collect();
    paths.push(path.to_owned());
    if paths.len() > SFTP_HISTORY_LIMIT {
        paths.drain(..paths.len() - SFTP_HISTORY_LIMIT);
    }
    SftpHistory {
        index: paths.len() as i32 - 1,
        paths,
    }
}

/// The index one step back or forward, or `None` at either end.
pub fn sftp_history_target(history: &SftpHistory, step: SftpHistoryStep) -> Option<i32> {
    let target = history.index + step.offset();
    if target >= 0 && target < history.paths.len() as i32 {
        Some(target)
    } else {
        None
    }
}

/// Moves onto `index` once its listing loaded.
///
/// The server reports the resolved path, which can differ from the stored one,
/// so the entry follows it.
pub fn land_sftp_history(history: &SftpHistory, index: i32, path: &str) -> SftpHistory {
    if index < 0 || index >= history.paths.len() as i32 {
        return record_sftp_visit(history, path);
    }
    let position = index as usize;
    if history.index == index && history.paths[position] == path {
        return history.clone();
    }

    let mut paths = history.paths.clone();
    paths[position] = path.to_owned();
    SftpHistory { paths, index }
}

/// Forgets an entry whose directory failed to load (deleted, no longer
/// readable), so the next step skips past it instead of failing again.
pub fn drop_sftp_history_entry(history: &SftpHistory, index: i32) -> SftpHistory {
    if index < 0 || index >= history.paths.len() as i32 || index == history.index {
        return history.clone();
    }

    SftpHistory {
        paths: history
            .paths
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != index as usize)
            .map(|(_, path)| path.clone())
            .collect(),
        index: if index < history.index {
            history.index - 1
        } else {
            history.index
        },
    }
}
