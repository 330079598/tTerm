//! Running the compose bar's lines one at a time, from `src/lib/composerRun.ts`.
//!
//! A line goes to the shell only once the one before it has ended and the
//! shell is back at its prompt, and a failed line holds the rest until the
//! user says what to do. Reading the shell's prompt marks and sending the
//! lines stays with the clients; these are the state conversions they drive.

use crate::js::is_js_whitespace;

/// `unfinished`: sent, but the run was stopped before it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunLineStatus {
    /// Waits for the lines before it.
    Pending,
    /// Sent, and its end has not been seen.
    Running,
    /// Ended without a failing status.
    Succeeded,
    /// Ended with a failing status.
    Failed,
    /// Sent, but the run was stopped before it ended.
    Unfinished,
}

/// One line of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLine {
    /// The line as typed.
    pub text: String,
    /// Where the line is in the run.
    pub status: RunLineStatus,
    /// The status the shell reported for the line, when it reported one.
    pub exit_code: Option<i32>,
}

/// Which line runs, or why no line does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    /// A line runs.
    Running,
    /// A line failed and the rest wait.
    Failed,
    /// Every line ran.
    Done,
    /// The user or the session ended it.
    Stopped,
}

/// The lines of one line-by-line run and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerRun {
    /// The lines, in the order they run.
    pub lines: Vec<RunLine>,
    /// Where the run is.
    pub phase: RunPhase,
    /// The line that runs, or the one that failed; -1 for a run without lines.
    pub current: i32,
}

/// What the shell's prompt marks reported, as the run needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerPromptEvent {
    /// The shell drew a prompt.
    Prompt {
        /// The status the shell gave with this prompt.
        exit_code: Option<i32>,
        /// Whether the shell marked that it ran the line; `None` for a shell
        /// that sends no command marks.
        started_command: Option<bool>,
        /// The status the shell gave, even with nothing run.
        status: Option<i32>,
    },
    /// The session's marks ended.
    Ended,
}

fn with_line(run: &ComposerRun, index: i32, line: PartialRunLine) -> Vec<RunLine> {
    run.lines
        .iter()
        .enumerate()
        .map(|(at, current)| {
            if at as i32 == index {
                RunLine {
                    text: line.text.clone().unwrap_or_else(|| current.text.clone()),
                    status: line.status.unwrap_or(current.status),
                    exit_code: line.exit_code,
                }
            } else {
                current.clone()
            }
        })
        .collect()
}

struct PartialRunLine {
    text: Option<String>,
    status: Option<RunLineStatus>,
    exit_code: Option<i32>,
}

/// Runs line `index` next, or ends the run when there is none.
fn run_from(lines: Vec<RunLine>, index: i32) -> ComposerRun {
    if index >= lines.len() as i32 {
        return ComposerRun {
            current: lines.len() as i32 - 1,
            lines,
            phase: RunPhase::Done,
        };
    }
    let lines = lines
        .into_iter()
        .enumerate()
        .map(|(at, line)| {
            if at as i32 == index {
                RunLine {
                    text: line.text,
                    status: RunLineStatus::Running,
                    exit_code: None,
                }
            } else {
                line
            }
        })
        .collect();
    ComposerRun {
        lines,
        phase: RunPhase::Running,
        current: index,
    }
}

/// Starts a run over `lines`.
pub fn start_run(lines: &[String]) -> ComposerRun {
    let pending = lines
        .iter()
        .map(|text| RunLine {
            text: text.clone(),
            status: RunLineStatus::Pending,
            exit_code: None,
        })
        .collect();
    run_from(pending, 0)
}

/// The running line ended with `exit_code`, which the shell may not tell (a
/// comment runs nothing); only a non-zero status counts as failing.
pub fn finish_line(run: &ComposerRun, exit_code: Option<i32>) -> ComposerRun {
    if run.phase != RunPhase::Running {
        return run.clone();
    }
    let failed = exit_code.is_some_and(|code| code != 0);
    let lines = with_line(
        run,
        run.current,
        PartialRunLine {
            text: None,
            status: Some(if failed {
                RunLineStatus::Failed
            } else {
                RunLineStatus::Succeeded
            }),
            exit_code,
        },
    );
    if failed {
        return ComposerRun {
            lines,
            phase: RunPhase::Failed,
            current: run.current,
        };
    }
    run_from(lines, run.current + 1)
}

/// The shell is back at a prompt, or its marks stopped.
///
/// A prompt after which the shell marks that it ran nothing is not the running
/// line's end when the shell marks the commands it runs: Enter pressed in the
/// terminal on an empty line, or a prompt drawn again, gives one before the line
/// is read, and the next line would go to the running one as its input. A
/// comment runs nothing, and a line the shell can't parse ends with a failing
/// status.
pub fn prompt_reached(run: &ComposerRun, event: &ComposerPromptEvent) -> ComposerRun {
    let ComposerPromptEvent::Prompt {
        exit_code,
        started_command,
        status,
    } = event
    else {
        return stop_run(run);
    };
    if *started_command != Some(false) {
        return finish_line(run, *exit_code);
    }
    if run.phase != RunPhase::Running {
        return run.clone();
    }
    let line = run
        .lines
        .get(run.current.max(0) as usize)
        .map(|line| line.text.as_str())
        .unwrap_or_default();
    if starts_with_comment(line) {
        return finish_line(run, None);
    }
    if let Some(status) = *status
        && status != 0
    {
        return finish_line(run, Some(status));
    }
    run.clone()
}

fn starts_with_comment(line: &str) -> bool {
    line.chars()
        .find(|character| !is_js_whitespace(*character))
        .is_some_and(|character| character == '#')
}

/// Runs the failed line again.
pub fn retry_line(run: &ComposerRun) -> ComposerRun {
    if run.phase != RunPhase::Failed {
        return run.clone();
    }
    run_from(run.lines.clone(), run.current)
}

/// Leaves the failed line as it is and goes on with the next one.
pub fn continue_run(run: &ComposerRun) -> ComposerRun {
    if run.phase != RunPhase::Failed {
        return run.clone();
    }
    run_from(run.lines.clone(), run.current + 1)
}

/// Sends no more lines; one still running carries on in the shell.
pub fn stop_run(run: &ComposerRun) -> ComposerRun {
    if run.phase != RunPhase::Running && run.phase != RunPhase::Failed {
        return run.clone();
    }
    let lines = if run.phase == RunPhase::Running {
        with_line(
            run,
            run.current,
            PartialRunLine {
                text: None,
                status: Some(RunLineStatus::Unfinished),
                exit_code: None,
            },
        )
    } else {
        run.lines.clone()
    };
    ComposerRun {
        lines,
        phase: RunPhase::Stopped,
        current: run.current,
    }
}

/// The run holds the bar: lines still wait to go.
pub fn is_run_active(run: Option<&ComposerRun>) -> bool {
    run.is_some_and(|run| run.phase == RunPhase::Running || run.phase == RunPhase::Failed)
}
