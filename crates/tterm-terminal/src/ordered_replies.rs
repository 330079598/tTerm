//! Provenance for the private mode report that orders SSH fixed replies.
//!
//! A fixed reply from the SSH adapter can overtake model replies the parser
//! has buffered in a synchronized update. The session therefore inserts a
//! reserved private mode query at the fixed reply's position in the stream
//! and turns the report into an ordering intent. A program may send the same
//! query itself, so the occurrence needs its source before the model handler
//! runs. Only synthetic occurrences become an ordering intent; real queries
//! still receive their ordinary mode report.
//!
//! Ordinary output is not parsed a second time. The session copies the
//! parser's synchronized buffer, and a syntax-only shadow parser runs only
//! over bytes that lead up to a synthetic marker. Bytes after the last
//! synthetic marker wait unparsed until another marker needs them; when the
//! update ends, any later real queries reach the model with no ordering entry
//! and keep their ordinary report. Once the synchronized output has been
//! applied, the shadow is dropped again.

use std::collections::VecDeque;
use std::mem;
use std::sync::{Mutex, PoisonError};

use alacritty_terminal::vte::ansi::PrivateMode;
use alacritty_terminal::vte::{Params, Parser, Perform};

use crate::event::Intent;

/// The private mode whose report marks a fixed reply's stream position.
///
/// It is an ordinary unknown DEC mode, so every parsed query reaches
/// `Handler::report_private_mode`; the provenance queue decides what the
/// occurrence means.
pub(crate) const MARKER_MODE: u16 = 0xe000;

/// The query the session writes for one fixed reply.
pub(crate) const MARKER_QUERY: &[u8] = b"\x1b[?57344$p";

/// Where one parsed occurrence of [`MARKER_MODE`] came from.
#[derive(Clone, Copy)]
pub(crate) enum MarkerSource {
    /// The program sent the query; its report must be answered normally.
    Remote,
    /// The session inserted the query to order one fixed reply.
    Synthetic(u64),
}

/// Marker provenance for the current synchronized update.
#[derive(Default)]
pub(crate) struct OrderedReplies {
    sources: VecDeque<MarkerSource>,
    shadow: Option<MarkerShadow>,
    /// The bytes the model parser currently holds for a synchronized update.
    sync_buffer: Vec<u8>,
    /// Remote bytes after the shadow's position that no marker needs yet.
    deferred: Vec<u8>,
    /// Synthetic query ranges inside [`Self::sync_buffer`].
    synthetic: Vec<MarkerSpan>,
}

impl OrderedReplies {
    /// Starts tracking before the model parses `bytes`.
    ///
    /// A synthetic marker needs its source in place before the model can
    /// report it. At that point the parser may already hold synchronized bytes
    /// that never went through a shadow, so they are replayed once from the
    /// parser's own buffer copy.
    pub(crate) fn before_write(&mut self, bytes: &[u8], source: MarkerSource, buffered: usize) {
        match source {
            MarkerSource::Synthetic(_) => {
                self.ensure_shadow(buffered);
                if let Some(shadow) = self.shadow.as_mut() {
                    if !self.deferred.is_empty() {
                        let deferred = mem::take(&mut self.deferred);
                        shadow.advance(&mut self.sources, &deferred, MarkerSource::Remote);
                    }
                    shadow.advance(&mut self.sources, bytes, source);
                }
            }
            MarkerSource::Remote => {
                if self.shadow.is_some() {
                    self.deferred.extend_from_slice(bytes);
                }
            }
        }
    }

    /// Tracks the model's synchronized buffer after a write.
    ///
    /// While no shadow exists, this copies only the suffix that the parser
    /// holds for a synchronized update. The buffer never exceeds the parser's
    /// own synchronized-output limit, so ordinary SSH traffic is neither
    /// copied nor parsed.
    pub(crate) fn after_write(
        &mut self,
        bytes: &[u8],
        source: MarkerSource,
        was_synchronized: bool,
        buffered: usize,
    ) {
        if self.shadow.is_none() {
            if was_synchronized || buffered > 0 {
                self.append_sync_buffer(bytes, None);
                self.trim_sync_buffer(buffered);
            }
            return;
        }

        let previous = self.sync_buffer.len();
        let order = match source {
            MarkerSource::Synthetic(order) => Some(order),
            MarkerSource::Remote => None,
        };
        self.append_sync_buffer(bytes, order);
        self.trim_sync_buffer(buffered);

        if buffered == 0 {
            self.deactivate();
        } else if buffered < previous + bytes.len() {
            // The update flushed or trimmed output but another BSU is still
            // open. Replay only what the parser now holds, so occurrences
            // that already reached the model cannot stay queued.
            self.rebuild_shadow();
        }
    }

    /// Creates the syntax-only parser and replays the current buffer once.
    fn ensure_shadow(&mut self, buffered: usize) {
        if self.shadow.is_some() {
            return;
        }
        if self.sync_buffer.is_empty() {
            // The parser may hold a synchronized update whose BSU has not
            // produced buffered bytes yet. There is nothing to replay then.
            debug_assert_eq!(buffered, 0);
        }
        self.replay();
    }

    /// Rebuilds provenance for the bytes the parser still holds after a flush.
    fn rebuild_shadow(&mut self) {
        self.sources.clear();
        self.deferred.clear();
        self.replay();
    }

    /// Replays the current synchronized buffer through a fresh shadow parser.
    fn replay(&mut self) {
        let mut parser = Parser::new();
        let mut cursor = 0;
        for span in &self.synthetic {
            let start = span.start.max(cursor);
            if start > cursor {
                Self::advance(
                    &mut parser,
                    &mut self.sources,
                    &self.sync_buffer[cursor..start],
                    MarkerSource::Remote,
                );
            }
            let end = span.end.min(self.sync_buffer.len());
            if end > start {
                Self::advance(
                    &mut parser,
                    &mut self.sources,
                    &self.sync_buffer[start..end],
                    MarkerSource::Synthetic(span.order),
                );
            }
            cursor = end;
        }
        if cursor < self.sync_buffer.len() {
            Self::advance(
                &mut parser,
                &mut self.sources,
                &self.sync_buffer[cursor..],
                MarkerSource::Remote,
            );
        }
        self.shadow = Some(MarkerShadow { parser });
    }

    fn advance(
        parser: &mut Parser,
        sources: &mut VecDeque<MarkerSource>,
        bytes: &[u8],
        source: MarkerSource,
    ) {
        let mut probe = MarkerProbe { source, sources };
        parser.advance(&mut probe, bytes);
    }

    fn append_sync_buffer(&mut self, bytes: &[u8], synthetic: Option<u64>) {
        if let Some(order) = synthetic {
            let start = self.sync_buffer.len();
            self.synthetic.push(MarkerSpan {
                start,
                end: start + bytes.len(),
                order,
            });
        }
        self.sync_buffer.extend_from_slice(bytes);
    }

    fn trim_sync_buffer(&mut self, keep: usize) {
        if self.sync_buffer.len() <= keep {
            return;
        }
        let discard = self.sync_buffer.len() - keep;
        self.sync_buffer.copy_within(discard.., 0);
        self.sync_buffer.truncate(keep);
        let mut kept = Vec::with_capacity(self.synthetic.len());
        for mut span in self.synthetic.drain(..) {
            if span.start < discard {
                if span.end <= discard {
                    continue;
                }
                span.start = 0;
            } else {
                span.start -= discard;
            }
            span.end = span.end.saturating_sub(discard).min(keep);
            if span.start < span.end {
                kept.push(span);
            }
        }
        self.synthetic = kept;
    }

    fn deactivate(&mut self) {
        self.shadow = None;
        self.sources.clear();
        self.deferred.clear();
        self.sync_buffer.clear();
        self.synthetic.clear();
    }

    /// Consumes the next parsed marker occurrence and reports whether it is
    /// synthetic. A remote occurrence is consumed and left to the model's
    /// ordinary mode report.
    pub(crate) fn intercept(&mut self, mode: PrivateMode, intents: &Mutex<Vec<Intent>>) -> bool {
        if mode.raw() != MARKER_MODE {
            return false;
        }
        match self.sources.pop_front() {
            Some(MarkerSource::Synthetic(order)) => {
                intents
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(Intent::OrderedReply(order));
                true
            }
            Some(MarkerSource::Remote) | None => false,
        }
    }
}

/// A parser used only for marker provenance, never for display.
struct MarkerShadow {
    parser: Parser,
}

impl MarkerShadow {
    fn advance(
        &mut self,
        sources: &mut VecDeque<MarkerSource>,
        bytes: &[u8],
        source: MarkerSource,
    ) {
        OrderedReplies::advance(&mut self.parser, sources, bytes, source);
    }
}

/// A synthetic query's byte range inside the copied synchronized buffer.
struct MarkerSpan {
    start: usize,
    end: usize,
    order: u64,
}

/// Records the provenance of one marker query parsed by the shadow.
struct MarkerProbe<'a> {
    source: MarkerSource,
    sources: &'a mut VecDeque<MarkerSource>,
}

impl Perform for MarkerProbe<'_> {
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore {
            return;
        }
        if action == 'p' && intermediates == b"?$" {
            let mode = params
                .iter()
                .next()
                .and_then(|param| param.first())
                .copied()
                .filter(|&mode| mode != 0)
                .unwrap_or(0);
            if mode == MARKER_MODE {
                self.sources.push_back(self.source);
            }
        }
    }
}
