use super::placement::PlacedMenu;
use super::{ContextMenu, MenuIcon, PartStyle};
use crate::components::icons::{PLUS, ROTATE_CCW, X, icon};
use crate::components::surface::{css_color, css_opacity, shadow_lg, surface};
use gpui_kit::{
    AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Render,
    SharedString, StatefulInteractiveElement, Styled, Window, div, point, px,
};

impl ContextMenu {
    fn base_style(&self) -> PartStyle {
        PartStyle {
            background: gpui_kit::transparent_black(),
            border: css_color(self.theme.colors.border, 1.0),
            foreground: css_color(self.theme.colors.card_foreground, 1.0),
            border_width: 0.0,
            radius: 0.0,
            ring_spread: 0.0,
            font_size: 13.0,
            line_height: 18.2,
            padding: [0.0; 4],
            opacity: 1.0,
        }
    }

    /// Every value recorded here is applied to the actual part in `part`.
    fn part(&self, slot: String, style: PartStyle, child: impl IntoElement) -> Div {
        let observer = self.observer.clone();
        div()
            .on_children_prepainted(move |bounds, _, _| {
                if let Some(bounds) = bounds.first()
                    && let Some(observer) = &observer
                {
                    observer(&slot, *bounds, style);
                }
            })
            .child(child)
    }

    fn styled(style: PartStyle) -> Div {
        let [top, right, bottom, left] = style.padding;
        div()
            .relative()
            .pt(px(top))
            .pr(px(right))
            .pb(px(bottom))
            .pl(px(left))
            .border(px(style.border_width))
            .border_color(gpui_kit::transparent_black())
            .rounded(px(style.radius))
            .font_family(".SystemUIFont")
            .text_size(px(style.font_size))
            .line_height(px(style.line_height))
            .font_weight(FontWeight(400.0))
            .text_color(style.foreground)
            .opacity(css_opacity(style.opacity))
    }

    fn row_style(&self, index: usize) -> PartStyle {
        let action = &self.actions[index];
        let base = self.base_style();
        let focused = self.highlighted == Some(index);
        let hovered = self.hovered == Some(index) && !action.disabled;
        PartStyle {
            background: if hovered {
                css_color(
                    self.theme.colors.accent,
                    if self.theme.uses_dark_variant() {
                        0.5
                    } else {
                        1.0
                    },
                )
            } else {
                gpui_kit::transparent_black()
            },
            foreground: css_color(
                if hovered {
                    self.theme.colors.accent_foreground
                } else {
                    self.theme.colors.card_foreground
                },
                1.0,
            ),
            border: css_color(
                if focused {
                    self.theme.colors.ring
                } else {
                    self.theme.colors.border
                },
                1.0,
            ),
            radius: 6.0,
            ring_spread: if focused { 3.0 } else { 0.0 },
            font_size: 14.0,
            line_height: 16.8,
            padding: [
                6.0,
                if action.icon.is_some() { 12.0 } else { 8.0 },
                6.0,
                if action.icon.is_some() { 12.0 } else { 8.0 },
            ],
            opacity: if action.disabled { 0.5 } else { 1.0 },
            ..base
        }
    }

    fn row_content(
        &self,
        index: usize,
        mut row: gpui_kit::Stateful<Div>,
        style: PartStyle,
    ) -> gpui_kit::Stateful<Div> {
        let action = &self.actions[index];
        let base = self.base_style();
        let child_style = PartStyle {
            background: gpui_kit::transparent_black(),
            radius: 0.0,
            ring_spread: 0.0,
            padding: [0.0; 4],
            opacity: 1.0,
            border: base.border,
            ..style
        };
        if let Some(name) = action.icon {
            let bytes = match name {
                MenuIcon::Plus => PLUS,
                MenuIcon::Reopen => ROTATE_CCW,
                MenuIcon::Close => X,
            };
            row = row.child(
                self.part(
                    format!("icon-{index}"),
                    child_style,
                    Self::styled(child_style)
                        .size(px(16.0))
                        .flex_shrink_0()
                        .child(icon(bytes, style.foreground, 16.0)),
                ),
            );
        }
        row = row.child(
            self.part(
                format!("label-{index}"),
                child_style,
                Self::styled(child_style)
                    .h(px(16.0))
                    .child(action.label.clone()),
            ),
        );
        if let Some(shortcut) = &action.shortcut {
            let shortcut_style = PartStyle {
                font_size: 12.0,
                line_height: 13.8,
                foreground: css_color(self.theme.colors.muted_foreground, 1.0),
                padding: [0.0, 0.0, 0.0, 12.0],
                ..child_style
            };
            row = row.child(
                div().ml_auto().child(
                    self.part(
                        format!("shortcut-{index}"),
                        shortcut_style,
                        Self::styled(shortcut_style)
                            .flex_shrink_0()
                            .h(px(13.0))
                            .child(shortcut.clone()),
                    ),
                ),
            );
        }
        row
    }

    fn row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let action = &self.actions[index];
        let base = self.base_style();
        if action.separator {
            let style = PartStyle {
                background: css_color(self.theme.colors.border, 1.0),
                ..base
            };
            return self
                .part(
                    format!("separator-{index}"),
                    style,
                    Self::styled(style)
                        .h(px(1.0))
                        .my(px(4.0))
                        .bg(style.background),
                )
                .into_any_element();
        }
        let focused = self.highlighted == Some(index);
        let style = self.row_style(index);
        let mut row = Self::styled(style)
            .id((self.id.clone(), index))
            .w_full()
            .h(px(28.0))
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(surface(
                style.background,
                style.border,
                0.0,
                style.radius,
                focused.then_some((css_color(self.theme.colors.ring, 0.5), style.ring_spread)),
            ));
        row = self.row_content(index, row, style);
        if !action.disabled {
            row = row
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    this.hovered = if *hovered {
                        Some(index)
                    } else {
                        this.hovered.filter(|&i| i != index)
                    };
                    cx.notify();
                }))
                .on_click(cx.listener(move |this, _, window, cx| this.activate(index, window, cx)));
        }
        self.part(format!("item-{index}"), style, row)
            .into_any_element()
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let style = PartStyle {
            background: css_color(self.theme.colors.popover, 1.0),
            border_width: 1.0,
            radius: 6.0,
            padding: [4.0, 0.0, 4.0, 0.0],
            ..self.base_style()
        };
        let body_style = PartStyle {
            padding: [4.0, 4.0, 4.0, 4.0],
            ..self.base_style()
        };
        let rows = (0..self.actions.len())
            .map(|index| self.row(index, cx))
            .collect::<Vec<_>>();
        let body = self.part(
            "body".into(),
            body_style,
            Self::styled(body_style).children(rows),
        );
        let card = Self::styled(style)
            .id(SharedString::from(format!("{}-content", self.id)))
            .w(px(
                (window.viewport_size().width.as_f32() - 20.0).clamp(208.0, 256.0)
            ))
            .shadow(shadow_lg())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .child(surface(
                style.background,
                style.border,
                style.border_width,
                style.radius,
                None,
            ))
            .child(body);
        PlacedMenu {
            origin: point(px(self.origin.0), px(self.origin.1)),
            child: self.part("".into(), style, card).into_any_element(),
        }
        .into_any_element()
    }
}
