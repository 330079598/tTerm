//! Distribution reports from a local WSL launcher.

use crate::distro::{DistroId, detect_distro_id, parse_os_report};

/// Handles one OSC 777 payload for a terminal that may be remote.
///
/// An SSH host's distribution is read on a channel of its own, so a report
/// printed by a remote program is ignored.
pub fn handle_os_report(data: &str, is_remote: bool) -> Option<DistroId> {
    let info = parse_os_report(data)?;
    (!is_remote).then(|| detect_distro_id(Some(&info)))
}

#[cfg(test)]
mod tests;
