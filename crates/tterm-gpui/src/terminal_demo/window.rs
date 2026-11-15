//! The prototype's window: one terminal view attached to a core-owned session.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::prelude::*;
use gpui_kit::{App, Bounds, Entity, Window, WindowBounds, WindowOptions, div, point, px, size};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::{AppConfig, load_config_file};
use tterm_core::events::AppEvent;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};

use super::binding::{SessionManager, TabBinding, scoped::ConnectionState};
use super::command::CommandSpec;
use super::conpty::{log_conpty_source, prepare_conpty};
use super::intents::DEFAULT_TITLE;
use super::keymap::{KeymapRouter, encoder_config, route_keymap};

const TAB_ID: &str = "gpui-main";
const INITIAL_GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const INITIAL_CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};

/// Runs the application with one prototype window until it closes.
pub(super) fn run_window(
    command: CommandSpec,
    services: Arc<AppServices>,
    events: UnboundedReceiver<AppEvent>,
) {
    prepare_conpty();
    let app_config = load_config_file().unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to load config, using defaults: {error}");
        AppConfig::default()
    });
    let mac = cfg!(target_os = "macos");
    let keymap = Rc::new(KeymapRouter::new(&app_config.keymap, mac));
    let keyboard = encoder_config(mac, app_config.mac_option_is_meta);
    let attention = Rc::new(RefCell::new(TabAttentionStore::new()));
    let sessions = Rc::new(SessionManager::new(Arc::clone(&services), attention));
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        tterm_terminal_view::view::bind_keys(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = open_window(cx, command, sessions, events, keyboard, keymap) {
            eprintln!("tterm-gpui: {error}");
            cx.quit();
        }
    });
}

fn open_window(
    cx: &mut App,
    command: CommandSpec,
    sessions: Rc<SessionManager>,
    events: UnboundedReceiver<AppEvent>,
    keyboard: tterm_terminal_view::input::keyboard::EncoderConfig,
    keymap: Rc<KeymapRouter>,
) -> Result<(), String> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(120.0), px(120.0)),
            size: size(px(1200.0), px(800.0)),
        })),
        ..WindowOptions::default()
    };
    let (handle, prototype) = gpui_kit::open_window(options, cx, move |_, cx| {
        cx.new(|cx| Prototype::new(cx, &command, sessions, events, keyboard, keymap))
    })
    .map_err(|error| error.to_string())?;
    handle
        .update(cx, |_, window, cx| {
            window.set_window_title(DEFAULT_TITLE);
            let view = prototype.read(cx).view.clone();
            view.update(cx, |view, cx| view.activate(window, cx));
            window.activate_window();
        })
        .map_err(|error| error.to_string())?;

    #[cfg(all(feature = "automation", debug_assertions))]
    super::automation::start(handle, prototype, cx);
    Ok(())
}

/// The window's root view: the attached terminal view.
pub(super) struct Prototype {
    view: Entity<TerminalView>,
    binding: Option<Rc<TabBinding>>,
    title: Arc<Mutex<Option<String>>>,
    current_title: String,
}

impl Prototype {
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn automation_state(&self) -> String {
        format!("title={}", self.current_title)
    }

    fn new(
        cx: &mut Context<Self>,
        command: &CommandSpec,
        sessions: Rc<SessionManager>,
        mut events: UnboundedReceiver<AppEvent>,
        keyboard: tterm_terminal_view::input::keyboard::EncoderConfig,
        keymap: Rc<KeymapRouter>,
    ) -> Self {
        let (binding, open_error) = match command.connection_options() {
            Ok(connection) => {
                let binding = sessions.open(TAB_ID, connection, INITIAL_GRID, INITIAL_CELL);
                let error = binding.scoped_state().start_error().map(str::to_string);
                (Some(binding), error)
            }
            Err(error) => (None, Some(error)),
        };
        if let Some(binding) = &binding {
            binding.enter();
        }
        let attachment = binding.as_ref().and_then(|binding| binding.attach());
        let Some(attachment) = attachment else {
            let error = open_error.unwrap_or_else(|| "Failed to start terminal".to_string());
            return failed(cx, binding, error);
        };
        log_conpty_source();
        let model = attachment.model();
        let title = attachment.title();
        let session_nonce = attachment.session_nonce();
        let binding = binding.or_else(|| sessions.tab(TAB_ID));
        let Some(binding) = binding else {
            return failed(cx, None, "The terminal tab was not registered".to_string());
        };
        let keymap_route = Rc::clone(&keymap);
        let config = TerminalViewConfig {
            input: Some(Rc::new(attachment.input())),
            resize: Some(Rc::new(attachment.resizer())),
            keyboard,
            keymap: Some(Rc::new(move |event| route_keymap(&keymap_route, event))),
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, model, config));

        let mut wake = attachment.subscribe();
        cx.spawn(async move |prototype, cx| {
            while wake.changed().await {
                wake.acknowledge();
                let updated = prototype.update(cx, |prototype, cx| {
                    let current = prototype
                        .binding
                        .as_ref()
                        .and_then(|binding| binding.session())
                        .is_some_and(|session| session.session_nonce() == session_nonce);
                    if current {
                        prototype.view.update(cx, |view, cx| view.notify_output(cx));
                    }
                    if prototype.title.lock().is_ok_and(|title| title.is_some()) {
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();

        let event_binding = Rc::clone(&binding);
        cx.spawn(async move |prototype, cx| {
            while let Some(event) = events.recv().await {
                if !handle_event(&event_binding, event) {
                    continue;
                }
                if prototype
                    .update(cx, |prototype, cx| {
                        prototype.view.update(cx, |view, cx| view.notify_output(cx));
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        Self {
            view,
            binding: Some(binding),
            title,
            current_title: DEFAULT_TITLE.to_string(),
        }
    }
}

fn failed(
    cx: &mut Context<Prototype>,
    binding: Option<Rc<TabBinding>>,
    error: String,
) -> Prototype {
    eprintln!("tterm-gpui: {error}");
    let terminal = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
    terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
    let view = cx.new(|cx| TerminalView::new(cx, terminal, Default::default()));
    Prototype {
        view,
        binding,
        title: Arc::new(Mutex::new(None)),
        current_title: DEFAULT_TITLE.to_string(),
    }
}

fn handle_event(binding: &TabBinding, event: AppEvent) -> bool {
    let Some(session) = binding.session() else {
        return false;
    };
    let session_nonce = session.session_nonce();
    match event {
        AppEvent::Ssh(SshEvent::HostKeyPrompt { tab_id, payload }) if tab_id == TAB_ID => {
            binding.set_host_key_prompt(session_nonce, Some(payload.clone()));
            session.show_host_key(payload);
            true
        }
        AppEvent::Ssh(SshEvent::HostKeyPromptExpired { tab_id, request_id })
            if tab_id == TAB_ID =>
        {
            let current = binding
                .scoped_state()
                .host_key_prompt()
                .is_some_and(|prompt| prompt.request_id == request_id);
            if !current {
                return false;
            }
            binding.set_host_key_prompt(session_nonce, None);
            session.expire_host_key(&request_id);
            true
        }
        AppEvent::Ssh(SshEvent::AuthPrompt { tab_id, payload }) if tab_id == TAB_ID => {
            session.show_auth(payload);
            true
        }
        AppEvent::Ssh(SshEvent::AuthPromptExpired { tab_id, request_id }) if tab_id == TAB_ID => {
            session.expire_auth(&request_id);
            true
        }
        AppEvent::Ssh(SshEvent::ConnectionProgress { tab_id, payload }) if tab_id == TAB_ID => {
            let state = connection_state(&payload);
            binding.set_connection_progress(session_nonce, Some(payload));
            binding.set_connection_state(session_nonce, state);
            true
        }
        AppEvent::Terminal(TerminalEvent::Output {
            tab_id,
            session_nonce: output_nonce,
            text,
        }) if tab_id == TAB_ID => session.accept_output(output_nonce, text.as_bytes()),
        AppEvent::Terminal(TerminalEvent::Exit { tab_id, .. }) if tab_id == TAB_ID => {
            binding.set_connection_state(session_nonce, ConnectionState::Disconnected);
            true
        }
        _ => false,
    }
}

fn connection_state(payload: &SshConnectionProgressPayload) -> ConnectionState {
    match payload.phase.as_str() {
        "ready" => ConnectionState::Connected,
        "retrying" | "serial_waiting" => ConnectionState::Reconnecting,
        "failed" | "retry_exhausted" => ConnectionState::Error,
        _ => ConnectionState::Connecting,
    }
}

impl Drop for Prototype {
    fn drop(&mut self) {
        if let Some(binding) = &self.binding {
            binding.leave();
        }
    }
}

impl Render for Prototype {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(mut title) = self.title.lock()
            && let Some(title) = title.take()
        {
            window.set_window_title(&title);
            self.current_title = title;
        }
        div().size_full().child(self.view.clone())
    }
}

#[cfg(test)]
mod tests {
    use tterm_core::events::ssh::SshConnectionProgressPayload;

    use super::{ConnectionState, connection_state};

    #[test]
    fn connection_progress_maps_to_the_scoped_connection_state() {
        assert_eq!(
            connection_state(&SshConnectionProgressPayload::new("ready", "ready")),
            ConnectionState::Connected
        );
        assert_eq!(
            connection_state(&SshConnectionProgressPayload::new("retrying", "retrying")),
            ConnectionState::Reconnecting
        );
        assert_eq!(
            connection_state(&SshConnectionProgressPayload::new(
                "retry_exhausted",
                "exhausted"
            )),
            ConnectionState::Error
        );
        assert_eq!(
            connection_state(&SshConnectionProgressPayload::new(
                "host_key_checking",
                "checking"
            )),
            ConnectionState::Connecting
        );
    }
}
