//! Rendering for the named, filtered theme-library browser.

use gpui_kit::component::input::Input;
use gpui_kit::component::{Sizable, Size};
use gpui_kit::{Div, FontWeight, MouseButton, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_core::theme::types::CatalogTheme;
use tterm_terminal::theme::{PaletteText, color_to_hex};
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons::{ROTATE_CCW, SEARCH, X, icon};
use tterm_ui::components::input::field_fill;
use tterm_ui::tokens::color::Hsl;
use tterm_ui::tokens::gpui::hsla;

use super::library::{
    LIBRARY_APPLY_ID, LIBRARY_CANCEL_ID, LIBRARY_COUNT_ID, LIBRARY_DETAIL_ID, LIBRARY_EMPTY_ID,
    LIBRARY_ERROR_ID, LIBRARY_ID, LIBRARY_LIST_ID, LIBRARY_LOAD_ERROR_ID, LIBRARY_RETRY_ID,
    LIBRARY_SEARCH_CLEAR_ID, LIBRARY_SEARCH_ID, ThemeFilter, row_id,
};
use super::*;

const PANEL_HEIGHT: f32 = 520.0;
const LIST_WIDTH: f32 = 248.0;
const ROW_HEIGHT: f32 = 30.0;

impl ThemeSetting {
    /// Renders the browsing panel that replaces the preset grid.
    pub(crate) fn library_panel(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::THEME_GALLERY_TITLE);
        let mut panel = div()
            .id(LIBRARY_ID)
            .debug_selector(|| LIBRARY_ID.to_string())
            .mt(px(16.0))
            .h(px(PANEL_HEIGHT))
            .w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.card, 1.0))
            .track_focus(&self.library.focus)
            .capture_key_down(cx.listener(Self::library_key))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .border_b(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .line_height(px(18.0))
                            .font_weight(FontWeight(600.0))
                            .text_color(hsla(colors.foreground, 1.0))
                            .child(title),
                    )
                    .child(self.library_search(cx))
                    .child(self.library_filters(cx)),
            )
            .child(
                div()
                    .min_h(px(0.0))
                    .flex()
                    .flex_1()
                    .child(self.library_list(cx))
                    .child(self.library_detail(cx)),
            );
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .id(LIBRARY_ERROR_ID)
                    .debug_selector(|| LIBRARY_ERROR_ID.to_string())
                    .flex_none()
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        panel.child(self.library_footer(cx))
    }

    fn library_search(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let field = self.library.search.clone();
        let label = self.text(cx, keys::THEME_GALLERY_SEARCH);
        let clear_label = self.text(cx, keys::TAB_CLEAR);
        let border = colors.input;
        let filled = !self.library.query.is_empty();
        let mut row = div()
            .id(LIBRARY_SEARCH_ID)
            .debug_selector(|| LIBRARY_SEARCH_ID.to_string())
            .h(px(30.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .pl(px(10.0))
            .pr(px(10.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(border, 1.0))
            .bg(field_fill(self.theme))
            .role(Role::ComboBox)
            .aria_label(label)
            .child(icon(SEARCH, hsla(colors.muted_foreground, 1.0), 14.0))
            .when_some(field.as_ref(), |row, field| {
                row.child(
                    div().min_w(px(0.0)).flex_1().child(
                        Input::new(field)
                            .with_size(Size::Small)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .text_color(hsla(colors.foreground, 1.0)),
                    ),
                )
            });
        if filled {
            row = row.child(
                div()
                    .id(LIBRARY_SEARCH_CLEAR_ID)
                    .debug_selector(|| LIBRARY_SEARCH_CLEAR_ID.to_string())
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(16.0))
                    .h(px(16.0))
                    .cursor_pointer()
                    .role(Role::Button)
                    .aria_label(clear_label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_library_search(window, cx);
                    }))
                    .rounded_full()
                    .bg(hsla(colors.input, 1.0))
                    .child(icon(X, hsla(colors.muted_foreground, 1.0), 10.0)),
            );
        }
        row
    }

    fn library_filters(&self, cx: &mut Context<Self>) -> Div {
        let mut row = div().flex().items_center().gap(px(4.0));
        for filter in [ThemeFilter::All, ThemeFilter::Dark, ThemeFilter::Light] {
            let label = self.text(cx, filter.label_key());
            let variant = if self.library.filter == filter {
                Variant::Secondary
            } else {
                Variant::Ghost
            };
            row = row.child(
                button(
                    filter.selector(),
                    label,
                    variant,
                    ButtonSize::Xs,
                    ButtonState {
                        disabled: self.saving,
                        ..ButtonState::default()
                    },
                    self.theme,
                )
                .debug_selector(move || filter.selector().to_string())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_library_filter(filter, cx);
                })),
            );
        }
        row
    }

    fn library_list(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let count = if self.library.catalog.is_some() {
            self.library.count_text(self.locale.read(cx))
        } else {
            self.text(cx, keys::THEME_GALLERY_LOADING)
        };
        let mut list = div()
            .id(LIBRARY_LIST_ID)
            .debug_selector(|| LIBRARY_LIST_ID.to_string())
            .w(px(LIST_WIDTH))
            .flex_none()
            .flex()
            .flex_col()
            .border_r(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .child(
                div()
                    .id(LIBRARY_COUNT_ID)
                    .debug_selector(|| LIBRARY_COUNT_ID.to_string())
                    .flex_none()
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_size(px(11.0))
                    .line_height(px(15.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .border_b(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .child(count),
            );
        if self.library.load_failed {
            list = list.child(
                div()
                    .id(LIBRARY_LOAD_ERROR_ID)
                    .debug_selector(|| LIBRARY_LOAD_ERROR_ID.to_string())
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .p(px(12.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(self.text(cx, keys::THEME_GALLERY_LOAD_FAILED))
                    .child(
                        button(
                            LIBRARY_RETRY_ID,
                            self.text(cx, keys::COMMON_RETRY),
                            Variant::Outline,
                            ButtonSize::Xs,
                            ButtonState::default(),
                            self.theme,
                        )
                        .debug_selector(|| LIBRARY_RETRY_ID.to_string())
                        .child(icon(ROTATE_CCW, hsla(colors.foreground, 1.0), 12.0))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.retry_library(window, cx)),
                        ),
                    ),
            );
        } else if self.library.catalog.is_none() {
            list = list.child(
                div()
                    .p(px(12.0))
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::THEME_GALLERY_LOADING)),
            );
        } else {
            let visible = self.library.visible();
            if visible.is_empty() {
                list = list.child(
                    div()
                        .id(LIBRARY_EMPTY_ID)
                        .debug_selector(|| LIBRARY_EMPTY_ID.to_string())
                        .p(px(12.0))
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(hsla(colors.muted_foreground, 1.0))
                        .child(self.text(cx, keys::THEME_GALLERY_NO_MATCH)),
                );
            } else {
                let selected = self.library.selected.clone();
                let current = self.effective_theme(cx).id;
                let mut rows = div()
                    .id("settings-theme-library-rows")
                    .min_h(px(0.0))
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.library.scroll)
                    .p(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .role(Role::ListBox);
                for theme in visible {
                    let selected = selected.as_deref() == Some(theme.id.as_str());
                    let current = current == theme.id;
                    rows = rows.child(self.library_row(theme, selected, current, cx));
                }
                list = list.child(rows);
            }
        }
        list
    }

    fn library_row(
        &self,
        theme: &'static CatalogTheme,
        selected: bool,
        current: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let palette = theme.terminal();
        let background = palette_color(&palette.background, Hsl::new(220.0, 13.0, 12.0));
        let selector = row_id(&theme.id);
        let selector_id = selector.clone();
        let label = theme.name.clone();
        let id = theme.id.clone();
        let swatches = ["red", "green", "yellow", "blue", "magenta", "cyan"]
            .map(|name| palette_field(palette, name))
            .map(|value| {
                div()
                    .flex_none()
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(palette_color(&value, Hsl::new(0.0, 0.0, 0.0)))
            });
        div()
            .id(selector)
            .debug_selector(move || selector_id.clone())
            .h(px(ROW_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(6.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .role(Role::ListBoxOption)
            .aria_label(label.clone())
            .aria_selected(selected)
            .when(selected, |row| row.bg(hsla(colors.primary, 0.16)))
            .when(!selected, |row| {
                row.hover(|style| style.bg(hsla(colors.muted, 1.0)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.select_library(&id, cx)))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .h(px(18.0))
                    .px(px(4.0))
                    .rounded(px(3.0))
                    .border(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .bg(background)
                    .children(swatches),
            )
            .child(
                div()
                    .min_w(px(0.0))
                    .flex_1()
                    .truncate()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(
                        if selected {
                            colors.foreground
                        } else {
                            colors.muted_foreground
                        },
                        1.0,
                    ))
                    .child(theme.name.clone()),
            )
            .when(current || selected, |row| {
                row.child(
                    div()
                        .flex_none()
                        .text_size(px(12.0))
                        .text_color(hsla(colors.primary, 1.0))
                        .child("✓"),
                )
            })
    }

    fn library_detail(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let selected = self.library.selected_theme().cloned();
        let current = self.effective_theme(cx).id;
        let mut detail = div()
            .id(LIBRARY_DETAIL_ID)
            .debug_selector(|| LIBRARY_DETAIL_ID.to_string())
            .min_w(px(0.0))
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .p(px(12.0))
            .overflow_y_scroll();
        let Some(theme) = selected else {
            return detail.child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.0))
                    .border(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .p(px(16.0))
                    .text_size(px(12.0))
                    .line_height(px(17.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::THEME_GALLERY_PICK_HINT)),
            );
        };
        let kind = if theme.is_dark() {
            self.text(cx, keys::THEME_GALLERY_DARK)
        } else {
            self.text(cx, keys::THEME_GALLERY_LIGHT)
        };
        let is_current = theme.id == current;
        detail = detail.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(px(15.0))
                        .line_height(px(20.0))
                        .font_weight(FontWeight(600.0))
                        .text_color(hsla(colors.foreground, 1.0))
                        .child(theme.name.clone()),
                )
                .child(self.library_badge(kind, false))
                .when(is_current, |row| {
                    row.child(self.library_badge(self.text(cx, keys::THEME_GALLERY_CURRENT), true))
                }),
        );
        let palette = theme.terminal();
        let mut chips = div().flex().flex_wrap().gap(px(4.0));
        for value in palette_values(palette) {
            chips = chips.child(
                div()
                    .w(px(22.0))
                    .h(px(22.0))
                    .rounded(px(3.0))
                    .border(px(1.0))
                    .border_color(hsla(colors.border, 1.0))
                    .bg(palette_color(&value, Hsl::new(0.0, 0.0, 0.0))),
            );
        }
        detail.child(chips).child(
            div()
                .text_size(px(12.0))
                .line_height(px(17.0))
                .text_color(hsla(colors.muted_foreground, 1.0))
                .child(self.text(cx, keys::THEME_GALLERY_PREVIEW_HINT)),
        )
    }

    fn library_badge(&self, label: String, current: bool) -> Div {
        let colors = self.theme.colors;
        div()
            .flex_none()
            .px(px(6.0))
            .py(px(2.0))
            .rounded(px(4.0))
            .border(px(1.0))
            .border_color(hsla(
                if current {
                    colors.primary
                } else {
                    colors.border
                },
                1.0,
            ))
            .text_size(px(11.0))
            .line_height(px(14.0))
            .text_color(hsla(
                if current {
                    colors.primary
                } else {
                    colors.muted_foreground
                },
                1.0,
            ))
            .child(label)
    }

    fn library_footer(&mut self, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        let cancel = self.text(cx, keys::COMMON_CANCEL);
        let apply = self.text(cx, keys::THEME_GALLERY_APPLY);
        let apply_enabled = !self.saving && self.library.selected.is_some();
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.0))
            .p(px(10.0))
            .border_t(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .child(
                button(
                    LIBRARY_CANCEL_ID,
                    cancel,
                    Variant::Outline,
                    ButtonSize::Sm,
                    ButtonState {
                        disabled: self.saving,
                        ..ButtonState::default()
                    },
                    self.theme,
                )
                .debug_selector(|| LIBRARY_CANCEL_ID.to_string())
                .on_click(cx.listener(|this, _, window, cx| {
                    this.cancel_library(window, cx);
                })),
            )
            .child(
                button(
                    LIBRARY_APPLY_ID,
                    apply,
                    Variant::Default,
                    ButtonSize::Sm,
                    ButtonState {
                        disabled: !apply_enabled,
                        ..ButtonState::default()
                    },
                    self.theme,
                )
                .debug_selector(|| LIBRARY_APPLY_ID.to_string())
                .on_click(cx.listener(|this, _, window, cx| this.apply_library(window, cx))),
            )
    }
}

/// Reads one ANSI palette field by its catalog name.
fn palette_field(palette: &PaletteText, name: &str) -> String {
    match name {
        "red" => palette.red.clone(),
        "green" => palette.green.clone(),
        "yellow" => palette.yellow.clone(),
        "blue" => palette.blue.clone(),
        "magenta" => palette.magenta.clone(),
        "cyan" => palette.cyan.clone(),
        _ => palette.foreground.clone(),
    }
}

/// The interface entry's swatch colours, in catalog order.
fn palette_values(palette: &PaletteText) -> Vec<String> {
    [
        &palette.background,
        &palette.foreground,
        &palette.cursor,
        &palette.red,
        &palette.green,
        &palette.yellow,
        &palette.blue,
        &palette.magenta,
        &palette.cyan,
        &palette.white,
        &palette.bright_black,
        &palette.bright_red,
        &palette.bright_green,
        &palette.bright_yellow,
        &palette.bright_blue,
        &palette.bright_magenta,
        &palette.bright_cyan,
        &palette.bright_white,
    ]
    .into_iter()
    .cloned()
    .collect()
}

/// Converts a catalog color string to the interface drawing color.
fn palette_color(value: &str, fallback: Hsl) -> gpui_kit::Hsla {
    color_to_hex(value)
        .and_then(|hex| gpui_kit::Rgba::try_from(hex.as_str()).ok())
        .map_or_else(|| hsla(fallback, 1.0), gpui_kit::Hsla::from)
}
