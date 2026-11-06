//! The reference side: where its cache lives and how it is regenerated.
//!
//! The reference screenshots are deliberately not in git. They live under
//! `target/parity/reference/<frontend commit>/`, where the commit is the last
//! one that touched the reference frontend. `UPDATE_REFERENCE=1` forces a
//! regeneration; a missing cache is generated on demand.
//!
//! Uncommitted frontend changes get their own `-dirty` suffix and are
//! regenerated on every run. Reusing a clean cache for a dirty tree would
//! compare GPUI against a screenshot that does not contain the change under
//! test.

use crate::layout::LayoutDocument;
use crate::manifest::ScreensManifest;
use crate::measure;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files whose last commit keys the reference cache.
pub const FRONTEND_PATHS: [&str; 10] = [
    "src",
    "parity/screens.json",
    "parity/fixtures",
    "scripts/parity",
    "package.json",
    "pnpm-lock.yaml",
    "vite.config.ts",
    "tsconfig.json",
    "index.html",
    "parity.html",
];

/// The reference manifest schema written by `scripts/parity/capture.mjs`.
pub const REFERENCE_MANIFEST_VERSION: u32 = 2;

/// The last commit that touched the reference frontend.
pub fn frontend_commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%H", "--"])
        .args(FRONTEND_PATHS)
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git could not read the frontend commit: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit.is_empty() {
        return Err("the reference frontend has no commit yet".to_owned());
    }
    Ok(commit)
}

/// Whether any file that shapes the reference has uncommitted changes.
pub fn frontend_dirty(root: &Path) -> Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain", "--"])
        .args(FRONTEND_PATHS)
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git could not check the frontend tree: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
}

/// Where one commit's reference lives, with a separate directory for a dirty
/// frontend.
pub fn reference_dir(root: &Path, commit: &str, dirty: bool) -> PathBuf {
    let name = if dirty {
        format!("{commit}-dirty")
    } else {
        commit.to_owned()
    };
    root.join("target")
        .join("parity")
        .join("reference")
        .join(name)
}

/// Whether `UPDATE_REFERENCE=1` (or any value other than 0/false) is set.
pub fn update_requested() -> bool {
    match std::env::var("UPDATE_REFERENCE") {
        Ok(value) => {
            let text = value.trim().to_ascii_lowercase();
            !text.is_empty() && text != "0" && text != "false"
        }
        Err(_) => false,
    }
}

/// Returns the reference directory, capturing it if it is missing or forced.
///
/// A dirty frontend always regenerates: its `-dirty` directory is never reused.
pub fn ensure_reference(
    root: &Path,
    commit: &str,
    dirty: bool,
    force: bool,
) -> Result<PathBuf, String> {
    let manifest = ScreensManifest::load(root)?;
    let dir = reference_dir(root, commit, dirty);
    let current = !dirty && cache_is_current(&dir, &manifest);
    if !should_regenerate(dirty, force, current) {
        return Ok(dir);
    }

    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("could not clear {}: {error}", dir.display()))?;
    }
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    if dirty {
        eprintln!(
            "reference: the frontend is dirty; regenerating {} and not reusing any cache",
            dir.display()
        );
    } else {
        eprintln!(
            "reference: {} the React screens for {commit}",
            if force { "regenerating" } else { "capturing" }
        );
    }
    let status = Command::new("node")
        .arg("scripts/parity/capture.mjs")
        .arg("--out")
        .arg(&dir)
        .arg("--commit")
        .arg(commit)
        .current_dir(root)
        .status()
        .map_err(|error| format!("could not run node: {error}"))?;
    if !status.success() {
        return Err(format!(
            "the reference capture failed (exit {status}); is Playwright installed? See \
             docs/gpui-migration/parity/README.md"
        ));
    }
    measure_reference(&dir, &manifest)?;
    Ok(dir)
}

/// Decides whether a reference directory must be captured again.
pub fn should_regenerate(dirty: bool, force: bool, current: bool) -> bool {
    dirty || force || !current
}

fn cache_is_current(dir: &Path, manifest: &ScreensManifest) -> bool {
    let manifest_path = dir.join("manifest.json");
    let Ok(text) = fs::read_to_string(&manifest_path) else {
        return false;
    };
    let Ok(reference) = serde_json::from_str::<ReferenceManifest>(&text) else {
        return false;
    };
    if reference.version != REFERENCE_MANIFEST_VERSION
        || reference.commit.is_none()
        || reference.screens.len() != manifest.screens.len()
    {
        return false;
    }
    manifest.screens.iter().all(|screen| {
        let Some(captured) = reference
            .screens
            .iter()
            .find(|captured| captured.id.as_deref() == Some(screen.id.as_str()))
        else {
            return false;
        };
        let layout_path = dir.join(&captured.layout);
        let image_path = dir.join(&captured.image);
        if !image_path.is_file() {
            return false;
        }
        LayoutDocument::load(&layout_path)
            .map(|document| document.screen == screen.id)
            .unwrap_or(false)
    })
}

/// Replaces the colours `collectLayout` read from computed styles with the
/// colours WebKit actually painted. The declared values stay in the document.
fn measure_reference(dir: &Path, manifest: &ScreensManifest) -> Result<(), String> {
    for screen in &manifest.screens {
        let image_path = dir.join(format!("{}.png", screen.id));
        let layout_path = dir.join(format!("{}.layout.json", screen.id));
        let image = image::open(&image_path)
            .map_err(|error| format!("could not read {}: {error}", image_path.display()))?
            .to_rgba8();
        let mut document = LayoutDocument::load(&layout_path)?;
        if document.screen != screen.id {
            return Err(format!(
                "{} says it is screen \"{}\", expected \"{}\"",
                layout_path.display(),
                document.screen,
                screen.id
            ));
        }
        measure::measure_document(&image, &mut document)
            .map_err(|error| format!("could not measure {}: {error}", screen.id))?;
        document.write(&layout_path)?;
    }
    Ok(())
}

/// The commit recorded in a reference directory's manifest.
pub fn recorded_commit(dir: &Path) -> Result<String, String> {
    let path = dir.join("manifest.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let manifest: ReferenceManifest = serde_json::from_str(&text)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
    manifest.commit.ok_or_else(|| {
        format!(
            "{} does not record the frontend commit; regenerate the reference",
            path.display()
        )
    })
}

#[derive(Deserialize)]
struct ReferenceManifest {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    commit: Option<String>,
    #[serde(default)]
    screens: Vec<ReferenceScreen>,
}

#[derive(Deserialize)]
struct ReferenceScreen {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    image: String,
    #[serde(default)]
    layout: String,
}

#[cfg(test)]
mod tests;
