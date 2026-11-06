//! The UI parity toolchain of the GPUI migration (W0-04).
//!
//! The React frontend is the reference. `scripts/parity/capture.mjs` runs it in
//! Vite's `parity` mode, where the Tauri IPC is answered from fixtures, and
//! writes a screenshot plus a layout JSON per screen with Playwright's WebKit.
//! This crate renders the same fixture-driven screens with GPUI's headless
//! renderer, writes the same two files, and compares them.
//!
//! The flow, thresholds and the meaning of every field are documented in
//! `docs/gpui-migration/parity/README.md`; the thresholds themselves come from
//! `docs/gpui-migration/ui-parity.md` section 5.2.
//!
//! ```text
//! cargo run -p tterm-parity -- capture   # GPUI side only, into target/parity/actual
//! cargo run -p tterm-parity -- compare   # reference (cached), actual, report
//! UPDATE_REFERENCE=1 cargo run -p tterm-parity -- compare
//! ```
//!
//! The reference screenshots are not in git. They are cached under
//! `target/parity/reference/<frontend commit>/`, where the commit is the last
//! one that touched the reference frontend. `UPDATE_REFERENCE=1` regenerates
//! that directory.

use std::path::{Path, PathBuf};

pub mod capture;
pub mod color;
pub mod compare;
pub mod fixture;
pub mod layout;
pub mod manifest;
pub mod marker;
pub mod reference;
pub mod screens;

/// The repository root: the crate lives at `<root>/crates/tterm-parity`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}
