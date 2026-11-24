//! Command and search marks on the overview ruler inside the scrollbar track.
//!
//! The ruler uses the same retained-history coordinates as the model's
//! [`CommandMarkPosition`] list and the same [`ScrollbarGeometry`] track as
//! the thumb. Marks are merged into one rectangle per logical pixel row and
//! the source list is capped, so a dense history cannot turn a frame into
//! thousands of quads.
//!
//! Search results are not produced yet. The model's current search API only
//! finds the next match, not a full scrollback set with a stable generation.
//! P2-07 must supply absolute lines that it can re-scan after an eviction;
//! production leaves [`OverviewSearchMark`] lists empty until then, and no
//! search UI is drawn here.

use tterm_terminal::command_marks::CommandMarkPosition;
use tterm_terminal::palette::Rgba;
use tterm_terminal::snapshot::Snapshot;

use super::scrollbar::ScrollbarGeometry;

/// The xterm overview rule divides its 10 px track into a one-pixel border and
/// three equal zones. Command marks sit on the left and search marks in the
/// centre, as the frontend registers them.
const BORDER: f32 = 1.0;
const TRACK_WIDTH: f32 = 10.0;
const OUTER_WIDTH: f32 = 3.0;
const CENTER_WIDTH: f32 = 3.0;
const COMMAND_X: f32 = BORDER;
const SEARCH_X: f32 = BORDER + OUTER_WIDTH;

/// xterm's fixed overview colors for finished commands and matches.
///
/// The command colors come from the frontend's `commandMarks.ts`; the search
/// colors come from `searchTypes.ts`. Both are fixed overview colors, not
/// palette-derived colors, because xterm keeps them independent of the theme.
pub(super) const SUCCEEDED_COLOR: Rgba = Rgba::rgb(0x22, 0xc5, 0x5e);
pub(super) const FAILED_COLOR: Rgba = Rgba::rgb(0xef, 0x44, 0x44);
pub(super) const MATCH_COLOR: Rgba = Rgba::rgb(0xfa, 0xcc, 0x15);
pub(super) const ACTIVE_MATCH_COLOR: Rgba = Rgba::rgb(0xfb, 0x92, 0x3c);

/// The shortest and tallest one-line mark in logical pixels.
const MIN_MARK_HEIGHT: f32 = 6.0;
const MAX_MARK_HEIGHT: f32 = 12.0;
/// The most source marks considered for one frame.
const MAX_SOURCE_MARKS: usize = 4096;

/// One future search match on the overview ruler.
///
/// `line` uses the same retained-history coordinate as command marks: zero is
/// the oldest row the model still holds. `active` selects the current match's
/// color. P2-07 owns producing these values and re-scanning them when rows are
/// evicted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewSearchMark {
    /// The absolute line of the match.
    pub line: i64,
    /// Whether this is the active match.
    pub active: bool,
}

/// A rectangle painted inside the scrollbar track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverviewRect {
    /// Left edge relative to the track.
    pub x: f32,
    /// Top edge relative to the track.
    pub y: f32,
    /// Width in logical pixels.
    pub width: f32,
    /// Height in logical pixels.
    pub height: f32,
    /// The mark's fixed overview color.
    pub color: Rgba,
}

/// The resolved marks for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OverviewLayout {
    /// Marks after merging adjacent rows; at most one run per pixel row and
    /// zone.
    pub rects: Vec<OverviewRect>,
}

/// Resolves command and search marks into track rectangles.
///
/// `history_lines + screen_lines` is the same total xterm maps decorations
/// against. The alternate screen has its own hidden history, and an empty
/// history has no ruler to draw, so both produce no rectangles.
pub(super) fn layout(
    geometry: &ScrollbarGeometry,
    history_lines: usize,
    screen_lines: usize,
    command_marks: &[CommandMarkPosition],
    search_marks: &[OverviewSearchMark],
    alternate_screen: bool,
) -> OverviewLayout {
    let track = geometry.track;
    if alternate_screen || history_lines == 0 || track < 1.0 {
        return OverviewLayout::default();
    }
    let total_lines = history_lines.saturating_add(screen_lines);
    if total_lines == 0 {
        return OverviewLayout::default();
    }
    let rows = track.ceil() as usize;
    let height = (track / total_lines as f32)
        .clamp(MIN_MARK_HEIGHT, MAX_MARK_HEIGHT)
        .round()
        .min(track)
        .max(1.0);
    let mut command_rows = vec![None; rows];
    let mut search_rows = vec![None; rows];

    let commands = &command_marks[command_marks.len().saturating_sub(MAX_SOURCE_MARKS)..];
    for mark in commands {
        let Some(exit_code) = mark.exit_code else {
            continue;
        };
        if mark.prompt_line < 0 {
            continue;
        }
        let color = if exit_code == 0 {
            SUCCEEDED_COLOR
        } else {
            FAILED_COLOR
        };
        paint(
            &mut command_rows,
            geometry,
            mark.prompt_line,
            total_lines,
            track,
            height,
            color,
        );
    }

    let searches = &search_marks[search_marks.len().saturating_sub(MAX_SOURCE_MARKS)..];
    for mark in searches {
        let color = if mark.active {
            ACTIVE_MATCH_COLOR
        } else {
            MATCH_COLOR
        };
        paint(
            &mut search_rows,
            geometry,
            mark.line,
            total_lines,
            track,
            height,
            color,
        );
    }

    let mut rects = Vec::new();
    collect_runs(&command_rows, COMMAND_X, OUTER_WIDTH, track, &mut rects);
    collect_runs(&search_rows, SEARCH_X, CENTER_WIDTH, track, &mut rects);
    OverviewLayout { rects }
}

/// Resolves the ruler from a frame's snapshot and the view's search input.
pub(super) fn layout_for_snapshot(
    geometry: &ScrollbarGeometry,
    snapshot: &Snapshot,
    search_marks: &[OverviewSearchMark],
) -> OverviewLayout {
    layout(
        geometry,
        snapshot.history_size(),
        snapshot.grid().screen_lines,
        snapshot.command_marks(),
        search_marks,
        snapshot
            .modes()
            .contains(tterm_terminal::mode::Modes::ALT_SCREEN),
    )
}

fn paint(
    rows: &mut [Option<Rgba>],
    geometry: &ScrollbarGeometry,
    line: i64,
    total_lines: usize,
    track: f32,
    height: f32,
    color: Rgba,
) {
    let Some(position) = geometry.line_fraction(line, total_lines) else {
        return;
    };
    let top = (position - height / 2.0).round();
    let top = top.clamp(0.0, (track - height).max(0.0));
    let start = top.floor().max(0.0) as usize;
    let end = ((top + height).ceil() as usize).min(rows.len());
    for row in rows.iter_mut().take(end).skip(start) {
        *row = Some(color);
    }
}

fn collect_runs(
    rows: &[Option<Rgba>],
    x: f32,
    width: f32,
    track: f32,
    rects: &mut Vec<OverviewRect>,
) {
    let mut start = 0;
    while start < rows.len() {
        let Some(color) = rows[start] else {
            start += 1;
            continue;
        };
        let mut end = start + 1;
        while end < rows.len() && rows[end] == Some(color) {
            end += 1;
        }
        let height = (end - start) as f32;
        rects.push(OverviewRect {
            x,
            y: start as f32,
            width: width.min(TRACK_WIDTH - x),
            height: height.min((track - start as f32).max(0.0)),
            color,
        });
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(line: i64, exit_code: i64) -> CommandMarkPosition {
        CommandMarkPosition {
            prompt_line: line,
            output_line: Some(line + 1),
            end_line: Some(line + 2),
            exit_code: Some(exit_code),
        }
    }

    fn search(line: i64, active: bool) -> OverviewSearchMark {
        OverviewSearchMark { line, active }
    }

    #[test]
    fn empty_marks_and_the_alternate_screen_paint_nothing() {
        let geometry = ScrollbarGeometry::new(200.0, 20, 100, 0);
        assert!(layout(&geometry, 20, 100, &[], &[], false).rects.is_empty());
        assert!(
            layout(
                &geometry,
                20,
                100,
                &[command(10, 0)],
                &[search(20, false)],
                true
            )
            .rects
            .is_empty()
        );
    }

    #[test]
    fn command_marks_use_the_left_zone_and_search_marks_the_centre() {
        let geometry = ScrollbarGeometry::new(200.0, 20, 100, 0);
        let layout = layout(
            &geometry,
            20,
            100,
            &[command(10, 0), command(60, 1)],
            &[search(30, false), search(90, true)],
            false,
        );
        let command_rects: Vec<_> = layout
            .rects
            .iter()
            .filter(|rect| rect.x == COMMAND_X)
            .collect();
        let search_rects: Vec<_> = layout
            .rects
            .iter()
            .filter(|rect| rect.x == SEARCH_X)
            .collect();
        assert_eq!(command_rects.len(), 2);
        assert_eq!(search_rects.len(), 2);
        assert!(command_rects[0].y < command_rects[1].y);
        assert_eq!(command_rects[0].color, SUCCEEDED_COLOR);
        assert_eq!(command_rects[1].color, FAILED_COLOR);
        assert_eq!(search_rects[0].color, MATCH_COLOR);
        assert_eq!(search_rects[1].color, ACTIVE_MATCH_COLOR);
        assert!(layout.rects.iter().all(|rect| rect.width > 0.0));
        assert!(
            layout
                .rects
                .iter()
                .all(|rect| rect.y >= 0.0 && rect.y + rect.height <= geometry.track)
        );
    }

    #[test]
    fn dense_marks_merge_into_at_most_one_run_per_pixel_row_and_zone() {
        let geometry = ScrollbarGeometry::new(300.0, 20, 1000, 0);
        let commands: Vec<_> = (0..5_000).map(|line| command(line, line % 2)).collect();
        let searches: Vec<_> = (0..5_000).map(|line| search(line, line % 3 == 0)).collect();
        let layout = layout(&geometry, 5_000, 1_000, &commands, &searches, false);
        assert!(
            layout.rects.len() <= geometry.track.ceil() as usize * 2,
            "the paint count is bounded by the track's pixel rows"
        );
    }

    #[test]
    fn empty_history_paints_nothing_even_with_marks() {
        let geometry = ScrollbarGeometry::new(200.0, 20, 0, 0);
        assert!(
            layout(
                &geometry,
                0,
                24,
                &[command(0, 0)],
                &[search(0, true)],
                false
            )
            .rects
            .is_empty(),
            "the overview ruler only maps retained history"
        );
    }

    #[test]
    fn empty_search_source_leaves_the_command_ruler_alone() {
        let geometry = ScrollbarGeometry::new(200.0, 20, 100, 0);
        let layout = layout(&geometry, 20, 100, &[command(10, 0)], &[], false);
        assert_eq!(layout.rects.len(), 1);
        assert_eq!(layout.rects[0].x, COMMAND_X);
        assert_eq!(layout.rects[0].color, SUCCEEDED_COLOR);
    }

    #[test]
    fn absolute_lines_map_from_the_oldest_history_row_to_the_newest_screen_row() {
        let geometry = ScrollbarGeometry::new(240.0, 24, 240, 0);
        let oldest = layout(&geometry, 240, 24, &[command(0, 0)], &[], false);
        let newest = layout(&geometry, 240, 24, &[command(263, 0)], &[], false);
        let oldest = oldest.rects[0];
        let newest = newest.rects[0];
        assert!(oldest.y < newest.y);
        assert!(oldest.y >= 0.0);
        assert!(newest.y + newest.height <= geometry.track);
    }

    #[test]
    fn marks_on_one_row_share_a_rectangle() {
        let geometry = ScrollbarGeometry::new(200.0, 20, 100, 0);
        let commands: Vec<_> = (0..50).map(|_| command(80, 0)).collect();
        let layout = layout(&geometry, 20, 100, &commands, &[], false);
        assert_eq!(layout.rects.len(), 1);
    }
}
