//! P0-10's drawing benchmark.
//!
//! Runs three scenarios against a headless 1200x800 window (DPR 1 on this
//! machine) and prints p50, p95 and maximum for the request-layout, prepaint
//! and paint phases, plus the row-cache hit rate:
//!
//! - idle: 120 draws with no new output;
//! - stream: one colored line per frame, the steady `tail -f` case;
//! - tui: a full screen of true-color text and wide characters per frame.
//!
//! Frame times come from GPUI's profiler (`gpui-pre`'s `FrameTiming.phases`),
//! not from hand-placed timers around the element.
//!
//! ```sh
//! cargo run -p tterm-gpui --release --example terminal_bench
//! ```

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::profiler::{FrameEvent, FrameTimingCollector, set_trace_enabled};
use gpui_kit::{AppContext, Entity, HeadlessAppContext, px, size};
use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::terminal::{Config, Terminal};
use tterm_terminal_view::view::{TerminalView, TerminalViewConfig, ViewStats};

const WINDOW_WIDTH: f32 = 1200.0;
const WINDOW_HEIGHT: f32 = 800.0;

#[derive(Default, Clone)]
struct PhaseSamples {
    request_layout: Vec<u64>,
    prepaint: Vec<u64>,
    paint: Vec<u64>,
    total: Vec<u64>,
}

impl PhaseSamples {
    fn push(&mut self, phases: gpui_kit::profiler::DrawPhases) {
        let layout = phases.request_layout.as_nanos() as u64;
        let prepaint = phases.prepaint.as_nanos() as u64;
        let paint = phases.paint.as_nanos() as u64;
        self.request_layout.push(layout);
        self.prepaint.push(prepaint);
        self.paint.push(paint);
        self.total.push(layout + prepaint + paint);
    }

    fn summary(values: &[u64]) -> (f64, f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let p50 = sorted[(sorted.len() * 50 / 100).min(sorted.len() - 1)];
        let p95 = sorted[(sorted.len() * 95 / 100).min(sorted.len() - 1)];
        let max = *sorted.last().expect("the list is not empty");
        (
            p50 as f64 / 1000.0,
            p95 as f64 / 1000.0,
            max as f64 / 1000.0,
        )
    }
}

struct ScenarioResult {
    name: &'static str,
    frames: u64,
    phases: PhaseSamples,
    stats: ViewStats,
    stats_before: ViewStats,
}

impl ScenarioResult {
    fn prepared_frames(&self) -> u64 {
        self.stats.frames.saturating_sub(self.stats_before.frames)
    }
}

impl ScenarioResult {
    fn print(&self) {
        let (rl50, rl95, rlmax) = PhaseSamples::summary(&self.phases.request_layout);
        let (pp50, pp95, ppmax) = PhaseSamples::summary(&self.phases.prepaint);
        let (p50, p95, pmax) = PhaseSamples::summary(&self.phases.paint);
        let (t50, t95, tmax) = PhaseSamples::summary(&self.phases.total);
        let hits = self
            .stats
            .cache_hits
            .saturating_sub(self.stats_before.cache_hits);
        let misses = self
            .stats
            .cache_misses
            .saturating_sub(self.stats_before.cache_misses);
        let rate = if hits + misses == 0 {
            0.0
        } else {
            100.0 * hits as f64 / (hits + misses) as f64
        };
        println!(
            "# {} hits={} misses={} damage={} blink_reuses={} rows_laid_out={}",
            self.name,
            hits,
            misses,
            self.stats
                .damage_reuses
                .saturating_sub(self.stats_before.damage_reuses),
            self.stats
                .blink_reuses
                .saturating_sub(self.stats_before.blink_reuses),
            self.stats
                .rows_laid_out
                .saturating_sub(self.stats_before.rows_laid_out),
        );
        println!(
            "{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{:.2}",
            self.name,
            self.frames,
            self.prepared_frames(),
            rl50,
            rl95,
            rlmax,
            pp50,
            pp95,
            ppmax,
            p50,
            p95,
            pmax,
            t50,
            t95,
            tmax,
            hits,
            rate
        );
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("terminal_bench: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let terminal = Arc::new(Terminal::new(
        Config::default(),
        GridSize::new(80, 24),
        CellSize::new(8, 16),
    ));
    let platform = gpui_kit::platform::current_platform(true);
    let mut context =
        HeadlessAppContext::with_platform(platform.text_system(), Arc::new(()), || {
            gpui_kit::platform::current_headless_renderer()
        });
    let view_slot: Rc<RefCell<Option<Entity<TerminalView>>>> = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&view_slot);
    let terminal_for_view = Arc::clone(&terminal);
    let window = context
        .open_window(size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)), |_, cx| {
            let view = cx
                .new(|cx| TerminalView::new(cx, terminal_for_view, TerminalViewConfig::default()));
            *captured.borrow_mut() = Some(view.clone());
            view
        })
        .map_err(|error| error.to_string())?;
    let view = view_slot
        .borrow()
        .clone()
        .ok_or_else(|| "the view was not created".to_string())?;
    context.run_until_parked();
    context
        .update_window(window.into(), |_, window, _| {
            println!(
                "# viewport={:?} scale_factor={}",
                window.viewport_size(),
                window.scale_factor()
            );
        })
        .map_err(|error| error.to_string())?;

    // The first draw measures the font and resizes the model; it is not part
    // of any scenario.
    draw(&mut context, window.into())?;
    draw(&mut context, window.into())?;
    println!(
        "# grid={:?}",
        context.update(|cx| view.read(cx).grid_size())
    );

    set_trace_enabled(true);
    let mut collector = FrameTimingCollector::new();
    let mut results = Vec::new();

    let idle = run_scenario(
        &mut context,
        window.into(),
        &view,
        "idle",
        120,
        |_| {},
        &mut collector,
    )?;
    results.push(idle);

    let stream = run_scenario(
        &mut context,
        window.into(),
        &view,
        "stream",
        240,
        |frame| {
            let line = format!(
                "\x1b[38;2;{};{};{}m{:03} The quick brown fox jumps over the lazy dog \
                 \x1b[38;5;{}m中文🙂\x1b[0m\r\n",
                (frame * 7) % 256,
                (frame * 13) % 256,
                (frame * 29) % 256,
                frame % 1000,
                16 + (frame % 200)
            );
            terminal.write(line.as_bytes());
        },
        &mut collector,
    )?;
    results.push(stream);

    let tui = run_scenario(
        &mut context,
        window.into(),
        &view,
        "tui",
        120,
        |frame| {
            let mut screen = String::from("\x1b[H");
            for row in 0..40 {
                let red = (row * 5 + frame * 3) % 256;
                let green = (row * 11 + frame * 7) % 256;
                let blue = (row * 17 + frame * 13) % 256;
                screen.push_str(&format!("\x1b[38;2;{red};{green};{blue}m"));
                screen.push_str(&format!(
                    "{row:02} 0123456789 中文🙂 {} ░▒▓█ ┌─┐\x1b[0m\r\n",
                    "x".repeat((row as usize + frame as usize) % 20)
                ));
            }
            terminal.write(screen.as_bytes());
        },
        &mut collector,
    )?;
    results.push(tui);

    set_trace_enabled(false);

    println!(
        "scenario,frames,prepared_frames,layout_p50_us,layout_p95_us,layout_max_us,\
         prepaint_p50_us,prepaint_p95_us,prepaint_max_us,\
         paint_p50_us,paint_p95_us,paint_max_us,\
         total_p50_us,total_p95_us,total_max_us,cache_hits,cache_hit_percent"
    );
    for result in results {
        result.print();
    }
    Ok(())
}

fn draw(context: &mut HeadlessAppContext, window: gpui_kit::AnyWindowHandle) -> Result<(), String> {
    context
        .update_window(window, |_, window, cx| {
            window.refresh();
            let _ = window.draw(cx);
        })
        .map_err(|error| error.to_string())
}

fn run_scenario(
    context: &mut HeadlessAppContext,
    window: gpui_kit::AnyWindowHandle,
    view: &Entity<TerminalView>,
    name: &'static str,
    frames: u32,
    mut write: impl FnMut(u32),
    collector: &mut FrameTimingCollector,
) -> Result<ScenarioResult, String> {
    let before = context.update(|cx| view.read(cx).stats());
    for frame in 0..frames {
        write(frame);
        // `Window::draw` reuses the previous element tree unless something is
        // dirty. `refresh` forces a full render without scheduling a second
        // frame, as a real output batch would.
        draw(context, window)?;
    }
    let stats = context.update(|cx| view.read(cx).stats());
    let phases = collect_draws(collector);
    Ok(ScenarioResult {
        name,
        frames: u64::from(frames),
        phases,
        stats,
        stats_before: before,
    })
}

fn collect_draws(collector: &mut FrameTimingCollector) -> PhaseSamples {
    let mut phases = PhaseSamples::default();
    for event in collector.collect_unseen() {
        if let FrameEvent::Draw(timing) = event {
            phases.push(timing.phases);
        }
    }
    phases
}
