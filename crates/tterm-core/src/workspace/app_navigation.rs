//! App page requests from places that do not own the tabs, from
//! `src/lib/appNavigation.ts`.

use serde::{Deserialize, Serialize};

/// A singleton app page that can be requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AppPage {
    /// The page that lists the terminal logs.
    TerminalLogs,
}

/// A tab whose logs the logs page should show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsTabRequest {
    /// The tab whose logs the page should show.
    pub tab_id: String,
    /// Whether the tab holds an SSH or a local session.
    pub session_type: SessionType,
    /// The SSH host, when the session is remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// The SSH port, when the session is remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// The SSH user, when the session is remote.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// The kind of session a logs request names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// A session to a remote host over SSH.
    Ssh,
    /// A shell running on this machine.
    Local,
}

type PageHandler = Box<dyn Fn(AppPage)>;

/// Handlers and pending requests for app pages.
#[derive(Default)]
pub struct AppNavigation {
    handler: Option<(u64, PageHandler)>,
    next_handler_id: u64,
    logs_tab_request: Option<LogsTabRequest>,
    logs_listeners: Vec<(u64, Box<dyn Fn()>)>,
    next_listener_id: u64,
}

impl AppNavigation {
    /// An app navigation with no handler and no pending request.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles page requests. Returns an id for
    /// [`Self::stop_handling_app_page_requests`]; stopping an older handler
    /// leaves a newer one in place.
    pub fn handle_app_page_requests(&mut self, next: impl Fn(AppPage) + 'static) -> u64 {
        let id = self.next_handler_id;
        self.next_handler_id += 1;
        self.handler = Some((id, Box::new(next)));
        id
    }

    /// Stops a handler if it is still the current one.
    pub fn stop_handling_app_page_requests(&mut self, id: u64) {
        if self
            .handler
            .as_ref()
            .is_some_and(|(current, _)| *current == id)
        {
            self.handler = None;
        }
    }

    /// Asks the current handler to open `page`.
    pub fn request_app_page(&self, page: AppPage) {
        if let Some((_, handler)) = &self.handler {
            handler(page);
        }
    }

    /// Opens the logs page on the newest session logged in a tab.
    pub fn show_logs_for_tab(&mut self, request: LogsTabRequest) {
        self.logs_tab_request = Some(request);
        for (_, listener) in &self.logs_listeners {
            listener();
        }
        self.request_app_page(AppPage::TerminalLogs);
    }

    /// The tab asked for since the last call, once.
    pub fn take_logs_tab_request(&mut self) -> Option<LogsTabRequest> {
        self.logs_tab_request.take()
    }

    /// Hears every logs tab request. Returns an id for
    /// [`Self::stop_listening_for_logs_tab_requests`].
    pub fn on_logs_tab_request(&mut self, listener: impl Fn() + 'static) -> u64 {
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.logs_listeners.push((id, Box::new(listener)));
        id
    }

    /// Stops a logs tab request listener.
    pub fn stop_listening_for_logs_tab_requests(&mut self, id: u64) {
        self.logs_listeners
            .retain(|(listener_id, _)| *listener_id != id);
    }
}
