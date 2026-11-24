//! Release measurement of a search over 100,000 retained rows.
//!
//! The fixture is `#[ignore]`d so the default workspace suite stays fast and
//! parallel. Run it alone on an idle machine:
//!
//! ```sh
//! cargo test -p tterm-terminal --release --test search_benchmark \
//!     -- --exact one_hundred_thousand_line_search_reports_time_and_allocations \
//!     --ignored --nocapture --test-threads=1
//! ```
//!
//! The test reports wall-clock time and allocator traffic for a plain query, a
//! regular expression, a case-sensitive query and a whole-word query. It has
//! no timing threshold: the numbers belong in the P2-07 report, not in a gate.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use tterm_terminal::grid::{CellSize, GridSize};
use tterm_terminal::search::{
    SEARCH_CHUNK_ROWS, SearchCancellation, SearchOptions, SearchOutcome, SearchQuery, SearchResult,
};
use tterm_terminal::terminal::{Config, Terminal};

/// Counts every allocation and every allocated byte in this test binary.
struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const COLUMNS: usize = 100;
const LINES: usize = 24;
const ROWS: usize = 100_000;
const CELL: CellSize = CellSize::new(8, 16);

fn model() -> Terminal {
    Terminal::new(
        Config {
            scrollback: ROWS,
            ..Config::default()
        },
        GridSize::new(COLUMNS, LINES),
        CELL,
    )
}

/// Builds exactly `ROWS` lines of `COLUMNS` ASCII columns.
///
/// Every line has one `needle`, every fifth has `needles` instead, and every
/// tenth also has an uppercase `NEEDLE` so the four queries have different
/// match counts.
fn output() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ROWS * (COLUMNS + 2));
    for index in 0..ROWS {
        let token = if index % 5 == 0 { "needles" } else { "needle" };
        let extra = if index % 10 == 0 { " NEEDLE" } else { "" };
        let mut line = format!("{index:06} {token} alpha Beta beta{extra} ");
        while line.len() < COLUMNS {
            line.push('x');
        }
        bytes.extend_from_slice(line.as_bytes());
        bytes.extend_from_slice(b"\r\n");
    }
    bytes
}

fn measure(model: &Terminal, name: &str, query: &str, options: SearchOptions) -> SearchResult {
    let query = SearchQuery::new(query, options).expect("the benchmark query compiles");
    let cancellation = SearchCancellation::new();
    let allocations_before = ALLOCATIONS.load(Ordering::Relaxed);
    let bytes_before = BYTES.load(Ordering::Relaxed);
    let started = Instant::now();
    let outcome = model.search_retained(&query, &cancellation);
    let elapsed = started.elapsed();
    let allocations = ALLOCATIONS.load(Ordering::Relaxed) - allocations_before;
    let bytes = BYTES.load(Ordering::Relaxed) - bytes_before;
    let result = match outcome {
        SearchOutcome::Complete(result) => result,
        other => panic!("a benchmark scan must complete, got {other:?}"),
    };
    println!(
        "P2-07 search: query={name} options=regex:{} case:{} whole:{} matches={} \
         elapsed_us={} allocations={} bytes={}",
        options.regex,
        options.case_sensitive,
        options.whole_word,
        result.match_count(),
        elapsed.as_micros(),
        allocations,
        bytes
    );
    result
}

/// Times the bounded copies a worker makes while it scans the buffer.
///
/// The search loop holds the model lock only for one of these calls, so the
/// slowest chunk is the longest parser wait the copy itself adds.
fn measure_chunk_copies(model: &Terminal) {
    let mut start = 0;
    let mut chunks = 0;
    let mut rows = 0;
    let mut slowest = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    loop {
        let began = Instant::now();
        let copied = model.retained_rows(start, SEARCH_CHUNK_ROWS);
        let elapsed = began.elapsed();
        total += elapsed;
        slowest = slowest.max(elapsed);
        let count = copied.rows().len();
        chunks += 1;
        rows += count;
        if count == 0 || start + count >= copied.total_lines() {
            break;
        }
        start += count;
    }
    println!(
        "P2-07 search copy: rows={rows} chunks={chunks} total_us={} slowest_chunk_us={}",
        total.as_micros(),
        slowest.as_micros()
    );
}

#[test]
#[ignore = "measures a 100,000-row search in release; run it with --ignored"]
fn one_hundred_thousand_line_search_reports_time_and_allocations() {
    println!(
        "P2-07 search environment: os={} arch={} columns={} rows={} screen_lines={} capacity={}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        COLUMNS,
        ROWS,
        LINES,
        ROWS
    );
    let model = model();
    let bytes = output();
    model.write(&bytes);
    drop(bytes);
    let snapshot = model.snapshot();
    println!(
        "P2-07 search fixture: retained_lines={} history_lines={}",
        snapshot.history_size() + snapshot.line_count(),
        snapshot.history_size()
    );
    drop(snapshot);
    measure_chunk_copies(&model);

    let plain = measure(&model, "plain", "needle", SearchOptions::default());
    assert_eq!(plain.match_count(), 110_000);

    let regex = measure(
        &model,
        "regex",
        r"alpha \w+ beta",
        SearchOptions {
            regex: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(regex.match_count(), 100_000);

    let case = measure(
        &model,
        "case",
        "NEEDLE",
        SearchOptions {
            case_sensitive: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(case.match_count(), 10_000);

    let whole = measure(
        &model,
        "whole",
        "needle",
        SearchOptions {
            whole_word: true,
            ..SearchOptions::default()
        },
    );
    assert_eq!(whole.match_count(), 90_000);
}
