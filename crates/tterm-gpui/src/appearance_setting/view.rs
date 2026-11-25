//! Rendering for the cursor samples and the three padding fields.

use gpui_kit::{FontWeight, Role, Stateful, div, px};
use tterm_core::i18n::keys;
use tterm_ui::components::input::{InputOptions, input};
use tterm_ui::tokens::gpui::hsla;

use super::*;

/// The prompt the reference page draws before each cursor sample.
const SAMPLE_PROMPT: &str = "guest@tterm:~$";

impl AppearanceSetting {
    /// Renders the cursor choices above the scrollback capacity.
    pub(crate) fn cursor_section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::FONT_SETTINGS_CURSOR_STYLE);
        let mut choices = div()
            .id(CURSOR_CHOICES_ID)
            .debug_selector(|| CURSOR_CHOICES_ID.to_string())
            .mt(px(8.0))
            .flex()
            .gap(px(8.0));
        for (style, label_key) in CURSOR_STYLES {
            choices = choices.child(self.cursor_option(style, label_key, cx));
        }
        div()
            .id(CURSOR_SECTION_ID)
            .debug_selector(|| CURSOR_SECTION_ID.to_string())
            .flex()
            .flex_col()
            .track_focus(&self.cursor_focus)
            .capture_key_down(cx.listener(Self::handle_key))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(hsla(colors.foreground, 1.0))
                    .child(title),
            )
            .child(choices)
    }

    fn cursor_option(
        &mut self,
        style: &'static str,
        label_key: TranslationKey,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let selected = self.cursor_style == style;
        let label = self.cursor_label(cx, label_key);
        let row_id = format!("{CURSOR_OPTION_PREFIX}{style}");
        let selector_id = row_id.clone();
        div()
            .id(row_id)
            .debug_selector(move || selector_id.clone())
            .flex()
            .flex_1()
            .flex_col()
            .gap(px(8.0))
            .p(px(8.0))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(
                if selected { colors.ring } else { colors.border },
                1.0,
            ))
            .bg(hsla(colors.primary, if selected { 0.1 } else { 0.0 }))
            .cursor_pointer()
            .role(Role::Button)
            .aria_label(label.clone())
            .aria_selected(selected)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_cursor(style, window, cx);
            }))
            .child(self.cursor_sample(style))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .child(
                        div()
                            .font_weight(if selected {
                                FontWeight(500.0)
                            } else {
                                FontWeight(400.0)
                            })
                            .text_color(hsla(
                                if selected {
                                    colors.foreground
                                } else {
                                    colors.muted_foreground
                                },
                                1.0,
                            ))
                            .child(label),
                    )
                    .when(selected, |row| {
                        row.child(
                            div()
                                .flex_none()
                                .text_color(hsla(colors.primary, 1.0))
                                .child("✓"),
                        )
                    }),
            )
    }

    fn cursor_sample(&self, style: &'static str) -> Div {
        let colors = self.theme.colors;
        div()
            .flex()
            .items_center()
            .gap(px(2.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .bg(hsla(colors.background, 0.85))
            .font_family("monospace")
            .text_size(px(13.0))
            .line_height(px(16.0))
            .text_color(hsla(colors.foreground, 1.0))
            .child(SAMPLE_PROMPT)
            .child(match style {
                "bar" => div()
                    .flex_none()
                    .w(px(2.0))
                    .h(px(14.0))
                    .bg(hsla(colors.foreground, 1.0)),
                "underline" => div()
                    .flex_none()
                    .w(px(8.0))
                    .h(px(14.0))
                    .flex()
                    .items_end()
                    .child(div().w_full().h(px(2.0)).bg(hsla(colors.foreground, 1.0))),
                _ => div()
                    .flex_none()
                    .w(px(8.0))
                    .h(px(14.0))
                    .bg(hsla(colors.foreground, 1.0)),
            })
    }

    /// Renders the padding fields below the scrollback capacity.
    pub(crate) fn padding_section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = self.theme.colors;
        let title = self.text(cx, keys::FONT_SETTINGS_TERMINAL_PADDING);
        let mut fields = div().mt(px(8.0)).flex().gap(px(12.0));
        for side in [PaddingSide::Left, PaddingSide::Right, PaddingSide::Bottom] {
            fields = fields.child(self.padding_field(side, cx));
        }
        let mut section = div()
            .id(PADDING_SECTION_ID)
            .debug_selector(|| PADDING_SECTION_ID.to_string())
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(Self::handle_key))
            .child(
                div()
                    .text_size(px(14.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .text_color(hsla(colors.foreground, 1.0))
                    .child(title),
            )
            .child(fields)
            .child(
                div()
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_TERMINAL_PADDING_DESC)),
            );
        if let Some(error) = &self.error {
            section = section.child(
                div()
                    .id(PADDING_ERROR_ID)
                    .debug_selector(|| PADDING_ERROR_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        section
    }

    fn padding_field(&self, side: PaddingSide, cx: &App) -> Div {
        let colors = self.theme.colors;
        let label = self.text(cx, side.label_key());
        let input = self.padding_input(side).map(|state| {
            input(
                state,
                InputOptions {
                    theme: Some(self.theme),
                    ..InputOptions::default()
                },
            )
            .id(side.input_id())
            .debug_selector(move || side.input_id().to_string())
        });
        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(label),
            )
            .children(input)
    }
}
