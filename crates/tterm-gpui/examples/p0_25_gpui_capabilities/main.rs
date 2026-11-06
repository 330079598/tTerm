//! P0-25's drawing capability probe: the command line, the window and the PNG.
//!
//! `--screenshot` renders one scene through the headless Metal renderer and
//! writes a PNG, which is how the report's assets are produced without
//! screen-recording permission. `--window --scene <name>` opens the same scene
//! in a real window, the only way to see the window-level blur; `--window`
//! alone stacks every scene. The scenes themselves live in the [`scenes`]
//! module.

use std::path::Path;

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, HeadlessAppContext, Render, Window, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, div, point, px, rgb, size,
};

mod scenes;

use scenes::{PAGE, Scene, TEXT, section};

/// The view the window and the screenshot both build.
struct Probe {
    scenes: Vec<Scene>,
}

impl Render for Probe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // The blur scene is only worth looking at when nothing paints the page
        // color behind it, otherwise the window is opaque and nothing shows.
        let transparent = self.scenes == [Scene::BlurWindow];
        let mut root = div().size_full().flex().flex_col().gap_6().p_5();
        if !transparent {
            root = root.bg(rgb(PAGE));
        }
        root = root.text_color(rgb(TEXT)).text_sm();
        for scene in &self.scenes {
            root = root.child(section(*scene));
        }
        root
    }
}

/// What the probe was asked to do.
enum Command {
    /// Print the usage and the scene list.
    Usage,
    /// Write one scene to a PNG through the headless renderer.
    Capture { path: String, scene: Scene },
    /// Open a real window, with every scene when `scene` is `None`.
    Window(Option<Scene>),
}

/// Turns the arguments into a [`Command`].
fn command(arguments: &[String]) -> Result<Command, String> {
    let mut scene = None;
    let mut screenshot = None;
    let mut window = false;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--scene" => {
                index += 1;
                let name = arguments.get(index).ok_or("--scene needs a name")?;
                scene =
                    Some(Scene::from_name(name).ok_or_else(|| format!("unknown scene: {name}"))?);
            }
            "--screenshot" => {
                index += 1;
                let path = arguments.get(index).ok_or("--screenshot needs a path")?;
                screenshot = Some(path.clone());
            }
            "--window" => window = true,
            other => return Err(format!("unknown argument: {other}")),
        }
        index += 1;
    }
    match (screenshot, window) {
        (Some(path), _) => {
            let scene = scene.ok_or("--screenshot also needs --scene")?;
            Ok(Command::Capture { path, scene })
        }
        (None, true) => Ok(Command::Window(scene)),
        (None, false) => Ok(Command::Usage),
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match command(&arguments) {
        Ok(Command::Usage) => usage(),
        Ok(Command::Window(scene)) => run_window(scene),
        Ok(Command::Capture { path, scene }) => match capture(Path::new(&path), scene) {
            Ok(()) => println!("p0_25_gpui_capabilities: wrote {path}"),
            Err(error) => {
                eprintln!("p0_25_gpui_capabilities: could not capture the scene: {error}");
                std::process::exit(1);
            }
        },
        Err(error) => {
            eprintln!("p0_25_gpui_capabilities: {error}");
            usage();
            std::process::exit(2);
        }
    }
}

/// Prints the usage and the scenes.
fn usage() {
    println!("usage: p0_25_gpui_capabilities [--window [--scene <name>]]");
    println!("       p0_25_gpui_capabilities --scene <name> --screenshot <path>");
    println!();
    for scene in Scene::ALL {
        println!("  {:<16} {}", scene.name(), scene.caption());
    }
}

/// Renders one scene headlessly and saves the PNG.
fn capture(path: &Path, scene: Scene) -> Result<(), String> {
    let text_system = gpui_kit::platform::current_platform(true).text_system();
    let mut context =
        HeadlessAppContext::with_platform(text_system, std::sync::Arc::new(()), || {
            gpui_kit::platform::current_headless_renderer()
        });
    let window = context
        .open_window(scene.window_size(), |_, cx| {
            cx.new(|_| Probe {
                scenes: vec![scene],
            })
        })
        .map_err(|error| error.to_string())?;
    context.run_until_parked();
    let image = context
        .capture_screenshot(window.into())
        .map_err(|error| error.to_string())?;
    image.save(path).map_err(|error| error.to_string())
}

/// Opens a real window. The blur scene asks for a blurred window background,
/// which is the only way to see what the compositor does behind the content.
fn run_window(scene: Option<Scene>) {
    let scenes: Vec<Scene> = match scene {
        Some(scene) => vec![scene],
        None => Scene::ALL.to_vec(),
    };
    let window_background = if scenes == [Scene::BlurWindow] {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    };
    let window_size = match scene {
        Some(scene) => scene.window_size(),
        None => size(px(780.), px(1500.)),
    };
    gpui_kit::application().run(move |cx: &mut App| {
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
                size: window_size,
            })),
            window_background,
            ..WindowOptions::default()
        };
        match gpui_kit::open_window(options, cx, move |_, cx| cx.new(move |_| Probe { scenes })) {
            Ok((handle, _)) => {
                if handle
                    .update(cx, |_, window, _| {
                        window.set_window_title("tTerm P0-25 probe")
                    })
                    .is_err()
                {
                    eprintln!("p0_25_gpui_capabilities: the window opened but could not be titled");
                }
            }
            Err(error) => {
                eprintln!("p0_25_gpui_capabilities: could not open the window: {error}");
                cx.quit();
            }
        }
    });
}
