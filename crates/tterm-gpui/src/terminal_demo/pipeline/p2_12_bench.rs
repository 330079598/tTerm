//! Diagnostic A/B benchmark for the P2-12 fourth-batch SSH reply path.
//!
//! Each test drives one fresh production GPUI parser with an SSH
//! [`OrderedQueries`] adapter and a synthetic single session. The four cases
//! differ only in payload: plain printable lines, ANSI-colored lines, fixed and
//! model query traffic, or synchronized output holding fixed replies. Every
//! case sends exactly [`TOTAL_BYTES`] in [`CHUNK_BYTES`] chunks, then a clear
//! screen and one visible sentinel. The fixture asserts that every byte was
//! applied and acknowledged, that the complete reply transcript matches, and
//! that the sentinel is visible. It deliberately asserts no time, throughput or
//! memory threshold; the coordinator measures process CPU, elapsed time and
//! RSS externally with `/usr/bin/time -l`, one case per fresh process.
//!
//! The synchronized case puts no model query inside the block. Fixed replies
//! are then the only replies between the start and end of the update, so the
//! immediate baseline and the deferred implementation produce the same reply
//! transcript even though the fourth batch moves when those bytes are emitted.
//! That keeps the case a cost comparison, not a deliberate ordering change.
//!
//! This source uses no API added by the fourth batch, so the same file and the
//! same two-line module declaration compile against the parent commit. Run one
//! ignored case through the release libtest binary:
//!
//! ```text
//! cargo test -p tterm-gpui --lib --release --no-run
//! /usr/bin/time -l <the libtest executable cargo reported> \
//!   --exact --ignored --nocapture --test-threads=1 \
//!   terminal_demo::pipeline::p2_12_bench::plain_printable_64_mib
//! ```
//!
//! Apply the same fixture to a clean `7991a62f` checkout by copying this file to
//! the same path and adding `#[cfg(test)] mod p2_12_bench;` beside the existing
//! test module declarations in `pipeline.rs`. Nothing else in that commit needs
//! to change.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tterm_core::terminal::ssh_query_handler::ordered::OrderedQueries;
use tterm_terminal::terminal::Terminal;

use super::tests::state;
use super::{Pipeline, start};

const CHUNK_BYTES: usize = 64 * 1024;
const BLOCKS: usize = 1024;
const TOTAL_BYTES: usize = CHUNK_BYTES * BLOCKS;
/// Every body line has one fixed width, so each block is built without a
/// partial escape sequence and the total is independent of formatting.
const LINE_BYTES: usize = 64;
const SENTINEL: &str = "P2-12-SENTINEL";
const TAIL: &[u8] = b"\x1b[2J\x1b[HP2-12-SENTINEL";

const QUERY_HEADER: &[u8] = b"\x1b[1;1H\x1b[6n\x1b[5n\x1b[c\x1b[>c\x1bP+q436f\x1b\\";
const SYNC_HEADER: &[u8] = b"\x1b[?2026h\x1b[c\x1b[>c\x1bP+q436f\x1b\\";
const SYNC_FOOTER: &[u8] = b"\x1b[?2026l\x1b[5n";

const DA1_REPLY: &[u8] = b"\x1b[?64;1;2;4;6;9;15;21;22c";
const DA2_REPLY: &[u8] = b"\x1b[>0;276;0c";
const CAPABILITY_REPLY: &[u8] = b"\x1bP1+r436f3d323536\x1b\\";
const STATUS_REPLY: &[u8] = b"\x1b[0n";
const CPR_REPLY: &[u8] = b"\x1b[1;1R";

/// One payload shape. Each shape has a fixed block size and reply pattern, so
/// the measured work is the same on every run and in both checkouts.
#[derive(Clone, Copy)]
enum Case {
    Plain,
    Ansi,
    Queries,
    Synchronized,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Self::Plain => "plain_printable",
            Self::Ansi => "ansi_colored",
            Self::Queries => "query_traffic",
            Self::Synchronized => "synchronized_fixed_replies",
        }
    }

    /// Builds one whole chunk. The producer sends [`BLOCKS`] copies of it.
    fn block(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(CHUNK_BYTES);
        match self {
            Self::Plain => fill_lines(&mut bytes, CHUNK_BYTES, false),
            Self::Ansi => fill_lines(&mut bytes, CHUNK_BYTES, true),
            Self::Queries => {
                bytes.extend_from_slice(QUERY_HEADER);
                fill_lines(&mut bytes, CHUNK_BYTES, false);
            }
            Self::Synchronized => {
                bytes.extend_from_slice(SYNC_HEADER);
                fill_lines(&mut bytes, CHUNK_BYTES - SYNC_FOOTER.len(), true);
                bytes.extend_from_slice(SYNC_FOOTER);
            }
        }
        assert_eq!(bytes.len(), CHUNK_BYTES, "{} block size", self.name());
        bytes
    }

    /// The replies one block must produce, in wire order.
    fn block_replies(self) -> Vec<u8> {
        let parts: &[&[u8]] = match self {
            Self::Plain | Self::Ansi => &[],
            Self::Queries => &[
                CPR_REPLY,
                STATUS_REPLY,
                DA1_REPLY,
                DA2_REPLY,
                CAPABILITY_REPLY,
            ],
            Self::Synchronized => &[DA1_REPLY, DA2_REPLY, CAPABILITY_REPLY, STATUS_REPLY],
        };
        parts.concat()
    }
}

fn fill_lines(bytes: &mut Vec<u8>, target: usize, colored: bool) {
    let mut number = 0;
    while bytes.len() + LINE_BYTES <= target {
        if colored {
            bytes.extend_from_slice(&colored_line(number));
        } else {
            bytes.extend_from_slice(&plain_line(number));
        }
        number += 1;
    }
    let remainder = target - bytes.len();
    bytes.extend(std::iter::repeat_n(b'.', remainder));
}

fn plain_line(number: usize) -> [u8; LINE_BYTES] {
    let mut line = [b'x'; LINE_BYTES];
    let prefix = format!("plain-{number:04}-");
    line[..prefix.len()].copy_from_slice(prefix.as_bytes());
    line[LINE_BYTES - 1] = b'\n';
    line
}

fn colored_line(number: usize) -> Vec<u8> {
    let mut line = Vec::with_capacity(LINE_BYTES);
    let color = 16 + number % 216;
    let prefix = format!("color-{number:04}-");
    line.extend_from_slice(format!("\x1b[38;5;{color}m").as_bytes());
    line.extend_from_slice(prefix.as_bytes());
    let fill = LINE_BYTES - line.len() - b"\x1b[0m\n".len();
    line.extend(std::iter::repeat_n(b'x', fill));
    line.extend_from_slice(b"\x1b[0m\n");
    debug_assert_eq!(line.len(), LINE_BYTES);
    line
}

fn visible_text(model: &Terminal) -> String {
    model
        .snapshot()
        .rows()
        .iter()
        .flat_map(|row| row.cells())
        .map(|cell| cell.ch)
        .collect()
}

fn run(case: Case) {
    let (model, state) = state(false, 64);
    let replies = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&replies);
    let acknowledged = Arc::new(AtomicU64::new(0));
    let acked = Arc::clone(&acknowledged);
    let pipeline = start(
        state,
        Arc::new(move |bytes| {
            captured
                .lock()
                .expect("bench reply capture")
                .extend_from_slice(&bytes);
        }),
        Arc::new(move |bytes| {
            acked.fetch_add(u64::try_from(bytes).unwrap_or(u64::MAX), Ordering::Relaxed);
        }),
        None,
        Some(OrderedQueries::default()),
    )
    .expect("start the production SSH parser");
    let metrics = pipeline.queue.metrics_handle();
    let Pipeline { queue, parser } = pipeline;

    let block = case.block();
    let mut expected_replies = Vec::new();
    let block_replies = case.block_replies();
    for _ in 0..BLOCKS {
        expected_replies.extend_from_slice(&block_replies);
    }

    let started = Instant::now();
    for _ in 0..BLOCKS {
        queue
            .send(block.clone())
            .expect("queue a fixed input block");
    }
    queue.send(TAIL.to_vec()).expect("queue the sentinel");
    drop(queue);
    parser.join().expect("parser exits");
    let elapsed = started.elapsed();

    let expected_bytes = (TOTAL_BYTES + TAIL.len()) as u64;
    let replies = std::mem::take(&mut *replies.lock().expect("bench reply capture"));
    let sent = metrics.sent_bytes();
    let applied = metrics.applied_bytes();
    let parsed = metrics.acknowledged_bytes();
    let acknowledged = acknowledged.load(Ordering::Acquire);
    let screen = visible_text(&model);
    let sentinel_visible = screen.starts_with(SENTINEL);
    eprintln!(
        "P2-12 fourth-batch diagnostic: case={} blocks={BLOCKS} chunk_bytes={CHUNK_BYTES} \
         bytes={expected_bytes} sent={sent} applied={applied} parsed={parsed} \
         acknowledged={acknowledged} replies={} expected_replies={} sentinel={} \
         internal_elapsed_us={}",
        case.name(),
        replies.len(),
        expected_replies.len(),
        sentinel_visible,
        elapsed.as_micros(),
    );

    assert_eq!(sent, expected_bytes, "queued bytes");
    assert_eq!(applied, expected_bytes, "applied bytes");
    assert_eq!(parsed, expected_bytes, "parser-acknowledged bytes");
    assert_eq!(acknowledged, expected_bytes, "session-acknowledged bytes");
    assert_eq!(replies, expected_replies, "complete reply transcript");
    assert!(
        sentinel_visible,
        "the final {} must start the visible screen: {screen:?}",
        SENTINEL
    );
    if matches!(case, Case::Queries | Case::Synchronized) {
        assert!(!replies.is_empty(), "{} must produce replies", case.name());
    }
}

#[test]
#[ignore = "P2-12 diagnostic; run one case per release process with /usr/bin/time -l"]
fn plain_printable_64_mib() {
    run(Case::Plain);
}

#[test]
#[ignore = "P2-12 diagnostic; run one case per release process with /usr/bin/time -l"]
fn ansi_colored_64_mib() {
    run(Case::Ansi);
}

#[test]
#[ignore = "P2-12 diagnostic; run one case per release process with /usr/bin/time -l"]
fn query_traffic_64_mib() {
    run(Case::Queries);
}

#[test]
#[ignore = "P2-12 diagnostic; run one case per release process with /usr/bin/time -l"]
fn synchronized_fixed_replies_64_mib() {
    run(Case::Synchronized);
}

#[path = "p2_12_layers.rs"]
mod layers;
