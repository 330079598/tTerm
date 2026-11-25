//! Rendering for the Terminal font editor: inputs, picker and preview.

use gpui_kit::{Bounds, FontFeatures, Hsla, Pixels, TextRun, canvas, font, point};

use super::*;

/// The React page keeps the prompt preview at this fixed line height so Nerd
/// Font icons do not collide when the user changes terminal spacing.
const NERD_PREVIEW_LINE_HEIGHT: f32 = 1.6;

impl FontSetting {
    /// Renders the text-rendering part of the Terminal settings page.
    ///
    /// The page puts the scrollback field and the preview between this section
    /// and its one Save, so the text controls carry no action of their own.
    pub(crate) fn text_section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut section = div()
            .id(SECTION_ID)
            .debug_selector(|| SECTION_ID.to_string())
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(Self::handle_key))
            .child(self.section_header(cx))
            .child(self.size_controls(cx))
            .child(self.spacing_controls(cx))
            .child(self.family_controls(cx));
        if let Some(error) = &self.error {
            section = section.child(
                div()
                    .id(ERROR_ID)
                    .debug_selector(|| ERROR_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(self.theme.colors.destructive, 1.0))
                    .child(error.clone()),
            );
        }
        if self.saved {
            section = section.child(
                div()
                    .id(SAVED_ID)
                    .debug_selector(|| SAVED_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(self.theme.colors.success, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_SAVED)),
            );
        }
        section
    }

    /// Renders the whole editor for a standalone test or capture.
    ///
    /// The settings route uses [`FontSetting::text_section`] and
    /// [`FontSetting::preview_section`] separately so it can place the
    /// scrollback field and its one Save between them.
    #[cfg(test)]
    pub(crate) fn section(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        self.text_section(cx)
            .child(self.preview_section(cx))
            .child(self.actions(cx))
    }

    fn section_header(&self, cx: &App) -> Div {
        let colors = self.theme.colors;
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .id(TITLE_ID)
                    .debug_selector(|| TITLE_ID.to_string())
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .line_height(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .child(icon(
                        SQUARE_TERMINAL,
                        hsla(colors.muted_foreground, 1.0),
                        16.0,
                    ))
                    .child(self.text(cx, keys::FONT_SETTINGS_TEXT_RENDERING)),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_TEXT_RENDERING_DESC)),
            )
    }

    fn size_controls(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let current = parse_size(&self.size_value(cx)).ok();
        let mut presets = div().mt(px(8.0)).flex().flex_wrap().gap(px(6.0));
        for size in FONT_SIZE_OPTIONS {
            presets = presets.child(preset_button(
                theme,
                size,
                current == Some(size),
                cx.listener(move |this, _, window, cx| this.set_size(size, window, cx)),
            ));
        }
        if let Some(state) = &self.size_input {
            presets = presets.child(
                div().w(px(64.0)).child(
                    input(
                        state,
                        InputOptions {
                            theme: Some(theme),
                            ..InputOptions::default()
                        },
                    )
                    .id(SIZE_INPUT_ID)
                    .debug_selector(|| SIZE_INPUT_ID.to_string()),
                ),
            );
        }
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(16.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_FONT_SIZE)),
            )
            .child(presets)
    }

    fn spacing_controls(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let line_height = self.line_height_input.as_ref().map(|state| {
            input(
                state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            )
            .id(LINE_HEIGHT_INPUT_ID)
            .debug_selector(|| LINE_HEIGHT_INPUT_ID.to_string())
        });
        let letter_spacing = self.letter_spacing_input.as_ref().map(|state| {
            input(
                state,
                InputOptions {
                    theme: Some(theme),
                    ..InputOptions::default()
                },
            )
            .id(LETTER_SPACING_INPUT_ID)
            .debug_selector(|| LETTER_SPACING_INPUT_ID.to_string())
        });
        div()
            .mt(px(16.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .flex()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(500.0))
                                    .child(self.text(cx, keys::FONT_SETTINGS_LINE_HEIGHT)),
                            )
                            .children(line_height),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight(500.0))
                                    .child(self.text(cx, keys::FONT_SETTINGS_LETTER_SPACING)),
                            )
                            .children(letter_spacing),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(theme.colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_SPACING_DESC)),
            )
    }

    fn family_controls(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let mut column = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(16.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_FONT_FAMILY)),
            )
            .child(div().mt(px(8.0)).flex().flex_col().gap(px(8.0)).when_some(
                self.family_input.as_ref(),
                |column, state| {
                    column.child(
                        input(
                            state,
                            InputOptions {
                                theme: Some(theme),
                                ..InputOptions::default()
                            },
                        )
                        .id(FAMILY_INPUT_ID)
                        .debug_selector(|| FAMILY_INPUT_ID.to_string()),
                    )
                },
            ));
        let family = self.family_value(cx);
        if self.nerd_warning(&family) {
            column = column.child(
                div()
                    .id(WARNING_ID)
                    .debug_selector(|| WARNING_ID.to_string())
                    .mt(px(8.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(theme.colors.warning, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_NERD_FONT_WARNING)),
            );
        }
        column.child(self.font_picker(theme, cx))
    }

    pub(crate) fn preview_section(&self, cx: &App) -> Div {
        let colors = self.theme.colors;
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(24.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(14.0))
                    .font_weight(FontWeight(500.0))
                    .child(icon(EYE, hsla(colors.muted_foreground, 1.0), 16.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_PREVIEW)),
            )
            .child(
                div()
                    .mt(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.muted_foreground, 1.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_PREVIEW_DESC)),
            )
            .child(self.preview_box(
                cx,
                PREVIEW_ID,
                &[PREVIEW_TEXT],
                self.preview_line_height(cx),
                self.preview_letter_spacing(cx),
            ))
            .child(
                div()
                    .mt(px(12.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight(500.0))
                    .child(self.text(cx, keys::FONT_SETTINGS_NERD_FONT_PREVIEW)),
            )
            // Nerd Font glyphs are drawn from one fixed grid. The main
            // preview shows the spacing draft; applying it here would break
            // prompt icons and powerline separators.
            .child(self.preview_box(
                cx,
                NERD_PREVIEW_ID,
                &NERD_PREVIEW_LINES,
                NERD_PREVIEW_LINE_HEIGHT,
                0.0,
            ))
    }

    #[cfg(test)]
    fn actions(&mut self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let save = self.text(cx, keys::FONT_SETTINGS_SAVE);
        let cancel = self.text(cx, keys::FONT_SETTINGS_CANCEL);
        let state = ButtonState {
            disabled: self.saving,
            ..ButtonState::default()
        };
        div()
            .mt(px(12.0))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(
                button(
                    CANCEL_ID,
                    cancel,
                    Variant::Outline,
                    ButtonSize::Sm,
                    ButtonState::default(),
                    theme,
                )
                .debug_selector(|| CANCEL_ID.to_string())
                .role(Role::Button)
                .on_click(cx.listener(|this, _, window, cx| this.cancel(window, cx))),
            )
            .child(
                button(
                    SAVE_ID,
                    save.clone(),
                    Variant::Default,
                    ButtonSize::Sm,
                    state,
                    theme,
                )
                .debug_selector(|| SAVE_ID.to_string())
                .role(Role::Button)
                .aria_label(save)
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            )
    }

    fn font_picker(&mut self, theme: Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = theme.colors;
        let mut picker = div()
            .id("settings-font-picker")
            .mt(px(8.0))
            .flex()
            .flex_col()
            .gap(px(8.0));
        if self.fonts_loading {
            return picker.child(self.picker_message(
                cx,
                LOADING_ID,
                keys::FONT_SETTINGS_LOADING_FONTS,
                colors.muted_foreground,
            ));
        }
        if let Some(error) = &self.fonts_error {
            return picker.child(
                div()
                    .id(LOAD_ERROR_ID)
                    .debug_selector(|| LOAD_ERROR_ID.to_string())
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .text_color(hsla(colors.warning, 1.0))
                    .child(error.clone()),
            );
        }
        let Some(fonts) = &self.system_fonts else {
            return picker;
        };
        if fonts.is_empty() {
            return picker.child(self.picker_message(
                cx,
                EMPTY_ID,
                keys::FONT_SETTINGS_NO_FONTS_FOUND,
                colors.muted_foreground,
            ));
        }
        if let Some(state) = &self.font_search_input {
            picker = picker.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(icon(SEARCH, hsla(colors.muted_foreground, 1.0), 14.0))
                    .child(
                        div().min_w(px(0.0)).flex_1().child(
                            input(
                                state,
                                InputOptions {
                                    theme: Some(theme),
                                    ..InputOptions::default()
                                },
                            )
                            .id(SEARCH_INPUT_ID)
                            .debug_selector(|| SEARCH_INPUT_ID.to_string()),
                        ),
                    ),
            );
        }
        let nerd_count = fonts.iter().filter(|font| is_nerd_font(font)).count();
        if nerd_count > 0 {
            picker = picker.child(self.nerd_toggle(theme, cx, nerd_count));
        }
        if self.visible_fonts(cx).is_empty() {
            return picker.child(self.picker_message(
                cx,
                NO_MATCH_ID,
                keys::FONT_SETTINGS_NO_MATCHING_FONTS,
                colors.muted_foreground,
            ));
        }
        picker.child(self.font_list(theme, cx))
    }

    fn picker_message(
        &self,
        cx: &App,
        id: &'static str,
        key: TranslationKey,
        color: tterm_ui::tokens::color::Hsl,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .px(px(8.0))
            .py(px(16.0))
            .text_size(px(12.0))
            .text_color(hsla(color, 1.0))
            .child(self.text(cx, key))
    }

    fn nerd_toggle(
        &self,
        theme: Theme,
        cx: &mut Context<Self>,
        nerd_count: usize,
    ) -> Stateful<Div> {
        let visible = self.visible_fonts(cx).len();
        let label = if self.show_nerd_fonts_only {
            format!("Nerd Font ({visible})")
        } else {
            format!("Nerd Font ({nerd_count})")
        };
        let variant = if self.show_nerd_fonts_only {
            Variant::Default
        } else {
            Variant::Outline
        };
        button(
            NERD_TOGGLE_ID,
            label.clone(),
            variant,
            ButtonSize::Xs,
            ButtonState::default(),
            theme,
        )
        .debug_selector(|| NERD_TOGGLE_ID.to_string())
        .role(Role::Button)
        .aria_label(label)
        .on_click(cx.listener(|this, _, _, cx| {
            this.show_nerd_fonts_only = !this.show_nerd_fonts_only;
            cx.notify();
        }))
    }

    fn font_list(&mut self, theme: Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let colors = theme.colors;
        let visible = self.visible_fonts(cx);
        let current = self.family_value(cx);
        let mut list = div()
            .id(FONT_LIST_ID)
            .debug_selector(|| FONT_LIST_ID.to_string())
            .h(px(FONT_LIST_HEIGHT))
            .overflow_y_scroll()
            .rounded(px(4.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .p(px(4.0));
        for (index, font) in visible.into_iter().enumerate() {
            list = list.child(self.font_row(theme, cx, index, &font, current.contains(&font)));
        }
        list
    }

    fn font_row(
        &mut self,
        theme: Theme,
        cx: &mut Context<Self>,
        index: usize,
        font: &str,
        selected: bool,
    ) -> Stateful<Div> {
        let colors = theme.colors;
        let nerd = is_nerd_font(font);
        let row_id = format!("{FONT_ROW_PREFIX}{index}");
        let label = font.to_string();
        let chosen = font.to_string();
        div()
            .id(row_id.clone())
            .debug_selector(move || row_id.clone())
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .when(selected, |row| row.bg(hsla(colors.primary, 0.15)))
            .cursor_pointer()
            .role(Role::Button)
            .aria_label(label.clone())
            .on_click(cx.listener(move |this, _, window, cx| {
                this.set_family_from_picker(chosen.clone(), window, cx);
            }))
            .child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(px(13.0))
                    .font_family(font.to_string())
                    .child(label),
            )
            .when(nerd, |row| {
                row.child(
                    div()
                        .flex_none()
                        .rounded(px(4.0))
                        .bg(hsla(colors.success, 0.15))
                        .px(px(6.0))
                        .text_size(px(10.0))
                        .text_color(hsla(colors.success, 1.0))
                        .child("NF"),
                )
            })
    }

    fn preview_box(
        &self,
        cx: &App,
        id: &'static str,
        lines: &[&str],
        line_height_multiplier: f32,
        letter_spacing: f32,
    ) -> Stateful<Div> {
        let colors = self.theme.colors;
        let size = self.preview_size(cx);
        let line_height = (size as f32 * line_height_multiplier).max(1.0);
        let family = self.preview_family(cx);
        let box_ = div()
            .id(id)
            .debug_selector(move || id.to_string())
            .mt(px(8.0))
            .rounded(px(8.0))
            .border(px(1.0))
            .border_color(hsla(colors.border, 1.0))
            .bg(hsla(colors.secondary, 1.0))
            .text_color(hsla(colors.foreground, 1.0))
            .px(px(16.0))
            .py(px(12.0))
            .font_family(family.clone())
            .text_size(px(size as f32))
            .line_height(px(line_height));
        if letter_spacing == 0.0 {
            let text_id = format!("{id}-text");
            let mut text = div()
                .id(text_id.clone())
                .debug_selector(move || text_id.clone())
                .flex()
                .flex_col();
            for line in lines {
                text = text.child(div().child((*line).to_string()));
            }
            box_.child(text)
        } else {
            let spaced_id = format!("{id}-spaced");
            let canvas_lines: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
            let total_height = line_height * lines.len() as f32;
            let color = hsla(colors.foreground, 1.0);
            box_.child(
                div()
                    .id(spaced_id.clone())
                    .debug_selector(move || spaced_id.clone())
                    .w_full()
                    .overflow_hidden()
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                SpacedPreview {
                                    lines: &canvas_lines,
                                    family: &family,
                                    font_size: size as f32,
                                    line_height,
                                    letter_spacing,
                                    color,
                                }
                                .paint(bounds, window);
                            },
                        )
                        .h(px(total_height))
                        .w_full(),
                    ),
            )
        }
    }

    fn nerd_warning(&self, family: &str) -> bool {
        let Some(fonts) = &self.system_fonts else {
            return false;
        };
        let nerd_count = fonts.iter().filter(|font| is_nerd_font(font)).count();
        if nerd_count == 0 {
            return false;
        }
        let resolved = first_installed_font(family, fonts);
        !is_nerd_font(&resolved)
    }
}

/// Paints shaped lines with an extra gap after every character.
///
/// GPUI has no letter-spacing style. Shaping the whole line once preserves the
/// font's kerning, then each decoded glyph is painted at its shaped position
/// plus `letter_spacing` times the number of characters before it. Counting
/// characters, not just painted glyphs, keeps word spaces in the gap and lets a
/// negative value pull the glyphs back together.
struct SpacedPreview<'a> {
    lines: &'a [String],
    family: &'a str,
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    color: Hsla,
}

impl SpacedPreview<'_> {
    /// Paints every line inside `bounds`, one line box per input line.
    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let mut face = font(self.family.to_string());
        // Letter spacing applies between characters; a ligature would collapse
        // several of them into one glyph and receive only one gap.
        face.features = FontFeatures::disable_ligatures();
        let font_size = px(self.font_size);
        for (index, text) in self.lines.iter().enumerate() {
            let run = TextRun {
                len: text.len(),
                font: face.clone(),
                color: self.color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line =
                window
                    .text_system()
                    .shape_line(text.clone().into(), font_size, &[run], None);
            let top = bounds.origin.y + px(self.line_height * index as f32);
            let padding_top = (px(self.line_height) - line.ascent - line.descent) / 2.0;
            let baseline = top + padding_top + line.ascent;
            for run in &line.runs {
                for glyph in &run.glyphs {
                    let characters_before = text
                        .get(..glyph.index)
                        .map(|prefix| prefix.chars().count() as f32)
                        .unwrap_or(0.0);
                    let origin = point(
                        bounds.origin.x
                            + glyph.position.x
                            + px(self.letter_spacing * characters_before),
                        baseline + glyph.position.y,
                    );
                    let _ = if glyph.is_emoji {
                        window.paint_emoji(origin, run.font_id, glyph.id, line.font_size)
                    } else {
                        window.paint_glyph(
                            origin,
                            run.font_id,
                            glyph.id,
                            line.font_size,
                            self.color,
                        )
                    };
                }
            }
        }
    }
}

fn preset_button(
    theme: Theme,
    size: u16,
    selected: bool,
    on_click: impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let variant = if selected {
        Variant::Default
    } else {
        Variant::Outline
    };
    let id = format!("{SIZE_PRESET_PREFIX}{size}");
    let label = size.to_string();
    button(
        id.clone(),
        label.clone(),
        variant,
        ButtonSize::Xs,
        ButtonState::default(),
        theme,
    )
    .debug_selector(move || id.clone())
    .role(Role::Button)
    .aria_label(label)
    .on_click(on_click)
}
