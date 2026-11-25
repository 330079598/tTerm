//! The page shell: header, sidebar, content and search results.
//!
//! Each measurement below comes from the 800x600 reference capture of the
//! production React page at DPR 1 (the layout JSON stores CSS pixels). The
//! page keeps those values instead of scaling them from the window so the
//! sidebar and header stay aligned when a split or a dialog shrinks the area.

use gpui_kit::component::input::Input;
use gpui_kit::component::{Sizable, Size};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Focusable, FontWeight, MouseButton, Role, Stateful, Window, div, px,
};
use tterm_core::i18n::keys;
use tterm_core::settings::search::SettingsSearchResult;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons::{
    CHEVRON_RIGHT, SEARCH, SEARCH_X, SETTINGS, SQUARE_TERMINAL, X, icon,
};
use tterm_ui::tokens::gpui::hsla;

use super::search;
use super::{
    SETTINGS_HEADER_ID, SETTINGS_PAGE_ID, SETTINGS_PANEL_ID, SETTINGS_SAVE_CONFIRMATION_ID,
    SETTINGS_SAVE_ERROR_ID, SETTINGS_SAVE_ID, SETTINGS_SEARCH_CLEAR_ID, SETTINGS_SEARCH_COUNT_ID,
    SETTINGS_SEARCH_EMPTY_ID, SETTINGS_SEARCH_FIELD_ID, SETTINGS_SEARCH_RESULT_0_ID,
    SETTINGS_SEARCH_RESULTS_ID, SETTINGS_SIDEBAR_ID, SETTINGS_TAB_LOCAL_SHELLS_ID,
    SETTINGS_TAB_TERMINAL_ID, SETTINGS_TITLE_ID, SettingsPage, SettingsTab,
};

const HEADER_HEIGHT: f32 = 73.0;
const HEADER_PAD_X: f32 = 24.0;
const HEADER_PAD_RIGHT: f32 = 48.0;
const HEADER_PAD_TOP: f32 = 24.0;
const HEADER_PAD_BOTTOM: f32 = 16.0;
const TITLE_ICON: f32 = 18.0;
const SIDEBAR_WIDTH: f32 = 192.0;
const SIDEBAR_PAD: f32 = 12.0;
/// The reference scrollbar takes 8px from the sidebar's item width.
const SIDEBAR_SCROLLBAR: f32 = 8.0;
const TAB_HEIGHT: f32 = 32.0;
const TAB_PAD_X: f32 = 8.0;
const TAB_GAP: f32 = 8.0;
const TAB_ICON: f32 = 16.0;
const SEARCH_WIDTH: f32 = 256.0;
const SEARCH_HEIGHT: f32 = 32.0;
const CONTENT_PAD: f32 = 24.0;
/// The scroll area's right padding (16) plus its scrollbar gutter (8).
const CONTENT_SCROLLBAR: f32 = 24.0;
const RESULT_ROW_PAD_X: f32 = 12.0;
const RESULT_ROW_PAD_Y: f32 = 10.0;

pub(super) fn page(
    page: &mut SettingsPage,
    window: &Window,
    cx: &mut Context<SettingsPage>,
) -> AnyElement {
    if !page.open {
        return div().into_any_element();
    }
    let colors = page.theme.colors;
    let searching = page.has_query();
    let body: AnyElement = if searching {
        let results = page.results(cx);
        search_results(page, &results, cx).into_any_element()
    } else {
        content(page, cx).into_any_element()
    };
    div()
        .id(SETTINGS_PAGE_ID)
        .debug_selector(|| SETTINGS_PAGE_ID.to_string())
        .size_full()
        .flex()
        .flex_col()
        .bg(hsla(colors.background, 1.0))
        .text_color(hsla(colors.foreground, 1.0))
        .text_size(px(13.0))
        .line_height(px(18.2))
        .child(header(page, window, cx))
        .child(
            div()
                .min_h(px(0.0))
                .flex()
                .flex_1()
                .child(sidebar(page, cx))
                .child(body),
        )
        .into_any_element()
}

fn header(
    page: &mut SettingsPage,
    window: &Window,
    cx: &mut Context<SettingsPage>,
) -> Stateful<Div> {
    let colors = page.theme.colors;
    let title = search::text(page.locale.read(cx), keys::SETTINGS_TITLE);
    div()
        .id(SETTINGS_HEADER_ID)
        .debug_selector(|| SETTINGS_HEADER_ID.to_string())
        .h(px(HEADER_HEIGHT))
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .border_b(px(1.0))
        .border_color(hsla(colors.border, 1.0))
        .pt(px(HEADER_PAD_TOP))
        .pb(px(HEADER_PAD_BOTTOM))
        .pl(px(HEADER_PAD_X))
        .pr(px(HEADER_PAD_RIGHT))
        .child(
            div()
                .id(SETTINGS_TITLE_ID)
                .debug_selector(|| SETTINGS_TITLE_ID.to_string())
                .flex()
                .items_center()
                .gap(px(8.0))
                .text_size(px(18.0))
                .line_height(px(18.0))
                .font_weight(FontWeight(600.0))
                .child(icon(SETTINGS, hsla(colors.foreground, 1.0), TITLE_ICON))
                .child(title),
        )
        .child(search_field(page, window, cx))
}

fn search_field(
    page: &mut SettingsPage,
    window: &Window,
    cx: &mut Context<SettingsPage>,
) -> Stateful<Div> {
    let colors = page.theme.colors;
    let label = search::text(page.locale.read(cx), keys::SETTINGS_SEARCH_LABEL);
    let clear_label = search::text(page.locale.read(cx), keys::TAB_CLEAR);
    let field = page.search_input.clone();
    let focused = field
        .as_ref()
        .is_some_and(|field| field.read(cx).focus_handle(cx).is_focused(window));
    let border = if focused { colors.ring } else { colors.input };
    let filled = !page.query.is_empty();
    let mut row = div()
        .id(SETTINGS_SEARCH_FIELD_ID)
        .debug_selector(|| SETTINGS_SEARCH_FIELD_ID.to_string())
        .w(px(SEARCH_WIDTH))
        .h(px(SEARCH_HEIGHT))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .pl(px(10.0))
        .pr(px(12.0))
        .rounded(px(6.0))
        .border(px(1.0))
        .border_color(hsla(border, 1.0))
        .bg(hsla(colors.input, 0.3))
        .role(Role::ComboBox)
        .aria_label(label)
        .capture_key_down(cx.listener(SettingsPage::search_key))
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
                .id(SETTINGS_SEARCH_CLEAR_ID)
                .debug_selector(|| SETTINGS_SEARCH_CLEAR_ID.to_string())
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
                    this.clear_search(window, cx);
                }))
                .rounded_full()
                .bg(hsla(colors.input, 1.0))
                .child(icon(X, hsla(colors.muted_foreground, 1.0), 10.0)),
        );
    }
    row
}

fn sidebar(page: &mut SettingsPage, cx: &mut Context<SettingsPage>) -> Stateful<Div> {
    let colors = page.theme.colors;
    let group = search::text(page.locale.read(cx), keys::SETTINGS_CATEGORIES_EXPERIENCE);
    div()
        .id(SETTINGS_SIDEBAR_ID)
        .debug_selector(|| SETTINGS_SIDEBAR_ID.to_string())
        .w(px(SIDEBAR_WIDTH))
        .flex_none()
        .border_r(px(1.0))
        .border_color(hsla(colors.border, 1.0))
        .bg(hsla(colors.muted, 0.3))
        .pt(px(SIDEBAR_PAD))
        .pb(px(SIDEBAR_PAD))
        .pl(px(SIDEBAR_PAD))
        .pr(px(SIDEBAR_PAD + SIDEBAR_SCROLLBAR))
        .child(
            div().flex().flex_col().child(
                div()
                    .px(px(TAB_PAD_X))
                    .mb(px(4.0))
                    .text_size(px(10.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(600.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(group.to_uppercase()),
            ),
        )
        .child(sidebar_tab(
            page,
            SETTINGS_TAB_TERMINAL_ID,
            SettingsTab::Terminal,
            keys::SETTINGS_TERMINAL,
            cx,
        ))
        .child(sidebar_tab(
            page,
            SETTINGS_TAB_LOCAL_SHELLS_ID,
            SettingsTab::LocalShells,
            keys::LOCAL_SHELLS_SIDEBAR,
            cx,
        ))
}

fn sidebar_tab(
    page: &mut SettingsPage,
    id: &'static str,
    tab: SettingsTab,
    label_key: tterm_core::i18n::translate::TranslationKey,
    cx: &mut Context<SettingsPage>,
) -> Stateful<Div> {
    let colors = page.theme.colors;
    let label = search::text(page.locale.read(cx), label_key);
    let selected = page.active_tab == tab;
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .h(px(TAB_HEIGHT))
        .w_full()
        .flex()
        .items_center()
        .gap(px(TAB_GAP))
        .px(px(TAB_PAD_X))
        .rounded(px(6.0))
        .cursor_pointer()
        .role(Role::Tab)
        .aria_selected(selected)
        .aria_label(label.clone())
        .bg(hsla(colors.background, 1.0))
        .text_color(hsla(
            if selected {
                colors.foreground
            } else {
                colors.muted_foreground
            },
            1.0,
        ))
        .text_size(px(13.0))
        .line_height(px(18.2))
        .font_weight(FontWeight(500.0))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.select_tab(tab, window, cx);
        }))
        .child(icon(
            SQUARE_TERMINAL,
            hsla(colors.muted_foreground, 1.0),
            TAB_ICON,
        ))
        .child(div().min_w(px(0.0)).truncate().child(label))
}

fn content(page: &mut SettingsPage, cx: &mut Context<SettingsPage>) -> Stateful<Div> {
    let mut panel = div()
        .id(SETTINGS_PANEL_ID)
        .track_scroll(&page.content_scroll)
        .debug_selector(|| SETTINGS_PANEL_ID.to_string())
        .min_w(px(0.0))
        .flex_1()
        .overflow_y_scroll()
        .p(px(CONTENT_PAD))
        .pr(px(CONTENT_PAD + CONTENT_SCROLLBAR));
    match page.active_tab {
        SettingsTab::Terminal => {
            for (index, section) in super::SETTINGS_SECTIONS.iter().enumerate() {
                let child = final_section(page, *section, cx);
                panel = panel.child(
                    div()
                        .when(index > 0 && !section.continues_previous(), |child| {
                            child.mt(px(24.0))
                        })
                        .child(child),
                );
            }
            panel
        }
        SettingsTab::LocalShells => {
            let section = page
                .local_shells
                .update(cx, |setting, cx| setting.section(cx));
            panel.child(section)
        }
    }
}

/// Renders one Terminal-page section in the shared order.
fn final_section(
    page: &mut SettingsPage,
    section: super::SettingsSection,
    cx: &mut Context<SettingsPage>,
) -> AnyElement {
    use super::SettingsSection;
    match section {
        SettingsSection::Font => page
            .font
            .update(cx, |setting, cx| setting.text_section(cx))
            .into_any_element(),
        SettingsSection::Cursor => page
            .appearance
            .update(cx, |setting, cx| setting.cursor_section(cx))
            .into_any_element(),
        SettingsSection::Clipboard => page
            .clipboard
            .update(cx, |setting, cx| setting.section(cx))
            .into_any_element(),
        SettingsSection::CommandMarks => page
            .command_marks
            .update(cx, |setting, cx| setting.section(cx))
            .into_any_element(),
        SettingsSection::Scrollback => page
            .scrollback
            .update(cx, |setting, cx| setting.section(cx))
            .into_any_element(),
        SettingsSection::Padding => page
            .appearance
            .update(cx, |setting, cx| setting.padding_section(cx))
            .into_any_element(),
        SettingsSection::Preview => page.font.read(cx).preview_section(cx).into_any_element(),
        SettingsSection::Save => page_save(page, cx).into_any_element(),
    }
}

/// The one Save below the Terminal page.
///
/// It writes the whole draft through one configuration edit; the field
/// editors deliberately carry no save action of their own.
fn page_save(page: &mut SettingsPage, cx: &mut Context<SettingsPage>) -> Div {
    let theme = page.theme;
    let save = search::text(page.locale.read(cx), keys::FONT_SETTINGS_SAVE);
    let saved = search::text(page.locale.read(cx), keys::FONT_SETTINGS_SAVED);
    let state = ButtonState {
        disabled: page.page_saving,
        ..ButtonState::default()
    };
    let mut content = div().w_full().flex().flex_col().gap(px(8.0));
    if let Some(error) = &page.page_error {
        content = content.child(
            div()
                .id(SETTINGS_SAVE_ERROR_ID)
                .debug_selector(|| SETTINGS_SAVE_ERROR_ID.to_string())
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(hsla(theme.colors.destructive, 1.0))
                .child(error.clone()),
        );
    }
    if page.page_saved {
        content = content.child(
            div()
                .id(SETTINGS_SAVE_CONFIRMATION_ID)
                .debug_selector(|| SETTINGS_SAVE_CONFIRMATION_ID.to_string())
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(hsla(theme.colors.success, 1.0))
                .child(saved),
        );
    }
    content.child(
        button(
            SETTINGS_SAVE_ID,
            save.clone(),
            Variant::Default,
            ButtonSize::Default,
            state,
            theme,
        )
        .w_full()
        .debug_selector(|| SETTINGS_SAVE_ID.to_string())
        .aria_label(save)
        .on_click(cx.listener(|this, _, window, cx| this.submit_page(window, cx))),
    )
}

fn search_results(
    page: &mut SettingsPage,
    results: &[SettingsSearchResult],
    cx: &mut Context<SettingsPage>,
) -> Stateful<Div> {
    let colors = page.theme.colors;
    let query = page.query.clone();
    let label = search::text(page.locale.read(cx), keys::SETTINGS_SEARCH_LABEL);
    let count = search::count_text(page.locale.read(cx), results.len());
    if results.is_empty() {
        return div()
            .id(SETTINGS_SEARCH_EMPTY_ID)
            .debug_selector(|| SETTINGS_SEARCH_EMPTY_ID.to_string())
            .min_w(px(0.0))
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .p(px(24.0))
            .text_size(px(14.0))
            .line_height(px(19.0))
            .text_color(hsla(colors.muted_foreground, 1.0))
            .child(icon(SEARCH_X, hsla(colors.muted_foreground, 1.0), 24.0))
            .child(search::empty_text(page.locale.read(cx), &query));
    }

    let list = div()
        .id(SETTINGS_SEARCH_RESULTS_ID)
        .debug_selector(|| SETTINGS_SEARCH_RESULTS_ID.to_string())
        .min_w(px(0.0))
        .flex_1()
        .overflow_y_scroll()
        .p(px(24.0))
        .child(
            div()
                .id(SETTINGS_SEARCH_COUNT_ID)
                .debug_selector(|| SETTINGS_SEARCH_COUNT_ID.to_string())
                .px(px(8.0))
                .mb(px(8.0))
                .text_size(px(12.0))
                .line_height(px(13.2))
                .text_color(hsla(colors.muted_foreground, 1.0))
                .child(count),
        );
    let mut rows = div()
        .id("settings-search-rows")
        .role(Role::ListBox)
        .aria_label(label)
        .flex()
        .flex_col();
    for (index, result) in results.iter().enumerate() {
        rows = rows.child(result_row(page, index, result, cx));
    }
    list.child(rows)
}

fn result_row(
    page: &mut SettingsPage,
    index: usize,
    result: &SettingsSearchResult,
    cx: &mut Context<SettingsPage>,
) -> Stateful<Div> {
    let colors = page.theme.colors;
    let active = index == page.active_index;
    let row_id = if index == 0 {
        SETTINGS_SEARCH_RESULT_0_ID.to_string()
    } else {
        format!("settings-search-result-{index}")
    };
    let mut title_line = div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .text_size(px(14.0))
        .line_height(px(18.2));
    if !result.is_page {
        title_line = title_line
            .child(
                div()
                    .flex_none()
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(result.tab_label.clone()),
            )
            .child(icon(
                CHEVRON_RIGHT,
                hsla(colors.muted_foreground, 1.0),
                12.0,
            ));
    }
    title_line = title_line.child(
        div()
            .min_w(px(0.0))
            .truncate()
            .font_weight(FontWeight(500.0))
            .child(result.title.clone()),
    );
    let mut details = div().min_w(px(0.0)).flex_1().child(title_line);
    if let Some(description) = &result.description {
        details = details.child(
            div()
                .mt(px(2.0))
                .text_size(px(12.0))
                .line_height(px(20.0))
                .text_color(hsla(colors.muted_foreground, 1.0))
                .child(description.clone()),
        );
    }
    let move_listener = cx.listener(move |this, _, _, cx| {
        if this.active_index != index {
            this.active_index = index;
            cx.notify();
        }
    });
    let open_listener = cx.listener(move |this, _, window, cx| {
        if let Some(result) = this.results(cx).get(index).cloned() {
            this.open_result(result, window, cx);
        }
    });
    div()
        .id(row_id.clone())
        .debug_selector(move || row_id.clone())
        .flex()
        .items_start()
        .gap(px(12.0))
        .px(px(RESULT_ROW_PAD_X))
        .py(px(RESULT_ROW_PAD_Y))
        .rounded(px(6.0))
        .border(px(1.0))
        .border_color(if active {
            hsla(colors.ring, 1.0)
        } else {
            gpui_kit::transparent_black()
        })
        .when(active, |row| row.bg(hsla(colors.primary, 0.1)))
        .cursor_pointer()
        .role(Role::ListBoxOption)
        .aria_selected(active)
        .on_mouse_move(move_listener)
        .on_click(open_listener)
        .child(icon(
            SQUARE_TERMINAL,
            hsla(colors.muted_foreground, 1.0),
            16.0,
        ))
        .child(details)
}
