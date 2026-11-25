//! Test access to the settings page's drafts, focus and search results.

use super::*;
use crate::clipboard_setting::ClipboardDraft;

impl SettingsPage {
    #[cfg(test)]
    pub(crate) fn active_tab(&self) -> SettingsTab {
        self.active_tab
    }

    #[cfg(test)]
    pub(crate) fn local_shells_editor(&self, cx: &App) -> Option<(String, String, String)> {
        self.local_shells.read(cx).editor_values(cx)
    }

    #[cfg(test)]
    pub(crate) fn local_shells_error(&self, cx: &App) -> Option<String> {
        self.local_shells.read(cx).error().map(str::to_string)
    }

    #[cfg(test)]
    pub(crate) fn local_shells_saving(&self, cx: &App) -> bool {
        self.local_shells.read(cx).is_saving()
    }

    #[cfg(test)]
    pub(crate) fn font_values(&self, cx: &App) -> (String, String) {
        let font = self.font.read(cx);
        (font.family(cx), font.size(cx))
    }

    #[cfg(test)]
    pub(crate) fn font_error(&self, cx: &App) -> Option<String> {
        self.font.read(cx).error().map(str::to_string)
    }

    #[cfg(test)]
    pub(crate) fn font_saved(&self, cx: &App) -> bool {
        self.font.read(cx).saved()
    }

    #[cfg(test)]
    pub(crate) fn font_preview(&self, cx: &App) -> (String, u16) {
        let font = self.font.read(cx);
        (font.preview_family(cx), font.preview_size(cx))
    }

    #[cfg(test)]
    pub(crate) fn font_family_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.font.read(cx).family_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn font_size_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.font.read(cx).size_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn font_line_height_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.font.read(cx).line_height_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn font_letter_spacing_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.font.read(cx).letter_spacing_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn font_spacing_values(&self, cx: &App) -> (String, String) {
        let font = self.font.read(cx);
        (font.line_height(cx), font.letter_spacing(cx))
    }

    #[cfg(test)]
    pub(crate) fn font_family_placeholder(&self, cx: &App) -> String {
        self.font.read(cx).family_placeholder(cx)
    }

    #[cfg(test)]
    pub(crate) fn font_system_fonts(&self, cx: &App) -> Vec<String> {
        self.font.read(cx).system_fonts().to_vec()
    }

    #[cfg(test)]
    pub(crate) fn search_query(&self) -> &str {
        &self.query
    }

    #[cfg(test)]
    pub(crate) fn search_value(&self, cx: &App) -> String {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).value().to_string())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn search_placeholder(&self, cx: &App) -> String {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).presentation().placeholder().to_string())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn search_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.search_input
            .as_ref()
            .map(|field| field.read(cx).presentation().focus_handle().clone())
    }

    #[cfg(test)]
    pub(crate) fn scrollback_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.scrollback.read(cx).page_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn scrollback_placeholder(&self, cx: &App) -> String {
        self.scrollback.read(cx).page_placeholder(cx)
    }

    #[cfg(test)]
    pub(crate) fn search_result_count(&self, cx: &App) -> usize {
        self.results(cx).len()
    }

    #[cfg(test)]
    pub(crate) fn active_index(&self) -> usize {
        self.active_index
    }

    #[cfg(test)]
    pub(crate) fn page_saving(&self) -> bool {
        self.page_saving
    }

    #[cfg(test)]
    pub(crate) fn page_saved(&self) -> bool {
        self.page_saved
    }

    #[cfg(test)]
    pub(crate) fn page_error(&self) -> Option<&str> {
        self.page_error.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn scrollback_value(&self, cx: &App) -> String {
        self.scrollback.read(cx).value(cx)
    }

    #[cfg(test)]
    pub(crate) fn appearance_cursor(&self, cx: &App) -> String {
        self.appearance.read(cx).cursor_style().to_string()
    }

    #[cfg(test)]
    pub(crate) fn appearance_padding(&self, cx: &App) -> (String, String, String) {
        self.appearance.read(cx).padding_values(cx)
    }

    #[cfg(test)]
    pub(crate) fn appearance_error(&self, cx: &App) -> Option<String> {
        self.appearance.read(cx).error().map(str::to_string)
    }

    #[cfg(test)]
    pub(crate) fn clipboard_values(&self, cx: &App) -> ClipboardDraft {
        self.clipboard.read(cx).values()
    }

    #[cfg(test)]
    pub(crate) fn clipboard_error(&self, cx: &App) -> Option<String> {
        self.clipboard.read(cx).error().map(str::to_string)
    }

    #[cfg(test)]
    pub(crate) fn appearance_cursor_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        Some(self.appearance.read(cx).cursor_focus())
    }

    #[cfg(test)]
    pub(crate) fn appearance_padding_left_focus(&self, cx: &App) -> Option<gpui_kit::FocusHandle> {
        self.appearance.read(cx).padding_left_focus(cx)
    }

    #[cfg(test)]
    pub(crate) fn empty_message(&self, cx: &App) -> String {
        search::empty_text(self.locale.read(cx), &self.query)
    }

    #[cfg(test)]
    pub(crate) fn result_titles(&self, cx: &App) -> Vec<String> {
        self.results(cx)
            .into_iter()
            .map(|result| result.title)
            .collect()
    }
}
