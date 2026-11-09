//! Select matching `src/components/ui/select.tsx`.
//!
//! React renders a native `<select>`: CSS draws the closed control and the
//! operating system draws the open list. Here the closed control is drawn to
//! match the CSS, and the list is a GPUI popup in the app's popover style; the
//! reference cannot capture a native menu, so the list is judged by a person.
//! Keyboard opening, Escape and focus transfer come from gpui-base's `Select`
//! root; highlight movement, choosing and outside clicks are handled here.

use super::icons::{CHEVRON_DOWN, icon};
use super::surface::{css_color, css_opacity, shadow_xs, surface};
use crate::tokens::color::Hsl;
use crate::tokens::gpui::hsla;
use crate::tokens::theme::Theme;
use gpui_kit::base::Select as SelectRoot;
use gpui_kit::base::actions::{SelectDown, SelectUp};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Bounds, BoxShadow, Context, ElementId, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, deferred, div, px,
};
use std::cell::Cell;
use std::rc::Rc;

/// The control height (`h-9`), padding (`px-3 py-1 pr-9`) and chevron size.
const HEIGHT: f32 = 36.0;
const PADDING_X: f32 = 12.0;
const PADDING_Y: f32 = 4.0;
const PADDING_RIGHT: f32 = 36.0;
const CHEVRON: f32 = 16.0;
/// Gap between the control and the open list, and the list rows: `py-1.5`
/// around a 20px line gives 32px rows.
const LIST_GAP: f32 = 4.0;
const OPTION_LINE_HEIGHT: f32 = 20.0;

/// Resolved colours of the closed control, shared by painting and parity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectStyle {
    pub background: Hsl,
    pub border: Hsl,
    pub foreground: Hsl,
    pub chevron: Hsl,
    /// `disabled:opacity-50` applies to the `<select>` only; the chevron is a
    /// sibling and keeps full opacity.
    pub opacity: f32,
    pub padding: (f32, f32, f32, f32),
}

/// `border-input bg-background focus-visible:border-ring`.
pub fn style(focused: bool, disabled: bool, theme: Theme) -> SelectStyle {
    let colors = theme.colors;
    SelectStyle {
        background: colors.background,
        border: if focused { colors.ring } else { colors.input },
        foreground: colors.foreground,
        chevron: colors.muted_foreground,
        opacity: if disabled { 0.5 } else { 1.0 },
        padding: (PADDING_Y, PADDING_RIGHT, PADDING_Y, PADDING_X),
    }
}

/// Emitted when the user chooses an option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectChanged(pub usize);

/// A single-choice select over a list of labels.
pub struct Select {
    id: SharedString,
    options: Vec<SharedString>,
    selected: usize,
    highlighted: usize,
    open: bool,
    disabled: bool,
    /// Draws the focus border without focus, for parity fixtures.
    force_focused: bool,
    theme: Theme,
    focus: FocusHandle,
    list_focus: FocusHandle,
    /// Where the control was last laid out, so a press on it is not also
    /// treated as a press outside the open list.
    control_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl EventEmitter<SelectChanged> for Select {}

impl Select {
    pub fn new(
        id: impl Into<SharedString>,
        options: Vec<SharedString>,
        selected: usize,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let selected = selected.min(options.len().saturating_sub(1));
        Self {
            id: id.into(),
            options,
            selected,
            highlighted: selected,
            open: false,
            disabled: false,
            force_focused: false,
            theme,
            focus: cx.focus_handle(),
            list_focus: cx.focus_handle(),
            control_bounds: Rc::new(Cell::new(None)),
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn force_focused(mut self, focused: bool) -> Self {
        self.force_focused = focused;
        self
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }

    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.open == open {
            return;
        }
        self.open = open;
        if open {
            self.highlighted = self.selected;
            self.list_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            self.focus.focus(window, cx);
            cx.notify();
        }
    }

    fn choose(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.options.len() {
            return;
        }
        let changed = index != self.selected;
        self.selected = index;
        self.highlighted = index;
        self.open = false;
        // gpui-base moves focus to the list after a confirm; return it to the
        // control once that handler has run.
        cx.defer_in(window, |this, window, cx| this.focus.focus(window, cx));
        if changed {
            cx.emit(SelectChanged(index));
        }
        cx.notify();
    }

    fn move_highlight(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.options.is_empty() {
            return;
        }
        let last = self.options.len() - 1;
        self.highlighted = self.highlighted.saturating_add_signed(step).min(last);
        cx.notify();
    }

    fn render_control(&self, focused: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let computed = style(focused, self.disabled, self.theme);
        let (top, right, bottom, left) = computed.padding;
        let label = self.options.get(self.selected).cloned().unwrap_or_default();
        let bounds = self.control_bounds.clone();
        div()
            .on_children_prepainted(move |children, _, _| bounds.set(children.first().copied()))
            .child(
                div()
                    .id(ElementId::Name(format!("{}-control", self.id).into()))
                    .relative()
                    .flex()
                    .items_center()
                    .h(px(HEIGHT))
                    .w_full()
                    .pt(px(top))
                    .pr(px(right))
                    .pb(px(bottom))
                    .pl(px(left))
                    .rounded(px(6.0))
                    .border(px(1.0))
                    .border_color(gpui_kit::transparent_black())
                    .shadow(vec![shadow_xs(computed.opacity)])
                    .text_size(px(14.0))
                    .text_color(hsla(computed.foreground, 1.0))
                    .opacity(css_opacity(computed.opacity))
                    .child(surface(
                        css_color(computed.background, 1.0),
                        css_color(computed.border, 1.0),
                        1.0,
                        6.0,
                        None,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(label),
                    )
                    .when(!self.disabled, |this| {
                        this.on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                if this.open {
                                    this.close(window, cx);
                                } else {
                                    this.set_open(true, window, cx);
                                }
                            }),
                        )
                    }),
            )
    }

    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.theme.colors;
        let bounds = self.control_bounds.clone();
        let items = self.options.iter().enumerate().map(|(index, label)| {
            let highlighted = index == self.highlighted;
            div()
                .id(index)
                .w_full()
                .px(px(8.0))
                .py(px(6.0))
                .rounded(px(4.0))
                .text_size(px(14.0))
                .line_height(px(OPTION_LINE_HEIGHT))
                .whitespace_nowrap()
                .overflow_hidden()
                .when(highlighted, |this| {
                    this.bg(hsla(colors.accent, 1.0))
                        .text_color(hsla(colors.accent_foreground, 1.0))
                })
                .on_mouse_move(cx.listener(move |this, _, _, cx| {
                    if this.highlighted != index {
                        this.highlighted = index;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.choose(index, window, cx)))
                .child(label.clone())
        });
        // `shadow-md`: 0 4px 6px -1px and 0 2px 4px -2px, black at 10%. GPUI's
        // blur radius is the Gaussian sigma, half the CSS value.
        let shadow = |y: f32, blur: f32, spread: f32| {
            BoxShadow::new(px(0.0), px(y), gpui_kit::black().opacity(0.1))
                .blur_radius(px(blur / 2.0))
                .spread_radius(px(spread))
        };
        deferred(
            div()
                .id(ElementId::Name(format!("{}-list", self.id).into()))
                .track_focus(&self.list_focus)
                .absolute()
                .top(px(HEIGHT + LIST_GAP))
                .left_0()
                .w_full()
                .max_h(px(288.0))
                .overflow_y_scroll()
                .p(px(4.0))
                .rounded(px(6.0))
                .border(px(1.0))
                .border_color(hsla(colors.border, 1.0))
                .bg(hsla(colors.popover, 1.0))
                .text_color(hsla(colors.popover_foreground, 1.0))
                .shadow(vec![shadow(4.0, 6.0, -1.0), shadow(2.0, 4.0, -2.0)])
                .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.move_highlight(-1, cx)))
                .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.move_highlight(1, cx)))
                .on_mouse_down_out(cx.listener(
                    move |this, event: &gpui_kit::MouseDownEvent, window, cx| {
                        // The control's own handler toggles the list.
                        let on_control = bounds.get().is_some_and(|b| b.contains(&event.position));
                        if !on_control {
                            this.close(window, cx);
                        }
                    },
                ))
                .children(items),
        )
        .with_priority(1)
    }
}

impl Render for Select {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.force_focused
            || self.focus.is_focused(window)
            || (self.open && self.list_focus.is_focused(window));
        let computed = style(focused, self.disabled, self.theme);
        let weak = cx.entity().downgrade();
        let confirm = cx.entity().downgrade();
        SelectRoot::new(ElementId::Name(self.id.clone()))
            .open(self.open)
            .disabled(self.disabled)
            .focus_handle(&self.focus)
            .content_focus_handle(&self.list_focus)
            .accessibility_value(self.options.get(self.selected).cloned().unwrap_or_default())
            .on_open_change(move |open, window, cx| {
                weak.update(cx, |this, cx| {
                    if open {
                        this.set_open(true, window, cx);
                    } else {
                        this.close(window, cx);
                    }
                })
                .ok();
            })
            .on_confirm(move |window, cx| {
                confirm
                    .update(cx, |this, cx| {
                        let index = this.highlighted;
                        this.choose(index, window, cx);
                    })
                    .ok();
            })
            .relative()
            .w_full()
            .child(self.render_control(focused, cx))
            // `right-3 top-1/2 -translate-y-1/2`, outside the faded control.
            .child(
                div()
                    .absolute()
                    .right(px(PADDING_X))
                    .top(px((HEIGHT - CHEVRON) / 2.0))
                    .child(icon(CHEVRON_DOWN, hsla(computed.chevron, 1.0), CHEVRON)),
            )
            .when(self.open, |this| this.child(self.render_list(cx)))
    }
}

#[cfg(test)]
mod tests;
