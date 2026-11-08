//! Independent P0-20 editor probe. It never opens an SSH connection or app data.

use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::prelude::*;
use gpui_kit::{Entity, Render, Window, WindowOptions, div, px};

mod document;
mod fixtures;
#[cfg(test)]
mod tests;

struct Probe {
    editor: Entity<EditorState>,
}

impl Probe {
    fn new(text: String, language: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language(language)
                .default_value(text)
        });
        editor.update(cx, |state, cx| {
            // Keep document rows and viewport offsets comparable in the large-file probe.
            state.set_soft_wrap(false, window, cx);
            state.focus(window, cx);
        });
        Self { editor }
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Editor::new(&self.editor).h_full().text_size(px(13.)))
    }
}

fn main() {
    let language = std::env::args().nth(1).unwrap_or_else(|| "rust".into());
    if language == "--local-check" {
        match document::local_check() {
            Ok(()) => println!("P0-20 local format roundtrips and conflict checks: passed"),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if language != "large" && !fixtures::LANGUAGES.contains(&language.as_str()) {
        eprintln!(
            "Use a language from {:?}, large, or --local-check",
            fixtures::LANGUAGES
        );
        std::process::exit(2);
    }
    let text = if language == "large" {
        match fixtures::large_file().and_then(|path| document::Document::open(&path)) {
            Ok(document) => document.text,
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
    } else {
        fixtures::sample(&language).to_string()
    };
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        if let Err(error) = gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| Probe::new(text, language, window, cx))
        }) {
            eprintln!("Could not open P0-20 editor: {error}");
            cx.quit();
        }
    });
}
