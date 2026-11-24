//! Scan scheduling, acceptance and keyboard navigation for the search bar.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::InputEvent;
use gpui_kit::{Context, Entity};
use tterm_terminal::search::{
    SearchCancellation, SearchDirection, SearchOutcome, SearchQuery, SearchResult,
};

use super::{OUTPUT_RESCAN_DEBOUNCE, SEARCH_DEBOUNCE, SearchBar, SearchStatus};

impl SearchBar {
    pub(super) fn on_input_event(
        &mut self,
        _input: Entity<gpui_kit::component::input::InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => {
                let value = self.input.read(cx).value().to_string();
                self.set_query(value, cx);
            }
            InputEvent::PressEnter { shift, .. } => self.run_enter(*shift, cx),
            InputEvent::Focus | InputEvent::Blur => {}
        }
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        if query == self.query {
            return;
        }
        self.query = query;
        self.clear_result(cx);
        self.status = if self.query.is_empty() {
            SearchStatus::Idle
        } else {
            SearchStatus::Waiting
        };
        self.schedule_scan(SEARCH_DEBOUNCE, cx);
    }

    pub(super) fn toggle_case_sensitive(&mut self, cx: &mut Context<Self>) {
        self.options.case_sensitive = !self.options.case_sensitive;
        self.restart_scan(cx);
    }

    pub(super) fn toggle_whole_word(&mut self, cx: &mut Context<Self>) {
        self.options.whole_word = !self.options.whole_word;
        self.restart_scan(cx);
    }

    pub(super) fn toggle_regex(&mut self, cx: &mut Context<Self>) {
        self.options.regex = !self.options.regex;
        self.restart_scan(cx);
    }

    fn restart_scan(&mut self, cx: &mut Context<Self>) {
        self.active_start = None;
        self.clear_result(cx);
        self.status = if self.query.is_empty() {
            SearchStatus::Idle
        } else {
            SearchStatus::Waiting
        };
        self.schedule_scan(Duration::ZERO, cx);
    }

    pub(super) fn navigate(&mut self, direction: SearchDirection, cx: &mut Context<Self>) {
        let Some(result) = self.result.clone() else {
            self.request_scan_if_possible(cx);
            return;
        };
        if result.content_version() != self.model.content_version() {
            self.clear_overlay(cx);
            self.on_overlay_stale(cx);
            return;
        }
        let count = result.match_count();
        if count == 0 {
            return;
        }
        let next = match (self.active, direction) {
            (Some(index), SearchDirection::Forward) => (index + 1) % count,
            (Some(index), SearchDirection::Backward) => (index + count - 1) % count,
            (None, SearchDirection::Forward) => 0,
            (None, SearchDirection::Backward) => count - 1,
        };
        let start = result.matches()[next].start;
        self.active = Some(next);
        self.active_start = Some(start);
        self.install_overlay(cx);
        self.reveal_active(cx);
        cx.notify();
    }

    fn run_enter(&mut self, shift: bool, cx: &mut Context<Self>) {
        let direction = if shift {
            SearchDirection::Backward
        } else {
            SearchDirection::Forward
        };
        if self
            .result
            .as_ref()
            .is_some_and(|result| !result.is_empty())
        {
            self.navigate(direction, cx);
        } else {
            self.request_scan_if_possible(cx);
        }
    }

    fn request_scan_if_possible(&mut self, cx: &mut Context<Self>) {
        if self.query.is_empty() || matches!(self.status, SearchStatus::Error(_)) {
            return;
        }
        self.schedule_scan(Duration::ZERO, cx);
    }

    /// Starts one debounced scan, cancelling the previous request.
    ///
    /// The request counter is bumped before the task starts, and the task
    /// checks it again when it delivers. A completion from a superseded query
    /// is dropped even if it arrives after the newer one.
    fn schedule_scan(&mut self, delay: Duration, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.request = self.request.wrapping_add(1);
        let request = self.request;
        self.cancel_scan();
        if self.query.is_empty() {
            self.status = SearchStatus::Idle;
            self.clear_result(cx);
            cx.notify();
            return;
        }
        let query = match SearchQuery::new(&self.query, self.options) {
            Ok(query) => query,
            Err(error) => {
                self.status = SearchStatus::Error(error.to_string());
                self.clear_result(cx);
                cx.notify();
                return;
            }
        };
        let cancellation = SearchCancellation::new();
        let task_cancellation = cancellation.clone();
        self.cancellation = Some(cancellation);
        self.status = SearchStatus::Waiting;
        let model = Arc::clone(&self.model);
        self.task = Some(cx.spawn(async move |bar, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            if task_cancellation.is_cancelled() {
                return;
            }
            let outcome = cx
                .background_executor()
                .spawn(async move { model.search_retained(&query, &task_cancellation) })
                .await;
            let _ = bar.update(cx, |bar, cx| bar.accept_outcome(request, outcome, cx));
        }));
        cx.notify();
    }

    /// Accepts only the newest request whose content version is still current.
    pub(super) fn accept_outcome(
        &mut self,
        request: u64,
        outcome: SearchOutcome,
        cx: &mut Context<Self>,
    ) {
        if request != self.request || !self.open {
            return;
        }
        match outcome {
            SearchOutcome::Cancelled => {}
            SearchOutcome::Stale => {
                self.result = None;
                self.active = None;
                self.clear_overlay(cx);
                self.status = SearchStatus::Waiting;
                self.schedule_scan(OUTPUT_RESCAN_DEBOUNCE, cx);
            }
            SearchOutcome::Complete(result) => {
                if result.content_version() != self.model.content_version() {
                    self.result = None;
                    self.active = None;
                    self.clear_overlay(cx);
                    self.status = SearchStatus::Waiting;
                    self.schedule_scan(OUTPUT_RESCAN_DEBOUNCE, cx);
                    return;
                }
                let result = Arc::new(result);
                let active = self.choose_active(&result, cx);
                self.status = if result.is_empty() {
                    SearchStatus::Empty
                } else {
                    SearchStatus::Ready
                };
                self.active = active;
                self.active_start = active
                    .and_then(|index| result.matches().get(index).map(|matched| matched.start));
                self.result = Some(result);
                self.install_overlay(cx);
                self.reveal_active(cx);
                cx.notify();
            }
        }
    }

    /// Chooses the active match after a scan.
    ///
    /// A rescan keeps the previous match when its starting cell still exists.
    /// A new query starts from the top of the viewport and wraps to the first
    /// match when the viewport is past every result.
    fn choose_active(&self, result: &SearchResult, cx: &Context<Self>) -> Option<usize> {
        if result.is_empty() {
            return None;
        }
        if let Some(start) = self.active_start
            && let Some(index) = result.index_from(start, SearchDirection::Forward)
            && result.matches()[index].start == start
        {
            return Some(index);
        }
        let origin = self
            .view
            .read_with(cx, |view, _| view.search_origin())
            .unwrap_or_default();
        result
            .index_from(origin, SearchDirection::Forward)
            .or(Some(0))
    }

    /// Called by the view when a newer content version invalidates the overlay.
    pub(super) fn on_overlay_stale(&mut self, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.result = None;
        self.active = None;
        self.status = SearchStatus::Waiting;
        self.schedule_scan(OUTPUT_RESCAN_DEBOUNCE, cx);
    }
}
