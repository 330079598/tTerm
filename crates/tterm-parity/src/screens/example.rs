//! The example screen: one button and one input, the W0-04 end-to-end case.
//!
//! It mirrors `src/parity/screens/ExampleButtonInput.tsx` element for element:
//! the same fixture supplies the input's value, the same tokens supply the
//! colours, and the same geometry (800 by 600, 32 px page padding, a 16 px
//! column gap, a 288 by 36 input, a 36 px button with 16 px side padding).
//!
//! The line height is 16 px on both sides on purpose. CSS accepts a fractional
//! computed line height, while GPUI rounds one to a whole pixel before drawing;
//! 16 px is exact in both.

use crate::color::{hsl_to_rgba, to_gpui};
use crate::fixture::FixtureFile;
use crate::layout::{ElementSizing, LayoutSink, PaddingSnapshot, StyleSnapshot};
use crate::manifest::{ScreenSpec, ScreensManifest};
use crate::marker::{parity_element, parity_text, parity_text_for};
use gpui_kit::{FontWeight, IntoElement, ParentElement, Styled, div, px};
use std::path::Path;
use tterm_ui::tokens::palette::{Colors, DARK, LIGHT};
use tterm_ui::tokens::theme::Theme;

/// The manifest id this view implements.
pub const SCREEN_ID: &str = "example.button-input.default";

const INPUT_WIDTH: f32 = 288.0;
const CONTROL_HEIGHT: f32 = 36.0;
const CONTENT_FONT_SIZE: f32 = 14.0;
const BODY_FONT_SIZE: f32 = 13.0;
const BODY_LINE_HEIGHT: f32 = 18.2;
const TEXT_LINE_HEIGHT: f32 = 16.0;
const PAGE_PADDING: f32 = 32.0;
const COLUMN_GAP: f32 = 16.0;
const ROW_GAP: f32 = 8.0;
const FIELD_PADDING_X: f32 = 12.0;
const FIELD_PADDING_Y: f32 = 4.0;
const BUTTON_PADDING_X: f32 = 16.0;
const BUTTON_PADDING_Y: f32 = 8.0;
const CORNER_RADIUS: f32 = 4.0;

fn parity_font_family() -> String {
    if std::env::var("PARITY_PLATFORM").as_deref() == Ok("windows") {
        "segoe ui".to_owned()
    } else {
        "system-ui".to_owned()
    }
}

/// The example view, built from one fixture file.
pub struct ExampleScreen {
    layout: LayoutSink,
    theme: Theme,
    profile_name: String,
}

impl ExampleScreen {
    /// Loads the fixture and the theme the manifest names.
    pub fn load(
        root: &Path,
        screen: &ScreenSpec,
        manifest: &ScreensManifest,
        layout: LayoutSink,
    ) -> Result<Self, String> {
        let theme = match manifest.theme.as_str() {
            "dark" => DARK,
            "light" => LIGHT,
            other => {
                return Err(format!(
                    "no GPUI tokens for parity theme \"{other}\"; expected dark or light"
                ));
            }
        };
        let fixture_path = ScreensManifest::fixture_path(root, screen);
        let fixture = FixtureFile::load(&fixture_path)?;
        let profile_name = fixture.first_profile_name()?;
        Ok(Self {
            layout,
            theme,
            profile_name,
        })
    }

    /// Paints the screen and records every marked element.
    pub fn render(&self) -> impl IntoElement {
        let colors = self.theme.colors;
        let root_style = root_style(&colors);
        let input_style = input_style(&colors);
        let button_style = button_style(&colors);

        let label = parity_text(
            &self.layout,
            "example.label",
            "Profile name",
            CONTENT_FONT_SIZE,
            TEXT_LINE_HEIGHT,
            FontWeight::NORMAL,
            to_gpui(hsl_to_rgba(colors.muted_foreground)),
        )
        .self_start();

        let input_text = parity_text_for(
            &self.layout,
            "example.input.text",
            self.profile_name.clone(),
            &input_style,
        );
        let button_text =
            parity_text_for(&self.layout, "example.button.text", "Save", &button_style);
        let input_border = to_gpui(hsl_to_rgba(colors.input));
        let button_background = to_gpui(hsl_to_rgba(colors.primary));

        let input = parity_element(
            &self.layout,
            "example.input",
            ElementSizing::Fixed,
            input_style,
            move |view| {
                view.flex()
                    .items_center()
                    .w(px(INPUT_WIDTH))
                    .h(px(CONTROL_HEIGHT))
                    .border_color(input_border)
                    .child(input_text)
            },
        );
        let button = parity_element(
            &self.layout,
            "example.button",
            ElementSizing::Text,
            button_style,
            move |view| {
                view.flex()
                    .items_center()
                    .justify_center()
                    .h(px(CONTROL_HEIGHT))
                    .bg(button_background)
                    .child(button_text)
            },
        );
        let row = div()
            .flex()
            .items_center()
            .gap(px(ROW_GAP))
            .child(input)
            .child(button);
        let screen_background = to_gpui(hsl_to_rgba(colors.background));

        parity_element(
            &self.layout,
            "example.screen",
            ElementSizing::Fixed,
            root_style,
            move |view| {
                view.size_full()
                    .flex()
                    .flex_col()
                    .gap(px(COLUMN_GAP))
                    .bg(screen_background)
                    .child(label)
                    .child(row)
            },
        )
        .size_full()
    }
}

/// The viewport of the example screen, as the manifest states it.
pub fn viewport(manifest: &ScreensManifest) -> crate::layout::ViewportSnapshot {
    crate::layout::ViewportSnapshot {
        width: manifest.viewport.width,
        height: manifest.viewport.height,
        device_scale_factor: manifest.viewport.device_scale_factor,
    }
}

fn root_style(colors: &Colors) -> StyleSnapshot {
    StyleSnapshot {
        background_color: hsl_to_rgba(colors.background),
        border_color: hsl_to_rgba(colors.border),
        border_width: 0.0,
        border_radius: 0.0,
        font_family: parity_font_family(),
        font_size: BODY_FONT_SIZE,
        font_weight: 400,
        line_height: BODY_LINE_HEIGHT,
        color: hsl_to_rgba(colors.foreground),
        padding: PaddingSnapshot::all(PAGE_PADDING),
        declared_background_color: None,
        declared_border_color: None,
    }
}

fn input_style(colors: &Colors) -> StyleSnapshot {
    StyleSnapshot {
        background_color: [0, 0, 0, 0],
        border_color: hsl_to_rgba(colors.input),
        border_width: 1.0,
        border_radius: CORNER_RADIUS,
        font_family: parity_font_family(),
        font_size: CONTENT_FONT_SIZE,
        font_weight: 400,
        line_height: TEXT_LINE_HEIGHT,
        color: hsl_to_rgba(colors.foreground),
        padding: PaddingSnapshot::symmetric(FIELD_PADDING_Y, FIELD_PADDING_X),
        declared_background_color: None,
        declared_border_color: None,
    }
}

fn button_style(colors: &Colors) -> StyleSnapshot {
    StyleSnapshot {
        background_color: hsl_to_rgba(colors.primary),
        border_color: hsl_to_rgba(colors.border),
        border_width: 0.0,
        border_radius: CORNER_RADIUS,
        font_family: parity_font_family(),
        font_size: CONTENT_FONT_SIZE,
        font_weight: 500,
        line_height: TEXT_LINE_HEIGHT,
        color: hsl_to_rgba(colors.primary_foreground),
        padding: PaddingSnapshot::symmetric(BUTTON_PADDING_Y, BUTTON_PADDING_X),
        declared_background_color: None,
        declared_border_color: None,
    }
}
