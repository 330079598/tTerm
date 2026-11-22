//! The native workspace window and its independently owned terminal sessions.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

mod drag;
mod events;
mod history;
mod motion;
mod nudge;
mod overflow;
mod picker;
#[cfg(test)]
mod split_session_tests;
mod split_state;
mod split_ui;
mod strip;
mod tabs;
use events::handle_event;
use history::{history_capacity, subscribe_history};
use tabs::Tabs;

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, Rgba, Window, div};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::config::AppConfig;
use tterm_core::config::app_config_normalize::normalize_loaded_config;
use tterm_core::config::load_config_file;
use tterm_core::events::AppEvent;
use tterm_core::i18n::catalogue::Catalogue;
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{ClipboardPolicy, Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::tokens::theme::Theme;

use super::binding::{SessionManager, TabBinding};
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
use crate::config_state::ConfigState;
use crate::locale::{Locale, LocaleGlobal};
use crate::scrollback_setting::ScrollbackSetting;

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
    config: Entity<ConfigState>,
    theme: Theme,
    background: Rgba,
}

/// Loads settings with the same sequence the Tauri client uses: the strict
/// file reader, then the shared UI normalization before any view reads them.
fn load_startup_config() -> AppConfig {
    startup_config(load_config_file())
}

/// Normalizes a loaded config, or starts from the normalized defaults when
/// the strict reader rejects the file. The Tauri frontend makes the same
/// fallback in `ConfigContext`.
fn startup_config(loaded: Result<AppConfig, String>) -> AppConfig {
    let normalized = loaded.and_then(|config| {
        normalize_loaded_config(&config)
            .map_err(|error| format!("Failed to normalize config: {error}"))
    });
    normalized.unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to load config, using defaults: {error}");
        default_startup_config()
    })
}

fn default_startup_config() -> AppConfig {
    match normalize_loaded_config(&AppConfig::default()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("tterm-gpui: failed to normalize the default config: {error}");
            AppConfig::default()
        }
    }
}

/// Runs the application with one prototype window until it closes.
pub(super) fn run_window(
    command: CommandSpec,
    services: Arc<AppServices>,
    events: UnboundedReceiver<AppEvent>,
) {
    prepare_conpty();
    let app_config = load_startup_config();
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
            scrollback: history_capacity(app_config.scrollback_lines),
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
        let config = cx.new(|_| ConfigState::new(app_config, &services.shared.runtime));
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
            config,
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
            let view = prototype.read(cx).tabs.active().view.clone();
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

/// The root owns the session manager even when the initial tab fails to open.
pub(super) struct Prototype {
    nudge: Option<(usize, std::time::Instant)>,
    splits: split_state::SplitState,
    drag_layout: drag::DragLayout,
    picker: Option<picker::TabPicker>,
    overflow: overflow::TabViewport,
    tabs: Tabs<WindowTab>,
    factory: TabFactory,
    current_title: String,
    ui: WindowUi,
}

struct TabFactory {
    sessions: Rc<SessionManager>,
    config: Entity<ConfigState>,
    keyboard: tterm_terminal_view::input::keyboard::EncoderConfig,
    keymap: Rc<KeymapRouter>,
}

struct WindowTab {
    default_shell: bool,
    search: picker::ConnectionLabel,
    fallback_key: tterm_core::i18n::translate::TranslationKey,
    view: Entity<TerminalView>,
    binding: Option<Rc<TabBinding>>,
    title: Arc<Mutex<Option<String>>>,
    program_title: RefCell<Option<String>>,
    paste_controller: Option<Entity<PasteController>>,
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
        let geometry = self.ui.geometry.clone();
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
        let scrollback_setting =
            cx.new(|cx| ScrollbackSetting::new(config.clone(), locale.clone(), theme, cx));
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        cx.observe(&geometry, |_, _, cx| cx.notify()).detach();
        subscribe_history(cx, &config, &sessions).detach();
        let ui = WindowUi {
            locale: locale.clone(),
            geometry,
            theme,
            background,
            scrollback_setting,
        };
        let factory = TabFactory {
            sessions,
            config,
            keyboard,
            keymap,
        };
        strip::register_actions(&factory.keymap);
        let tab = factory.open(1, command, &locale, cx);
        cx.spawn(async move |prototype, cx| {
            while let Some(event) = events.recv().await {
                if prototype
                    .update(cx, |prototype, cx| {
                        let tterm_core::events::EventName::TabScoped { tab_id, .. } = event.name()
                        else {
                            return;
                        };
                        let Some(binding) = prototype.factory.sessions.tab(&tab_id) else {
                            return;
                        };
                        if handle_event(&binding, event) && binding.is_active() {
                            if let Some(tab) = prototype
                                .visible_ids()
                                .into_iter()
                                .flatten()
                                .filter_map(|id| prototype.tabs.get(id))
                                .find(|tab| {
                                    tab.binding
                                        .as_ref()
                                        .is_some_and(|other| Rc::ptr_eq(other, &binding))
                                })
                            {
                                tab.view.update(cx, |view, cx| view.notify_output(cx));
                            }
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            tabs: Tabs::new(tab),
            splits: Default::default(),
            picker: None,
            overflow: Default::default(),
            drag_layout: Default::default(),
            nudge: None,
            factory,
            current_title: DEFAULT_TITLE.to_string(),
            ui,
        }
    }
}

impl TabFactory {
    fn open(
        &self,
        id: usize,
        command: CommandSpec,
        locale: &Entity<Locale>,
        cx: &mut Context<Prototype>,
    ) -> WindowTab {
        let fallback_key = match &command {
            CommandSpec::SshProfile { .. } => keys::TAB_SSH,
            _ => keys::TAB_TERMINAL,
        };
        let connection = command.connection_options();
        let search = picker::ConnectionLabel::new(&command, connection.as_ref().ok());
        let drop_text = connection.as_ref().ok().and_then(local_drop_text);
        let (binding, open_error) = match connection {
            Ok(connection) => {
                let binding = self.sessions.open(
                    format!("gpui-{id}"),
                    connection,
                    INITIAL_GRID,
                    INITIAL_CELL,
                );
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
            return failed(cx, binding, fallback_key, search, error);
        };
        log_conpty_source();
        let model = attachment.model();
        let title = attachment.title();
        let session_nonce = attachment.session_nonce();
        let Some(binding) = binding else {
            return failed(
                cx,
                None,
                fallback_key,
                search,
                "The terminal tab was not registered".to_string(),
            );
        };
        let keymap_route = Rc::clone(&self.keymap);
        let initial = self.config.read(cx).config().clone();
        let paste_controller = cx.new(|cx| {
            PasteController::new(
                cx,
                locale.clone(),
                &initial,
                Rc::clone(&binding),
                session_nonce,
            )
        });
        let paste_gate = super::clipboard::view_gate(&paste_controller, &binding, session_nonce);
        let config = TerminalViewConfig {
            input: Some(Rc::new(attachment.input())),
            resize: Some(Rc::new(attachment.resizer())),
            keyboard: self.keyboard,
            keymap: Some(Rc::new(move |event| route_keymap(&keymap_route, event))),
            paste_gate: Some(paste_gate),
            copy_on_select: initial.copy_on_select,
            right_click_paste: initial.right_click_paste,
            drop_text,
            ..TerminalViewConfig::default()
        };
        let view = cx.new(|cx| TerminalView::new(cx, model, config));

        let mut wake = attachment.subscribe();
        let wake_title = Arc::clone(&title);
        cx.spawn(async move |prototype, cx| {
            while wake.changed().await {
                wake.acknowledge();
                let updated = prototype.update(cx, |prototype, cx| {
                    if prototype.visible_ids().contains(&Some(id))
                        && let Some(tab) = prototype.tabs.get(id)
                    {
                        tab.view.update(cx, |view, cx| view.notify_output(cx));
                    }
                    if wake_title.lock().is_ok_and(|title| title.is_some()) {
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();

        WindowTab {
            default_shell: matches!(command, CommandSpec::Shell),
            search,
            fallback_key,
            view,
            binding: Some(binding),
            title,
            program_title: RefCell::default(),
            paste_controller: Some(paste_controller),
        }
    }
}

impl Prototype {
    fn default_window_title(&self, cx: &App) -> String {
        let locale = self.ui.locale.read(cx);
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
        self.ui
            .locale
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

struct WindowUi {
    locale: Entity<Locale>,
    geometry: Entity<WindowGeometry>,
    theme: Theme,
    background: Rgba,
    scrollback_setting: Entity<ScrollbackSetting>,
}

fn failed(
    cx: &mut Context<Prototype>,
    binding: Option<Rc<TabBinding>>,
    fallback_key: tterm_core::i18n::translate::TranslationKey,
    search: picker::ConnectionLabel,
    error: String,
) -> WindowTab {
    eprintln!("tterm-gpui: {error}");
    let terminal = Arc::new(Terminal::new(Config::default(), INITIAL_GRID, INITIAL_CELL));
    terminal.write(format!("\x1b[31m[{error}]\x1b[0m\r\n").as_bytes());
    let view = cx.new(|cx| TerminalView::new(cx, terminal, Default::default()));
    WindowTab {
        default_shell: false,
        search,
        fallback_key,
        view,
        binding,
        title: Arc::new(Mutex::new(None)),
        program_title: RefCell::default(),
        paste_controller: None,
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

impl Render for Prototype {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let strip = self.tab_strip(window, cx);
        let title = self
            .tabs
            .active()
            .program_title
            .borrow()
            .clone()
            .unwrap_or_else(|| self.default_window_title(cx));
        self.apply_window_title(window, title);
        let geometry = self.ui.geometry.clone();
        let on_action: ActionHandler = Rc::new(move |action, window, cx| match action {
            TitleBarAction::Minimize => window.minimize_window(),
            TitleBarAction::ToggleMaximize => window.zoom_window(),
            TitleBarAction::Close => {
                geometry.update(cx, |geometry, cx| {
                    geometry.request_close(window, cx);
                });
            }
        });
        let settings_button = self
            .ui
            .scrollback_setting
            .update(cx, |setting, cx| setting.button(cx));
        let titlebar = TitleBar::new(self.current_title.clone(), self.ui.theme)
            .slot(strip)
            .tools(settings_button)
            .on_action(on_action)
            .render(window);
        div()
            .capture_key_down(cx.listener(Self::workspace_key))
            .size_full()
            .relative()
            .child(shell::frame(
                titlebar,
                self.workspace_panes(window, cx),
                self.ui.background,
            ))
            .children(self.tabs.active().paste_controller.clone())
            .child(self.ui.scrollback_setting.clone())
            .children(self.picker_panel(window, cx))
    }
}

#[cfg(test)]
mod tests {
    use tterm_core::config::AppConfig;
    use tterm_core::config::limits::DEFAULT_HIDDEN_RENDERER_RELEASE_SECS;
    use tterm_core::events::ssh::SshConnectionProgressPayload;

    use super::events::connection_state;
    use super::{DEFAULT_TITLE, TitleUpdate, classify_title_update, startup_config};
    use crate::terminal_demo::binding::scoped::ConnectionState;

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

    #[test]
    fn startup_config_normalizes_before_views_read_it() {
        let loaded = AppConfig {
            ui_style: "floating".to_string(),
            close_behavior: "close".to_string(),
            hidden_renderer_release_secs: 7,
            ..AppConfig::default()
        };

        let config = startup_config(Ok(loaded));

        assert_eq!(config.ui_style, "classic");
        assert_eq!(config.close_behavior, "ask");
        assert_eq!(
            config.hidden_renderer_release_secs,
            DEFAULT_HIDDEN_RENDERER_RELEASE_SECS
        );
    }

    #[test]
    fn startup_config_uses_normalized_defaults_when_reading_fails() {
        let config = startup_config(Err("damaged config".to_string()));

        assert_eq!(config.ui_style, "islands");
        assert_eq!(config.secret_storage_mode, "system");
    }
}
