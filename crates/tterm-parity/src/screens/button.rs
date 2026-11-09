//! Token-backed Button parity screens.
use crate::color::{hsl_to_rgba, to_gpui};
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::ScreensManifest;
use crate::marker::{parity_child, parity_element};
use gpui_kit::{IntoElement, ParentElement, Styled, div, px};
use tterm_ui::components::button::{
    ButtonSize, ButtonState, Variant, button, style as button_style,
};
use tterm_ui::tokens::palette::{DARK, LIGHT};

pub const VARIANTS_SCREEN_ID: &str = "ui.button.variants";
pub const SIZES_SCREEN_ID: &str = "ui.button.sizes";

pub struct ButtonScreen {
    layout: LayoutSink,
    theme: tterm_ui::tokens::theme::Theme,
    variants: bool,
}
impl ButtonScreen {
    pub fn load(
        manifest: &ScreensManifest,
        layout: LayoutSink,
        variants: bool,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => return Err(format!("unsupported theme {other}")),
        };
        Ok(Self {
            layout,
            theme,
            variants,
        })
    }
    pub fn render(&self) -> impl IntoElement {
        let c = self.theme.colors;
        let root = StyleSnapshot {
            background_color: hsl_to_rgba(c.background),
            border_color: hsl_to_rgba(c.border),
            border_width: 0.0,
            border_radius: 0.0,
            font_family: "system-ui".into(),
            font_size: 14.0,
            font_weight: 400,
            line_height: 16.8,
            color: hsl_to_rgba(c.foreground),
            padding: PaddingSnapshot::all(32.0),
            declared_background_color: None,
            declared_border_color: None,
            opacity: 1.0,
        };
        let children = if self.variants {
            self.variant_children()
        } else {
            self.size_children()
        };
        let bg = to_gpui(hsl_to_rgba(c.background));
        parity_element(
            &self.layout,
            if self.variants {
                "ui.button.variants.screen"
            } else {
                "ui.button.sizes.screen"
            },
            ElementSizing::Fixed,
            root,
            move |view| {
                view.size_full()
                    .p(px(32.0))
                    .bg(bg)
                    .child(div().flex().flex_col().gap(px(12.0)).children(children))
            },
        )
        .size_full()
    }
    fn variant_children(&self) -> Vec<gpui_kit::AnyElement> {
        let variants = [
            ("default", Variant::Default),
            ("destructive", Variant::Destructive),
            ("outline", Variant::Outline),
            ("secondary", Variant::Secondary),
            ("ghost", Variant::Ghost),
            ("link", Variant::Link),
        ];
        let mut out = variants
            .into_iter()
            .map(|(n, v)| self.item(n, v, ButtonSize::Default, ButtonState::default(), n))
            .collect::<Vec<_>>();
        out.push(self.item(
            "hover",
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                hovered: true,
                ..ButtonState::default()
            },
            "hover",
        ));
        out.push(self.item(
            "focus",
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                focused: true,
                ..ButtonState::default()
            },
            "focus",
        ));
        out.push(self.item(
            "disabled",
            Variant::Default,
            ButtonSize::Default,
            ButtonState {
                disabled: true,
                ..ButtonState::default()
            },
            "disabled",
        ));
        out
    }
    fn size_children(&self) -> Vec<gpui_kit::AnyElement> {
        [
            ("default", ButtonSize::Default),
            ("xs", ButtonSize::Xs),
            ("sm", ButtonSize::Sm),
            ("lg", ButtonSize::Lg),
            ("icon", ButtonSize::Icon),
            ("icon-xs", ButtonSize::IconXs),
            ("icon-sm", ButtonSize::IconSm),
            ("icon-lg", ButtonSize::IconLg),
        ]
        .into_iter()
        .map(|(n, s)| self.item(n, Variant::Default, s, ButtonState::default(), n))
        .collect()
    }
    fn item(
        &self,
        name: &'static str,
        variant: Variant,
        size: ButtonSize,
        state: ButtonState,
        label: &'static str,
    ) -> gpui_kit::AnyElement {
        let c = self.theme.colors;
        let computed = button_style(variant, size, state, c);
        let style = StyleSnapshot {
            background_color: with_alpha(
                hsl_to_rgba(computed.background),
                computed.background_alpha,
            ),
            border_color: hsl_to_rgba(computed.border),
            border_width: computed.border_width,
            border_radius: 6.0,
            font_family: "system-ui".into(),
            font_size: computed.font_size,
            font_weight: 500,
            line_height: computed.line_height,
            color: hsl_to_rgba(computed.foreground),
            padding: PaddingSnapshot::symmetric(computed.padding_y, computed.padding_x),
            declared_background_color: None,
            declared_border_color: None,
            opacity: computed.opacity,
        };
        let prefix = if self.variants {
            "ui.button.variants"
        } else {
            "ui.button.sizes"
        };
        parity_child(
            &self.layout,
            format!("{prefix}.{name}"),
            ElementSizing::Fixed,
            style,
            button(
                format!("{prefix}.{name}"),
                if matches!(
                    size,
                    ButtonSize::Icon | ButtonSize::IconXs | ButtonSize::IconSm | ButtonSize::IconLg
                ) {
                    "＋"
                } else {
                    label
                },
                variant,
                size,
                state,
                c,
            ),
        )
        .into_any_element()
    }
}

fn with_alpha(mut color: [u8; 4], alpha: f32) -> [u8; 4] {
    color[3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    color
}
