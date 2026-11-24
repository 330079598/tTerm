//! Painting the search bar's input, count, options and navigation.

use gpui_kit::{
    Context, Div, InteractiveElement as _, ParentElement, Role, Stateful,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};
use tterm_core::i18n::keys;
use tterm_ui::components::button::{ButtonSize, ButtonState, Variant, button};
use tterm_ui::components::icons;
use tterm_ui::tokens::gpui::hsla;

use super::{
    CASE_OPTION_SELECTOR, CLOSE_SELECTOR, COUNT_SELECTOR, NEXT_SELECTOR, PREVIOUS_SELECTOR,
    REGEX_OPTION_SELECTOR, SearchBar, SearchStatus, WORD_OPTION_SELECTOR, text,
};

impl SearchBar {
    pub(super) fn render_controls(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        root: Stateful<Div>,
    ) -> Stateful<Div> {
        let locale = self.locale.read(cx);
        let colors = self.theme.colors;
        let muted = hsla(colors.muted_foreground, 1.0);
        let destructive = hsla(colors.destructive, 1.0);
        let count = self.count_text(cx);
        let missing = matches!(self.status, SearchStatus::Empty | SearchStatus::Error(_));
        let count_color = if missing { destructive } else { muted };
        let case_label = text(locale, keys::TERMINAL_SEARCH_CASE_SENSITIVE);
        let word_label = text(locale, keys::TERMINAL_SEARCH_WHOLE_WORD);
        let regex_label = text(locale, keys::TERMINAL_SEARCH_REGEX);
        let previous_label = text(locale, keys::TERMINAL_SEARCH_PREVIOUS);
        let next_label = text(locale, keys::TERMINAL_SEARCH_NEXT);
        let close_label = text(locale, keys::TERMINAL_SEARCH_CLOSE);

        let case = option_button(
            CASE_OPTION_SELECTOR,
            icons::CASE_SENSITIVE,
            self.options.case_sensitive,
            self.theme,
            case_label.clone(),
        )
        .on_click(cx.listener(|bar, _, _, cx| bar.toggle_case_sensitive(cx)));
        let word = option_button(
            WORD_OPTION_SELECTOR,
            icons::WHOLE_WORD,
            self.options.whole_word,
            self.theme,
            word_label.clone(),
        )
        .on_click(cx.listener(|bar, _, _, cx| bar.toggle_whole_word(cx)));
        let regex = option_button(
            REGEX_OPTION_SELECTOR,
            icons::REGEX,
            self.options.regex,
            self.theme,
            regex_label.clone(),
        )
        .on_click(cx.listener(|bar, _, _, cx| bar.toggle_regex(cx)));
        let previous = icon_button(
            PREVIOUS_SELECTOR,
            icons::ARROW_UP,
            self.theme,
            previous_label.clone(),
        )
        .on_click(cx.listener(|bar, _, _, cx| {
            bar.navigate(tterm_terminal::search::SearchDirection::Backward, cx)
        }));
        let next = icon_button(
            NEXT_SELECTOR,
            icons::ARROW_DOWN,
            self.theme,
            next_label.clone(),
        )
        .on_click(cx.listener(|bar, _, _, cx| {
            bar.navigate(tterm_terminal::search::SearchDirection::Forward, cx)
        }));
        let close = icon_button(CLOSE_SELECTOR, icons::X, self.theme, close_label.clone())
            .on_click(cx.listener(|bar, _, window, cx| bar.close(window, true, cx)));

        root.child(
            div()
                .id(COUNT_SELECTOR)
                .debug_selector(|| COUNT_SELECTOR.to_string())
                .min_w(px(74.0))
                .text_center()
                .text_size(px(12.0))
                .whitespace_nowrap()
                .text_color(count_color)
                .aria_label(count.clone())
                .child(count),
        )
        .child(case)
        .child(word)
        .child(regex)
        .child(previous)
        .child(next)
        .child(close)
        .child(div().debug_selector(|| "probe-controls".to_string()))
    }
}

fn option_button(
    selector: &'static str,
    icon: &'static [u8],
    active: bool,
    theme: tterm_ui::tokens::theme::Theme,
    label: String,
) -> Stateful<Div> {
    let variant = if active {
        Variant::Secondary
    } else {
        Variant::Ghost
    };
    let color = if active {
        hsla(theme.colors.foreground, 1.0)
    } else {
        hsla(theme.colors.muted_foreground, 1.0)
    };
    button(
        selector,
        "",
        variant,
        ButtonSize::IconXs,
        ButtonState::default(),
        theme,
    )
    .debug_selector(move || selector.to_string())
    .role(Role::Button)
    .aria_label(label)
    .child(icons::icon(icon, color, 16.0))
}

fn icon_button(
    selector: &'static str,
    icon: &'static [u8],
    theme: tterm_ui::tokens::theme::Theme,
    label: String,
) -> Stateful<Div> {
    button(
        selector,
        "",
        Variant::Ghost,
        ButtonSize::IconXs,
        ButtonState::default(),
        theme,
    )
    .debug_selector(move || selector.to_string())
    .role(Role::Button)
    .aria_label(label)
    .child(icons::icon(
        icon,
        hsla(theme.colors.muted_foreground, 1.0),
        16.0,
    ))
}
