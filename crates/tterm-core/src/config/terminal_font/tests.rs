use serde_json::Value;

use super::{
    LINUX_FONT_FAMILY, MACOS_FONT_FAMILY, WINDOWS_FONT_FAMILY, default_font_family,
    detected_font_family, first_installed_font,
};

/// The expectations generated from the real TypeScript module by
/// `scripts/migration/P1-11c-oracle.mjs`.
fn oracle() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/gpui-migration/reports/P1-11c-evidence/node-macos.json"
    )))
    .expect("the P1-11c oracle is valid JSON")
}

fn fonts(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn puts_a_nerd_font_before_each_systems_monospace_font() {
    assert!(
        MACOS_FONT_FAMILY.starts_with("\"JetBrains Mono Nerd Font\", ")
            && MACOS_FONT_FAMILY.contains("Menlo")
    );
    assert!(
        WINDOWS_FONT_FAMILY.starts_with("\"JetBrains Mono Nerd Font\", ")
            && WINDOWS_FONT_FAMILY.contains("Consolas")
    );
    assert!(
        LINUX_FONT_FAMILY.starts_with("\"JetBrains Mono Nerd Font\", ")
            && LINUX_FONT_FAMILY.contains("DejaVu")
    );
}

#[test]
fn gives_an_unknown_platform_linuxs_fonts() {
    assert_eq!(default_font_family("freebsd"), default_font_family("linux"));
    assert_eq!(default_font_family("freebsd"), LINUX_FONT_FAMILY);
}

#[test]
fn names_the_first_font_of_the_chain_the_system_has() {
    let chain = "\"JetBrains Mono Nerd Font\", \"JetBrains Mono\", Menlo, monospace";

    assert_eq!(
        first_installed_font(chain, &fonts(&["Menlo", "jetbrains mono"])),
        "JetBrains Mono"
    );
    assert_eq!(first_installed_font(chain, &fonts(&["Menlo"])), "Menlo");
}

#[test]
fn keeps_the_chain_when_none_of_it_is_installed() {
    let chain = "\"JetBrains Mono Nerd Font\", \"JetBrains Mono\", Menlo, monospace";

    assert_eq!(first_installed_font(chain, &[]), chain);
}

#[test]
fn trims_each_entry_like_javascript() {
    let chain = "\u{feff}Menlo\u{00a0}, monospace";

    assert_eq!(
        first_installed_font(chain, &fonts(&["MENLO"])),
        "Menlo",
        "JavaScript trim removes a byte order mark and a no-break space"
    );
}

#[test]
fn drops_one_quote_from_each_end_of_each_entry() {
    assert_eq!(
        first_installed_font("\"\"Menlo\"\", monospace", &fonts(&["\"menlo\""])),
        "\"Menlo\""
    );
    assert_eq!(
        first_installed_font("'Menlo', monospace", &fonts(&["menlo"])),
        "Menlo"
    );
}

#[test]
fn default_font_chain_matches_the_typescript_module() {
    let frontend = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/lib/terminalFont.ts"
    ));

    for chain in [MACOS_FONT_FAMILY, WINDOWS_FONT_FAMILY, LINUX_FONT_FAMILY] {
        assert!(
            frontend.contains(chain),
            "src/lib/terminalFont.ts lacks the chain {chain:?}"
        );
    }
}

#[test]
fn oracle_matches_the_ported_font_helpers() {
    let oracle = oracle();
    let values = &oracle["values"];

    let families = &values["terminalFont.families"];
    assert_eq!(
        families["macos"].as_str().unwrap(),
        MACOS_FONT_FAMILY,
        "macOS chain"
    );
    assert_eq!(
        families["windows"].as_str().unwrap(),
        WINDOWS_FONT_FAMILY,
        "Windows chain"
    );
    assert_eq!(
        families["linux"].as_str().unwrap(),
        LINUX_FONT_FAMILY,
        "Linux chain"
    );
    assert_eq!(
        default_font_family(families["freebsd"].as_str().unwrap()),
        LINUX_FONT_FAMILY
    );
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    assert_eq!(detected_font_family(), default_font_family(platform));
    if cfg!(target_os = "macos") {
        assert_eq!(
            families["detected"].as_str().unwrap(),
            detected_font_family(),
            "the oracle ran on macOS"
        );
    }

    let entries = &values["terminalFont.entries"];
    let chain = entries["chain"].as_str().unwrap();
    let system_fonts: Vec<String> = entries["lookup"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(first_installed_font(chain, &system_fonts), "JetBrains Mono");

    let expected_entries: Vec<&str> = entries["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(
        chain
            .split(',')
            .map(super::clean_font_name)
            .collect::<Vec<_>>(),
        expected_entries
    );
}
