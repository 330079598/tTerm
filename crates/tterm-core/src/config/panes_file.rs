//! Persists saved pane layout templates without replacing other settings.

use std::{collections::HashSet, fs, io::ErrorKind, path::Path};

use indexmap::IndexMap;
use serde_json::value::{RawValue, to_raw_value};

use super::AppConfig;
use super::atomic::atomic_write;
use super::panes::{PaneConfig, PaneLayoutTemplate};
use super::paths::ensure_config_dir;
use crate::workspace::pane_templates::{is_builtin_template_id, normalize_layout_shape};

/// Replaces only `panes.templates`, or creates a complete first config.
pub fn save_pane_templates(templates: Vec<PaneLayoutTemplate>) -> Result<(), String> {
    save_at(&ensure_config_dir()?.join("config.json"), &templates)
}

pub(super) fn save_at(path: &Path, templates: &[PaneLayoutTemplate]) -> Result<(), String> {
    let templates = canonical_pane_templates(templates)?;
    let _edit = super::edit::lock()?;
    let previous = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let config = AppConfig {
                panes: PaneConfig {
                    templates,
                    ..PaneConfig::default()
                },
                ..AppConfig::default()
            };
            return super::compatible_file::save_at(path, &config);
        }
        Err(error) => {
            return Err(format!(
                "Cannot read settings at '{}': {error}",
                path.display()
            ));
        }
    };
    serde_json::from_str::<AppConfig>(&previous).map_err(|error| {
        format!(
            "Repair the settings at '{}' before saving: {error}",
            path.display()
        )
    })?;
    // Keep unrelated fields as raw JSON so a template edit does not round
    // future numbers or repair values the strict reader already accepted.
    let mut fields: IndexMap<String, &RawValue> = serde_json::from_str(&previous)
        .map_err(|error| format!("Cannot read settings fields: {error}"))?;
    let mut panes: IndexMap<String, &RawValue> = match fields.get("panes") {
        Some(panes) => serde_json::from_str(panes.get())
            .map_err(|error| format!("Cannot read pane settings: {error}"))?,
        None => IndexMap::new(),
    };
    let templates = to_raw_value(&templates).map_err(|error| error.to_string())?;
    panes.insert("templates".into(), &templates);
    let panes = to_raw_value(&panes).map_err(|error| error.to_string())?;
    fields.insert("panes".into(), &panes);
    let bytes = serde_json::to_vec_pretty(&fields).map_err(|error| error.to_string())?;
    atomic_write(path, bytes)
}

/// Validates and canonicalizes templates with the shared typed layout shape.
///
/// The returned roots use the exact `{type, orientation, children, sizes}`
/// representation both clients read, with sizes normalized to one.
pub fn canonical_pane_templates(
    templates: &[PaneLayoutTemplate],
) -> Result<Vec<PaneLayoutTemplate>, String> {
    let mut seen = HashSet::new();
    templates
        .iter()
        .map(|template| {
            if template.id.is_empty() {
                return Err("A pane template needs an id.".to_string());
            }
            if is_builtin_template_id(&template.id) {
                return Err(format!(
                    "The pane template id '{}' is reserved for a built-in layout.",
                    template.id
                ));
            }
            if !seen.insert(template.id.as_str()) {
                return Err(format!(
                    "The pane template id '{}' is repeated.",
                    template.id
                ));
            }
            let shape = normalize_layout_shape(&template.root).ok_or_else(|| {
                format!(
                    "The pane template '{}' has an invalid split layout.",
                    template.id
                )
            })?;
            let root = serde_json::to_value(shape).map_err(|error| {
                format!("Cannot save the pane template '{}': {error}", template.id)
            })?;
            Ok(PaneLayoutTemplate {
                id: template.id.clone(),
                name: template.name.clone(),
                root,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use serde_json::{Value, json};

    use super::*;

    struct TestConfig {
        dir: PathBuf,
        path: PathBuf,
    }

    impl TestConfig {
        fn new() -> Self {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/test-tmp")
                .join(format!("tterm-pane-templates-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            let path = dir.join("config.json");
            Self { dir, path }
        }
    }

    impl Drop for TestConfig {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn template(id: &str, name: &str, root: Value) -> PaneLayoutTemplate {
        PaneLayoutTemplate {
            id: id.to_string(),
            name: name.to_string(),
            root,
        }
    }

    fn seed(path: &Path) {
        let config = AppConfig {
            font_size: 17,
            scrollback_lines: 4321,
            panes: PaneConfig {
                show_headers: false,
                dim_inactive_percent: 15,
                templates: Vec::new(),
            },
            ..AppConfig::default()
        };
        crate::config::compatible_file::save_at(path, &config).unwrap();
    }

    fn read(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    const FRONTEND_SAVED: &str =
        include_str!("../../../../src/lib/fixtures/paneTemplates.savedByFrontend.json");
    const RUST_SAVED: &str =
        include_str!("../../../../src/lib/fixtures/paneTemplates.savedByRust.json");

    #[test]
    fn replaces_only_templates_and_preserves_other_config_fields() {
        let config = TestConfig::new();
        seed(&config.path);
        let extended = fs::read_to_string(&config.path).unwrap().replacen(
            r#""panes""#,
            r#""future":{"exact":1.2300e+45},"panes""#,
            1,
        );
        let extended = extended.replacen(
            r#""templates": []"#,
            r#""future_pane_setting":{"keep":[1,2,3]},"future_pane_number":1.2300e+45,"templates": []"#,
            1,
        );
        fs::write(&config.path, extended).unwrap();
        let templates = vec![template(
            "ops",
            "Ops",
            json!({
                "type": "split",
                "orientation": "horizontal",
                "children": [
                    {"type": "pane"},
                    {
                        "type": "split",
                        "orientation": "vertical",
                        "children": [{"type": "pane"}, {"type": "pane"}],
                        "sizes": [0.35, 0.65]
                    }
                ],
                "sizes": [0.62, 0.38]
            }),
        )];

        save_at(&config.path, &templates).unwrap();

        let saved = read(&config.path);
        assert_eq!(saved["font_size"], 17);
        assert_eq!(saved["scrollback_lines"], 4321);
        assert_eq!(saved["future"]["exact"], json!(1.23e45));
        assert_eq!(saved["panes"]["show_headers"], json!(false));
        assert_eq!(saved["panes"]["dim_inactive_percent"], 15);
        assert_eq!(
            saved["panes"]["future_pane_setting"],
            json!({"keep": [1, 2, 3]})
        );
        assert_eq!(saved["panes"]["future_pane_number"], json!(1.23e45));
        assert_eq!(
            saved["panes"]["templates"],
            serde_json::to_value(templates).unwrap()
        );
        let raw = fs::read_to_string(&config.path).unwrap();
        assert!(raw.contains(r#""future": {"exact":1.2300e+45}"#));
        assert!(raw.contains(r#""future_pane_number":1.2300e+45"#));
    }

    #[test]
    fn creates_a_complete_config_when_the_file_does_not_exist() {
        let config = TestConfig::new();
        let templates = vec![template(
            "rows",
            "Rows",
            json!({
                "type": "split",
                "orientation": "vertical",
                "children": [{"type": "pane"}, {"type": "pane"}],
                "sizes": [0.5, 0.5]
            }),
        )];

        save_at(&config.path, &templates).unwrap();

        let loaded = crate::config::load_config_at(&config.path).unwrap();
        assert_eq!(loaded.theme, AppConfig::default().theme);
        assert_eq!(loaded.panes.templates, templates);
    }

    #[test]
    fn refuses_to_write_invalid_templates_or_overwrite_a_damaged_config() {
        let config = TestConfig::new();
        seed(&config.path);
        let original = fs::read_to_string(&config.path).unwrap();
        for invalid in [
            vec![template("", "No id", json!({"type": "pane"}))],
            vec![template(
                "builtin:rows-2",
                "Reserved",
                json!({
                    "type": "split",
                    "orientation": "vertical",
                    "children": [{"type": "pane"}, {"type": "pane"}],
                    "sizes": [0.5, 0.5]
                }),
            )],
            vec![
                template(
                    "same",
                    "First",
                    json!({
                        "type": "split",
                        "orientation": "vertical",
                        "children": [{"type": "pane"}, {"type": "pane"}],
                        "sizes": [0.5, 0.5]
                    }),
                ),
                template(
                    "same",
                    "Second",
                    json!({
                        "type": "split",
                        "orientation": "vertical",
                        "children": [{"type": "pane"}, {"type": "pane"}],
                        "sizes": [0.5, 0.5]
                    }),
                ),
            ],
            vec![template("single", "Single pane", json!({"type": "pane"}))],
        ] {
            assert!(save_at(&config.path, &invalid).is_err());
            assert_eq!(fs::read_to_string(&config.path).unwrap(), original);
        }

        for damaged in ["{", r#"{"theme":false}"#, r#"{"panes":[]}"#] {
            fs::write(&config.path, damaged).unwrap();
            assert!(save_at(&config.path, &[]).is_err());
            assert_eq!(fs::read_to_string(&config.path).unwrap(), damaged);
        }
    }

    #[test]
    fn scrollback_and_template_edits_share_one_read_modify_write_lock() {
        let config = TestConfig::new();
        seed(&config.path);
        let before = fs::read_to_string(&config.path).unwrap();
        let held = crate::config::edit::lock().unwrap();
        let (finished_tx, finished_rx) = mpsc::channel();
        let scrollback_path = config.path.clone();
        let scrollback_finished = finished_tx.clone();
        let scrollback = thread::spawn(move || {
            let result = crate::config::scrollback_file::save_at(&scrollback_path, 9876);
            let _ = scrollback_finished.send(());
            result
        });
        let template_path = config.path.clone();
        let templates = vec![template(
            "race",
            "Race",
            json!({
                "type": "split",
                "orientation": "horizontal",
                "children": [{"type": "pane"}, {"type": "pane"}],
                "sizes": [0.4, 0.6]
            }),
        )];
        let template_finished = finished_tx;
        let template_edit = thread::spawn(move || {
            let result = save_at(&template_path, &templates);
            let _ = template_finished.send(());
            result
        });

        assert!(
            finished_rx
                .recv_timeout(Duration::from_millis(200))
                .is_err()
        );
        assert_eq!(fs::read_to_string(&config.path).unwrap(), before);
        drop(held);

        scrollback.join().unwrap().unwrap();
        template_edit.join().unwrap().unwrap();
        let saved = crate::config::load_config_at(&config.path).unwrap();
        assert_eq!(saved.scrollback_lines, 9876);
        assert_eq!(saved.panes.templates.len(), 1);
        assert_eq!(saved.panes.templates[0].id, "race");
        assert!(!saved.panes.show_headers);
    }

    #[test]
    fn reads_the_frontend_saved_fixture_and_round_trips_through_the_writer() {
        let config: AppConfig = serde_json::from_str(FRONTEND_SAVED).unwrap();
        let fixture: Value = serde_json::from_str(FRONTEND_SAVED).unwrap();
        assert!(!config.panes.show_headers);
        assert_eq!(config.panes.dim_inactive_percent, 15);
        assert_eq!(config.panes.templates[0].id, "frontend-ops");
        assert_eq!(config.panes.templates[0].name, "Frontend Ops");
        assert_eq!(config.panes.templates[0].root["sizes"], json!([0.62, 0.38]));
        assert_eq!(
            config.panes.templates[0].root["children"][1]["sizes"],
            json!([0.35, 0.65])
        );
        assert_eq!(config.panes.templates[1].id, "frontend-triad");
        assert_eq!(config.panes.templates[1].name, "Frontend Triad");
        assert_eq!(
            config.panes.templates[1].root["sizes"],
            json!([0.2, 0.3, 0.5])
        );
        let serialized = serde_json::to_value(&config.panes).unwrap();
        assert_eq!(serialized["show_headers"], fixture["panes"]["show_headers"]);
        assert_eq!(
            serialized["dim_inactive_percent"],
            fixture["panes"]["dim_inactive_percent"]
        );
        assert_eq!(serialized["templates"], fixture["panes"]["templates"]);

        let test = TestConfig::new();
        fs::write(&test.path, FRONTEND_SAVED).unwrap();
        save_at(&test.path, &config.panes.templates).unwrap();

        let saved = read(&test.path);
        assert_eq!(saved["future"], fixture["future"]);
        assert_eq!(
            saved["panes"]["future_pane_setting"],
            fixture["panes"]["future_pane_setting"]
        );
        assert_eq!(saved["panes"]["templates"], fixture["panes"]["templates"]);
        assert_eq!(saved["panes"]["show_headers"], json!(false));
        assert_eq!(saved["panes"]["dim_inactive_percent"], 15);
        assert_eq!(
            crate::config::load_config_at(&test.path)
                .unwrap()
                .panes
                .templates,
            config.panes.templates
        );
    }

    #[test]
    fn rust_saved_fixture_uses_the_typed_tauri_config_format() {
        let config: AppConfig = serde_json::from_str(RUST_SAVED).unwrap();
        let fixture: Value = serde_json::from_str(RUST_SAVED).unwrap();
        assert_eq!(
            serde_json::to_value(&config.panes).unwrap(),
            fixture["panes"]
        );
        assert_eq!(config.panes.templates[0].id, "rust-nested");
        assert_eq!(config.panes.templates[0].name, "Rust Nested");
        assert_eq!(config.panes.templates[0].root["sizes"], json!([0.44, 0.56]));
        assert_eq!(
            config.panes.templates[0].root["children"][1]["sizes"],
            json!([0.28, 0.72])
        );
        assert_eq!(config.panes.templates[1].id, "rust-even");
        assert_eq!(config.panes.templates[1].name, "Rust Even");
        assert_eq!(config.panes.templates[1].root["sizes"], json!([0.5, 0.5]));
    }

    #[test]
    fn tauri_and_native_saves_produce_the_same_template_shape() {
        let config: AppConfig = serde_json::from_str(RUST_SAVED).unwrap();
        let fixture: Value = serde_json::from_str(RUST_SAVED).unwrap();
        let tauri = TestConfig::new();
        crate::config::compatible_file::save_at(&tauri.path, &config).unwrap();
        let native = TestConfig::new();
        crate::config::compatible_file::save_at(
            &native.path,
            &AppConfig {
                panes: PaneConfig {
                    show_headers: false,
                    dim_inactive_percent: 30,
                    templates: Vec::new(),
                },
                ..AppConfig::default()
            },
        )
        .unwrap();

        save_at(&native.path, &config.panes.templates).unwrap();

        let tauri_saved = crate::config::load_config_at(&tauri.path).unwrap();
        let native_saved = crate::config::load_config_at(&native.path).unwrap();
        assert_eq!(tauri_saved.panes.templates, config.panes.templates);
        assert_eq!(native_saved.panes.templates, config.panes.templates);
        assert!(!native_saved.panes.show_headers);
        assert_eq!(native_saved.panes.dim_inactive_percent, 30);
        assert_eq!(
            read(&native.path)["panes"]["templates"],
            fixture["panes"]["templates"]
        );
        assert_eq!(
            read(&tauri.path)["panes"]["templates"],
            fixture["panes"]["templates"]
        );
    }
}
