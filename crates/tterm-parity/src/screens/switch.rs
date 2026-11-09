//! GPUI Switch parity screen, matching `src/parity/screens/P024Primitives.tsx`.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{BoundsSnapshot, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::{ParityPart, parity_child_with_parts};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use tterm_ui::components::switch::{SwitchState, style as switch_style, switch};
use tterm_ui::tokens::palette::{DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

pub const SCREEN_ID: &str = "ui.switch.default";

pub struct SwitchScreen {
    layout: LayoutSink,
    theme: Theme,
}

/// The four switches of the React screen, in order.
const STATES: [(&str, SwitchState); 4] = [
    (
        "ui.switch.checked",
        SwitchState {
            checked: true,
            focused: false,
            disabled: false,
            backdrop: None,
        },
    ),
    (
        "ui.switch.unchecked",
        SwitchState {
            checked: false,
            focused: false,
            disabled: false,
            backdrop: None,
        },
    ),
    (
        "ui.switch.focus",
        SwitchState {
            checked: true,
            focused: true,
            disabled: false,
            backdrop: None,
        },
    ),
    (
        "ui.switch.disabled",
        SwitchState {
            checked: true,
            focused: false,
            disabled: true,
            backdrop: None,
        },
    ),
];

impl SwitchScreen {
    pub fn load(manifest: &ScreensManifest, layout: LayoutSink) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self { layout, theme })
    }

    pub fn render(&mut self) -> impl IntoElement {
        let children = STATES
            .iter()
            .enumerate()
            .map(|(index, (id, state))| {
                let computed = switch_style(*state, self.theme);
                let style = StyleSnapshot {
                    background_color: hsl_to_rgba(computed.track),
                    border_color: computed.border,
                    border_width: 1.0,
                    border_radius: 10.0,
                    ring_spread: computed.ring_spread,
                    ring_spreads: if computed.ring_spread > 0.0 {
                        vec![computed.ring_spread]
                    } else {
                        Vec::new()
                    },
                    font_family: "system-ui".into(),
                    font_size: 13.0,
                    font_weight: 400,
                    line_height: 18.2,
                    color: hsl_to_rgba(self.theme.colors.foreground),
                    padding: PaddingSnapshot::all(0.0),
                    declared_background_color: None,
                    declared_border_color: None,
                    opacity: computed.opacity,
                };
                let thumb = StyleSnapshot {
                    background_color: hsl_to_rgba(computed.thumb),
                    border_width: 0.0,
                    border_radius: computed.thumb_size / 2.0,
                    ring_spread: 0.0,
                    ring_spreads: Vec::new(),
                    ..style.clone()
                };
                let (thumb_x, thumb_y) = computed.thumb_offset;
                parity_child_with_parts(
                    &self.layout,
                    *id,
                    style,
                    vec![ParityPart {
                        slot: "switch-thumb",
                        bounds: BoundsSnapshot::new(
                            thumb_x,
                            thumb_y,
                            computed.thumb_size,
                            computed.thumb_size,
                        ),
                        style: thumb,
                    }],
                    switch(
                        format!("switch-{index}"),
                        *state,
                        self.theme,
                        |_next, _event, _window, _cx| {},
                    ),
                )
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .p(px(32.0))
            .bg(to_gpui(hsl_to_rgba(self.theme.colors.background)))
            .child(div().flex().flex_col().gap(px(16.0)).children(children))
    }
}
