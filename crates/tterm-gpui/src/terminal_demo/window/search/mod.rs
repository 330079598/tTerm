//! The per-pane terminal search bar and its background scan.
//!
//! One bar belongs to one [`WindowTab`](super::WindowTab) and controls one
//! [`TerminalView`](tterm_terminal_view::view::TerminalView). It reuses the model's
//! retained-history scan, keeps at most one pending task, and installs only a
//! result whose request, query and content version are still current. The bar
//! never writes to the PTY: while it is open its input owns the keyboard, and
//! Escape returns focus to the terminal.
//!
//! `bar` paints the controls, `controller` owns the scan lifecycle and
//! navigation, and `tests` drives the production view and bar with real key
//! and mouse events.

mod bar;
#[cfg(all(debug_assertions, feature = "automation"))]
pub(crate) mod capture;
mod controller;

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::InputState;
use gpui_kit::{
    App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement, Render, Styled, Subscription, Task, WeakEntity, Window, div,
};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::{ArgumentValue, TranslationRequest};
use tterm_terminal::search::{SearchCancellation, SearchOptions, SearchPosition, SearchResult};
use tterm_terminal::terminal::Terminal;
use tterm_terminal_view::view::TerminalView;
use tterm_terminal_view::view::search::SearchStaleListener;
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::theme::Theme;

use super::Prototype;
use crate::locale::Locale;

/// How long typing settles before a scan starts.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);
/// How long output settles before an invalidated result is scanned again.
const OUTPUT_RESCAN_DEBOUNCE: Duration = Duration::from_millis(160);

/// The debug selector of the bar and its controls.
pub(super) const BAR_SELECTOR: &str = "terminal-search-bar";
pub(super) const INPUT_SELECTOR: &str = "terminal-search-input";
pub(super) const COUNT_SELECTOR: &str = "terminal-search-count";
pub(super) const CASE_OPTION_SELECTOR: &str = "terminal-search-case";
pub(super) const WORD_OPTION_SELECTOR: &str = "terminal-search-word";
pub(super) const REGEX_OPTION_SELECTOR: &str = "terminal-search-regex";
pub(super) const PREVIOUS_SELECTOR: &str = "terminal-search-previous";
pub(super) const NEXT_SELECTOR: &str = "terminal-search-next";
pub(super) const CLOSE_SELECTOR: &str = "terminal-search-close";

/// What the bar currently shows in its result slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SearchStatus {
    /// The query is empty.
    Idle,
    /// A scan is pending or running.
    Waiting,
    /// A completed scan found no match.
    Empty,
    /// A completed scan's matches are installed.
    Ready,
    /// The query could not be compiled.
    Error(String),
}

/// The per-tab search bar entity.
pub(super) struct SearchBar {
    model: Arc<Terminal>,
    view: WeakEntity<TerminalView>,
    locale: Entity<Locale>,
    theme: Theme,
    input: Entity<InputState>,
    query: String,
    options: SearchOptions,
    status: SearchStatus,
    result: Option<Arc<SearchResult>>,
    active: Option<usize>,
    /// The start of the active match, kept across a rescan.
    active_start: Option<SearchPosition>,
    open: bool,
    request: u64,
    cancellation: Option<SearchCancellation>,
    task: Option<Task<()>>,
    _input_subscription: Subscription,
}

impl SearchBar {
    /// Builds a closed bar for one pane, with the current placeholder.
    pub(super) fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        model: Arc<Terminal>,
        view: WeakEntity<TerminalView>,
        locale: Entity<Locale>,
        theme: Theme,
    ) -> Self {
        let placeholder = placeholder_text(locale.read(cx));
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let subscription = cx.subscribe(&input, Self::on_input_event);
        cx.observe(&locale, |_, _, cx| cx.notify()).detach();
        Self {
            model,
            view,
            locale,
            theme,
            input,
            query: String::new(),
            options: SearchOptions::default(),
            status: SearchStatus::Idle,
            result: None,
            active: None,
            active_start: None,
            open: false,
            request: 0,
            cancellation: None,
            task: None,
            _input_subscription: subscription,
        }
    }

    /// Whether the bar is visible.
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the bar, clears the old query and focuses the input.
    pub(super) fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.cancel_scan();
        self.query.clear();
        self.status = SearchStatus::Idle;
        self.result = None;
        self.active = None;
        self.active_start = None;
        let placeholder = placeholder_text(self.locale.read(cx));
        let input = self.input.clone();
        input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
            input.set_value("", window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        let listener = self.stale_listener(cx);
        let view = self.view.clone();
        let _ = view.update(cx, |view, cx| {
            view.clear_search_overlay(cx);
            view.set_search_stale_listener(Some(listener), cx);
        });
        cx.notify();
    }

    /// Closes the bar. `restore_focus` returns the keyboard to the terminal.
    pub(super) fn close(
        &mut self,
        window: &mut Window,
        restore_focus: bool,
        cx: &mut Context<Self>,
    ) {
        self.open = false;
        self.cancel_scan();
        self.query.clear();
        self.status = SearchStatus::Idle;
        self.result = None;
        self.active = None;
        self.active_start = None;
        let input = self.input.clone();
        input.update(cx, |input, cx| input.set_value("", window, cx));
        let view = self.view.clone();
        let _ = view.update(cx, |view, cx| {
            view.clear_search_overlay(cx);
            view.set_search_stale_listener(None, cx);
            if restore_focus {
                view.focus_handle().focus(window, cx);
            }
        });
        cx.notify();
    }

    /// Drops the accepted result and its highlights, keeping the query.
    fn clear_result(&mut self, cx: &mut Context<Self>) {
        self.result = None;
        self.active = None;
        self.active_start = None;
        self.clear_overlay(cx);
    }

    fn clear_overlay(&mut self, cx: &mut Context<Self>) {
        let view = self.view.clone();
        let _ = view.update(cx, |view, cx| view.clear_search_overlay(cx));
    }

    fn install_overlay(&mut self, cx: &mut Context<Self>) {
        let Some(result) = self.result.clone() else {
            self.clear_overlay(cx);
            return;
        };
        let overlay = tterm_terminal_view::view::search::SearchOverlay::new(result, self.active);
        let view = self.view.clone();
        let _ = view.update(cx, |view, cx| view.set_search_overlay(overlay, cx));
    }

    fn reveal_active(&mut self, cx: &mut Context<Self>) {
        let Some(line) = self
            .active
            .and_then(|index| self.result.as_ref()?.matches().get(index))
            .map(|matched| matched.start.line)
        else {
            return;
        };
        let view = self.view.clone();
        let _ = view.update(cx, |view, cx| view.reveal_search_line(line, cx));
    }

    fn stale_listener(&self, cx: &mut Context<Self>) -> SearchStaleListener {
        let bar = cx.entity().downgrade();
        Rc::new(move |cx: &mut App| {
            let _ = bar.update(cx, |bar, cx| bar.on_overlay_stale(cx));
        })
    }

    fn cancel_scan(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.task.take();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.prefer_character_input {
            return;
        }
        if event.keystroke.key == "escape" {
            window.prevent_default();
            cx.stop_propagation();
            self.close(window, true, cx);
        }
    }

    fn count_text(&self, cx: &App) -> String {
        let locale = self.locale.read(cx);
        match &self.status {
            SearchStatus::Idle => text(locale, keys::TERMINAL_SEARCH_READY),
            SearchStatus::Waiting => "…".to_string(),
            SearchStatus::Empty => text(locale, keys::TERMINAL_SEARCH_NO_RESULTS),
            SearchStatus::Error(_) => text(locale, keys::TERMINAL_SEARCH_INVALID_REGEX),
            SearchStatus::Ready => {
                let total = self
                    .result
                    .as_ref()
                    .map_or(0, |result| result.match_count());
                if total == 0 {
                    return text(locale, keys::TERMINAL_SEARCH_NO_RESULTS);
                }
                let current = self.active.map_or(0, |index| index + 1).to_string();
                let total = total.to_string();
                let arguments = [
                    ("current", ArgumentValue::Text(&current)),
                    ("total", ArgumentValue::Text(&total)),
                ];
                let request =
                    TranslationRequest::new(keys::TERMINAL_SEARCH_RESULTS).arguments(&arguments);
                locale.translate(request).unwrap_or_else(|error| {
                    eprintln!("tterm-gpui: failed to translate the search count: {error}");
                    format!("{current} / {total}")
                })
            }
        }
    }
}

fn text(locale: &Locale, key: tterm_core::i18n::translate::TranslationKey) -> String {
    locale.text(key).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate a search label: {error}");
        key.as_str().to_string()
    })
}

fn placeholder_text(locale: &Locale) -> String {
    text(locale, keys::TERMINAL_SEARCH_PLACEHOLDER)
}

impl Render for SearchBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let root = div()
            .id(BAR_SELECTOR)
            .debug_selector(|| BAR_SELECTOR.to_string())
            .absolute()
            .top(gpui_kit::px(12.0))
            .right(gpui_kit::px(14.0))
            .flex()
            .items_center()
            .gap(gpui_kit::px(6.0))
            .w(gpui_kit::px(540.0))
            .max_w(gpui_kit::relative(0.95))
            .flex_wrap()
            .p(gpui_kit::px(7.0))
            .rounded(gpui_kit::px(12.0))
            .border(gpui_kit::px(1.0))
            .border_color(tterm_ui::tokens::gpui::hsla(self.theme.colors.border, 1.0))
            .bg(tterm_ui::tokens::gpui::hsla(
                self.theme.colors.background,
                0.92,
            ))
            .shadow(vec![
                gpui_kit::BoxShadow::new(
                    gpui_kit::px(0.0),
                    gpui_kit::px(12.0),
                    tterm_ui::tokens::gpui::hsla(self.theme.colors.background, 0.28),
                )
                .blur_radius(gpui_kit::px(16.0)),
            ])
            .text_color(tterm_ui::tokens::gpui::hsla(
                self.theme.colors.foreground,
                1.0,
            ))
            .occlude()
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .id(INPUT_SELECTOR)
                    .debug_selector(|| INPUT_SELECTOR.to_string())
                    .w(gpui_kit::px(220.0))
                    .min_w(gpui_kit::px(180.0))
                    .flex_1()
                    .child(input(
                        &self.input,
                        InputOptions {
                            theme: Some(self.theme),
                            focused: self.input.read(cx).focus_handle(cx).is_focused(window),
                            height: 28.0,
                            ..InputOptions::default()
                        },
                    )),
            )
            .child(div().debug_selector(|| "probe-root".to_string()));
        self.render_controls(window, cx, root).into_any_element()
    }
}

#[cfg(test)]
impl SearchBar {
    /// The current result status.
    pub(super) fn status(&self) -> &SearchStatus {
        &self.status
    }

    /// The accepted matches.
    pub(super) fn result(&self) -> Option<&Arc<SearchResult>> {
        self.result.as_ref()
    }

    /// The active match index.
    pub(super) fn active_index(&self) -> Option<usize> {
        self.active
    }

    /// The match options in effect.
    pub(super) fn options(&self) -> SearchOptions {
        self.options
    }

    /// The request counter, so a test can deliver an older completion.
    pub(super) fn request_id(&self) -> u64 {
        self.request
    }

    /// A clone of the current cancellation token, for interleaving a scan.
    pub(super) fn current_cancellation(&self) -> Option<SearchCancellation> {
        self.cancellation.clone()
    }
}

impl Prototype {
    /// Opens the search bar for the active pane, or refocuses its input.
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.tabs.active_id();
        let existing = self.tabs.get(id).and_then(|tab| tab.search_bar.clone());
        let bar = match existing {
            Some(bar) => bar,
            None => {
                let Some(tab) = self.tabs.get(id) else {
                    return;
                };
                let model = tab.view.read(cx).model();
                let view = tab.view.downgrade();
                let locale = self.ui.locale.clone();
                let theme = self.ui.theme;
                let bar = cx.new(|cx| SearchBar::new(cx, window, model, view, locale, theme));
                if let Some(tab) = self.tabs.get_mut(id) {
                    tab.search_bar = Some(bar.clone());
                }
                bar
            }
        };
        bar.update(cx, |bar, cx| bar.open(window, cx));
        cx.notify();
    }

    /// Closes the active pane's search and reports whether one was open.
    pub(super) fn close_active_search(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let id = self.tabs.active_id();
        let Some(bar) = self.tabs.get(id).and_then(|tab| tab.search_bar.clone()) else {
            return false;
        };
        if !bar.read(cx).is_open() {
            return false;
        }
        bar.update(cx, |bar, cx| bar.close(window, true, cx));
        cx.notify();
        true
    }

    /// Closes every search bar outside the pane that now owns the keyboard.
    ///
    /// A bar belongs to one pane. Switching panes, splitting, closing or
    /// reordering must not leave another pane's matches painted or its count
    /// visible, so every workspace transition funnels through this check.
    pub(super) fn close_search_in_other_panes(
        &self,
        active: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (id, tab) in self.tabs.ordered() {
            if id == active {
                continue;
            }
            let Some(bar) = tab.search_bar.clone() else {
                continue;
            };
            if bar.read(cx).is_open() {
                bar.update(cx, |bar, cx| bar.close(window, false, cx));
            }
        }
    }
}
