//! The native workspace window and its independently owned terminal sessions.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

mod agent_resume;
mod agent_resume_notice;
use agent_resume::controller::{AttachOptions, Controller};
mod context_menu;
mod drag;
mod events;
mod failed_tab;
mod history;
mod launch;
mod live_theme;
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
mod shell_launcher;
#[cfg(test)]
mod split_session_tests;
mod split_state;
mod split_ui;
pub(super) mod ssh_tui_field;
mod strip;
mod tabs;
mod template_actions;
mod template_ui;
mod terminal_font;
#[cfg(test)]
mod terminal_font_tests;
mod title;
mod tunnel_page;
#[cfg(test)]
mod tunnel_page_tests;
use events::handle_event;
use failed_tab::{FailedTab, failed, saved_agent_resume};
use history::{history_capacity, subscribe_history};
use live_theme::{Subscriptions, is_dark_appearance};
use tabs::Tabs;

use gpui_kit::prelude::*;
use gpui_kit::{App, Entity, Rgba, Task, Window, div, px};
use tokio::sync::mpsc::UnboundedReceiver;
use tterm_core::agent_status::AgentResumeSession;
#[cfg(test)]
use tterm_core::config::AppConfig;
use tterm_core::events::AppEvent;
use tterm_core::i18n::catalogue::Catalogue;
use tterm_core::i18n::keys;
use tterm_core::i18n::language::Language;
use tterm_core::services::AppServices;
use tterm_core::workspace::tab::Tab;
use tterm_core::workspace::tab_attention::TabAttentionStore;
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::palette::TerminalPalette;
use tterm_terminal::snapshot::CursorShape;
use tterm_terminal::terminal::Config;
#[cfg(test)]
use tterm_terminal::terminal::Terminal;
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig};
use tterm_ui::tokens::theme::Theme;

use self::context_menu::{PaneContextMenu, view_handler};
use super::binding::{SessionManager, TabBinding};
use super::clipboard::{PasteController, local_drop_text};
use super::command::{CommandSpec, TabLaunch};
use super::conpty::{log_conpty_source, prepare_conpty};
use super::intents::DEFAULT_TITLE;
use super::keymap::{KeymapRouter, encoder_config, route_keymap};
use crate::app_window::chrome::shell;
use crate::app_window::chrome::titlebar::TitleBar;
use crate::app_window::chrome::{ActionHandler, TitleBarAction};
use crate::app_window::startup;
use crate::app_window::state;
use crate::app_window::state::WindowGeometry;
use crate::app_window::theme::{self, ResolvedTheme};
use crate::config_state::ConfigState;
use crate::local_shells_setting::LocalShellsSetting;
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

#[cfg(test)]
use launch::startup_config;
use launch::{PrototypeStartup, load_startup_config};

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
    let clipboard_policy = super::session::clipboard_write_policy(app_config.osc52_clipboard_write);
    let sessions = Rc::new(
        SessionManager::new(Arc::clone(&services), attention).with_terminal_config(Config {
            clipboard: clipboard_policy,
            scrollback: history_capacity(app_config.scrollback_lines),
            cursor_shape: CursorShape::from_config(&app_config.cursor_style),
            command_marks: app_config.command_marks,
            ..Config::default()
        }),
    );
    let lifecycle = state::WindowLifecycle::start(&services.shared.runtime);
    let workspace = session_workspace::startup(
        &services.shared.runtime,
        command.uses_saved_workspace(),
        app_config.restore_terminal_content,
    );
    let theme_config = app_config.clone();
    let app_id = services.shared.identity.identifier.clone();
    gpui_kit::application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
        let locale = cx.new(|_| Locale::new(language));
        let config = cx.new(|_| ConfigState::new(app_config, &services.shared.runtime));
        config.update(cx, |state, cx| state.discover_shells(cx));
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
        let opened = match startup::open(
            cx,
            lifecycle.initial,
            Arc::clone(&lifecycle.geometry),
            lifecycle.queue(),
            &app_id,
            move |cx, geometry, window| {
                let startup = PrototypeStartup {
                    command,
                    workspace,
                    sessions,
                    events,
                    keyboard,
                    keymap,
                    locale,
                    config,
                    theme: theme::resolve(&theme_config, is_dark_appearance(window.appearance())),
                };
                cx.new(|cx| Prototype::new(cx, startup, geometry))
            },
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
            if super::field::is_armed() {
                super::field::after_window(handle, prototype, cx);
            } else {
                super::automation::start(handle, prototype, cx);
            }
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
    system_dark: bool,
    theme_subscriptions: Subscriptions,
    ui: WindowUi,
}

struct TabFactory {
    sessions: Rc<SessionManager>,
    config: Entity<ConfigState>,
    keyboard: tterm_terminal_view::input::keyboard::EncoderConfig,
    keymap: Rc<KeymapRouter>,
    theme: ResolvedTheme,
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
    context_menu: Option<Entity<PaneContextMenu>>,
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
    pub(super) fn automation_state(&self, cx: &App) -> String {
        format!("title={}", self.logical_window_title(cx))
    }

    /// Opens the production saved-connections route for the capture.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn capture_open_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_saved_connections(window, cx);
    }

    /// Opens the production settings page for the capture.
    ///
    /// This uses the page's own open path, so the capture does not depend on
    /// the route's private helper while the route is being split out.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn capture_open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui
            .settings_page
            .update(cx, |page, cx| page.open(window, cx));
        self.settings_open = true;
        cx.notify();
    }

    /// The state the visual-acceptance capture polls between actions.
    ///
    /// It is compiled only into the debug automation build and only reads
    /// booleans and counts, so the capture can wait for the production views
    /// instead of sleeping for a fixed time.
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn capture_state(&self, cx: &App) -> String {
        let (profiles_open, profiles_loading, profiles_count) =
            self.ui.saved_connections.read(cx).capture_state(cx);
        format!(
            "settings_open={}\nsettings_fonts_loaded={}\nprofiles_open={profiles_open}\nprofiles_loading={profiles_loading}\nprofiles_count={profiles_count}\n",
            self.settings_open,
            self.ui.settings_page.read(cx).capture_fonts_loaded(cx)
        )
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
            theme: app_theme,
        } = startup;
        let theme = app_theme.ui;
        let session_workspace::StartupWorkspace {
            load,
            persistence,
            restore,
            legacy,
        } = workspace;
        let setting =
            cx.new(|cx| ScrollbackSetting::new(config.clone(), locale.clone(), theme, cx));
        let local_shells =
            cx.new(|cx| LocalShellsSetting::new(config.clone(), locale.clone(), theme, cx));
        let page = cx
            .new(|cx| SettingsPage::new(locale.clone(), theme, setting.clone(), local_shells, cx));
        let theme_subscriptions = Subscriptions::install(cx, &page, &config);
        cx.observe(&page, |_, _, cx| cx.notify()).detach();
        let template_ui = TemplateUi::for_workspace(config.clone(), locale.clone(), theme, cx);
        let saved_connections =
            SavedConnectionsUi::for_workspace(sessions.services(), locale.clone(), theme, cx);
        let tunnel_page = TunnelPage::new(sessions.services(), locale.clone(), theme, cx);
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        cx.observe(&geometry, |_, _, cx| cx.notify()).detach();
        subscribe_history(cx, &config, &sessions).detach();
        let ui = WindowUi::new(
            locale.clone(),
            geometry,
            &app_theme,
            page,
            template_ui,
            saved_connections,
            tunnel_page,
        );
        let services = sessions.services();
        let factory = TabFactory {
            sessions,
            config,
            keyboard,
            keymap,
            theme: app_theme,
        };
        strip::register_actions(&factory.keymap);
        template_ui::register_actions(&factory.keymap);
        let restored = restore
            .then(|| session_workspace::snapshot(load))
            .flatten()
            .and_then(|snapshot| {
                session_workspace::restore::restore_workspace(
                    &factory, snapshot, legacy, &locale, cx,
                )
                .map(|restored| (restored.tabs, restored.splits, restored.next_saved_tab))
            });
        let (tabs, splits, next_saved_tab) = restored.unwrap_or_else(|| {
            let tab = factory.open(1, command, &locale, cx);
            (Tabs::new(tab), split_state::SplitState::new(1), 2)
        });
        let prototype = Self {
            tabs,
            splits,
            workspace: persistence,
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
            system_dark: is_dark_appearance(cx.window_appearance()),
            theme_subscriptions,
            ui,
        };
        prototype.publish_workspace(cx);
        prototype.subscribe_shared_font(cx).detach();
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
                        if handle_event(&binding, event, &services) && binding.is_active() {
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
    fn open_binding(
        &self,
        id: usize,
        connection: Result<tterm_core::core::session::types::PtyConnectionOptions, String>,
        launch: &mut TabLaunch,
        palette: &TerminalPalette,
    ) -> (Option<Rc<TabBinding>>, Option<String>) {
        let saved_profile_id = connection
            .as_ref()
            .ok()
            .and_then(|connection| launch.saved_profile_id(connection));
        match connection {
            Ok(connection) => {
                let binding = self.sessions.open_with_history(
                    format!("gpui-{id}"),
                    connection,
                    INITIAL_GRID,
                    INITIAL_CELL,
                    palette.clone(),
                    launch.take_restored_history(),
                );
                binding.set_saved_profile_id(saved_profile_id);
                let error = binding.scoped_state().start_error().map(str::to_string);
                (Some(binding), error)
            }
            Err(error) => (None, Some(error)),
        }
    }

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
        mut launch: TabLaunch,
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
        let palette = self.theme.palette_for_tab(&saved);
        let restored = launch.is_restored();
        let default_shell = launch.is_default_shell();
        let may_resume_agent = launch.may_resume_agent();
        let is_ssh = launch.is_ssh();
        let local_shell = launch.local_shell().map(str::to_owned);
        let local_custom_path = launch.local_custom_path().map(str::to_owned);
        let agent_resume = saved_agent_resume(&saved);
        let drop_text = connection.as_ref().ok().and_then(local_drop_text);
        let (binding, open_error) = self.open_binding(id, connection, &mut launch, &palette);
        let font = self.shared_font(cx);
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
                font,
                palette.clone(),
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
                font,
                palette,
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
        let context_menu = cx.new(|_| PaneContextMenu::new(self.theme.ui));
        let config = TerminalViewConfig {
            font,
            palette,
            padding: self.shared_padding(cx),
            input: Some(Rc::new(attachment.input())),
            resize: Some(Rc::new(attachment.resizer())),
            keyboard: self.keyboard,
            keymap: Some(Rc::new(move |event| route_keymap(&keymap_route, event))),
            paste_gate: Some(paste_gate),
            copy_on_select: initial.copy_on_select,
            right_click_paste: initial.right_click_paste,
            context_menu: Some(view_handler(context_menu.clone())),
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
            context_menu: Some(context_menu),
        }
    }
}

impl Prototype {
    /// Switches the shared locale for this session only. Persisting the
    /// choice stays with the configuration settings (P3-02).
    #[cfg(all(feature = "automation", debug_assertions))]
    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.ui
            .locale
            .update(cx, |locale, cx| locale.set_language(language, cx));
        cx.notify();
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
        self.observe_theme(window, cx);
        self.sync_layout_template_target(cx);
        let settings_open = self.settings_is_open();
        let tunnel_page_open = self.tunnel_page_is_open();
        let page_open = settings_open || tunnel_page_open;
        let strip = if settings_open {
            div()
        } else {
            self.tab_strip(window, cx)
        };
        let title = self.logical_window_title(cx);
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
