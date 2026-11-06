//! The reference side: where its cache lives and how it is regenerated.
//!
//! The reference screenshots are deliberately not in git. They live under
//! `target/parity/reference/<frontend commit>/`, where the commit is the last
//! one that touched the reference frontend. `UPDATE_REFERENCE=1` forces a
//! regeneration; a missing cache is generated on demand. Uncommitted frontend
//! changes do not change the key, so the tool warns when the tree is dirty.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The files whose last commit keys the reference cache.
pub const FRONTEND_PATHS: [&str; 9] = [
    "src",
    "parity",
    "scripts/parity",
    "package.json",
    "pnpm-lock.yaml",
    "vite.config.ts",
    "tsconfig.json",
    "index.html",
    "parity.html",
];

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

/// Where one commit's reference lives.
pub fn reference_dir(root: &Path, commit: &str) -> PathBuf {
    root.join("target")
        .join("parity")
        .join("reference")
        .join(commit)
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
pub fn ensure_reference(root: &Path, commit: &str, force: bool) -> Result<PathBuf, String> {
    let dir = reference_dir(root, commit);
    if !force && dir.join("manifest.json").is_file() {
        return Ok(dir);
    }
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("could not clear {}: {error}", dir.display()))?;
    }
    fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create {}: {error}", dir.display()))?;
    eprintln!(
        "reference: {} the React screens for {commit}",
        if force { "regenerating" } else { "capturing" }
    );
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
    Ok(dir)
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
    commit: Option<String>,
}
