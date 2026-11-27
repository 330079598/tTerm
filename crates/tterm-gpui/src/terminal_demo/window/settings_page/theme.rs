//! Live palette updates for the settings route and its editors.

use gpui_kit::Context;
use tterm_ui::tokens::theme::Theme;

use super::SettingsPage;

impl SettingsPage {
    /// The live theme editor, so the window can subscribe to its transient
    /// previews without reaching through the page's private fields.
    pub(crate) fn theme_setting_entity(
        &self,
    ) -> gpui_kit::Entity<crate::theme_setting::ThemeSetting> {
        self.theme_setting.clone()
    }

    /// Replaces every page palette without closing the route or dropping a
    /// draft. The search field is painted from `self.theme` on each frame, so
    /// notifying the page also updates it.
    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.scrollback
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.font
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.theme_setting
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.appearance
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.clipboard
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.command_marks
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        self.local_shells
            .update(cx, |setting, cx| setting.set_theme(theme, cx));
        cx.notify();
    }

    /// Keeps the appearance editor's draft rules in step with the window.
    pub(crate) fn set_system_dark(&mut self, system_dark: bool, cx: &mut Context<Self>) {
        self.theme_setting
            .update(cx, |setting, cx| setting.set_system_dark(system_dark, cx));
    }
}
