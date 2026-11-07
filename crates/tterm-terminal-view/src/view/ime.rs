//! Terminal text services expose only the caret and transient preedit.

use super::TerminalView;
use gpui_kit::{
    Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window,
};
use std::ops::Range;
use tterm_terminal::terminal::Scroll;

#[derive(PartialEq)]
pub(super) struct CandidateGeometry {
    bounds: Bounds<Pixels>,
    preedit_width: Pixels,
    scale_factor: f32,
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        *adjusted = Some(0..0);
        Some(String::new())
    }
    fn selected_text_range(
        &mut self,
        _ignore_disabled: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: 0..0,
            reversed: false,
        })
    }
    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.composition
            .active()
            .then(|| 0..self.composition.text.encode_utf16().count())
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ime_confirm = false;
        self.composition.clear();
        window.notify_text_input_changed(&self.focus_handle);
        self.invalidate(cx);
    }
    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime_confirm &= self.composition.active();
        self.composition.clear();
        self.terminal.clear_selection();
        self.terminal.scroll(Scroll::Bottom);
        self.send_bytes(text.as_bytes(), cx);
        window.notify_text_input_changed(&self.focus_handle);
        window.invalidate_character_coordinates();
        self.invalidate(cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ime_confirm = false;
        self.composition.update(text, selected);
        window.notify_text_input_changed(&self.focus_handle);
        window.invalidate_character_coordinates();
        self.invalidate(cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let frame = self.last_frame.as_ref()?;
        let mut bounds =
            self.composition
                .bounds(&frame.snapshot, self.input_origin?, frame.metrics);
        if let Some(preedit) = &self.preedit {
            let mut units = 0;
            let byte = self
                .composition
                .text
                .char_indices()
                .find_map(|(byte, ch)| {
                    if units >= range.start {
                        return Some(byte);
                    }
                    units += ch.len_utf16();
                    None
                })
                .unwrap_or(self.composition.text.len());
            bounds.origin.x += preedit.line.x_for_index(byte);
        }
        Some(bounds)
    }
    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        self.composition.clear();
        if let Some(text) = item.text() {
            self.paste_text(&text, cx);
        }
        window.notify_text_input_changed(&self.focus_handle);
    }
    fn accepts_text_input(&self, _window: &mut Window, _cx: &mut Context<Self>) -> bool {
        self.visible
    }
}

impl TerminalView {
    pub(super) fn prepare_ime(
        &mut self,
        data: &std::sync::Arc<super::frame::FrameData>,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = self
            .composition
            .bounds(&data.snapshot, origin, data.metrics);
        self.preedit = if self.composition.active() {
            Some(super::composition::PreeditPaint::new(
                self.composition.text.clone(),
                bounds,
                &self.fonts,
                data.metrics,
                self.palette.clone(),
                window,
            ))
        } else {
            None
        };
        let geometry = CandidateGeometry {
            bounds,
            preedit_width: self
                .preedit
                .as_ref()
                .map_or(gpui_kit::px(0.), |preedit| preedit.line.width()),
            scale_factor: window.scale_factor(),
        };
        if self.ime_geometry.as_ref() != Some(&geometry) {
            self.ime_geometry = Some(geometry);
            self.stats.ime_geometry_updates = self.stats.ime_geometry_updates.saturating_add(1);
            window.invalidate_character_coordinates();
        }
        if self.composition.hidden_since.is_some() && self.ime_hide_task.is_none() {
            self.ime_hide_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(super::composition::TRANSIENT_HIDE)
                    .await;
                let _ = cx.update(|_, cx| {
                    view.update(cx, |view, cx| {
                        view.invalidate(cx);
                    })
                });
            }));
        } else if self.composition.hidden_since.is_none() {
            self.ime_hide_task.take();
        }
    }
}
