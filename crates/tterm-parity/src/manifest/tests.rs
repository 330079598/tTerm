use super::*;

fn manifest() -> ScreensManifest {
    ScreensManifest {
        version: MANIFEST_VERSION,
        viewport: ViewportSpec {
            width: 800,
            height: 600,
            device_scale_factor: 2.0,
        },
        windows_viewport: None,
        theme: "dark".to_owned(),
        ui_scale_percent: 100,
        platform: PlatformSpec {
            os: "macos".to_owned(),
            version: "14.5.0".to_owned(),
            family: "unix".to_owned(),
            arch: "aarch64".to_owned(),
            os_type: "macos".to_owned(),
            exe_extension: String::new(),
            eol: "\n".to_owned(),
        },
        platforms: std::collections::BTreeMap::new(),
        screens: vec![ScreenSpec {
            id: "example.button-input.default".to_owned(),
            title: "Example".to_owned(),
            fixture: "example.button-input.default.json".to_owned(),
        }],
    }
}

#[test]
fn the_repository_manifest_is_valid() {
    let root = crate::repo_root();
    let loaded = ScreensManifest::load(&root).expect("the committed manifest is valid");
    assert_eq!(loaded.version, MANIFEST_VERSION);
    assert_eq!(loaded.viewport.device_scale_factor, 2.0);
    assert_eq!(loaded.screens.len(), 1);
}

#[test]
fn a_duplicate_screen_id_is_rejected() {
    let mut value = manifest();
    value.screens.push(value.screens[0].clone());
    expect_error(value, "twice");
}

#[test]
fn a_zero_viewport_is_rejected() {
    let mut value = manifest();
    value.viewport.width = 0;
    expect_error(value, "zero viewport");
}

#[test]
fn an_unknown_theme_is_rejected() {
    let mut value = manifest();
    value.theme = "sepia".to_owned();
    expect_error(value, "dark or light");
}

#[test]
fn a_fixture_path_is_rejected() {
    let mut value = manifest();
    value.screens[0].fixture = "nested/file.json".to_owned();
    expect_error(value, "file name only");
}

#[test]
fn the_wrong_schema_is_rejected() {
    let mut value = manifest();
    value.version = 1;
    expect_error(value, "expected 2");
}

#[test]
fn a_scale_other_than_one_hundred_is_rejected() {
    let mut value = manifest();
    value.ui_scale_percent = 125;
    expect_error(value, "only supports 100");
}

fn expect_error(manifest: ScreensManifest, fragment: &str) {
    let error = manifest.validate().expect_err("the manifest is invalid");
    assert!(
        error.contains(fragment),
        "expected {fragment:?} in {error:?}"
    );
}
