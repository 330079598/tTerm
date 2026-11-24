//! The native workspace window and its independently owned terminal sessions.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

mod agent_resume;
mod agent_resume_notice;
use agent_resume::controller::{AttachOptions, Controller};
mod drag;
mod events;
mod failed_tab;
mod history;
mod motion;
mod nudge;
mod overflow;
mod picker;
mod profile_launch;
mod saved_connections;
#[cfg(test)]
mod saved_connections_tests;
pub(super) mod search;
#[cfg(test)]
mod search_workspace_tests;
mod session_workspace;
mod settings_page;
#[cfg(all(debug_assertions, feature = "automation"))]
pub(super) mod settings_page_capture;
#[cfg(test)]
mod settings_page_tests;
#[cfg(test)]
mod split_session_tests;
mod split_state;
mod split_ui;
mod strip;
mod tabs;
mod template_actions;
mod template_ui;
mod tunnel_page;
#[cfg(test)]
mod tunnel_page_tests;
use events::handle_event;
use failed_tab::{FailedTab, failed, saved_agent_resume};
use history::{history_capacity, subscribe_history};
use tabs::Tabs;

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, Rgba, Task, Window, div, px};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::agent_status::AgentResumeSession;
use tterm_core::config::AppConfig;
use tterm_core::config::app_config_normalize::normalize_loaded_config;
use tterm_core::config::load_config_file;
use tterm_core::events::AppEvent;
use tterm_core::i18n::catalogue::Catalogue;
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab::Tab;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
#[cfg(test)]
use tterm_terminal::terminal::Terminal;
use tterm_terminal::terminal::{ClipboardPolicy, Config};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::tokens::theme::Theme;

use super::binding::{SessionManager, TabBinding};
use super::clipboard::{PasteController, local_drop_text};
use super::command::{CommandSpec, TabLaunch};
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
use saved_connections::SavedConnectionsUi;
use settings_page::SettingsPage;
use template_ui::TemplateUi;
use tunnel_page::TunnelPage;

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
    workspace: session_workspace::StartupWorkspace,
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
    let workspace =
        session_workspace::startup(&services.shared.runtime, command.uses_saved_workspace());
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
        session_workspace::install_quit_flush(workspace.persistence.clone(), cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let startup = PrototypeStartup {
            command,
            workspace,
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
        let close_root = prototype.clone();
        if let Err(error) = handle.update(cx, |_, window, cx| {
            window.set_window_title(&initial_title);
            prototype.update(cx, |prototype, cx| {
                prototype.initialize_workspace(window, cx);
            });
            window.on_window_should_close(cx, move |window, cx| {
                close_root.update(cx, |prototype, cx| prototype.begin_close(window, cx))
            });
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
    workspace: session_workspace::save::WorkspacePersistence,
    workspace_timer: Option<Task<()>>,
    next_saved_tab: u64,
    closing: bool,
    settings_open: bool,
    pending_profile_opens: std::collections::HashSet<String>,
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
    saved_id: String,
    saved: Tab,
    restored: bool,
    default_shell: bool,
    may_resume_agent: bool,
    agent_resume: Option<AgentResumeSession>,
    agent_resume_controller: Option<Controller>,
    search: picker::ConnectionLabel,
    fallback_key: tterm_core::i18n::translate::TranslationKey,
    view: Entity<TerminalView>,
    search_bar: Option<Entity<search::SearchBar>>,
    binding: Option<Rc<TabBinding>>,
    title: Arc<Mutex<Option<String>>>,
    program_title: RefCell<Option<String>>,
    paste_controller: Option<Entity<PasteController>>,
}

impl WindowTab {
    pub(super) fn saved_id(&self) -> &str {
        &self.saved_id
    }

    pub(super) fn saved(&self) -> &Tab {
        &self.saved
    }

    pub(super) fn agent_resume(&self) -> Option<&AgentResumeSession> {
        self.agent_resume.as_ref()
    }

    pub(super) fn set_agent_resume(&mut self, resume: Option<AgentResumeSession>) {
        self.agent_resume = resume;
    }

    pub(super) fn may_resume_agent(&self) -> bool {
        self.may_resume_agent
    }
}

impl Prototype {
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn automation_state(&self) -> String {
        format!("title={}", self.current_title)
    }

    /// The automation endpoint's close path, using the same gate as the
    /// platform close button.
    pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_close(window, cx);
    }

    fn new(
        cx: &mut Context<Self>,
        startup: PrototypeStartup,
        geometry: Entity<WindowGeometry>,
    ) -> Self {
        let PrototypeStartup {
            command,
            workspace,
            sessions,
            mut events,
            keyboard,
            keymap,
            locale,
            config,
            theme,
            background,
        } = startup;
        let setting =
            cx.new(|cx| ScrollbackSetting::new(config.clone(), locale.clone(), theme, cx));
        let page = cx.new(|cx| SettingsPage::new(locale.clone(), theme, setting.clone(), cx));
        cx.observe(&page, |_, _, cx| cx.notify()).detach();
        let template_ui = TemplateUi::for_workspace(config.clone(), locale.clone(), theme, cx);
        let saved_connections =
            SavedConnectionsUi::for_workspace(sessions.services(), locale.clone(), theme, cx);
        let tunnel_page = TunnelPage::new(sessions.services(), locale.clone(), theme, cx);
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        cx.observe(&geometry, |_, _, cx| cx.notify()).detach();
        subscribe_history(cx, &config, &sessions).detach();
        let ui = WindowUi {
            locale: locale.clone(),
            geometry,
            theme,
            background,
            settings_page: page,
            template_ui,
            saved_connections,
            tunnel_page,
        };
        let factory = TabFactory {
            sessions,
            config,
            keyboard,
            keymap,
        };
        strip::register_actions(&factory.keymap);
        template_ui::register_actions(&factory.keymap);
        let restored = workspace
            .restore
            .then(|| session_workspace::snapshot(workspace.load.clone()))
            .flatten()
            .and_then(|snapshot| {
                session_workspace::restore::restore_workspace(&factory, snapshot, &locale, cx)
                    .map(|restored| (restored.tabs, restored.splits, restored.next_saved_tab))
            });
        let (tabs, splits, next_saved_tab) = restored.unwrap_or_else(|| {
            let tab = factory.open(1, command, &locale, cx);
            (Tabs::new(tab), split_state::SplitState::new(1), 2)
        });
        let prototype = Self {
            tabs,
            splits,
            workspace: workspace.persistence,
            workspace_timer: None,
            next_saved_tab,
            closing: false,
            settings_open: false,
            pending_profile_opens: Default::default(),
            picker: None,
            overflow: Default::default(),
            drag_layout: Default::default(),
            nudge: None,
            factory,
            current_title: DEFAULT_TITLE.to_string(),
            ui,
        };
        prototype.publish_workspace(cx);
        cx.spawn(async move |prototype, cx| {
            while let Some(event) = events.recv().await {
                if prototype
                    .update(cx, |prototype, cx| {
                        if prototype.ui.tunnel_page.handle_event(&event, cx) {
                            return;
                        }
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
        prototype
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
        self.open_launch(
            id,
            TabLaunch::command(format!("tab-{id}"), command),
            locale,
            cx,
        )
    }

    fn open_launch(
        &self,
        id: usize,
        launch: TabLaunch,
        locale: &Entity<Locale>,
        cx: &mut Context<Prototype>,
    ) -> WindowTab {
        let fallback_key = if launch.is_ssh() {
            keys::TAB_SSH
        } else {
            keys::TAB_TERMINAL
        };
        let connection = launch.connection_options();
        let search =
            picker::ConnectionLabel::new_parts(launch.search_kind(), connection.as_ref().ok());
        let saved = launch.saved().clone();
        let saved_id = launch.saved_id().to_string();
        let restored = launch.is_restored();
        let default_shell = launch.is_default_shell();
        let may_resume_agent = launch.may_resume_agent();
        let is_ssh = launch.is_ssh();
        let local_shell = launch.local_shell().map(str::to_owned);
        let local_custom_path = launch.local_custom_path().map(str::to_owned);
        let agent_resume = saved_agent_resume(&saved);
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
        let attachment = binding.as_ref().and_then(|binding| binding.attach());
        let Some(attachment) = attachment else {
            let error = open_error.unwrap_or_else(|| "Failed to start terminal".to_string());
            return failed(
                cx,
                FailedTab {
                    saved_id,
                    saved,
                    restored,
                    default_shell,
                    may_resume_agent,
                    agent_resume,
                    binding,
                    fallback_key,
                    search,
                },
                error,
            );
        };
        log_conpty_source();
        let model = attachment.model();
        let title = attachment.title();
        let session_nonce = attachment.session_nonce();
        let Some(binding) = binding else {
            return failed(
                cx,
                FailedTab {
                    saved_id,
                    saved,
                    restored,
                    default_shell,
                    may_resume_agent,
                    agent_resume,
                    binding: None,
                    fallback_key,
                    search,
                },
                "The terminal tab was not registered".to_string(),
            );
        };
        let agent_resume_controller = binding.session().and_then(|session| {
            Controller::attach(
                cx.weak_entity(),
                AttachOptions {
                    tab: id,
                    binding: Rc::clone(&binding),
                    session,
                    restored: agent_resume.clone(),
                    may_resume: may_resume_agent,
                    is_ssh,
                    local_shell,
                    local_custom_path,
                    config: self.config.clone(),
                },
                cx,
            )
        });
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
                    if prototype.visible_ids().contains(&id)
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
            saved_id,
            saved,
            restored,
            default_shell,
            may_resume_agent,
            agent_resume,
            agent_resume_controller,
            search,
            fallback_key,
            view,
            search_bar: None,
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
    settings_page: Entity<SettingsPage>,
    template_ui: Entity<TemplateUi>,
    saved_connections: Entity<SavedConnectionsUi>,
    tunnel_page: TunnelPage,
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
        self.sync_layout_template_target(cx);
        let settings_open = self.settings_is_open();
        let tunnel_page_open = self.tunnel_page_is_open();
        let page_open = settings_open || tunnel_page_open;
        let strip = if settings_open {
            div()
        } else {
            self.tab_strip(window, cx)
        };
        let title = self
            .tabs
            .active()
            .program_title
            .borrow()
            .clone()
            .unwrap_or_else(|| self.default_window_title(cx));
        self.apply_window_title(window, title);
        let close_root = cx.entity().downgrade();
        let on_action: ActionHandler = Rc::new(move |action, window, cx| match action {
            TitleBarAction::Minimize => window.minimize_window(),
            TitleBarAction::ToggleMaximize => window.zoom_window(),
            TitleBarAction::Close => {
                let _ = close_root.update(cx, |prototype, cx| {
                    prototype.request_close(window, cx);
                });
            }
        });
        let settings_button = self.settings_button(cx);
        let tunnels_button = self.tunnel_page_button(cx);
        let tools = if settings_open {
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(tunnels_button)
                .child(settings_button)
        } else {
            let layout_button = self.layout_template_button(cx);
            let saved_connections_button = self.saved_connections_button(cx);
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(tunnels_button)
                .child(layout_button)
                .child(saved_connections_button)
                .child(settings_button)
        };
        let titlebar = TitleBar::new(self.current_title.clone(), self.ui.theme)
            .slot(strip)
            .tools(tools)
            .on_action(on_action)
            .render(window);
        let content = if self.settings_is_open() {
            self.settings_page(window, cx)
        } else if tunnel_page_open {
            self.tunnel_page(cx)
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .min_h(px(0.0))
                .children(self.agent_resume_notice(cx))
                .child(self.workspace_panes(window, cx))
                .into_any_element()
        };
        div()
            .capture_key_down(cx.listener(Self::workspace_key))
            .size_full()
            .relative()
            .child(shell::frame(titlebar, content, self.ui.background))
            .children(
                (!page_open)
                    .then(|| self.tabs.active().paste_controller.clone())
                    .flatten(),
            )
            .children(
                (!page_open)
                    .then(|| self.picker_panel(window, cx))
                    .flatten(),
            )
            .children((!page_open).then(|| self.ui.template_ui.clone()))
            .children((!page_open).then(|| self.ui.saved_connections.clone()))
    }
}

#[cfg(test)]
mod tests;
