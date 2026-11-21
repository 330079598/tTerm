//! Native workspace actions share one focus transition for the mouse and keymap.

use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, KeyDownEvent, Window, div, px};
use tterm_core::i18n::keys;
use tterm_core::keymap::actions::KeymapActionId;
use tterm_ui::tokens::gpui::hsla;

use super::{CommandSpec, KeymapRouter, Prototype, TitleUpdate, WindowTab, classify_title_update};

pub(super) fn register_actions(router: &KeymapRouter) {
    use KeymapActionId::{TabsCloseActive, TabsNext, TabsPrev, WorkspaceNewTab};
    for action in [WorkspaceNewTab, TabsNext, TabsPrev, TabsCloseActive] {
        router.register(action, Box::new(|_, _| true));
    }
}

impl WindowTab {
    fn leave(&self, cx: &mut Context<Prototype>) {
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
    pub(super) fn select_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if id != self.tabs.active_id() {
            self.tabs.active().leave(cx);
            self.tabs.select(id);
        }
        self.tabs.active().enter(window, cx);
    }

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.active().leave(cx);
        self.tabs.push(|id| {
            self.factory
                .open(id, CommandSpec::Shell, &self.ui.locale, cx)
        });
        self.tabs.active().enter(window, cx);
    }

    fn close_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.neighbour(true).is_none() {
            return;
        }
        let was_active = id == self.tabs.active_id();
        if was_active {
            self.tabs.active().leave(cx);
        }
        if let Some(closed) = self.tabs.close(id)
            && let Some(binding) = closed.binding
            && let Err(error) = self.factory.sessions.close_tab(binding.tab_id())
        {
            eprintln!("Failed to close terminal tab: {error}");
        }
        if was_active {
            self.tabs.active().enter(window, cx);
        } else {
            cx.notify();
        }
    }

    pub(super) fn workspace_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
            Some(KeymapActionId::WorkspaceNewTab) => self.new_tab(window, cx),
            Some(KeymapActionId::TabsCloseActive) => {
                self.close_tab(self.tabs.active_id(), window, cx)
            }
            Some(action @ (KeymapActionId::TabsNext | KeymapActionId::TabsPrev)) => {
                if let Some(id) = self.tabs.neighbour(action == KeymapActionId::TabsNext) {
                    self.select_tab(id, window, cx);
                }
            }
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
    }

    pub(super) fn tab_strip(&self, cx: &mut Context<Self>) -> Div {
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
        let index = tabs.partition_point(|(id, _)| *id < self.tabs.active_id());
        let closable = tabs.len() > 1;
        for (index, (id, tab)) in tabs.into_iter().enumerate() {
            let pending = tab.title.lock().ok().and_then(|mut title| title.take());
            if let Some(update) = classify_title_update(pending) {
                *tab.program_title.borrow_mut() = match update {
                    TitleUpdate::Program(title) => Some(title),
                    TitleUpdate::Default => None,
                };
            }
            let label = tab.display_title(self.ui.locale.read(cx));
            let active = self.tabs.active_id() == id;
            let group = format!("session-tab-{id}");
            let mut item = div()
                .id(("session-tab", id))
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
                    item.child(
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
                        .child(label),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select_tab(id, window, cx)));
            if closable {
                item = item.child(
                    div()
                        .id(("close-session-tab", id))
                        .aria_label(close.clone())
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
                        .hover(|style| style.bg(hsla(colors.muted, 0.82)))
                        .child("×")
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(id, window, cx);
                        })),
                );
            }
            strip = strip.child(item);
        }
        let viewport = self
            .overflow
            .wrap(strip, self.tabs.active_id(), index, &self.ui.locale, cx);
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
}
