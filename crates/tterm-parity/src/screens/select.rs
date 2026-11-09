//! GPUI Select parity screen, matching `src/parity/screens/P024Primitives.tsx`.
//!
//! Only the closed control is compared: the React side is a native `<select>`
//! whose open list the operating system draws.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::parity_child;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, SharedString, Styled, div, px,
};
use tterm_ui::components::select::{Select, style as select_style};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const SCREEN_ID: &str = "ui.select.default";

/// The React screen's selects: id, options, selected index, focused, disabled.
const SELECTS: [(&str, &[&str], usize, bool, bool); 4] = [
    ("ui.select.default", &["One", "Two"], 0, false, false),
    ("ui.select.focus", &["One", "Two"], 1, true, false),
    ("ui.select.disabled", &["One", "Two"], 0, false, true),
    (
        "ui.select.long",
        &["A very long selected value that truncates"],
        0,
        false,
        false,
    ),
];

pub struct SelectScreen {
    layout: LayoutSink,
    theme: Theme,
    selects: Vec<Entity<Select>>,
}

impl SelectScreen {
    pub fn load(manifest: &ScreensManifest, layout: LayoutSink) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            selects: Vec::new(),
        })
    }

    fn ensure_selects(&mut self, cx: &mut impl AppContext) {
        if !self.selects.is_empty() {
            return;
        }
        let theme = self.theme;
        self.selects = SELECTS
            .iter()
            .map(|(id, options, selected, focused, disabled)| {
                let options = options
                    .iter()
                    .map(|label| SharedString::from(*label))
                    .collect();
                cx.new(|cx| {
                    Select::new(*id, options, *selected, theme, cx)
                        .force_focused(*focused)
                        .disabled(*disabled)
                })
            })
            .collect();
    }

    fn style(&self, focused: bool, disabled: bool) -> StyleSnapshot {
        let computed = select_style(focused, disabled, self.theme);
        let (top, right, bottom, left) = computed.padding;
        StyleSnapshot {
            background_color: hsl_to_rgba(computed.background),
            border_color: hsl_to_rgba(computed.border),
            border_width: 1.0,
            border_radius: 6.0,
            ring_spread: 0.0,
            ring_spreads: Vec::new(),
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(computed.foreground),
            padding: PaddingSnapshot {
                top,
                right,
                bottom,
                left,
            },
            declared_background_color: None,
            declared_border_color: None,
            opacity: computed.opacity,
        }
    }

    pub fn render(&mut self, cx: &mut Context<super::ScreenView>) -> impl IntoElement {
        self.ensure_selects(cx);
        let children = SELECTS
            .iter()
            .zip(&self.selects)
            .map(|((id, _, _, focused, disabled), select)| {
                parity_child(
                    &self.layout,
                    *id,
                    ElementSizing::Fixed,
                    self.style(*focused, *disabled),
                    select.clone(),
                )
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .p(px(32.0))
            .bg(to_gpui(hsl_to_rgba(self.theme.colors.background)))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .children(children)
    }
}
