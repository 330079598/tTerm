//! The P0-08 window: a bilingual greeting and a `gpui-kit` button.
//!
//! The terminal prototype (P0-10 onward) replaces this window, so it deliberately
//! holds no application state, services or persistence.

use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::Button;
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, FocusHandle, KeyBinding, SharedString, TestSupportExt, Window, WindowBounds,
    WindowOptions, actions, div, point, px, size, white,
};

actions!(tterm_gpui, [Increment, Quit]);

/// The greeting P0-08 asks for. Mixed Chinese and English text is the point:
/// it exercises font fallback through GPUI's text system.
const GREETING: &str = "中英文混排 mixed Chinese and English text：最小 GPUI 程序已经打开窗口。A minimal GPUI window is up.";

/// The window content. `clicks` is the smallest state that proves an action and
/// a button both reach the same handler.
pub struct Hello {
    clicks: usize,
    focus: FocusHandle,
}

impl Hello {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            clicks: 0,
            focus: cx.focus_handle(),
        }
    }

    /// The action handler; the button calls the same method so the test covers
    /// the path both inputs take.
    fn increment(&mut self, _: &Increment, _: &mut Window, cx: &mut Context<Self>) {
        self.clicks += 1;
        cx.notify();
    }
}

impl Render for Hello {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let counter = SharedString::from(format!("点击次数 / Clicks: {}", self.clicks));
        div()
            .id("tterm-gpui")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .p_8()
            .bg(white())
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::increment))
            .child(div().text_2xl().font_bold().child("tTerm GPUI 原型"))
            .child(div().text_center().child(GREETING))
            .child(
                Button::new("increment")
                    .label("点我 +1 / Click me +1")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.increment(&Increment, window, cx);
                    })),
            )
            .child(
                div()
                    .id("clicks")
                    .test_support()
                    .aria_label(counter.clone())
                    .child(counter),
            )
    }
}

/// Starts the application and opens the only window the prototype has.
///
/// Closing the window quits, and so does Cmd+Q: GPUI forwards the key to the
/// binding below, whose handler shuts the application down.
pub fn run() {
    gpui_kit::application().run(|cx: &mut App| {
        gpui_kit::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        // macOS keeps an application alive after its last window closes;
        // closing the prototype window should end the process instead.
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        open_window(cx);
    });
}

/// Opens the prototype window, reporting a failure instead of panicking.
///
/// `open_window` returns a `Result` whose error is `anyhow::Error`; the
/// production binary must not unwrap it (AGENTS.md).
fn open_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(120.), px(120.)),
            size: size(px(720.), px(480.)),
        })),
        ..WindowOptions::default()
    };
    match gpui_kit::open_window(options, cx, |_, cx| cx.new(Hello::new)) {
        Ok((handle, _)) => {
            let titled = handle.update(cx, |_, window, _| {
                window.set_window_title("tTerm GPUI Dev");
            });
            if titled.is_err() {
                eprintln!("tterm-gpui: the window opened but could not be titled");
            }
        }
        Err(error) => {
            eprintln!("tterm-gpui: could not open the main window: {error}");
            cx.quit();
        }
    }
}

#[cfg(test)]
mod tests;
