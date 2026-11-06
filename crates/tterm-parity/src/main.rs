//! `cargo run -p tterm-parity -- <capture|compare>`.
//!
//! `capture` only renders the GPUI side, which is the quick loop while a view
//! is being built. `compare` makes sure the cached reference for the current
//! frontend commit exists, captures the GPUI side, writes the HTML report and
//! the machine-readable result, and exits 1 when a screen fails a threshold.

use std::path::Path;
use std::process::exit;
use tterm_parity::capture::capture_all;
use tterm_parity::compare::{self, ScreenReport};
use tterm_parity::reference;
use tterm_parity::repo_root;

fn main() {
    if let Err(error) = run() {
        eprintln!("tterm-parity: {error}");
        exit(2);
    }
}

fn run() -> Result<(), String> {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "capture" => capture(),
        "compare" => compare(),
        other => {
            eprintln!(
                "usage: tterm-parity <capture|compare>\n  \
                 capture  render the GPUI screens into target/parity/actual\n  \
                 compare  ensure the reference, capture, compare and write target/parity/report"
            );
            Err(format!("unknown command \"{other}\""))
        }
    }
}

fn capture() -> Result<(), String> {
    let root = repo_root();
    let output = root.join("target").join("parity").join("actual");
    let captured = capture_all(&root, &output)?;
    for screen in &captured {
        println!(
            "gpui {}: {} element(s) -> {}",
            screen.id,
            screen.element_count,
            screen.image.display()
        );
    }
    println!("actual: {}", output.display());
    Ok(())
}

fn compare() -> Result<(), String> {
    let root = repo_root();
    let commit = reference::frontend_commit(&root)?;
    if reference::frontend_dirty(&root)? {
        eprintln!(
            "warning: the reference frontend has uncommitted changes, but the cache key is the \
             last commit ({commit}); set UPDATE_REFERENCE=1 to regenerate the reference"
        );
    }
    let reference_dir = reference::ensure_reference(&root, &commit, reference::update_requested())?;
    let actual_dir = root.join("target").join("parity").join("actual");
    let captured = capture_all(&root, &actual_dir)?;
    for screen in &captured {
        println!(
            "gpui {}: {} element(s) -> {}",
            screen.id,
            screen.element_count,
            screen.layout.display()
        );
    }

    let report_dir = root.join("target").join("parity").join("report");
    let outcome = compare::run(&root, &reference_dir, &actual_dir, &report_dir)?;
    println!();
    for screen in &outcome.screens {
        print_screen(screen, &report_dir);
    }
    let failed = outcome
        .screens
        .iter()
        .filter(|screen| !screen.passed)
        .count();
    println!(
        "\n{} screen(s): {} passed, {} failed",
        outcome.screens.len(),
        outcome.screens.len() - failed,
        failed
    );
    println!("report: {}", outcome.report_path.display());
    println!("result: {}", outcome.result_path.display());
    if !outcome.passed {
        exit(1);
    }
    Ok(())
}

fn print_screen(screen: &ScreenReport, report_dir: &Path) {
    let status = if screen.passed { "PASS" } else { "FAIL" };
    let max_delta = screen
        .layout
        .elements
        .iter()
        .map(|element| element.max_bounds_delta)
        .fold(0.0_f32, f32::max);
    println!("{} {status}", screen.id);
    println!(
        "  layout: {} element(s), max bounds delta {:.2} px, {} failure(s)",
        screen.layout.elements.len(),
        max_delta,
        screen.layout.failures.len()
    );
    println!(
        "  color: mean deltaE00 {:.4}, max {:.4}, {} px above 1",
        screen.colors.mean_delta_e, screen.colors.max_delta_e, screen.colors.pixels_above_one
    );
    println!(
        "  ssim: full {:.6}, masked {:.6}",
        screen.ssim.full, screen.ssim.masked
    );
    let heatmap = if screen.heatmap.is_empty() {
        "(not generated)".to_owned()
    } else {
        report_dir.join(&screen.heatmap).display().to_string()
    };
    println!("  heatmap: {heatmap}");
    for failure in &screen.failures {
        println!("  failure: {failure}");
    }
}
