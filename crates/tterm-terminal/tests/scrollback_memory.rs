//! Allocator-backed evidence that clearing the history releases its rows.
//!
//! Alacritty's ring keeps trimmed rows allocated until the ring is truncated
//! or the allocation grows past its cache. The scenarios wrap the system
//! allocator so they can distinguish "history_size is zero" from "the row
//! storage is gone". A capacity below Alacritty's 1000-row cache makes the
//! difference visible.
//!
//! The counter covers every allocation in the process, so the whole binary has
//! exactly one `#[test]` entry point and every scenario is a plain function it
//! calls in turn. Libtest then starts no parallel worker, and no harness
//! bookkeeping can allocate between a scenario's two reads. Adding a second
//! `#[test]` here would let another worker allocate inside a measurement, so
//! the scenarios must stay helpers of the single test.
//!
//! Measurements are taken at the same stable point in each scenario: the write
//! has returned and every snapshot created for assertions has been dropped. The
//! difference uses `checked_sub`, so a measurement that grew is reported
//! instead of overflowing.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicIsize, Ordering};

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::mode::Modes;
use tterm_terminal::terminal::{Config, Terminal};

/// Counts live bytes allocated through the global allocator.
struct CountingAllocator;

static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            LIVE_BYTES.fetch_add(layout.size() as isize, Ordering::SeqCst);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            LIVE_BYTES.fetch_add(new_size as isize - layout.size() as isize, Ordering::SeqCst);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const COLUMNS: usize = 80;
const LINES: usize = 24;
const CAPACITY: usize = 500;
const ROWS: usize = 2_000;

fn live_bytes() -> isize {
    LIVE_BYTES.load(Ordering::SeqCst)
}

fn released(before: isize, after: isize) -> isize {
    before
        .checked_sub(after)
        .expect("live-byte difference must fit in isize")
}

fn model() -> Terminal {
    Terminal::new(
        Config {
            scrollback: CAPACITY,
            ..Config::default()
        },
        GridSize::new(COLUMNS, LINES),
        CellSize::new(8, 16),
    )
}

fn output(lines: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(lines * 16);
    for index in 0..lines {
        bytes.extend_from_slice(format!("line {index:05}\r\n").as_bytes());
    }
    bytes
}

/// Fills the bounded history and leaves the model at the bottom of the screen.
fn fill(model: &Terminal) {
    let bytes = output(ROWS);
    model.write(&bytes);
    drop(bytes);

    let snapshot = model.snapshot();
    assert_eq!(snapshot.history_size(), CAPACITY);
    assert_eq!(snapshot.display_offset(), 0);
    drop(snapshot);
}

fn clear_and_prompt() -> Vec<u8> {
    let mut bytes = b"\x1b[H\x1b[2J\x1b[3J".to_vec();
    for index in 0..30 {
        bytes.extend_from_slice(format!("fresh {index:02}\r\n").as_bytes());
    }
    bytes
}

fn write_in_chunks(model: &Terminal, bytes: &[u8], chunk: Option<usize>) {
    match chunk {
        Some(size) => {
            for part in bytes.chunks(size) {
                model.write(part);
            }
        }
        None => model.write(bytes),
    }
}

fn screen_text(model: &Terminal) -> Vec<String> {
    let snapshot = model.snapshot();
    let text = snapshot
        .rows()
        .iter()
        .map(|row| {
            row.cells()
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect();
    drop(snapshot);
    text
}

/// The seven allocator scenarios, run in one test so no other worker exists.
///
/// Each scenario keeps its own strict assertions and evidence line. They cannot
/// be separate tests: libtest would run them on parallel threads, and the
/// process-wide counter would then include allocations made between one
/// scenario's two reads by another worker.
#[test]
fn history_clear_scenarios_hold_under_one_isolated_measurement() {
    clearing_the_history_returns_the_row_storage_to_the_allocator();
    clearing_before_the_alternate_screen_switch_releases_the_main_history();
    a_synchronized_clear_does_not_release_before_it_executes();
    clearing_the_alternate_screen_does_not_release_the_main_history();
    a_reset_returns_the_row_storage_to_the_allocator();
    a_synchronized_reset_releases_only_after_the_update_ends();
    the_same_stream_leaves_the_same_live_memory_when_split();
}

fn clearing_the_history_returns_the_row_storage_to_the_allocator() {
    let model = model();
    fill(&model);

    let before = live_bytes();
    model.write(b"\x1b[H\x1b[2J\x1b[3J");
    let cleared = model.snapshot();
    assert_eq!(cleared.history_size(), 0);
    assert_eq!(cleared.display_offset(), 0);
    drop(cleared);
    let after = live_bytes();
    let released = released(before, after);

    println!(
        "history-clear: capacity={CAPACITY} rows={CAPACITY} live_before={before} live_after={after} released={released}"
    );
    assert!(
        released > 400_000,
        "the cleared rows must leave the allocator, not just the history length: released {released} bytes"
    );

    model.write(b"after-0\r\nafter-1\r\n");
    assert!(
        screen_text(&model)
            .iter()
            .any(|line| line.contains("after-1")),
        "output after the clear must render normally"
    );
}

fn clearing_before_the_alternate_screen_switch_releases_the_main_history() {
    let model = model();
    fill(&model);

    let before = live_bytes();
    model.write(b"\x1b[3J\x1b[?1049h");
    let cleared = model.snapshot();
    assert!(cleared.modes().contains(Modes::ALT_SCREEN));
    assert_eq!(cleared.history_size(), 0);
    drop(cleared);
    let after = live_bytes();
    let released = released(before, after);

    println!(
        "history-clear-alt: capacity={CAPACITY} live_before={before} live_after={after} released={released}"
    );
    assert!(
        released > 1_000_000,
        "a clear followed by the alternate-screen switch must release the main rows: released {released} bytes"
    );
}

fn a_synchronized_clear_does_not_release_before_it_executes() {
    let model = model();
    fill(&model);

    let before = live_bytes();
    model.write(b"\x1b[?2026h\x1b[3J");
    let buffered = model.snapshot();
    assert_eq!(
        buffered.history_size(),
        CAPACITY,
        "ED 3 is buffered until the synchronized update ends"
    );
    drop(buffered);
    let waiting = live_bytes();
    assert!(
        waiting >= before,
        "no row storage may be released before Alacritty executes the clear: {before} -> {waiting}"
    );

    model.write(b"\x1b[?2026l");
    let cleared = model.snapshot();
    assert_eq!(cleared.history_size(), 0);
    drop(cleared);
    let after = live_bytes();
    let released = released(before, after);

    println!(
        "history-clear-sync: live_before={before} live_waiting={waiting} live_after={after} released={released}"
    );
    assert!(
        released > 400_000,
        "the clear must release the rows once the update ends: released {released} bytes"
    );
}

fn clearing_the_alternate_screen_does_not_release_the_main_history() {
    let model = model();
    fill(&model);
    model.write(b"\x1b[?1049h");

    let before = live_bytes();
    model.write(b"\x1b[H\x1b[2J\x1b[3J");
    let cleared = model.snapshot();
    assert!(cleared.modes().contains(Modes::ALT_SCREEN));
    drop(cleared);
    let after = live_bytes();
    let released = released(before, after);

    println!("history-clear-on-alt: live_before={before} live_after={after} released={released}");
    assert!(
        released < 100_000,
        "an alternate-screen clear must not release the hidden main history: released {released} bytes"
    );

    model.write(b"\x1b[?1049l");
    let back = model.snapshot();
    assert_eq!(back.history_size(), CAPACITY);
    assert_eq!(back.display_offset(), 0);
}

fn a_reset_returns_the_row_storage_to_the_allocator() {
    let model = model();
    fill(&model);

    let before = live_bytes();
    let generation = model.snapshot().scrollback_generation();
    model.write(b"\x1bc");
    let reset = model.snapshot();
    assert_eq!(reset.history_size(), 0);
    assert!(reset.scrollback_generation() > generation);
    drop(reset);
    let after = live_bytes();
    let released = released(before, after);

    println!("history-reset: live_before={before} live_after={after} released={released}");
    assert!(
        released > 400_000,
        "a reset must release the cleared rows: released {released} bytes"
    );
}

fn a_synchronized_reset_releases_only_after_the_update_ends() {
    let model = model();
    fill(&model);

    let before = live_bytes();
    let generation = model.snapshot().scrollback_generation();
    model.write(b"\x1b[?2026h\x1bc");
    let buffered = model.snapshot();
    assert_eq!(buffered.history_size(), CAPACITY);
    assert_eq!(buffered.scrollback_generation(), generation);
    drop(buffered);
    assert!(
        live_bytes() >= before,
        "the buffered reset must not release rows"
    );

    model.write(b"\x1b[?2026l");
    let reset = model.snapshot();
    assert_eq!(reset.history_size(), 0);
    assert!(reset.scrollback_generation() > generation);
    drop(reset);
    let after = live_bytes();
    let released = released(before, after);

    println!("history-reset-sync: live_before={before} live_after={after} released={released}");
    assert!(
        released > 400_000,
        "the reset must release the rows once the update ends: released {released} bytes"
    );
}

fn the_same_stream_leaves_the_same_live_memory_when_split() {
    fn screen_hash(model: &Terminal) -> u64 {
        let snapshot = model.snapshot();
        let mut hasher = DefaultHasher::new();
        for row in snapshot.rows() {
            for cell in row.cells() {
                cell.ch.hash(&mut hasher);
            }
            '\n'.hash(&mut hasher);
        }
        drop(snapshot);
        hasher.finish()
    }

    fn run(chunk: Option<usize>) -> (isize, isize, usize, u64) {
        let model = model();
        fill(&model);
        let stream = clear_and_prompt();
        let before = live_bytes();
        write_in_chunks(&model, &stream, chunk);
        let history = model.snapshot().history_size();
        let after = live_bytes();
        let signature = screen_hash(&model);
        drop(stream);
        (before, after, history, signature)
    }

    let once = run(None);
    let byte_by_byte = run(Some(1));
    let in_sevens = run(Some(7));

    for (name, run) in [
        ("one write", &once),
        ("single bytes", &byte_by_byte),
        ("seven-byte chunks", &in_sevens),
    ] {
        println!(
            "history-clear-batch: {name} live_before={} live_after={} released={}",
            run.0,
            run.1,
            released(run.0, run.1)
        );
    }

    assert_eq!(
        once.1, byte_by_byte.1,
        "single-byte writes changed live memory"
    );
    assert_eq!(once.1, in_sevens.1, "seven-byte writes changed live memory");
    assert_eq!(once.2, byte_by_byte.2);
    assert_eq!(once.2, in_sevens.2);
    assert_eq!(once.3, byte_by_byte.3, "the visible rows differ");
    assert_eq!(once.3, in_sevens.3);
    assert!(once.2 < CAPACITY, "the prompt must not refill the history");
    assert_eq!(
        released(once.0, once.1),
        released(byte_by_byte.0, byte_by_byte.1),
        "chunking changed how many bytes the clear released"
    );
    assert_eq!(released(once.0, once.1), released(in_sevens.0, in_sevens.1));
}
