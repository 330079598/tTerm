//! Native workspace actions share one focus transition for the mouse and keymap.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, KeyDownEvent, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::keymap::actions::KeymapActionId;
use tterm_ui::tokens::gpui::hsla;

use super::split_state::{Edge, FOCUS_ACTIONS};
use super::{
    CommandSpec, KeymapRouter, Prototype, TabLaunch, TitleUpdate, WindowTab, classify_title_update,
};

pub(super) fn register_actions(router: &KeymapRouter) {
    use KeymapActionId::{
        SettingsOpen, TabsCloseActive, TabsNext, TabsPrev, WorkspaceNewTab,
        WorkspaceRestoreSingleGroup, WorkspaceSplitBelow, WorkspaceSplitRight,
        WorkspaceTogglePaneZoom,
    };
    for action in [
        WorkspaceNewTab,
        TabsNext,
        TabsPrev,
        TabsCloseActive,
        WorkspaceSplitRight,
        WorkspaceSplitBelow,
        WorkspaceRestoreSingleGroup,
        WorkspaceTogglePaneZoom,
        SettingsOpen,
    ]
    .into_iter()
    .chain(FOCUS_ACTIONS)
    {
        router.register(action, Box::new(|_, _| true));
    }
}

impl WindowTab {
    pub(super) fn live_default_shell(&self) -> bool {
        self.default_shell
            && self
                .binding
                .as_ref()
                .and_then(|binding| binding.session())
                .is_some_and(|session| session.is_live())
    }

    pub(super) fn leave(&self, cx: &mut Context<Prototype>) {
        self.view.update(cx, |view, cx| view.deactivate(cx));
        if let Some(binding) = &self.binding {
            binding.leave();
        }
    }

    fn enter(&self, window: &mut Window, cx: &mut Context<Prototype>) {
        if let Some(binding) = &self.binding {
            binding.enter();
        }
        self.view.update(cx, |view, cx| view.activate(window, cx));
        cx.notify();
    }
}

impl Prototype {
    pub(super) fn visible_ids(&self) -> Vec<usize> {
        self.splits.visible_ids(self.tabs.active_id())
    }

    pub(super) fn refresh_panes(
        &self,
        previous: Vec<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let visible = self.visible_ids();
        for id in &previous {
            if !visible.contains(id)
                && let Some(tab) = self.tabs.get(*id)
            {
                tab.leave(cx);
            }
        }
        for id in visible {
            if id != self.tabs.active_id()
                && !previous.contains(&id)
                && let Some(tab) = self.tabs.get(id)
            {
                if let Some(binding) = &tab.binding {
                    binding.enter();
                }
                tab.view.update(cx, |view, cx| view.set_visible(true, cx));
            }
        }
        self.tabs.active().enter(window, cx);
    }

    pub(super) fn focus_pane(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if id != self.tabs.active_id() && self.visible_ids().contains(&id) {
            self.select_tab(id, window, cx);
        }
    }

    pub(super) fn split_tab(
        &mut self,
        source: usize,
        target: usize,
        edge: super::split_state::Edge,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.visible_ids();
        if !previous.contains(&target)
            || self.tabs.get(source).is_none()
            || !self.splits.can_split(source, target)
            || !self.splits.split(source, target, edge)
        {
            return;
        }
        self.tabs.select(source);
        self.refresh_panes(previous, window, cx);
        self.mark_workspace_dirty(cx);
    }

    pub(super) fn select_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        if !self.close_settings(window, cx) {
            return;
        }
        let previous = self.visible_ids();
        if self.tabs.get(id).is_none() {
            return;
        }
        let changed = self.tabs.select(id);
        self.splits.focus(id);
        self.refresh_panes(previous, window, cx);
        if changed {
            self.mark_workspace_dirty(cx);
        }
    }

    pub(super) fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tunnel_page_is_open() {
            self.close_tunnel_page(window, cx);
        }
        if !self.close_settings(window, cx) {
            return;
        }
        let previous = self.visible_ids();
        let saved_id = self.next_saved_id();
        self.tabs.push(|id| {
            self.factory.open_launch(
                id,
                TabLaunch::command(saved_id, CommandSpec::Shell),
                &self.ui.locale,
                cx,
            )
        });
        self.splits.open(self.tabs.active_id());
        self.refresh_panes(previous, window, cx);
        self.mark_workspace_dirty(cx);
    }

    fn split_shell(&mut self, edge: Edge, window: &mut Window, cx: &mut Context<Self>) {
        let original = self.tabs.active_id();
        if !self.tabs.active().live_default_shell() || self.splits.is_split(original) {
            self.nudge_active_tab(cx);
            return;
        }
        let previous = self.visible_ids();
        let saved_id = self.next_saved_id();
        self.tabs.push(|id| {
            self.factory.open_launch(
                id,
                TabLaunch::command(saved_id, CommandSpec::Shell),
                &self.ui.locale,
                cx,
            )
        });
        let new = self.tabs.active_id();
        if new == original {
            return;
        }
        self.splits.open(new);
        if self.tabs.active().live_default_shell() && self.splits.split(new, original, edge) {
            self.refresh_panes(previous, window, cx);
            self.mark_workspace_dirty(cx);
        } else {
            self.splits.close(new);
            self.release_tab(new);
            self.tabs.select(original);
            self.nudge_active_tab(cx);
        }
    }

    fn restore_single_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.visible_ids();
        if self.splits.ungroup(self.tabs.active_id()) {
            self.refresh_panes(previous, window, cx);
            self.mark_workspace_dirty(cx);
        } else {
            self.nudge_active_tab(cx);
        }
    }

    fn toggle_pane_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous = self.visible_ids();
        if self.splits.toggle_zoom(self.tabs.active_id()) {
            self.refresh_panes(previous, window, cx);
            self.mark_workspace_dirty(cx);
        } else {
            self.nudge_active_tab(cx);
        }
    }

    pub(super) fn close_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.close_settings(window, cx) {
            return;
        }
        if self.tabs.neighbour(true).is_none() {
            return;
        }
        let previous = self.visible_ids();
        let was_active = id == self.tabs.active_id();
        let peer = self.splits.close(id);
        self.release_tab(id);
        if was_active {
            if let Some(peer) = peer {
                self.tabs.select(peer);
                self.splits.focus(self.tabs.active_id());
            }
            self.refresh_panes(previous, window, cx);
        } else {
            cx.notify();
        }
        self.mark_workspace_dirty(cx);
    }

    fn release_tab(&mut self, id: usize) {
        if let Some(closed) = self.tabs.close(id)
            && let Some(binding) = closed.binding
            && let Err(error) = self.factory.sessions.close_tab(binding.tab_id())
        {
            eprintln!("Failed to close terminal tab: {error}");
        }
    }

    pub(super) fn workspace_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tunnel_page_is_open() {
            if event.keystroke.key == "escape" {
                self.close_tunnel_page(window, cx);
                window.prevent_default();
                cx.stop_propagation();
            } else if self.factory.keymap.dispatch(event) == Some(KeymapActionId::SettingsOpen) {
                self.close_tunnel_page(window, cx);
                self.open_settings(window, cx);
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if self.settings_open {
            if event.keystroke.key == "escape" {
                let cleared = self
                    .ui
                    .settings_page
                    .update(cx, |page, cx| page.clear_search(window, cx));
                if cleared || self.close_settings(window, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            } else if self.factory.keymap.dispatch(event).is_some() {
                window.prevent_default();
                cx.stop_propagation();
            }
            return;
        }
        if event.keystroke.key == "escape" && cx.stop_active_drag(window) {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if !self
            .tabs
            .active()
            .view
            .read(cx)
            .focus_handle()
            .is_focused(window)
        {
            return;
        }
        match self.factory.keymap.dispatch(event) {
            Some(KeymapActionId::WorkspaceTogglePaneZoom) => self.toggle_pane_zoom(window, cx),
            Some(KeymapActionId::WorkspaceNewTab) => self.new_tab(window, cx),
            Some(KeymapActionId::WorkspaceSplitRight) => self.split_shell(Edge::Right, window, cx),
            Some(KeymapActionId::WorkspaceSplitBelow) => self.split_shell(Edge::Bottom, window, cx),
            Some(KeymapActionId::WorkspaceRestoreSingleGroup) => {
                self.restore_single_group(window, cx)
            }
            Some(KeymapActionId::WorkspaceChooseLayout) => self.open_layout_templates(window, cx),
            Some(KeymapActionId::SettingsOpen) => self.open_settings(window, cx),
            Some(KeymapActionId::TabsCloseActive) => {
                self.close_tab(self.tabs.active_id(), window, cx)
            }
            Some(action @ (KeymapActionId::TabsNext | KeymapActionId::TabsPrev)) => {
                if let Some(id) = self.tabs.neighbour(action == KeymapActionId::TabsNext) {
                    self.select_tab(id, window, cx);
                }
            }
            Some(action) if FOCUS_ACTIONS.contains(&action) => {
                let active = self.tabs.active_id();
                if let Some(id) = self.splits.focus_target(active, action) {
                    self.focus_pane(id, window, cx);
                } else {
                    self.nudge_active_tab(cx);
                }
            }
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    fn advance_drag(&mut self, window: &Window, cx: &Context<Self>) {
        if self.drag_layout.gesture.get().is_some() {
            if cx.has_active_drag() {
                super::drag::advance_scroll(&self.overflow.handle, window);
            } else {
                self.drag_layout.gesture.set(None);
                self.drag_layout.motion = Default::default();
            }
        }
    }

    pub(super) fn tab_strip(&mut self, window: &Window, cx: &mut Context<Self>) -> Div {
        self.advance_drag(window, cx);
        let placement = self.placement(window);
        let now = std::time::Instant::now();
        let mut flash = self.tab_flash(window, now);
        let colors = self.ui.theme.colors;
        let mut strip = div()
            .flex()
            .items_center()
            .gap(px(4.))
            .text_sm()
            .text_color(hsla(colors.foreground, 1.));
        let config = self.factory.config.read(cx).config();
        let width = (config.tab_width_mode == "standard").then_some(config.tab_standard_width);
        let icons = config.tab_icons.enabled;
        let locale = self.ui.locale.read(cx);
        let new = locale.text(keys::TAB_NEW).unwrap_or_else(|_| "+".into());
        let close = locale.text(keys::TAB_CLOSE).unwrap_or_else(|_| "×".into());
        let tabs = self.tabs.ordered();
        let index = self.tabs.active_index();
        let closable = tabs.len() > 1;
        let page_open = self.tunnel_page_is_open();
        for (index, (id, tab)) in tabs.enumerate() {
            let pending = tab.title.lock().ok().and_then(|mut title| title.take());
            if let Some(update) = classify_title_update(pending) {
                *tab.program_title.borrow_mut() = match update {
                    TitleUpdate::Program(title) => Some(title),
                    TitleUpdate::Default => None,
                };
            }
            let label = tab.display_title(self.ui.locale.read(cx));
            let active = !page_open && self.tabs.active_id() == id;
            let group = format!("session-tab-{id}");
            let mut item = div()
                .id(("session-tab", id))
                .debug_selector(move || format!("session-tab-{id}"))
                .group(group.clone())
                .relative()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .h(px(30.))
                .pl(px(9.))
                .pr(px(7.))
                .rounded(px(5.))
                .min_w(px(96.))
                .max_w(px(220.))
                .cursor_pointer()
                .bg(hsla(
                    if active {
                        colors.tab_active
                    } else {
                        colors.tab_background
                    },
                    1.,
                ))
                .when(!active, |item| {
                    item.hover(|style| style.bg(hsla(colors.tab_hover, 1.)))
                })
                .when_some(width, |item, width| {
                    let width = px(f32::from(width));
                    item.w(width).min_w(width).max_w(width)
                })
                .when(active, |item| {
                    item.children(flash.take()).child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(5.))
                            .right(px(5.))
                            .h(px(2.5))
                            .bg(hsla(colors.tab_active_border, 1.)),
                    )
                })
                .child(
                    div()
                        .min_w(px(12.))
                        .text_size(px(11.))
                        .opacity(if active { 0.72 } else { 0.55 })
                        .child((index + 1).to_string()),
                )
                .when(icons, |item| {
                    item.child(div().text_size(px(12.)).child("›_"))
                })
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_size(px(13.))
                        .child(label.clone()),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select_tab(id, window, cx)));
            if closable {
                item =
                    super::drag::reorderable(item, id, label, self.ui.theme, &self.drag_layout, cx);
                item = item.child(self.close_button(id, active, group, close.clone(), cx));
            }
            let shift = placement
                .as_ref()
                .map_or(0., |layout| layout.displacement(index));
            let offset = self.drag_layout.motion.offset(id, shift, now);
            let hidden = self
                .drag_layout
                .gesture
                .get()
                .is_some_and(|(dragged, _)| dragged == id);
            strip = strip.child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .child(item.left(px(offset)).when(hidden, |item| item.opacity(0.))),
            );
        }
        if page_open {
            strip = strip.child(self.tunnel_page_tab(cx));
        }
        let focus_index = if page_open { self.tabs.len() } else { index };
        let viewport = self.overflow.wrap(
            super::drag::drop_target(strip, cx),
            self.tabs.active_id(),
            focus_index,
            &self.ui.locale,
            cx,
        );
        div()
            .flex()
            .min_w_0()
            .items_center()
            .text_color(hsla(colors.foreground, 1.))
            .child(viewport)
            .child(
                div()
                    .id("new-session-tab")
                    .aria_label(new)
                    .size(px(28.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(4.))
                    .hover(|style| style.bg(hsla(colors.muted, 1.)))
                    .cursor_pointer()
                    .child("+")
                    .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
            )
    }

    fn close_button(
        &self,
        id: usize,
        active: bool,
        group: String,
        label: String,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let muted = self.ui.theme.colors.muted;
        div()
            .id(("close-session-tab", id))
            .debug_selector(move || format!("close-session-tab-{id}"))
            .aria_label(label)
            .size(px(18.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(3.))
            .when(!active, |button| {
                button
                    .opacity(0.)
                    .group_hover(group, |style| style.opacity(1.))
            })
            .hover(move |style| style.bg(hsla(muted, 0.82)))
            .child("×")
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                cx.stop_propagation();
                this.close_tab(id, window, cx);
            }))
    }
}
