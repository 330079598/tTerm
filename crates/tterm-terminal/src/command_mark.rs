//! Shell integration command marks.
//!
//! OSC 133 and OSC 633 report where a prompt starts, where a command's output
//! starts and when it finished. The notification rules in `tterm-core` only
//! need the decoded value, so the type lives here, on the terminal side, for
//! the command-mark parser that follows in the next migration group.

/// One command mark reported by shell integration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandMark {
    /// A prompt starts (`A`).
    Prompt,
    /// A command's output starts (`C`).
    Output,
    /// A command finished, with the status its shell reported (`D`).
    Done {
        /// The exit status, when the mark carried one.
        exit_code: Option<i64>,
    },
}

#[cfg(test)]
mod tests;
