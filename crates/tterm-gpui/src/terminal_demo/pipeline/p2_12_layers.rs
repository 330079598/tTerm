//! Layer diagnostic for the P2-12 fourth-batch SSH reply path.
//!
//! `p2_12_bench` measures the whole pipeline: adapter, parser thread and
//! bounded queue. This module replays the adapter's output tape straight into
//! a fresh model, so the coordinator can compare the model's display cost with
//! and without the optional ordered-reply layer while the queue and the parser
//! thread are absent.
//!
//! The tape is prepared once, before timing, with the same `OrderedQueries`
//! adapter and the same `BATCH_LIMIT` chunk boundaries the queue's `send`
//! produces. Every tape entry is one display slice or one fixed reply, in wire
//! order. Replay writes each display slice with `Terminal::write`, drains the
//! model's intents after that slice and answers them through `handle_intent`,
//! and merges the tape's fixed replies at their recorded positions. The merged
//! transcript must equal the case's whole reply transcript exactly.
//!
//! Only the plain and query payloads are replayed. The ANSI case adds no layer
//! evidence, and the synchronized case defers fixed replies, which this tape
//! does not model. No test asserts a time threshold; each pass prints its
//! elapsed microseconds so the coordinator can compare passes from the same
//! process.
//!
//! The ordered-reply API is new. Its opt-in is one isolated statement in
//! `replay` so a temporary baseline worktree can replace only that statement.

use std::time::{Duration, Instant};

use tterm_core::terminal::ssh_query_handler::ordered::{OrderedQueries, Part};

use super::super::BATCH_LIMIT;
use super::super::tests::state;
use super::{BLOCKS, Case, SENTINEL, TAIL, TOTAL_BYTES, visible_text};

use crate::terminal_demo::intents::handle_intent;

/// The bytes inside `QUERY_HEADER` that the adapter answers itself, so they
/// never become display slices: `CSI c`, `CSI > c` and the XTGETTCAP query.
const QUERY_FIXED_INPUT_BYTES: usize =
    b"\x1b[c".len() + b"\x1b[>c".len() + b"\x1bP+q436f\x1b\\".len();

/// One recorded step of the adapter's output, in wire order.
enum TapePart {
    /// Bytes the adapter forwards to the model.
    Display(Vec<u8>),
    /// Bytes the adapter answers itself and the pipeline writes to the server.
    Reply(Vec<u8>),
}

/// The complete input both model passes replay, prepared without timing.
struct Tape {
    parts: Vec<TapePart>,
}

impl Tape {
    /// Feeds the same chunk sequence the queue carries through one adapter.
    fn build(case: Case) -> Self {
        let block = case.block();
        let mut queries = OrderedQueries::default();
        let mut tape = Self { parts: Vec::new() };
        for _ in 0..BLOCKS {
            for chunk in block.chunks(BATCH_LIMIT) {
                tape.feed(&mut queries, chunk);
            }
        }
        for chunk in TAIL.chunks(BATCH_LIMIT) {
            tape.feed(&mut queries, chunk);
        }
        tape
    }

    /// Appends one adapter feed's parts. They are copied because the adapter
    /// borrows its own buffers for the duration of the feed.
    fn feed(&mut self, queries: &mut OrderedQueries, bytes: &[u8]) {
        queries.feed(bytes, |part| match part {
            Part::Display(display) => self.parts.push(TapePart::Display(display.to_vec())),
            Part::Reply(reply) => self.parts.push(TapePart::Reply(reply.to_vec())),
        });
    }
}

/// The fixed facts about one supported payload.
struct Payload {
    /// The evidence label the standardized line reports.
    label: &'static str,
    /// The input bytes the model must receive; fixed queries are answered by
    /// the adapter and never reach it.
    display_bytes: usize,
    /// The display writes the tape must contain: one per chunk for plain
    /// output, and two per query block because the header replies split it.
    display_slices: usize,
}

/// Describes a payload this diagnostic supports, rejecting the other two.
fn supported(case: Case) -> Payload {
    let input = TOTAL_BYTES + TAIL.len();
    match case {
        Case::Plain => Payload {
            label: "plain",
            display_bytes: input,
            display_slices: BLOCKS + 1,
        },
        Case::Queries => Payload {
            label: "query",
            display_bytes: input - QUERY_FIXED_INPUT_BYTES * BLOCKS,
            display_slices: 2 * BLOCKS + 1,
        },
        Case::Ansi | Case::Synchronized => {
            panic!("the layer diagnostic replays only plain and query traffic")
        }
    }
}

/// The whole reply transcript the model and the fixed replies must produce.
fn expected_transcript(case: Case) -> Vec<u8> {
    let block_replies = case.block_replies();
    let mut transcript = Vec::with_capacity(block_replies.len() * BLOCKS);
    for _ in 0..BLOCKS {
        transcript.extend_from_slice(&block_replies);
    }
    transcript
}

/// What one model pass observed. The counters let a pair of passes prove they
/// did the same work with and without the ordered-reply layer.
struct ModelPass {
    transcript: Vec<u8>,
    display_bytes: usize,
    slices: usize,
    elapsed: Duration,
    sentinel: bool,
}

/// Replays `tape` into a fresh model and prints one evidence line.
fn replay(case: Case, tape: &Tape, ordered: bool) -> ModelPass {
    let payload = supported(case);
    let expected = expected_transcript(case);
    let (model, session) = state(false, 64);
    if ordered {
        // The coordinator replaces only this statement in the temporary
        // baseline worktree, where the ordered-reply API does not exist.
        model.enable_ordered_replies();
    }
    let mut transcript = Vec::with_capacity(expected.len());
    let mut display_bytes = 0;
    let mut slices = 0;
    let started = Instant::now();
    for part in &tape.parts {
        match part {
            TapePart::Display(bytes) => {
                model.write(bytes);
                display_bytes += bytes.len();
                slices += 1;
                for intent in model.drain_intents() {
                    handle_intent(intent, &session, session.clipboard.epoch(), |reply| {
                        transcript.extend_from_slice(&reply)
                    });
                }
            }
            TapePart::Reply(bytes) => transcript.extend_from_slice(bytes),
        }
    }
    let elapsed = started.elapsed();
    assert!(
        model.drain_intents().is_empty(),
        "every model intent must be drained after the display that produced it"
    );
    let sentinel = visible_text(&model).starts_with(SENTINEL);
    let pass = ModelPass {
        transcript,
        display_bytes,
        slices,
        elapsed,
        sentinel,
    };
    let label = payload.label;
    let ordered = u8::from(ordered);
    let display_bytes = pass.display_bytes;
    let slices = pass.slices;
    let replies = pass.transcript.len();
    let elapsed_us = pass.elapsed.as_micros();
    let sentinel = pass.sentinel;
    eprintln!(
        "P212-layer case={label} ordered={ordered} display_bytes={display_bytes} \
         slices={slices} replies={replies} elapsed_us={elapsed_us} sentinel={sentinel}"
    );
    assert_eq!(
        pass.transcript, expected,
        "{label} transcript must match the wire reply order"
    );
    assert_eq!(
        pass.display_bytes, payload.display_bytes,
        "{label} must write every non-query input byte to the model"
    );
    assert_eq!(
        pass.transcript.len(),
        expected.len(),
        "{label} must produce every expected reply byte"
    );
    assert_eq!(
        pass.slices, payload.display_slices,
        "{label} must replay every display slice the tape contains"
    );
    assert!(
        pass.sentinel,
        "the final {SENTINEL} must start the visible {label} screen"
    );
    pass
}

/// Runs one ordering-disabled model pass over a freshly prepared tape.
fn run_model(case: Case) {
    let tape = Tape::build(case);
    replay(case, &tape, false);
}

/// Runs the same tape through fresh models with ordering disabled and enabled.
fn run_pair(case: Case) {
    let tape = Tape::build(case);
    let order = if enabled_first() {
        [true, false]
    } else {
        [false, true]
    };
    let mut passes = Vec::with_capacity(order.len());
    for ordered in order {
        passes.push(replay(case, &tape, ordered));
    }
    assert_eq!(
        passes[0].transcript, passes[1].transcript,
        "the same tape must produce the same transcript with and without ordering"
    );
    assert_eq!(
        passes[0].display_bytes, passes[1].display_bytes,
        "the same tape must write the same display bytes with and without ordering"
    );
    assert_eq!(
        passes[0].slices, passes[1].slices,
        "the same tape must produce the same display slices with and without ordering"
    );
}

/// Whether the enabled pass runs first. The coordinator sets
/// `P212_ENABLED_FIRST=1` to put the new layer first and `0` to put it last, so
/// a process warm-up effect can be attributed rather than hidden. An absent
/// variable keeps the historical disabled-first order.
fn enabled_first() -> bool {
    match std::env::var("P212_ENABLED_FIRST") {
        Ok(value) if value == "1" => true,
        Ok(value) if value == "0" => false,
        Ok(value) => panic!("P212_ENABLED_FIRST must be 0 or 1, not {value:?}"),
        Err(_) => false,
    }
}

#[test]
#[ignore = "P2-12 layer diagnostic; run explicitly with --ignored"]
fn plain_model() {
    run_model(Case::Plain);
}

#[test]
#[ignore = "P2-12 layer diagnostic; run explicitly with --ignored"]
fn query_model() {
    run_model(Case::Queries);
}

#[test]
#[ignore = "P2-12 layer diagnostic; run explicitly with --ignored"]
fn plain_pair() {
    run_pair(Case::Plain);
}

#[test]
#[ignore = "P2-12 layer diagnostic; run explicitly with --ignored"]
fn query_pair() {
    run_pair(Case::Queries);
}
