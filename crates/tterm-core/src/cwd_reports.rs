//! Follows local and remote shell directory reports.
//!
//! OSC 7 and OSC 9;9 parsing lives in `tterm-terminal`; the per-tab state and
//! the rule that an outer `ssh` pauses remote following live here.

use tterm_terminal::terminal_cwd::{
    is_same_host, is_unnamed_host, parse_cwd_report, parse_osc7_report,
};

/// Receives a remote directory, or none when the session ends.
pub type RemoteCwdCallback = Box<dyn FnMut(Option<&str>)>;

/// Callbacks and session information for one terminal tab.
pub struct CwdReportOptions {
    /// The tab whose backend PTY events are followed.
    pub tab_id: String,
    /// The session generation that owns those events.
    pub session_nonce: i64,
    /// Whether OSC reports come from the remote shell.
    pub is_remote: Box<dyn FnMut() -> bool>,
    /// Receives each new local directory.
    pub on_cwd: Box<dyn FnMut(&str)>,
    /// Receives each new remote directory, or none when the session ends.
    pub on_remote_cwd: RemoteCwdCallback,
    /// Reads the connected host's name when shell integration found it.
    pub remote_hostname: Box<dyn FnMut() -> Option<String>>,
    /// Receives another host's name while the terminal is on it.
    pub on_remote_away_host: RemoteCwdCallback,
    /// Called on every valid remote OSC 7 report.
    pub on_remote_report: Box<dyn FnMut()>,
}

/// Follows the working directory reported by one shell.
pub struct CwdReports {
    session_nonce: i64,
    options: CwdReportOptions,
    last_reported_cwd: Option<String>,
    last_remote_cwd: Option<String>,
    away_host: Option<String>,
    first_named_host: Option<String>,
}

impl CwdReports {
    /// Creates a report follower from its callbacks.
    pub fn new(options: CwdReportOptions) -> Self {
        Self {
            session_nonce: options.session_nonce,
            options,
            last_reported_cwd: None,
            last_remote_cwd: None,
            away_host: None,
            first_named_host: None,
        }
    }

    /// The tab this follower belongs to.
    pub fn tab_id(&self) -> &str {
        &self.options.tab_id
    }

    /// Handles an OSC 7 or OSC 9;9 report.
    pub fn handle_osc(&mut self, osc: i64, data: &str) {
        if (self.options.is_remote)() {
            if osc == 7 {
                self.report_remote(data);
            }
            return;
        }
        if let Some(cwd) = parse_cwd_report(osc, data) {
            self.report_cwd(&cwd);
        }
    }

    /// Handles a `pty-cwd-<tab>` event, ignoring another session generation.
    pub fn handle_pty_cwd(&mut self, session_nonce: i64, cwd: &str) {
        if session_nonce == self.session_nonce {
            self.report_cwd(cwd);
        }
    }

    /// Clears the remote directory and away host when the session goes.
    pub fn dispose(&mut self) {
        if self.last_remote_cwd.take().is_some() {
            (self.options.on_remote_cwd)(None);
        }
        self.set_away_host(None);
    }

    fn report_cwd(&mut self, cwd: &str) {
        if self.last_reported_cwd.as_deref() == Some(cwd) {
            return;
        }
        self.last_reported_cwd = Some(cwd.to_owned());
        (self.options.on_cwd)(cwd);
    }

    fn report_remote(&mut self, data: &str) {
        let Some(report) = parse_osc7_report(data) else {
            return;
        };
        (self.options.on_remote_report)();
        if !is_unnamed_host(&report.host) {
            let first_named_host = self
                .first_named_host
                .get_or_insert_with(|| report.host.clone());
            let hostname =
                (self.options.remote_hostname)().unwrap_or_else(|| first_named_host.clone());
            if !is_same_host(&report.host, &hostname) {
                self.set_away_host(Some(&report.host));
                return;
            }
        }
        self.set_away_host(None);
        if self.last_remote_cwd.as_deref() == Some(report.path.as_str()) {
            return;
        }
        self.last_remote_cwd = Some(report.path.clone());
        (self.options.on_remote_cwd)(Some(&report.path));
    }

    fn set_away_host(&mut self, host: Option<&str>) {
        if self.away_host.as_deref() == host {
            return;
        }
        self.away_host = host.map(str::to_owned);
        (self.options.on_remote_away_host)(host);
    }
}

#[cfg(test)]
mod tests;
