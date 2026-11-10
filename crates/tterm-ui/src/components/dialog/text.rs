//! Shaped inline text whose observer reports each painted row independently.
use super::presentation::TextObserver;
use gpui_kit::{
    App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, SharedString, TextAlign, Window, WrappedLine, point, px, size,
};
use std::{cell::RefCell, rc::Rc};

pub(super) struct DialogText {
    slot: &'static str,
    text: SharedString,
    lines: Rc<RefCell<Vec<WrappedLine>>>,
    observer: Option<TextObserver>,
    center_multiline: bool,
}
impl DialogText {
    pub fn new(
        slot: &'static str,
        text: SharedString,
        observer: Option<TextObserver>,
        center_multiline: bool,
    ) -> Self {
        Self {
            slot,
            text,
            observer,
            center_multiline,
            lines: Rc::new(RefCell::new(Vec::new())),
        }
    }
}
impl IntoElement for DialogText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for DialogText {
    type RequestLayoutState = Pixels;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Pixels) {
        let text = self.text.clone();
        let lines = self.lines.clone();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let run = style.to_run(text.len());
        // The existing WebKit inline line box uses the floored CSS advance.
        // This controls row height only; glyph advances and wrap width stay exact.
        let height = px(style
            .line_height
            .to_pixels(font_size.into(), window.rem_size())
            .as_f32()
            .floor());
        let id = window.request_measured_layout(
            Default::default(),
            move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let mut shaped = Vec::new();
                for paragraph in text.split('\n') {
                    let mut remainder = paragraph;
                    loop {
                        let mut run = run.clone();
                        run.len = remainder.len();
                        let native = window
                            .text_system()
                            .shape_text(
                                remainder.to_owned().into(),
                                font_size,
                                std::slice::from_ref(&run),
                                width,
                                None,
                            )
                            .unwrap_or_default();
                        let Some(line) = native.first() else { break };
                        let mut end = line.wrap_boundaries.first().map_or(remainder.len(), |b| {
                            line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix].index
                        });
                        end = preserve_glued_word(remainder, end);
                        // GPUI tests overflow on the trailing space itself. CSS
                        // pre-line hangs that space, so a word ending within the
                        // available width stays on this row. Keep glyph advances
                        // and available width exact; only repair that break.
                        if end < remainder.len()
                            && let Some(width) = width
                        {
                            let word_end = remainder[end..]
                                .find(is_breakable_space)
                                .map_or(remainder.len(), |offset| end + offset);
                            if line.unwrapped_layout.x_for_index(word_end) <= width {
                                end = word_end;
                                while remainder.as_bytes().get(end) == Some(&b' ') {
                                    end += 1;
                                }
                            }
                        }
                        if end == 0 && !remainder.is_empty() {
                            break;
                        }
                        let row = remainder[..end].trim_end_matches(' ');
                        run.len = row.len();
                        shaped.extend(
                            window
                                .text_system()
                                .shape_text(
                                    row.to_owned().into(),
                                    font_size,
                                    std::slice::from_ref(&run),
                                    None,
                                    None,
                                )
                                .unwrap_or_default(),
                        );
                        if end == remainder.len() {
                            break;
                        }
                        remainder = &remainder[end..];
                    }
                }
                let mut measured = size(px(0.0), px(0.0));
                for line in &shaped {
                    let extent = line.size(height);
                    measured.width = measured.width.max(extent.width.ceil());
                    measured.height += extent.height;
                }
                *lines.borrow_mut() = shaped.into_iter().collect();
                measured
            },
        );
        (id, height)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        height: &mut Pixels,
        window: &mut Window,
        _: &mut App,
    ) {
        let Some(observer) = &self.observer else {
            return;
        };
        let mut y = bounds.origin.y;
        let mut index = 0;
        let align = if self.center_multiline && self.lines.borrow().len() > 1 {
            TextAlign::Center
        } else {
            window.text_style().text_align
        };
        for line in self.lines.borrow().iter() {
            let layout = &line.unwrapped_layout;
            let mut start = px(0.0);
            for end in line
                .wrap_boundaries
                .iter()
                .map(|b| layout.runs[b.run_ix].glyphs[b.glyph_ix].position.x)
                .chain(std::iter::once(layout.width))
            {
                let width = end - start;
                let offset = match align {
                    TextAlign::Center => (bounds.size.width - width) / 2.0,
                    TextAlign::Right => bounds.size.width - width,
                    TextAlign::Left => px(0.0),
                };
                observer(
                    &format!("{}-{index}", self.slot),
                    Bounds::new(point(bounds.origin.x + offset, y), size(width, *height)),
                );
                index += 1;
                y += *height;
                start = end;
            }
        }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        height: &mut Pixels,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut origin = bounds.origin;
        let align = if self.center_multiline && self.lines.borrow().len() > 1 {
            TextAlign::Center
        } else {
            window.text_style().text_align
        };
        for line in self.lines.borrow().iter() {
            let _ = line.paint(origin, *height, align, Some(bounds), window, cx);
            origin.y += line.size(*height).height;
        }
    }
}

fn is_glue(character: char) -> bool {
    matches!(character, '\u{00a0}' | '\u{2007}' | '\u{202f}')
}

fn is_breakable_space(character: char) -> bool {
    character.is_whitespace() && !is_glue(character)
}

fn preserve_glued_word(text: &str, end: usize) -> usize {
    if end == text.len() {
        return end;
    }
    let start = text[..end].rfind(is_breakable_space).map_or(0, |index| {
        index + text[index..].chars().next().map_or(0, char::len_utf8)
    });
    let word_end = text[end..]
        .find(is_breakable_space)
        .map_or(text.len(), |offset| end + offset);
    if start < end && text[start..word_end].chars().any(is_glue) {
        // WebKit overflows an unbreakable word rather than splitting its glue.
        if start == 0 { word_end } else { start }
    } else {
        end
    }
}
