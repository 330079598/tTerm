//! The prototype's window: one terminal view attached to a core-owned session.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, Rgba, Window, div};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::{AppConfig, load_config_file};
use tterm_core::events::AppEvent;
use tterm_core::events::ssh::{SshConnectionProgressPayload, SshEvent};
use tterm_core::events::terminal::TerminalEvent;
use tterm_core::i18n::catalogue::Catalogue;
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{ClipboardPolicy, Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::tokens::theme::Theme;

use super::binding::{SessionManager, TabBinding, scoped::ConnectionState};
use super::clipboard::{PasteController, local_drop_text};
use super::command::CommandSpec;
use super::conpty::{log_conpty_source, prepare_conpty};
use super::intents::DEFAULT_TITLE;
use super::keymap::{KeymapRouter, encoder_config, route_keymap};
use crate::app_window::background;
use crate::app_window::chrome::shell;
use crate::app_window::chrome::titlebar::TitleBar;
use crate::app_window::chrome::{ActionHandler, TitleBarAction};
use crate::app_window::startup;
use crate::app_window::state;
use crate::app_window::state::WindowGeometry;
use crate::locale::{Locale, LocaleGlobal};

const TAB_ID: &str = "gpui-main";
const INITIAL_GRID: GridSize = GridSize {
    columns: 80,
    screen_lines: 24,
};
const INITIAL_CELL: CellSize = CellSize {
    width: 8,
    height: 16,
};

struct PrototypeStartup {
    command: CommandSpec,
    sessions: Rc<SessionManager>,
    events: UnboundedReceiver<AppEvent>,
    keyboard: tterm_terminal_view::input::keyboard::EncoderConfig,
    keymap: Rc<KeymapRouter>,
    locale: Entity<Locale>,
    config: AppConfig,
    theme: Theme,
    background: Rgba,
}

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
    if let Err(error) = Catalogue::embedded() {
        eprintln!("tterm-gpui: failed to load translations from src/i18n/locales: {error}");
        return;
    }
    let language = Language::from_locale(&app_config.language);
    let mac = cfg!(target_os = "macos");
    let keymap = Rc::new(KeymapRouter::new(&app_config.keymap, mac));
    let keyboard = encoder_config(mac, app_config.mac_option_is_meta);
    let attention = Rc::new(RefCell::new(TabAttentionStore::new()));
    let clipboard_policy = if app_config.osc52_clipboard_write {
        ClipboardPolicy::CopyOnly
    } else {
        ClipboardPolicy::Disabled
    };
    let sessions = Rc::new(
        SessionManager::new(Arc::clone(&services), attention).with_terminal_config(Config {
            clipboard: clipboard_policy,
            scrollback: tterm_core::config::scrollback::resolve_scrollback_lines(Some(f64::from(
                app_config.scrollback_lines,
            ))) as usize,
            ..Config::default()
        }),
    );
    let lifecycle = state::WindowLifecycle::start(&services.shared.runtime);
    let theme = background::theme(lifecycle.initial.background);
    let background = background::content(lifecycle.initial.background);
    let app_id = services.shared.identity.identifier.clone();
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let locale = cx.new(|_| Locale::new(language));
        LocaleGlobal::set(cx, locale.clone());
        tterm_terminal_view::view::bind_keys(cx);
        lifecycle.install_quit_flush(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let startup = PrototypeStartup {
            command,
            sessions,
            events,
            keyboard,
            keymap,
            locale,
            config: app_config,
            theme,
            background,
        };
        let opened = match startup::open(
            cx,
            lifecycle.initial,
            Arc::clone(&lifecycle.geometry),
            lifecycle.queue(),
            &app_id,
            |cx, geometry| cx.new(|cx| Prototype::new(cx, startup, geometry)),
        ) {
            Ok(opened) => opened,
            Err(error) => {
                eprintln!("tterm-gpui: {error}");
                cx.quit();
                return;
            }
        };
        let handle = opened.handle;
        let prototype = opened.root;
        let initial_title = prototype.read(cx).default_window_title(cx);
        if let Err(error) = handle.update(cx, |_, window, cx| {
            window.set_window_title(&initial_title);
            let view = prototype.read(cx).view.clone();
            view.update(cx, |view, cx| view.activate(window, cx));
            window.activate_window();
        }) {
            eprintln!("tterm-gpui: {error}");
            cx.quit();
        } else {
            #[cfg(all(feature = "automation", debug_assertions))]
            super::automation::start(handle, prototype, cx);
        }
    });
}

/// The window's root view: the attached terminal view.
pub(super) struct Prototype {
    // The manager closes every session on drop, so the root owns it until shutdown.
    _sessions: Option<Rc<SessionManager>>,
    view: Entity<TerminalView>,
    binding: Option<Rc<TabBinding>>,
    title: Arc<Mutex<Option<String>>>,
    current_title: String,
    title_is_default: bool,
    locale: Entity<Locale>,
    paste_controller: Option<Entity<PasteController>>,
    geometry: Entity<WindowGeometry>,
    theme: Theme,
    background: Rgba,
}

impl Prototype {
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn automation_state(&self) -> String {
        format!("title={}", self.current_title)
    }

    /// The automation endpoint's close path. It waits for the final
    /// background write like the caption and Linux buttons do.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let geometry = self.geometry.clone();
        let _ = geometry.update(cx, |geometry, cx| geometry.request_close(window, cx));
    }

    fn new(
        cx: &mut Context<Self>,
        startup: PrototypeStartup,
        geometry: Entity<WindowGeometry>,
    ) -> Self {
        let PrototypeStartup {
            command,
            sessions,
            mut events,
            keyboard,
            keymap,
            locale,
            config,
            theme,
            background,
        } = startup;
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        cx.observe(&geometry, |_, _, cx| cx.notify()).detach();
        let connection = command.connection_options();
        let drop_text = connection.as_ref().ok().and_then(local_drop_text);
        let (binding, open_error) = match connection {
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
            return failed(cx, binding, error, locale, geometry, theme, background);
        };
        log_conpty_source();
        let model = attachment.model();
        let title = attachment.title();
        let session_nonce = attachment.session_nonce();
        let binding = binding.or_else(|| sessions.tab(TAB_ID));
        let Some(binding) = binding else {
            return failed(
                cx,
                None,
                "The terminal tab was not registered".to_string(),
                locale,
                geometry,
                theme,
                background,
            );
        };
        let keymap_route = Rc::clone(&keymap);
        let paste_controller = cx.new(|cx| {
            PasteController::new(
                cx,
                locale.clone(),
                &config,
                Rc::clone(&binding),
                session_nonce,
            )
        });
        let paste_gate = super::clipboard::view_gate(&paste_controller, &binding, session_nonce);
        let config = TerminalViewConfig {
            input: Some(Rc::new(attachment.input())),
            resize: Some(Rc::new(attachment.resizer())),
            keyboard,
            keymap: Some(Rc::new(move |event| route_keymap(&keymap_route, event))),
            paste_gate: Some(paste_gate),
            copy_on_select: config.copy_on_select,
            right_click_paste: config.right_click_paste,
            drop_text,
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
            _sessions: Some(sessions),
            view,
            binding: Some(binding),
            title,
            current_title: DEFAULT_TITLE.to_string(),
            title_is_default: true,
            locale,
            paste_controller: Some(paste_controller),
            geometry,
            theme,
            background,
        }
    }

    fn default_window_title(&self, cx: &App) -> String {
        let locale = self.locale.read(cx);
        let title = match locale.text(keys::APP_TITLE) {
            Ok(title) => title,
            Err(error) => {
                eprintln!("tterm-gpui: failed to translate the window title: {error}");
                return DEFAULT_TITLE.to_string();
            }
        };
        let subtitle = match locale.text(keys::APP_SUBTITLE) {
            Ok(subtitle) => subtitle,
            Err(error) => {
                eprintln!("tterm-gpui: failed to translate the window subtitle: {error}");
                return title;
            }
        };
        format!("{title} — {subtitle}")
    }

    /// Switches the shared locale for this session only. Persisting the
    /// choice stays with the configuration settings (P3-02).
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.locale
            .update(cx, |locale, cx| locale.set_language(language, cx));
        cx.notify();
    }

    fn apply_window_title(&mut self, window: &mut Window, title: String) {
        if self.current_title != title {
            window.set_window_title(&title);
            self.current_title = title;
        }
    }
}

fn failed(
    cx: &mut Context<Prototype>,
    binding: Option<Rc<TabBinding>>,
    error: String,
    locale: Entity<Locale>,
    geometry: Entity<WindowGeometry>,
    theme: Theme,
    background: Rgba,
) -> Prototype {
    eprintln!("tterm-gpui: {error}");
    let terminal = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
    terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
    let view = cx.new(|cx| TerminalView::new(cx, terminal, Default::default()));
    Prototype {
        _sessions: None,
        view,
        binding,
        title: Arc::new(Mutex::new(None)),
        current_title: DEFAULT_TITLE.to_string(),
        title_is_default: true,
        locale,
        paste_controller: None,
        geometry,
        theme,
        background,
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

/// A pending title update: either the parser's default marker, which resolves
/// in the current language, or a program-supplied title kept verbatim.
#[derive(Debug, PartialEq, Eq)]
enum TitleUpdate {
    Default,
    Program(String),
}

fn classify_title_update(pending: Option<String>) -> Option<TitleUpdate> {
    match pending {
        Some(title) if title == DEFAULT_TITLE => Some(TitleUpdate::Default),
        Some(title) => Some(TitleUpdate::Program(title)),
        None => None,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pending_title = self.title.lock().ok().and_then(|mut title| title.take());
        match classify_title_update(pending_title) {
            Some(TitleUpdate::Default) => {
                self.title_is_default = true;
                let title = self.default_window_title(cx);
                self.apply_window_title(window, title);
            }
            Some(TitleUpdate::Program(title)) => {
                self.title_is_default = false;
                self.apply_window_title(window, title);
            }
            None if self.title_is_default => {
                let title = self.default_window_title(cx);
                self.apply_window_title(window, title);
            }
            None => {}
        }
        let geometry = self.geometry.clone();
        let on_action: ActionHandler = Rc::new(move |action, window, cx| match action {
            TitleBarAction::Minimize => window.minimize_window(),
            TitleBarAction::ToggleMaximize => window.zoom_window(),
            TitleBarAction::Close => {
                geometry.update(cx, |geometry, cx| {
                    geometry.request_close(window, cx);
                });
            }
        });
        let titlebar = TitleBar::new(self.current_title.clone(), self.theme)
            .on_action(on_action)
            .render(window);
        div()
            .size_full()
            .relative()
            .child(shell::frame(titlebar, self.view.clone(), self.background))
            .children(self.paste_controller.clone())
    }
}

#[cfg(test)]
mod tests {
    use tterm_core::events::ssh::SshConnectionProgressPayload;

    use super::{
        ConnectionState, DEFAULT_TITLE, TitleUpdate, classify_title_update, connection_state,
    };

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

    #[test]
    fn default_title_marker_resolves_through_the_locale() {
        assert_eq!(
            classify_title_update(Some(DEFAULT_TITLE.to_string())),
            Some(TitleUpdate::Default)
        );
        assert_eq!(classify_title_update(None), None);
    }

    #[test]
    fn a_program_title_is_preserved_verbatim() {
        assert_eq!(
            classify_title_update(Some("cargo test".to_string())),
            Some(TitleUpdate::Program("cargo test".to_string()))
        );
    }
}
