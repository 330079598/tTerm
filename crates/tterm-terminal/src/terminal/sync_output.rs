//! Aborting a synchronized update whose program never sends ESU.
//!
//! Alacritty's parser starts a 150 ms timer on BSU (DECSET 2026), but only its
//! own event loop checks it. A client that owns the parser must do the same,
//! or an interrupted full-screen redraw would stay invisible until the 2 MiB
//! buffer limit.

use std::time::Instant;

use alacritty_terminal::term::TermMode;

use super::*;

impl Terminal {
    /// When an open synchronized update should be aborted by its owner.
    ///
    /// The deadline is recorded when the parser sees BSU, even before any
    /// bytes have been buffered.
    pub fn synchronized_output_deadline(&self) -> Option<Instant> {
        self.state.lock().parser.sync_timeout().sync_timeout()
    }

    /// Ends an open synchronized update once its deadline has passed.
    ///
    /// Returns whether this call aborted one. The buffered bytes are applied
    /// in stream order, so model replies they contain reach the session at the
    /// same point as if ESU had arrived.
    pub fn flush_expired_synchronized_output(&self) -> bool {
        self.flush_synchronized_output_at(Instant::now())
    }

    /// The time-injectable form of
    /// [`Terminal::flush_expired_synchronized_output`], so tests do not sleep
    /// for the parser's timeout.
    pub(crate) fn flush_synchronized_output_at(&self, now: Instant) -> bool {
        let mut state = self.state.lock();
        let Some(deadline) = state.parser.sync_timeout().sync_timeout() else {
            return false;
        };
        if now < deadline {
            return false;
        }

        state.content_version = state.content_version.wrapping_add(1);
        state.word_drag = None;
        let was_alternate = state.term.mode().contains(TermMode::ALT_SCREEN);
        let cleared_main;
        {
            let State {
                parser,
                term,
                command_marks,
                scroll_region,
                ordered_replies,
                ..
            } = &mut *state;
            let mut region = *scroll_region;
            let mut handler = match ordered_replies.as_mut() {
                Some(ordered) => HistoryRelease::with_region_and_markers(
                    term,
                    region,
                    ordered,
                    self.intents.as_ref(),
                ),
                None => HistoryRelease::with_region(term, region),
            };
            parser.stop_sync(&mut handler);
            region = handler.region();
            cleared_main = handler.cleared_main();
            apply_mark_changes(command_marks, handler.mark_changes());
            *scroll_region = region;
        }
        let is_alternate = state.term.mode().contains(TermMode::ALT_SCREEN);
        if was_alternate != is_alternate || cleared_main {
            state.scrollback_generation = state.scrollback_generation.wrapping_add(1);
        }
        if let Some(ordered) = state.ordered_replies.as_mut() {
            ordered.after_write(&[], MarkerSource::Remote, true, 0);
        }
        true
    }
}
