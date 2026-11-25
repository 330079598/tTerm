//! The staged field report written beside the screenshots.

use std::path::PathBuf;

/// One stage's evidence. A stage that fails keeps the observed screen and any
/// screenshots captured before the failure.
pub(super) struct StageResult {
    name: String,
    passed: bool,
    detail: String,
    observed: String,
    screenshots: Vec<String>,
}

pub(super) struct Report {
    output: PathBuf,
    stages: Vec<StageResult>,
    failure: Option<String>,
}

impl Report {
    pub(super) fn new(output: PathBuf) -> Self {
        Self {
            output,
            stages: Vec::new(),
            failure: None,
        }
    }

    pub(super) fn stage(
        &mut self,
        name: &str,
        passed: bool,
        detail: &str,
        observed: &str,
        screenshots: Vec<String>,
    ) {
        self.stages.push(StageResult {
            name: name.to_string(),
            passed,
            detail: detail.to_string(),
            observed: tail(observed, 12_000),
            screenshots,
        });
    }

    pub(super) fn finish_success(&mut self) {
        self.failure = None;
    }

    pub(super) fn finish_failure(&mut self, error: &str) {
        if self.failure.is_none() {
            self.failure = Some(error.to_string());
        }
    }

    pub(super) fn write(&self) -> Result<(), String> {
        let stages: Vec<_> = self
            .stages
            .iter()
            .map(|stage| {
                serde_json::json!({
                    "name": stage.name,
                    "passed": stage.passed,
                    "detail": stage.detail,
                    "observed": stage.observed,
                    "screenshots": stage.screenshots,
                })
            })
            .collect();
        let value = serde_json::json!({
            "schema": "TTERM.gpui.ssh_tui_field.v1",
            "outcome": if self.outcome() { "pass" } else { "fail" },
            "failure": self.failure,
            "output_dir": self.output.display().to_string(),
            "stages": stages,
        });
        let text = serde_json::to_string_pretty(&value)
            .map_err(|error| format!("The field report could not be encoded: {error}"))?;
        let path = self.output.join("report.json");
        std::fs::write(&path, text).map_err(|error| {
            format!(
                "The field report '{}' could not be written: {error}",
                path.display()
            )
        })
    }

    fn outcome(&self) -> bool {
        self.failure.is_none() && self.stages.iter().all(|stage| stage.passed)
    }
}

fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    text.chars().skip(count - max).collect()
}
