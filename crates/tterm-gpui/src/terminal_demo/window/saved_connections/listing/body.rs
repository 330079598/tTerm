//! The picker's search field, order toggle, group tree and profile rows.

use gpui_kit::component::input::Input;
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, FontWeight, Role, Stateful, div, px};
use tterm_core::connections::profile_groups::GroupNode;
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationKey, TranslationRequest};
use tterm_ui::components::icons::{CHEVRON_DOWN, CHEVRON_RIGHT, SEARCH, icon};
use tterm_ui::tokens::gpui::hsla;

use super::super::view::status_line;
use super::super::{
    EMPTY_ID, EMPTY_NEW_BUTTON_ID, ROW_EDIT_PREFIX, ROW_ID_PREFIX, SavedConnectionsUi,
};
use super::{
    GROUP_ID_PREFIX, ProfileSort, SEARCH_EMPTY_ID, SEARCH_INPUT_ID, SORT_GROUP_ID, SORT_RECENT_ID,
    UNGROUPED_ID, UNGROUPED_KEY,
};

impl SavedConnectionsUi {
    /// The search field and the By group / Recently used toggle.
    pub(in super::super) fn search_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(div().min_w_0().flex_1().child(self.search_field(cx)))
            .child(self.sort_toggle(cx))
    }

    fn search_field(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let Some(state) = &self.search else {
            return div().id(SEARCH_INPUT_ID);
        };
        let colors = self.theme.colors;
        let label = self.text(cx, keys::CONNECTION_SEARCH);
        div()
            .id(SEARCH_INPUT_ID)
            .debug_selector(|| SEARCH_INPUT_ID.to_string())
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(34.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .border(px(1.0))
            .border_color(hsla(colors.input, 1.0))
            .bg(hsla(colors.input, 0.3))
            .child(icon(SEARCH, hsla(colors.muted_foreground, 1.0), 14.0))
            .child(
                Input::new(state)
                    .w_full()
                    .aria_label(label)
                    .appearance(false)
                    .bordered(false)
                    .focus_bordered(false)
                    .text_color(hsla(colors.foreground, 1.0)),
            )
    }

    fn sort_toggle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, keys::PROFILES_SORT_LABEL);
        div()
            .id("saved-connections-sort")
            .flex()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded(px(6.0))
            .bg(hsla(colors.muted, 0.55))
            .role(Role::Group)
            .aria_label(label)
            .child(self.sort_option(
                SORT_GROUP_ID,
                keys::PROFILES_SORT_GROUP,
                ProfileSort::Group,
                cx,
            ))
            .child(self.sort_option(
                SORT_RECENT_ID,
                keys::PROFILES_SORT_RECENT,
                ProfileSort::Recent,
                cx,
            ))
    }

    fn sort_option(
        &self,
        id: &'static str,
        label_key: TranslationKey,
        sort: ProfileSort,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let label = self.text(cx, label_key);
        let aria_label = label.clone();
        let selected = self.listing.sort() == sort;
        let focus = self.navigation.sort_focus(id, cx);
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .role(Role::Button)
            .aria_label(aria_label)
            .aria_selected(selected)
            .track_focus(&focus)
            .tab_stop(true)
            .focus(|style| style.bg(hsla(colors.accent, 0.8)))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .text_size(px(11.0))
            .font_weight(FontWeight(500.0))
            .whitespace_nowrap()
            .cursor_pointer()
            .when(selected, |option| {
                option
                    .bg(hsla(colors.background, 1.0))
                    .text_color(hsla(colors.foreground, 1.0))
            })
            .when(!selected, |option| {
                option
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .hover(|style| style.text_color(hsla(colors.foreground, 1.0)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.choose_sort(sort, cx)))
            .on_key_down(
                cx.listener(move |this, event: &gpui_kit::KeyDownEvent, _, cx| {
                    if !matches!(event.keystroke.key.as_str(), "enter" | "space") {
                        return;
                    }
                    this.choose_sort(sort, cx);
                    cx.stop_propagation();
                }),
            )
            .child(label)
    }

    /// Everything below the search controls: rows, groups, or an empty state.
    pub(in super::super) fn listing_content(&self, cx: &mut Context<Self>) -> Div {
        if self.profiles.is_empty() {
            return self.empty_library(cx);
        }
        if self.listing.is_filtering() && self.listing.visible_is_empty() {
            return self.search_empty(cx);
        }
        if self.listing.is_flat() {
            return self.flat_list(cx);
        }
        self.grouped_list(cx)
    }

    fn empty_library(&self, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        let new_button = self.new_connection_button(EMPTY_NEW_BUTTON_ID, true, cx);
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.0))
            .py(px(12.0))
            .child(status_line(
                EMPTY_ID,
                self.text(cx, keys::SAVED_CONNECTIONS_EMPTY),
                colors.muted_foreground,
                Role::Status,
            ))
            .child(new_button)
    }

    fn search_empty(&self, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .py(px(18.0))
            .text_center()
            .child(status_line(
                SEARCH_EMPTY_ID,
                self.text(cx, keys::PROFILES_SEARCH_EMPTY),
                colors.muted_foreground,
                Role::Status,
            ))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::PROFILES_SEARCH_EMPTY_DESCRIPTION)),
            )
    }

    fn flat_list(&self, cx: &mut Context<Self>) -> Div {
        let aria_label = self.text(cx, keys::SAVED_CONNECTIONS_TITLE);
        let mut list = div()
            .id("saved-connections-flat-list")
            .flex()
            .flex_col()
            .gap(px(4.0))
            .role(Role::ListBox)
            .aria_label(aria_label);
        for (index, id) in self.listing.visible_ids().iter().enumerate() {
            if let Some(row) = self.listing_row(id, index, true, cx) {
                list = list.child(row);
            }
        }
        div().child(list)
    }

    fn grouped_list(&self, cx: &mut Context<Self>) -> Div {
        let mut column = div().flex().flex_col().gap(px(10.0));
        for node in self.listing.roots().iter().filter(|node| node.total > 0) {
            column = column.child(self.group_section(node, cx));
        }
        if !self.listing.ungrouped().is_empty() {
            column = column.child(self.ungrouped_section(cx));
        }
        column
    }

    /// A group heading followed by its nested groups and then its own rows.
    fn group_section(&self, node: &GroupNode, cx: &mut Context<Self>) -> Div {
        let colors = self.theme.colors;
        let key = node.group.id.clone();
        let collapsed = self.listing.is_collapsed(&key);
        let selector = format!("{GROUP_ID_PREFIX}{key}");
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(self.group_header(
                selector,
                &key,
                node.group.name.clone(),
                node.total,
                collapsed,
                cx,
            ));
        if collapsed {
            return section;
        }

        let children: Vec<&GroupNode> = node
            .children
            .iter()
            .filter(|child| child.total > 0)
            .collect();
        if !children.is_empty() {
            let mut nested = div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .ml(px(4.0))
                .pl(px(8.0))
                .border_l(px(1.0))
                .border_color(hsla(colors.border, 0.7));
            for child in children {
                nested = nested.child(self.group_section(child, cx));
            }
            section = section.child(nested);
        }

        if !node.profiles.is_empty() {
            let mut rows = div()
                .id(format!("saved-connections-group-rows-{}", node.group.id))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .role(Role::ListBox)
                .aria_label(node.path.clone());
            for profile in &node.profiles {
                if let Some(index) = self.listing.row_index(&profile.id)
                    && let Some(row) = self.listing_row(&profile.id, index, false, cx)
                {
                    rows = rows.child(row);
                }
            }
            section = section.child(rows);
        }
        section
    }

    fn ungrouped_section(&self, cx: &mut Context<Self>) -> Div {
        let collapsed = self.listing.is_collapsed(UNGROUPED_KEY);
        let label = self.text(cx, keys::PROFILES_UNGROUPED);
        let count = self.listing.ungrouped().len();
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(self.group_header(
                UNGROUPED_ID.to_string(),
                UNGROUPED_KEY,
                label.clone(),
                count,
                collapsed,
                cx,
            ));
        if collapsed {
            return section;
        }
        let mut rows = div()
            .id("saved-connections-ungrouped-rows")
            .flex()
            .flex_col()
            .gap(px(4.0))
            .role(Role::ListBox)
            .aria_label(label);
        for profile in self.listing.ungrouped() {
            if let Some(index) = self.listing.row_index(&profile.id)
                && let Some(row) = self.listing_row(&profile.id, index, false, cx)
            {
                rows = rows.child(row);
            }
        }
        section = section.child(rows);
        section
    }

    fn group_header(
        &self,
        selector: String,
        key: &str,
        label: String,
        count: usize,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let debug_selector = selector.clone();
        let aria_label = self.group_action_label(&label, collapsed, cx);
        let icon_data = if collapsed {
            CHEVRON_RIGHT
        } else {
            CHEVRON_DOWN
        };
        let focus = self.group_focus.get(key);
        let key = key.to_string();
        let toggle_key = key.clone();
        let mut header = div()
            .id(selector)
            .debug_selector(move || debug_selector.clone())
            .role(Role::Button)
            .aria_label(aria_label)
            .aria_expanded(!collapsed)
            .flex()
            .items_center()
            .gap(px(6.0))
            .w_full()
            .px(px(4.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(hsla(colors.muted, 0.35)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.listing.toggle_group(&key) {
                    this.settle_selection(cx);
                    cx.notify();
                }
            }))
            .child(icon(icon_data, hsla(colors.muted_foreground, 1.0), 14.0))
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_size(px(12.0))
                    .font_weight(FontWeight(600.0))
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(count.to_string()),
            );
        if let Some(focus) = focus {
            header = header
                .track_focus(focus)
                .tab_stop(true)
                .focus(|style| style.bg(hsla(colors.accent, 0.8)))
                .on_key_down(
                    cx.listener(move |this, event: &gpui_kit::KeyDownEvent, _, cx| {
                        if !matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            return;
                        }
                        if this.listing.toggle_group(&toggle_key) {
                            this.settle_selection(cx);
                            cx.notify();
                        }
                        cx.stop_propagation();
                    }),
                );
        }
        header
    }

    fn group_action_label(&self, label: &str, collapsed: bool, cx: &Context<Self>) -> String {
        let key = if collapsed {
            keys::PROFILES_EXPAND_GROUP
        } else {
            keys::PROFILES_COLLAPSE_GROUP
        };
        let arguments = [("group", ArgumentValue::Text(label))];
        self.locale
            .read(cx)
            .translate(TranslationRequest::new(key).arguments(&arguments))
            .unwrap_or_else(|_| label.to_string())
    }

    /// One profile row; the index is its position in the visible order so the
    /// row selectors and keyboard navigation agree.
    fn listing_row(
        &self,
        id: &str,
        index: usize,
        show_group: bool,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let colors = self.theme.colors;
        let profile = self.listing.profile(id)?;
        let selector = format!("{ROW_ID_PREFIX}{index}");
        let selected = self.selected_id.as_deref() == Some(id);
        let select_label = self.select_label(&profile.summary().name, cx);
        let row_id = id.to_string();
        let measurement = self.navigation.row_measurement(id, &self.scroll_handle);
        let mut row = div()
            .id(("saved-connections-row", index))
            .debug_selector(move || selector)
            .role(Role::ListBoxOption)
            .aria_label(select_label)
            .aria_selected(selected)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(12.0))
            .py(px(9.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .when(selected, |row| row.bg(hsla(colors.accent, 1.0)))
            .when(!selected, |row| {
                row.hover(|style| style.bg(hsla(colors.accent, 0.65)))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_profile(&row_id, cx);
            }));
        let endpoint = profile.summary().endpoint();
        let subtitle = match (show_group, profile.group_path()) {
            (true, Some(group_path)) if !group_path.is_empty() => {
                format!("{group_path} · {endpoint}")
            }
            _ => endpoint,
        };
        let details = div()
            .min_w_0()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .child(profile.summary().name.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(subtitle),
            );
        row = row.child(details);
        if selected && !self.opening {
            let edit_selector = format!("{ROW_EDIT_PREFIX}{index}");
            row = row.child(self.row_edit_button(edit_selector, id, &profile.summary().name, cx));
        }
        Some(div().on_children_prepainted(measurement).child(row))
    }
}
