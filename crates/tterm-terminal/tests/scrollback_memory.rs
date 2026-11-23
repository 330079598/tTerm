//! Allocator-backed evidence that clearing the history also releases its rows.
//!
//! Alacritty's ring keeps trimmed rows allocated until the ring is truncated
//! or the allocation grows past its cache. The test wraps the system allocator
//! so it can distinguish "history_size is zero" from "the row storage is
//! gone", and uses a capacity below Alacritty's 1000-row cache where the
//! difference is visible.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

use tterm_terminal::grid::{CellSize, GridSize};
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

fn live_bytes() -> isize {
    LIVE_BYTES.load(Ordering::SeqCst)
}

fn output(lines: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(lines * 16);
    for index in 0..lines {
        bytes.extend_from_slice(format!("line {index:05}\r\n").as_bytes());
    }
    bytes
}

#[test]
fn clearing_the_history_returns_the_row_storage_to_the_allocator() {
    const COLUMNS: usize = 80;
    const LINES: usize = 24;
    const CAPACITY: usize = 500;

    let model = Terminal::new(
        Config {
            scrollback: CAPACITY,
            ..Config::default()
        },
        GridSize::new(COLUMNS, LINES),
        CellSize::new(8, 16),
    );
    let bytes = output(2_000);
    model.write(&bytes);
    drop(bytes);

    let before_frame = model.snapshot();
    assert_eq!(before_frame.history_size(), CAPACITY);
    let before = live_bytes();
    drop(before_frame);

    let mut clear_and_prompt = b"\x1b[H\x1b[2J\x1b[3J".to_vec();
    for index in 0..30 {
        clear_and_prompt.extend_from_slice(format!("fresh {index:02}\r\n").as_bytes());
    }
    model.write(&clear_and_prompt);
    let cleared = model.snapshot();
    assert!(cleared.history_size() < CAPACITY);
    assert_eq!(cleared.display_offset(), 0);
    assert!((0..cleared.line_count()).any(|line| {
        cleared.row(line).is_some_and(|row| {
            row.cells()
                .iter()
                .map(|cell| cell.ch)
                .collect::<String>()
                .contains("fresh 29")
        })
    }));
    drop(cleared);

    let after = live_bytes();
    let released = before - after;
    println!(
        "history-clear: capacity={CAPACITY} rows={CAPACITY} live_before={before} live_after={after} released={released}"
    );
    assert!(
        released > 400_000,
        "the cleared rows must leave the allocator, not just the history length: released {released} bytes"
    );

    model.write(b"after-0\r\nafter-1\r\n");
    let fresh = model.snapshot();
    assert!(fresh.history_size() <= CAPACITY);
    assert!(
        (0..fresh.line_count()).any(|line| {
            fresh.row(line).is_some_and(|row| {
                row.cells()
                    .iter()
                    .map(|cell| cell.ch)
                    .collect::<String>()
                    .contains("after-1")
            })
        }),
        "output after the clear must render normally"
    );
}
