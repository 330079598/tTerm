//! Independent P0-19 native drag probe. No terminal, SSH or application data is opened.

use gpui_kit::prelude::*;
use gpui_kit::{
    Bounds, MouseButton, Point, Render, Window, WindowBounds, WindowOptions, div, point, px, rgb,
    size,
};

#[cfg(windows)]
mod native;
#[cfg(test)]
mod tests;
mod transfer;

const TITLE: &str = "tTerm P0-19 drag-out probe";

struct Probe {
    press: Option<Point<gpui_kit::Pixels>>,
    busy: bool,
    message: String,
    target: std::path::PathBuf,
}

impl Probe {
    #[cfg(windows)]
    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !window.is_window_active() {
            return;
        }
        let drag = match native::Drag::prepare(&self.target) {
            Ok(drag) => drag,
            Err(error) => {
                self.message = error;
                cx.notify();
                return;
            }
        };
        self.busy = true;
        // This is the first content work: the pointer has crossed the drag threshold.
        // Generation and the eventual copy stay off the GPUI thread.
        let folder = drag.folder.clone();
        let generated = cx
            .background_executor()
            .spawn(async move { transfer::generate(&folder) });
        let outcome = drag.run(window);
        let target = self.target.clone();
        cx.spawn(async move |view, cx| {
            let mut drag = match outcome {
                Ok(drag) => drag,
                Err(error) => {
                    let _ = generated.await;
                    let _ = view.update(cx, |view, cx| view.finish(Err(error), cx));
                    return;
                }
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !drag.has_copy() && drag.accepted && std::time::Instant::now() < deadline {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
            }
            let copied = drag.finish();
            let generated = generated.await;
            let result = cx
                .background_executor()
                .spawn(async move {
                    let file = generated?;
                    if !copied? {
                        return Ok(
                            "Cancelled or unknown target; generated staging content discarded"
                                .into(),
                        );
                    }
                    transfer::publish(&file, &target)
                })
                .await;
            let _ = view.update(cx, |view, cx| view.finish(result, cx));
        })
        .detach();
    }

    #[cfg(not(windows))]
    fn start(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.message = "Native delayed drag is not implemented on this platform; see README".into();
        cx.notify();
    }

    #[cfg(windows)]
    fn finish(&mut self, result: Result<String, String>, cx: &mut Context<Self>) {
        self.busy = false;
        self.message = result.unwrap_or_else(|error| error);
        println!("{}", self.message);
        cx.notify();
    }
}

impl Render for Probe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_6()
            .bg(rgb(0x18212c))
            .text_color(rgb(0xffffff))
            .flex()
            .flex_col()
            .gap_4()
            .child(TITLE)
            .child(format!("Drop only into: {}", self.target.display()))
            .child(
                div()
                    .id("drag-file")
                    .p_6()
                    .bg(rgb(0x345779))
                    .child("Drag: generated-200MB.bin (200,000,000 bytes)")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|view, event: &gpui_kit::MouseDownEvent, _, _| {
                            if !view.busy {
                                view.press = Some(event.position);
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|view, _, _, _| view.press = None),
                    )
                    .on_mouse_move(cx.listener(
                        |view, event: &gpui_kit::MouseMoveEvent, window, cx| {
                            let Some(start) = view.press else {
                                return;
                            };
                            if !event.dragging() {
                                view.press = None;
                                return;
                            }
                            let dx = f32::from(event.position.x - start.x);
                            let dy = f32::from(event.position.y - start.y);
                            if dx.hypot(dy) >= 4.0 {
                                view.press = None;
                                view.start(window, cx);
                            }
                        },
                    )),
            )
            .child(self.message.clone())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.as_slice() == ["--local-check"] {
        if let Err(error) = transfer::local_check() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let target = match args.as_slice() {
        [flag, target] if flag == "--target" => {
            transfer::validate_target(std::path::Path::new(target))
        }
        _ => Err(
            "Usage: p0_19_drag_out --target <new empty directory under TEMP> | --local-check"
                .into(),
        ),
    };
    let target = match target {
        Ok(target) => target,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(80.), px(80.)),
                size: size(px(680.), px(340.)),
            })),
            app_id: Some("com.stone.tTerm.gpui-dev".into()),
            ..Default::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
            window.set_window_title(TITLE);
            cx.new(|_| Probe {
                press: None,
                busy: false,
                message: "No content generated before drag".into(),
                target,
            })
        }) {
            eprintln!("Could not open probe: {error}");
            cx.quit();
        }
    });
}
