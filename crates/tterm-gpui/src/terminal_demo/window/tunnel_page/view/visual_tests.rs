//! Opt-in screenshots of the production page over the parity fixtures.
//!
//! The test uses a real off-screen window because GPUI has no Windows
//! headless renderer. It is ignored so the normal suite never opens a window.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Context, Entity, Render, Window, WindowBounds, WindowOptions, div, point, px, size,
};
use image::imageops;
use tterm_core::events::tunnel::{TunnelFailure, TunnelState, TunnelStatus, TunnelTraffic};
use tterm_core::i18n::language::Language;
use tterm_core::tunnel::types::TunnelRule;
use tterm_ui::tokens::palette::DARK;

use super::super::data::{TunnelPageModel, TunnelPageState, TunnelRow};
use super::TunnelPageView;
use crate::locale::Locale;

const VIEWPORT_WIDTH: f32 = 800.0;
const VIEWPORT_HEIGHT: f32 = 600.0;
const WINDOW_HEIGHT: f32 = 660.0;

struct Page {
    view: Entity<TunnelPageView>,
    model: TunnelPageModel,
}

impl Render for Page {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(VIEWPORT_WIDTH))
            .h(px(VIEWPORT_HEIGHT))
            .flex()
            .flex_col()
            .child(
                self.view
                    .update(cx, |view, cx| view.render_page(&self.model, cx)),
            )
    }
}

#[derive(serde::Deserialize)]
struct Fixture {
    commands: Commands,
}

#[derive(serde::Deserialize)]
struct Commands {
    #[serde(default)]
    list_profiles: Vec<Profile>,
    #[serde(default)]
    list_tunnels: Vec<TunnelRule>,
    #[serde(default)]
    list_tunnel_statuses: Vec<Status>,
}

#[derive(serde::Deserialize)]
struct Profile {
    id: String,
    name: String,
    username: Option<String>,
    host: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    id: String,
    state: State,
    message: Option<String>,
    bound_port: Option<u16>,
    active_connections: u64,
    total_connections: u64,
    failed_connections: u64,
    bytes_up: u64,
    bytes_down: u64,
    connected_at: Option<u64>,
    retry_attempt: u32,
    last_failure: Option<Failure>,
    lifetime: Traffic,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Failure {
    message: String,
    at: u64,
}

#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Traffic {
    bytes_up: u64,
    bytes_down: u64,
    connections: u64,
    since: Option<u64>,
}

#[derive(serde::Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum State {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error,
    #[serde(rename = "needsCredentials")]
    NeedsCredentials,
}

impl State {
    fn tunnel_state(self) -> TunnelState {
        match self {
            Self::Stopped => TunnelState::Stopped,
            Self::Starting => TunnelState::Starting,
            Self::Running => TunnelState::Running,
            Self::Reconnecting => TunnelState::Reconnecting,
            Self::Error => TunnelState::Error,
            Self::NeedsCredentials => TunnelState::NeedsCredentials,
        }
    }
}

impl Status {
    fn tunnel_status(&self) -> TunnelStatus {
        TunnelStatus {
            id: self.id.clone(),
            state: self.state.tunnel_state(),
            message: self.message.clone(),
            bound_port: self.bound_port,
            active_connections: self.active_connections,
            total_connections: self.total_connections,
            failed_connections: self.failed_connections,
            bytes_up: self.bytes_up,
            bytes_down: self.bytes_down,
            connected_at: self.connected_at,
            retry_attempt: self.retry_attempt,
            last_failure: self.last_failure.as_ref().map(|failure| TunnelFailure {
                message: failure.message.clone(),
                at: failure.at,
            }),
            lifetime: TunnelTraffic {
                bytes_up: self.lifetime.bytes_up,
                bytes_down: self.lifetime.bytes_down,
                connections: self.lifetime.connections,
                since: self.lifetime.since,
            },
        }
    }
}

/// Joins one fixture the same way the data controller joins a live snapshot.
fn model_from_fixture(json: &str) -> TunnelPageModel {
    let fixture: Fixture = serde_json::from_str(json).expect("the fixture parses");
    let profiles = fixture
        .commands
        .list_profiles
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<HashMap<_, _>>();
    let statuses = fixture
        .commands
        .list_tunnel_statuses
        .into_iter()
        .map(|status| (status.id.clone(), status))
        .collect::<HashMap<_, _>>();
    let rows = fixture
        .commands
        .list_tunnels
        .into_iter()
        .map(|rule| {
            let status = statuses
                .get(&rule.id)
                .map(Status::tunnel_status)
                .unwrap_or_else(|| TunnelStatus::stopped(&rule.id, TunnelTraffic::default()));
            let profile = profiles.get(&rule.profile_id);
            let host_label = match profile {
                Some(profile) if !profile.name.trim().is_empty() => profile.name.clone(),
                Some(profile) => format!(
                    "{}@{}",
                    profile.username.as_deref().unwrap_or_default(),
                    profile.host.as_deref().unwrap_or_default()
                ),
                None => String::new(),
            };
            TunnelRow {
                host_missing: profile.is_none(),
                rule,
                status,
                host_label,
            }
        })
        .collect::<Vec<_>>();
    let running_count = rows
        .iter()
        .filter(|row| row.status.state == TunnelState::Running)
        .count();
    TunnelPageModel {
        state: TunnelPageState::Ready,
        rules: rows,
        running_count,
    }
}

#[test]
#[ignore = "writes reference PNGs through a real off-screen window; run with --ignored"]
fn renders_parity_fixtures() {
    use_reference_dpi();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = root.join("target/parity/tunnel-page");
    fs::create_dir_all(&output).expect("the screenshot directory is writable");
    // This GPUI snapshot exports `VisualTestAppContext` only on macOS. A real
    // window can still render to an image on Windows, so the test uses the
    // same off-screen window path as the parity capture.
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        tterm_ui::init(cx);
        for (fixture, name) in [
            ("tunnels.empty.json", "tunnels.empty"),
            ("tunnels.list.running.json", "tunnels.list.running"),
        ] {
            let path = root.join("parity/fixtures").join(fixture);
            let json = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            let model = model_from_fixture(&json);
            capture(cx, model, &output.join(format!("gpui-{name}.png")));
        }
        cx.quit();
    });
}

fn capture(cx: &mut App, model: TunnelPageModel, image_path: &Path) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(-10_000.0), px(-10_000.0)),
            size: size(px(VIEWPORT_WIDTH), px(WINDOW_HEIGHT)),
        })),
        focus: false,
        show: true,
        ..WindowOptions::default()
    };
    let (handle, _) = gpui_kit::open_window(options, cx, |_, cx| {
        let locale = cx.new(|_| Locale::new(Language::En));
        let view = cx.new(|_| TunnelPageView::new(locale, DARK));
        cx.new(|_| Page { view, model })
    })
    .expect("the tunnel page window opens");
    handle
        .update(cx, |_, window, _| {
            window.set_scale_factor(1.0);
        })
        .expect("the tunnel page uses the reference viewport");
    for _ in 0..2 {
        handle
            .update(cx, |_, window, cx| {
                let _ = window.draw(cx);
            })
            .expect("the tunnel page draws");
    }
    let image = handle
        .update(cx, |_, window, _| window.render_to_image())
        .expect("the tunnel page window updates")
        .expect("the tunnel page renders to an image");
    let viewport = handle
        .update(cx, |_, window, _| window.viewport_size())
        .expect("the tunnel page reports its viewport");
    let scale = image.width() as f32 / viewport.width.as_f32();
    let crop_width = (VIEWPORT_WIDTH * scale).round() as u32;
    let crop_height = (VIEWPORT_HEIGHT * scale).round() as u32;
    assert!(
        crop_width <= image.width() && crop_height <= image.height(),
        "the window is too small for the fixed page: image {}x{}, crop {crop_width}x{crop_height}",
        image.width(),
        image.height()
    );
    let page_image = imageops::crop_imm(&image, 0, 0, crop_width, crop_height).to_image();
    let page_image = if page_image.width() == VIEWPORT_WIDTH as u32
        && page_image.height() == VIEWPORT_HEIGHT as u32
    {
        page_image
    } else {
        imageops::thumbnail(&page_image, VIEWPORT_WIDTH as u32, VIEWPORT_HEIGHT as u32)
    };
    println!(
        "tunnel page {}: image {}x{}, viewport {}x{}, scale {scale}, output {}x{}",
        image_path.display(),
        image.width(),
        image.height(),
        viewport.width.as_f32(),
        viewport.height.as_f32(),
        page_image.width(),
        page_image.height()
    );
    page_image
        .save(image_path)
        .unwrap_or_else(|error| panic!("write {}: {error}", image_path.display()));
    let _ = handle.update(cx, |_, window, _| window.remove_window());
}

#[cfg(windows)]
fn use_reference_dpi() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetThreadDpiAwarenessContext(context: isize) -> isize;
    }
    // An unaware window gets the 96 DPI baseline the Windows reference uses,
    // even when this machine's monitor runs at a higher scale.
    const DPI_AWARENESS_CONTEXT_UNAWARE: isize = -1;
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE);
    }
}

#[cfg(not(windows))]
fn use_reference_dpi() {}
