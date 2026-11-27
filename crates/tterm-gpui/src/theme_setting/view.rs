//! Rendering for the theme presets, the follow-system switch and the library.

use gpui_kit::{Div, FontWeight, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::switch::{SwitchState, switch};
use tterm_ui::tokens::gpui::hsla;

use super::library::LIBRARY_OPEN_ID;
use super::*;

impl ThemeSetting {
    /// Renders the section into the Appearance page.
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::THEME_TITLE);
        let description = self.text(cx, keys::THEME_DESCRIPTION);
        let mut section = div()
            .id(SECTION_ID)
            .debug_selector(|| SECTION_ID.to_string())
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::handle_key))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .line_height(px(14.0))
                            .font_weight(FontWeight(500.0))
                            .text_color(hsla(colors.foreground, 1.0))
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(description),
                    ),
            )
            .child(self.follow_row(cx))
            .when(!self.library.open, |section| {
                section
                    .child(self.presets(cx))
                    .child(self.library_entry(cx))
            })
            .when(self.library.open, |section| {
                section.child(self.library_panel(cx))
            });
        if !self.library.open
            && let Some(error) = &self.error
        {
            section = section.child(
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        section
    }

    fn library_entry(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = self.text(cx, keys::THEME_GALLERY_OPEN);
        button(
            LIBRARY_OPEN_ID,
            label,
            Variant::Outline,
            ButtonSize::Sm,
            ButtonState {
                disabled: self.saving,
                ..ButtonState::default()
            },
            self.theme,
        )
        .debug_selector(|| LIBRARY_OPEN_ID.to_string())
        .mt(px(12.0))
        .on_click(cx.listener(|this, _, window, cx| this.open_library(window, cx)))
    }

    fn follow_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, keys::THEME_FOLLOW_SYSTEM);
        let description = self.text(cx, keys::THEME_FOLLOW_SYSTEM_DESC);
        let entity = cx.entity().downgrade();
        let switch_label = label.clone();
        let control = switch(
            FOLLOW_SWITCH_ID,
            SwitchState {
                checked: self.draft.follow_system,
                disabled: self.saving,
                ..SwitchState::default()
            },
            self.theme,
            move |next, _, window, app| {
                let _ = entity.update(app, |this, cx| this.set_follow_system(next, window, cx));
            },
        )
        .track_focus(&self.follow_focus)
        .flex_shrink_0()
        .debug_selector(|| FOLLOW_SWITCH_ID.to_string())
        .accessibility_label(switch_label);
        div()
            .id(FOLLOW_ROW_ID)
            .debug_selector(|| FOLLOW_ROW_ID.to_string())
            .mt(px(16.0))
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .line_height(px(18.0))
                            .font_weight(FontWeight(500.0))
                            .text_color(hsla(colors.foreground, 1.0))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .text_color(hsla(colors.muted_foreground, 1.0))
                            .child(description),
                    ),
            )
            .child(control)
    }

    fn presets(&self, cx: &mut Context<Self>) -> Div {
        let mut area = div().mt(px(16.0)).flex().flex_col().gap(px(16.0));
        if self.draft.follow_system {
            area = area
                .child(self.group(keys::THEME_LIGHT_APPEARANCE, ThemeSlot::Light, false, cx))
                .child(self.group(keys::THEME_DARK_APPEARANCE, ThemeSlot::Dark, true, cx));
        } else {
            area = area.child(self.grid(ThemeSlot::Single, None, cx));
        }
        area
    }

    fn group(
        &self,
        label_key: TranslationKey,
        slot: ThemeSlot,
        dark: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, label_key);
        let (id, selector_id) = match slot {
            ThemeSlot::Light => (LIGHT_GROUP_ID, LIGHT_GROUP_ID),
            _ => (DARK_GROUP_ID, DARK_GROUP_ID),
        };
        div()
            .id(id)
            .debug_selector(move || selector_id.to_string())
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(label),
            )
            .child(self.grid(slot, Some(dark), cx))
    }

    fn grid(&self, slot: ThemeSlot, tone: Option<bool>, cx: &mut Context<Self>) -> Div {
        let mut grid = div().flex().flex_wrap().gap(px(8.0));
        for id in PRESETS
            .into_iter()
            .filter(|id| tone.is_none_or(|dark| is_dark(id) == dark))
        {
            grid = grid.child(self.card(slot, id, cx));
        }
        grid
    }

    fn card(&self, slot: ThemeSlot, id: &'static str, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let selected = self.selected_in(slot) == id;
        let preview = crate::app_window::theme::preset_ui_theme(id);
        let name = self.text(cx, name_key(id));
        let label = name.clone();
        let option_id = option_id(slot, id);
        let selector_id = option_id.clone();
        div()
            .id(option_id)
            .debug_selector(move || selector_id.clone())
            .w(px(172.0))
            .flex()
            .flex_col()
            .gap(px(6.0))
            .p(px(6.0))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(
                if selected { colors.ring } else { colors.border },
                1.0,
            ))
            .bg(hsla(colors.card, 1.0))
            .when(self.saving, |card| card.opacity(0.5))
            .cursor_pointer()
            .role(Role::Button)
            .aria_label(label)
            .aria_selected(selected)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_preset(slot, id, window, cx);
            }))
            .child(self.swatch(preview))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(2.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .child(
                        div()
                            .font_weight(if selected {
                                FontWeight(500.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(hsla(
                                if selected {
                                    colors.foreground
                                } else {
                                    colors.muted_foreground
                                },
                                1.0,
                            ))
                            .child(name),
                    )
                    .when(selected, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_color(hsla(colors.primary, 1.0))
                                .child("✓"),
                        )
                    }),
            )
    }

    fn swatch(&self, preview: Theme) -> Div {
        let colors = preview.colors;
        div()
            .h(px(44.0))
            .w_full()
            .rounded(px(5.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.background, 1.0))
            .flex()
            .items_center()
            .justify_between()
            .px(px(8.0))
            .child(
                div()
                    .font_family("monospace")
                    .text_size(px(13.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.foreground, 1.0))
                    .child("Aa"),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(10.0))
                    .h(px(10.0))
                    .rounded_full()
                    .bg(hsla(colors.primary, 1.0)),
            )
    }
}
