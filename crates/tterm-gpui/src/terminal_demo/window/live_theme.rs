//! Live theme propagation from saved settings and the platform appearance.
//!
//! A theme reaches the window through two events: the appearance editor emits
//! a transient preview while the user browses, and the config emits an accepted
//! change after the file saved. Both paths resolve to one palette and repaint
//! every open surface in place, keeping each tab's view, session and draft.

use gpui_kit::{Context, Entity, Subscription, Window};

use super::saved_connections::SavedConnectionsUi;
use super::settings_page::SettingsPage;
use super::template_ui::TemplateUi;
use super::tunnel_page::TunnelPage;
use super::{Prototype, WindowUi};
use crate::app_window::state::WindowGeometry;
use crate::app_window::theme::{self, ResolvedTheme};
use crate::config_state::{ConfigState, ThemeChanged};
use crate::locale::Locale;
use crate::theme_setting::{ThemePreviewRequested, ThemeSetting};

/// The theme subscriptions that live as long as the window.
pub(super) struct Subscriptions {
    _preview: Subscription,
    _config: Subscription,
    appearance: Option<Subscription>,
}

impl Subscriptions {
    /// Subscribes to appearance previews and accepted config changes.
    pub(super) fn install(
        cx: &mut Context<Prototype>,
        page: &Entity<SettingsPage>,
        config: &Entity<ConfigState>,
    ) -> Self {
        let theme_setting = page.read(cx).theme_setting_entity();
        Self {
            _preview: subscribe_theme_preview(cx, &theme_setting),
            _config: subscribe_theme_changes(cx, config),
            appearance: None,
        }
    }

    /// Installs the platform appearance observer once.
    fn install_appearance(&mut self, window: &mut Window, cx: &mut Context<Prototype>) {
        if self.appearance.is_none() {
            self.appearance = Some(cx.observe_window_appearance(
                window,
                |prototype, window, cx| {
                    let system_dark = is_dark_appearance(window.appearance());
                    if system_dark != prototype.system_dark {
                        prototype.system_dark = system_dark;
                        prototype.ui.settings_page.update(cx, |page, cx| {
                            page.set_system_dark(system_dark, cx);
                        });
                        prototype.refresh_theme(cx);
                    }
                },
            ));
        }
    }
}

/// Whether the platform appearance is one of the dark variants.
pub(super) fn is_dark_appearance(appearance: gpui_kit::WindowAppearance) -> bool {
    matches!(
        appearance,
        gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
    )
}

/// Subscribes the window to transient previews from the appearance editor.
pub(super) fn subscribe_theme_preview(
    cx: &mut Context<Prototype>,
    theme_setting: &Entity<ThemeSetting>,
) -> Subscription {
    cx.subscribe(
        theme_setting,
        |prototype, _, event: &ThemePreviewRequested, cx| {
            prototype.apply_resolved_theme(event.resolved.clone(), cx);
        },
    )
}

/// Subscribes the window to theme changes the config accepted and saved.
pub(super) fn subscribe_theme_changes(
    cx: &mut Context<Prototype>,
    config: &Entity<ConfigState>,
) -> Subscription {
    cx.subscribe(config, |prototype, _, _: &ThemeChanged, cx| {
        prototype.refresh_theme(cx)
    })
}

impl WindowUi {
    /// Builds the window UI with the theme it starts on.
    pub(super) fn new(
        locale: Entity<Locale>,
        geometry: Entity<WindowGeometry>,
        theme: &ResolvedTheme,
        settings_page: Entity<SettingsPage>,
        template_ui: Entity<TemplateUi>,
        saved_connections: Entity<SavedConnectionsUi>,
        tunnel_page: TunnelPage,
    ) -> Self {
        Self {
            locale,
            geometry,
            theme: theme.ui,
            background: theme.background(),
            settings_page,
            template_ui,
            saved_connections,
            tunnel_page,
        }
    }
}

impl Prototype {
    /// Re-resolves the effective theme after a saved change or a system
    /// appearance change and repaints every open surface in place.
    fn refresh_theme(&mut self, cx: &mut Context<Self>) {
        let resolved = theme::resolve(self.factory.config.read(cx).config(), self.system_dark);
        self.apply_resolved_theme(resolved, cx);
    }

    /// Paints one resolved palette across every open surface. A transient
    /// library preview and an accepted configuration edit share this path.
    fn apply_resolved_theme(&mut self, resolved: ResolvedTheme, cx: &mut Context<Self>) {
        if resolved == self.factory.theme {
            return;
        }
        self.factory.theme = resolved.clone();
        self.ui.theme = resolved.ui;
        self.ui.background = resolved.background();
        let ui_theme = resolved.ui;
        self.ui
            .settings_page
            .update(cx, |page, cx| page.set_theme(ui_theme, cx));
        self.ui
            .template_ui
            .update(cx, |template_ui, cx| template_ui.set_theme(ui_theme, cx));
        self.ui.saved_connections.update(cx, |profiles, cx| {
            profiles.set_theme(ui_theme, cx);
        });
        self.ui.tunnel_page.set_theme(ui_theme, cx);

        // Every tab keeps its view, session and draft. A connection theme
        // still wins for its own palette and OSC replies, while the window
        // chrome follows the application theme.
        for (_, tab) in self.tabs.ordered() {
            let palette = resolved.palette_for_tab(&tab.saved);
            tab.view
                .update(cx, |view, cx| view.set_palette(palette.clone(), cx));
            if let Some(binding) = &tab.binding {
                binding.set_palette(palette);
            }
            if let Some(menu) = &tab.context_menu {
                menu.update(cx, |menu, cx| menu.set_theme(ui_theme, cx));
            }
            if let Some(search_bar) = &tab.search_bar {
                search_bar.update(cx, |bar, cx| bar.set_theme(ui_theme, cx));
            }
        }
        cx.notify();
    }

    /// Installs the platform appearance observer once and applies any change
    /// that happened before the first frame.
    pub(super) fn observe_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_subscriptions.install_appearance(window, cx);
        let system_dark = is_dark_appearance(window.appearance());
        if system_dark != self.system_dark {
            self.system_dark = system_dark;
            self.ui
                .settings_page
                .update(cx, |page, cx| page.set_system_dark(system_dark, cx));
            self.refresh_theme(cx);
        }
    }

    /// Drives the same appearance path as the platform observer in a native
    /// window test, where the test platform cannot emit the native callback.
    #[cfg(test)]
    pub(super) fn set_system_dark_for_test(&mut self, system_dark: bool, cx: &mut Context<Self>) {
        if self.system_dark == system_dark {
            return;
        }
        self.system_dark = system_dark;
        self.ui
            .settings_page
            .update(cx, |page, cx| page.set_system_dark(system_dark, cx));
        self.refresh_theme(cx);
    }
}
