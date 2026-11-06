//! Preparing a frame: measure, resize, take a snapshot, lay out rows.

use std::sync::Arc;

use gpui_kit::{Bounds, Context, Pixels, Window};
use tterm_terminal::damage::Damage;
use tterm_terminal::grid::GridSize;

use super::blink::{BLINK_INTERVAL, BlinkState};
use super::font::{grid_for, measure_cell};
use super::frame::{FrameData, PreparedFrame, cursor_for};
use super::row::{plan_row, row_hash, shape_row};
use super::{TerminalView, bottom_anchored, grid_origin};

impl TerminalView {
    /// Prepares one frame from the current model snapshot.
    ///
    /// The cell grid is measured only when the font or scale factor changes.
    /// A resize is sent to the model when the measured grid or cell size
    /// changes. The snapshot is then taken once, and every visible row either
    /// hits the content cache, is reused because Alacritty did not damage it,
    /// or is planned and shaped.
    pub(crate) fn prepare(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> PreparedFrame {
        self.stats.frames = self.stats.frames.saturating_add(1);
        if !self.visible {
            return PreparedFrame::Empty {
                background: self.palette.background,
            };
        }

        let scale_factor = window.scale_factor();
        let text_system = window.text_system().clone();
        let mut metrics_changed = false;
        if self.geometry.is_none() || (self.geometry_scale - scale_factor).abs() > 1e-4 {
            let geometry = measure_cell(&text_system, &self.fonts, &self.font, scale_factor);
            if self.geometry != Some(geometry) {
                metrics_changed = true;
                self.style_epoch = self.style_epoch.wrapping_add(1);
                self.cache.clear();
                self.last_frame = None;
            }
            self.geometry = Some(geometry);
            self.geometry_scale = scale_factor;
        }
        let metrics = self
            .geometry
            .expect("the geometry was measured immediately above");

        let grid = grid_for(bounds.size.width, bounds.size.height, &metrics);
        let cell_size = metrics.model_cell_size();
        if metrics_changed || self.last_grid != Some(grid) {
            self.terminal.resize(grid, cell_size);
            if let Some(resize) = &self.resize {
                resize(grid, cell_size);
            }
            self.last_grid = Some(grid);
            self.stats.resizes = self.stats.resizes.saturating_add(1);
            self.last_frame = None;
        }

        let focused = self.focus_handle.is_focused(window);
        let window_active = window.is_window_active();
        let blink_only = self.blink_only
            && self
                .last_frame
                .as_ref()
                .is_some_and(|frame| frame.grid() == grid);
        self.blink_only = false;

        let data = if blink_only {
            self.stats.blink_reuses = self.stats.blink_reuses.saturating_add(1);
            self.last_frame
                .clone()
                .expect("the blink fast path requires a previous frame")
        } else {
            self.build_frame(grid, metrics, &text_system)
        };

        let cursor = cursor_for(&data.snapshot, focused);
        let should_blink = self.visible
            && BlinkState::should_run(
                true,
                focused,
                window_active,
                cursor.is_some(),
                data.snapshot.cursor().blinking,
            );
        self.update_blink(should_blink, cx);
        // Read the phase after the timer state is settled, so the frame that
        // loses focus shows the cursor instead of inheriting a hidden phase.
        let show_cursor = if data.snapshot.cursor().blinking && self.blink.running {
            self.blink.on
        } else {
            true
        };

        let origin = grid_origin(
            bounds,
            data.snapshot.grid(),
            &data.metrics,
            bottom_anchored(&data.snapshot),
            data.metrics.scale_factor,
        );
        PreparedFrame::Full {
            data,
            origin,
            cursor,
            show_cursor,
        }
    }

    fn build_frame(
        &mut self,
        grid: GridSize,
        metrics: super::font::CellGeometry,
        text_system: &std::rc::Rc<gpui_kit::WindowTextSystem>,
    ) -> Arc<FrameData> {
        let snapshot = self.terminal.snapshot();
        let previous = self.last_frame.clone();
        let damaged = damaged_rows(snapshot.damage(), snapshot.line_count());
        let mut rows = Vec::with_capacity(snapshot.line_count());
        let style_epoch = self.style_epoch;
        let palette = &self.palette;
        let fonts = &self.fonts;
        let cache = &mut self.cache;
        let cache_frame = self.cache_frame;
        let device_metrics = metrics.device_metrics();
        let font_size = metrics.font_size;

        for (index, row) in snapshot.rows().iter().enumerate() {
            if !damaged[index]
                && let Some(previous_row) =
                    previous.as_ref().and_then(|frame| frame.rows.get(index))
            {
                rows.push(Arc::clone(previous_row));
                cache.record_damage_reuse();
                self.stats.damage_reuses = self.stats.damage_reuses.saturating_add(1);
                continue;
            }
            let key = row_hash(row.cells(), style_epoch);
            let (layout, hit) = cache.get_or_insert_with(key, cache_frame, || {
                let plan = plan_row(row.cells(), palette, device_metrics);
                shape_row(plan, text_system, fonts, font_size)
            });
            if hit {
                self.stats.cache_hits = self.stats.cache_hits.saturating_add(1);
            } else {
                self.stats.cache_misses = self.stats.cache_misses.saturating_add(1);
                self.stats.rows_laid_out = self.stats.rows_laid_out.saturating_add(1);
            }
            rows.push(layout);
        }
        self.cache_frame = self.cache_frame.wrapping_add(1);
        // `grid` is the size the model was just resized to; keep the warning
        // away when a future model revision reports it differently.
        debug_assert_eq!(snapshot.grid(), grid);

        let frame = Arc::new(FrameData {
            snapshot,
            rows,
            metrics,
            palette: self.palette.clone(),
        });
        self.last_frame = Some(Arc::clone(&frame));
        frame
    }

    pub(crate) fn update_blink(&mut self, should_run: bool, cx: &mut Context<Self>) {
        if !should_run {
            if self.blink.set_running(false) {
                self.blink_task.take();
            }
            return;
        }
        self.blink.set_running(true);
        if self.blink_task.is_some() {
            return;
        }
        let task = cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                let keep_going = weak
                    .update(cx, |view, cx| {
                        if view.blink_task.is_none() {
                            return false;
                        }
                        view.blink.toggle();
                        view.stats.blink_toggles = view.stats.blink_toggles.saturating_add(1);
                        view.blink_only = true;
                        view.request_redraw(cx);
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        });
        self.blink_task = Some(task);
    }
}

fn damaged_rows(damage: &Damage, line_count: usize) -> Vec<bool> {
    let mut damaged = vec![false; line_count];
    match damage {
        Damage::Full => damaged.fill(true),
        Damage::Rows(rows) => {
            for row in rows {
                if let Some(entry) = damaged.get_mut(row.line) {
                    *entry = true;
                }
            }
        }
    }
    damaged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_damage_marks_every_row() {
        assert_eq!(damaged_rows(&Damage::Full, 3), [true, true, true]);
    }

    #[test]
    fn partial_damage_marks_only_the_named_rows() {
        let damage = Damage::Rows(vec![
            tterm_terminal::damage::DamagedLine {
                line: 0,
                left: 0,
                right: 1,
            },
            tterm_terminal::damage::DamagedLine {
                line: 2,
                left: 4,
                right: 8,
            },
        ]);
        assert_eq!(damaged_rows(&damage, 3), [true, false, true]);
    }

    #[test]
    fn an_out_of_range_damage_row_is_ignored() {
        let damage = Damage::Rows(vec![tterm_terminal::damage::DamagedLine {
            line: 99,
            left: 0,
            right: 1,
        }]);
        assert_eq!(damaged_rows(&damage, 2), [false, false]);
    }
}
