use indexmap::IndexMap;

use crate::distro::DISTRO_IDS;
use crate::distro::DistroId;
use crate::workspace::tab::{Tab, TabType};
use crate::workspace::tab_icons::{
    AGENT_LOGO_COLORS, AGENT_LOGO_IDS, DEFAULT_TAB_ICON_CONFIG, ResolvedTabIcon, TAB_ICON_COLORS,
    TAB_ICON_IDS, TabIconConfig, TabIconStyle, agent_tab_icon, apply_tab_icon_style,
    built_in_shell_icon, has_local_badge, has_tab_icon, inherited_tab_icon, is_agent_tab_icon,
    is_brand_tab_icon, is_distro_tab_icon, normalize_saved_ssh_tab, normalize_tab_icon_config,
    normalize_tab_icon_style, picked_logo_color, resolve_tab_icon, shell_display_name, shell_key,
    shell_tab_icon, shells_for_settings, ssh_tab_icon, tab_icon_agent, tab_icon_brand,
    tab_icon_distro, with_local_distro, with_shell_icon_style,
};

use super::oracle_11d2;

fn style(icon: Option<&str>, color: Option<&str>) -> TabIconStyle {
    TabIconStyle {
        icon: icon.map(str::to_string),
        color: color.map(str::to_string),
    }
}

fn icon(icon: &str, color: &str) -> ResolvedTabIcon {
    ResolvedTabIcon {
        icon: icon.to_string(),
        color: color.to_string(),
    }
}

fn config(entries: &[(&str, TabIconStyle)]) -> TabIconConfig {
    let mut shells = IndexMap::new();
    for (key, value) in entries {
        shells.insert((*key).to_string(), value.clone());
    }
    TabIconConfig {
        enabled: true,
        agents: true,
        shells,
    }
}

fn tab(tab_type: TabType, shell_name: Option<&str>) -> Tab {
    let mut tab = Tab::new("a", "a", tab_type);
    tab.shell_name = shell_name.map(str::to_string);
    tab
}

#[test]
fn draws_a_shell_with_its_built_in_look_until_the_user_picks_otherwise() {
    assert_eq!(
        resolve_tab_icon(
            &tab(TabType::Terminal, Some("pwsh")),
            &DEFAULT_TAB_ICON_CONFIG,
            None
        ),
        Some(icon("square-chevron-right", "#3b9ae1"))
    );
    assert_eq!(
        resolve_tab_icon(
            &tab(TabType::Terminal, Some("elvish")),
            &DEFAULT_TAB_ICON_CONFIG,
            None
        ),
        Some(icon("terminal", ""))
    );
}

#[test]
fn lays_the_shells_choice_over_the_built_in_look_and_the_tabs_over_both() {
    let icons = config(&[("pwsh", style(None, Some("#e5534b")))]);
    assert_eq!(
        resolve_tab_icon(&tab(TabType::Terminal, Some("PWSH")), &icons, None),
        Some(icon("square-chevron-right", "#e5534b"))
    );
    let mut chosen = tab(TabType::Terminal, Some("pwsh"));
    chosen.icon_style = Some(style(Some("rocket"), Some("")));
    assert_eq!(
        resolve_tab_icon(&chosen, &icons, None),
        Some(icon("rocket", ""))
    );
}

#[test]
fn draws_an_ssh_tab_as_a_server_until_its_hosts_distribution_is_known() {
    assert_eq!(
        resolve_tab_icon(&tab(TabType::Ssh, None), &DEFAULT_TAB_ICON_CONFIG, None),
        Some(icon("server", ""))
    );
    let mut with_distro = tab(TabType::Ssh, None);
    with_distro.distro_id = Some("debian".to_string());
    assert_eq!(
        resolve_tab_icon(&with_distro, &DEFAULT_TAB_ICON_CONFIG, None),
        Some(icon("distro:debian", ""))
    );
}

#[test]
fn lays_an_ssh_tabs_own_choice_over_its_hosts_distribution() {
    let mut tab = tab(TabType::Ssh, None);
    tab.distro_id = Some("ubuntu".to_string());
    tab.icon_style = Some(style(Some("database"), Some("#e5534b")));
    assert_eq!(
        resolve_tab_icon(&tab, &DEFAULT_TAB_ICON_CONFIG, None),
        Some(icon("database", "#e5534b"))
    );
}

#[test]
fn draws_a_wsl_tab_with_the_distribution_its_launcher_reported() {
    assert_eq!(
        resolve_tab_icon(
            &tab(TabType::Terminal, Some("WSL")),
            &DEFAULT_TAB_ICON_CONFIG,
            None
        ),
        Some(icon("terminal", "#e9853a"))
    );
    let mut tab = tab(TabType::Terminal, Some("WSL"));
    tab.distro_id = Some("arch".to_string());
    assert_eq!(
        resolve_tab_icon(&tab, &DEFAULT_TAB_ICON_CONFIG, None),
        Some(icon("distro:arch", ""))
    );
}

#[test]
fn lays_the_users_choice_for_wsl_over_the_reported_distribution() {
    let mut tab = tab(TabType::Terminal, Some("WSL"));
    tab.distro_id = Some("arch".to_string());
    assert_eq!(
        resolve_tab_icon(&tab, &config(&[("wsl", style(Some("rocket"), None))]), None),
        Some(icon("rocket", ""))
    );
}

#[test]
fn keeps_a_distribution_off_a_local_tab_that_no_longer_runs_wsl() {
    let mut tab = tab(TabType::Terminal, Some("pwsh"));
    tab.distro_id = Some("arch".to_string());
    assert_eq!(
        resolve_tab_icon(&tab, &DEFAULT_TAB_ICON_CONFIG, None)
            .map(|icon| icon.icon)
            .as_deref(),
        Some("square-chevron-right")
    );
}

#[test]
fn shows_nothing_while_icons_are_off_or_for_other_tabs() {
    let disabled = TabIconConfig {
        enabled: false,
        ..TabIconConfig::default()
    };
    assert_eq!(
        resolve_tab_icon(&tab(TabType::Terminal, None), &disabled, None),
        None
    );
    assert_eq!(
        resolve_tab_icon(&tab(TabType::Ssh, None), &disabled, None),
        None
    );
    assert_eq!(
        resolve_tab_icon(
            &tab(TabType::RemoteFileEditor, None),
            &DEFAULT_TAB_ICON_CONFIG,
            None
        ),
        None
    );
}

#[test]
fn shows_the_logo_of_an_agent_running_in_the_tab_over_the_shells_or_hosts_icon() {
    let pwsh = tab(TabType::Terminal, Some("pwsh"));
    assert_eq!(
        resolve_tab_icon(&pwsh, &DEFAULT_TAB_ICON_CONFIG, Some("claude")),
        Some(icon("agent:claude", "#d97757"))
    );
    let mut ssh = tab(TabType::Ssh, None);
    ssh.distro_id = Some("debian".to_string());
    assert_eq!(
        resolve_tab_icon(&ssh, &DEFAULT_TAB_ICON_CONFIG, Some("pi")),
        Some(icon("agent:pi", ""))
    );
    assert_eq!(
        resolve_tab_icon(&ssh, &DEFAULT_TAB_ICON_CONFIG, Some("codex")),
        Some(icon("agent:codex", ""))
    );
}

#[test]
fn keeps_the_usual_icon_for_an_agent_without_a_logo_or_with_agent_logos_off() {
    let pwsh = tab(TabType::Terminal, Some("pwsh"));
    assert_eq!(
        resolve_tab_icon(&pwsh, &DEFAULT_TAB_ICON_CONFIG, Some("my-agent"))
            .map(|icon| icon.icon)
            .as_deref(),
        Some("square-chevron-right")
    );
    let no_agents = TabIconConfig {
        agents: false,
        ..TabIconConfig::default()
    };
    assert_eq!(
        resolve_tab_icon(&pwsh, &no_agents, Some("claude"))
            .map(|icon| icon.icon)
            .as_deref(),
        Some("square-chevron-right")
    );
}

#[test]
fn lays_the_tabs_own_choice_over_an_agents_logo() {
    let mut chosen = tab(TabType::Ssh, None);
    chosen.icon_style = Some(style(Some("database"), Some("#e5534b")));
    assert_eq!(
        resolve_tab_icon(&chosen, &DEFAULT_TAB_ICON_CONFIG, Some("claude")),
        Some(icon("database", "#e5534b"))
    );
    let mut tinted = tab(TabType::Ssh, None);
    tinted.icon_style = Some(style(None, Some("#e5534b")));
    assert_eq!(
        resolve_tab_icon(&tinted, &DEFAULT_TAB_ICON_CONFIG, Some("opencode")),
        Some(icon("agent:opencode", "#e5534b"))
    );
}

#[test]
fn shows_an_agent_logo_picked_for_the_tab_whether_or_not_an_agent_runs() {
    let mut tab = tab(TabType::Terminal, None);
    tab.icon_style = Some(style(Some("agent:claude"), None));
    let no_agents = TabIconConfig {
        agents: false,
        ..TabIconConfig::default()
    };
    assert_eq!(
        resolve_tab_icon(&tab, &no_agents, None)
            .map(|icon| icon.icon)
            .as_deref(),
        Some("agent:claude")
    );
    assert_eq!(
        resolve_tab_icon(&tab, &DEFAULT_TAB_ICON_CONFIG, Some("codex"))
            .map(|icon| icon.icon)
            .as_deref(),
        Some("agent:claude")
    );
}

#[test]
fn keeps_known_icons_and_well_formed_colors_only() {
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "fish", "color": "#AABBCC" })),
        Some(style(Some("fish"), Some("#aabbcc")))
    );
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "unicorn", "color": "red" })),
        None
    );
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "color": "" })),
        Some(style(None, Some("")))
    );
    assert_eq!(normalize_tab_icon_style(&serde_json::json!("pwsh")), None);
}

#[test]
fn keeps_the_logos_of_known_distributions_only() {
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "distro:arch" })),
        Some(style(Some("distro:arch"), None))
    );
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "distro:windows" })),
        None
    );
}

#[test]
fn keeps_the_logos_of_known_shells_and_tools_only() {
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "logo:docker" })),
        Some(style(Some("logo:docker"), None))
    );
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "logo:fish" })),
        None
    );
}

#[test]
fn keeps_a_picked_ai_agent_logo_for_agents_with_one_only() {
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "agent:claude" })),
        Some(style(Some("agent:claude"), None))
    );
    assert_eq!(
        normalize_tab_icon_style(&serde_json::json!({ "icon": "agent:my-agent" })),
        None
    );
}

#[test]
fn gives_a_logo_its_own_color_and_leaves_glyphs_and_distributions_their_color() {
    assert_eq!(
        picked_logo_color("logo:docker"),
        Some("#2496ed".to_string())
    );
    assert_eq!(picked_logo_color("logo:github"), Some(String::new()));
    assert_eq!(
        picked_logo_color("agent:claude"),
        Some("#d97757".to_string())
    );
    assert_eq!(picked_logo_color("agent:codex"), Some(String::new()));
    assert_eq!(picked_logo_color("rocket"), None);
    assert_eq!(picked_logo_color("distro:arch"), None);
}

#[test]
fn badges_a_wsl_tabs_distribution_logo_reported_or_chosen() {
    assert!(has_local_badge(
        TabType::Terminal,
        Some("WSL"),
        &icon("distro:ubuntu", "")
    ));
}

#[test]
fn leaves_glyphs_ssh_hosts_and_other_shells_unbadged() {
    assert!(!has_local_badge(
        TabType::Terminal,
        Some("WSL"),
        &icon("rocket", "")
    ));
    assert!(!has_local_badge(
        TabType::Ssh,
        None,
        &icon("distro:ubuntu", "")
    ));
    assert!(!has_local_badge(
        TabType::Terminal,
        Some("zsh"),
        &icon("distro:ubuntu", "")
    ));
}

#[test]
fn gives_a_local_tab_the_reported_distribution_once() {
    let mut local = tab(TabType::Terminal, Some("WSL"));
    local.title = "WSL".to_string();
    local.is_active = true;
    let next = with_local_distro(&local, DistroId::Debian);
    assert_eq!(next.distro_id.as_deref(), Some("debian"));
    assert_eq!(with_local_distro(&next, DistroId::Debian), next);
}

#[test]
fn leaves_an_ssh_tabs_distribution_to_its_host() {
    let mut ssh = tab(TabType::Ssh, Some("WSL"));
    ssh.title = "WSL".to_string();
    let result = with_local_distro(&ssh, DistroId::Debian);
    assert_eq!(result, ssh);
}

#[test]
fn drops_an_unknown_distribution_and_icon_from_a_saved_ssh_tab() {
    let mut saved = tab(TabType::Ssh, None);
    saved.title = "host".to_string();
    saved.distro_id = Some("beos".to_string());
    saved.icon_style = Some(style(Some("unicorn"), None));
    let tab = normalize_saved_ssh_tab(&saved);
    assert_eq!(tab.distro_id, None);
    assert_eq!(tab.icon_style, None);
}

#[test]
fn shows_icons_unless_turned_off_and_keys_shells_in_lower_case() {
    assert_eq!(
        normalize_tab_icon_config(&serde_json::Value::Null),
        TabIconConfig::default()
    );
    assert_eq!(
        normalize_tab_icon_config(&serde_json::json!({
            "enabled": false,
            "agents": false,
            "shells": { "Git Bash": { "icon": "box" }, "cmd": { "icon": "nope" } }
        })),
        TabIconConfig {
            enabled: false,
            agents: false,
            shells: IndexMap::from([("git bash".to_string(), style(Some("box"), None))]),
        }
    );
}

#[test]
fn sets_a_shells_choice_and_forgets_it_for_none() {
    let set = with_shell_icon_style(
        &TabIconConfig::default(),
        "zsh",
        Some(style(Some("star"), None)),
    );
    assert_eq!(
        set.shells,
        IndexMap::from([("zsh".to_string(), style(Some("star"), None))])
    );
    assert!(with_shell_icon_style(&set, "zsh", None).shells.is_empty());
}

#[test]
fn lists_the_platforms_shells_then_others_with_a_choice_saved() {
    let icons = config(&[
        ("nu", style(Some("box"), None)),
        ("zsh", style(Some("star"), None)),
    ]);
    assert_eq!(
        shells_for_settings("macos", &icons, &[]),
        vec!["zsh", "bash", "fish", "sh", "nu"]
    );
    assert!(
        shells_for_settings("windows", &TabIconConfig::default(), &[])
            .contains(&"git bash".to_string())
    );
}

#[test]
fn adds_the_shells_found_on_this_machine_after_the_platforms() {
    assert_eq!(
        shells_for_settings(
            "linux",
            &TabIconConfig::default(),
            &[Some("Nu"), None, Some("zsh")]
        ),
        vec!["zsh", "bash", "fish", "sh", "nu"]
    );
    let icons = config(&[
        ("nu", style(Some("box"), None)),
        ("elvish", style(Some("star"), None)),
    ]);
    let shells = shells_for_settings("windows", &icons, &[Some("nu")]);
    assert_eq!(&shells[shells.len() - 2..], &["nu", "elvish"]);
}

#[test]
fn is_what_a_shells_tabs_show_without_a_choice_of_their_own() {
    assert_eq!(
        shell_tab_icon(&TabIconConfig::default(), Some("Git Bash")),
        icon("logo:git", "#f03c2e")
    );
    assert_eq!(
        shell_tab_icon(&TabIconConfig::default(), Some("nu")),
        icon("logo:nushell", "#4e9a06")
    );
}

#[test]
fn lays_a_shells_chosen_color_over_its_logo() {
    let icons = config(&[("bash", style(None, Some("#a371f7")))]);
    assert_eq!(
        shell_tab_icon(&icons, Some("bash")),
        icon("logo:bash", "#a371f7")
    );
}

#[test]
fn oracle_matches_shell_keys_and_normalized_styles() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];
    for case in icons["shellKeys"].as_array().expect("shell keys") {
        assert_eq!(
            shell_key(case["input"].as_str()),
            case["key"].as_str().expect("a key"),
            "{case:?}"
        );
    }
    for case in icons["normalizedStyles"].as_array().expect("styles") {
        assert_eq!(
            serde_json::to_value(normalize_tab_icon_style(&case["input"])).unwrap(),
            case["style"],
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_normalized_configs_and_shell_lists() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];
    for case in icons["normalizedConfigs"].as_array().expect("configs") {
        assert_eq!(
            serde_json::to_value(normalize_tab_icon_config(&case["input"])).unwrap(),
            case["config"],
            "{case:?}"
        );
    }
    for case in icons["shellsForSettings"].as_array().expect("settings") {
        let mut config = TabIconConfig {
            enabled: case["config"]["enabled"].as_bool().expect("enabled"),
            agents: case["config"]["agents"].as_bool().expect("agents"),
            shells: IndexMap::new(),
        };
        for (key, style) in case["config"]["shells"].as_object().expect("shells") {
            config.shells.insert(
                key.clone(),
                serde_json::from_value(style.clone()).expect("a saved style"),
            );
        }
        let found: Vec<Option<&str>> = case["found"]
            .as_array()
            .expect("found")
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(
            serde_json::to_value(shells_for_settings(
                case["platform"].as_str().expect("platform"),
                &config,
                &found
            ))
            .unwrap(),
            case["shells"],
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_resolved_icons_badges_and_picked_colors() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];
    for case in icons["resolved"].as_array().expect("resolved") {
        let mut tab = Tab::new("a", "a", tab_type(&case["tab"]["type"]));
        tab.shell_name = case["tab"]["shellName"].as_str().map(str::to_string);
        tab.distro_id = case["tab"]["distroId"].as_str().map(str::to_string);
        tab.icon_style = case["tab"]
            .get("iconStyle")
            .map(|value| serde_json::from_value(value.clone()).expect("a style"));
        let config = icon_config_from_json(&case["config"]);
        assert_eq!(
            serde_json::to_value(resolve_tab_icon(&tab, &config, case["agent"].as_str())).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    for case in icons["pickedColors"].as_array().expect("colors") {
        assert_eq!(
            serde_json::to_value(picked_logo_color(case["icon"].as_str().expect("icon"))).unwrap(),
            case["color"],
            "{case:?}"
        );
    }
    for case in icons["badges"].as_array().expect("badges") {
        let resolved: ResolvedTabIcon =
            serde_json::from_value(case["icon"].clone()).expect("an icon");
        assert_eq!(
            has_local_badge(
                tab_type(&case["tab"]["type"]),
                case["tab"]["shellName"].as_str(),
                &resolved
            ),
            case["badge"].as_bool().expect("a badge"),
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_serialized_configs_and_helper_results() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];
    assert_eq!(
        serde_json::to_string(&icon_config_from_json(&serde_json::json!({
            "enabled": true,
            "agents": true,
            "shells": {
                "git bash": { "icon": "box" },
                "zsh": { "color": "#aabbcc" },
                "elvish": { "icon": "logo:docker", "color": "" }
            }
        })))
        .unwrap(),
        icons["serializedConfig"].as_str().expect("bytes")
    );
    let normalized_config_input = serde_json::json!({
        "enabled": true,
        "agents": true,
        "shells": {
            "  PWSH ": { "icon": "box" },
            "10": { "icon": "rocket" },
            "2": { "icon": "star" },
            "1": { "color": "#AABBCC" },
            "zsh": { "icon": "logo:docker", "color": "" },
            "": { "icon": "star" }
        }
    });
    let normalized_config = normalize_tab_icon_config(&normalized_config_input);
    assert_eq!(
        serde_json::to_value(&normalized_config).unwrap(),
        icons["serializedNormalizedConfig"]["config"]
    );
    assert_eq!(
        serde_json::to_string(&normalized_config).unwrap(),
        icons["serializedNormalizedConfig"]["bytes"]
            .as_str()
            .expect("config bytes")
    );
    for case in icons["shellTabIcons"].as_array().expect("shell icons") {
        assert_eq!(
            serde_json::to_value(shell_tab_icon(
                &config(&[("bash", style(None, Some("#a371f7")))]),
                case["shellName"].as_str()
            ))
            .unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    assert_eq!(
        serde_json::to_value(normalize_saved_ssh_tab(
            &serde_json::from_value(serde_json::json!({
                "id": "a",
                "title": "host",
                "type": "ssh",
                "isActive": false,
                "distroId": "beos",
                "iconStyle": { "icon": "unicorn" }
            }))
            .expect("a saved tab")
        ))
        .unwrap(),
        icons["normalizedSsh"]
    );
    let shell_styles = icons["withShellStyles"].clone();
    let set = with_shell_icon_style(
        &TabIconConfig::default(),
        "zsh",
        Some(style(Some("star"), None)),
    );
    assert_eq!(serde_json::to_value(&set).unwrap(), shell_styles["set"]);
    assert_eq!(
        serde_json::to_value(with_shell_icon_style(&set, "zsh", None)).unwrap(),
        shell_styles["cleared"]
    );
    assert_eq!(
        serde_json::to_value(with_shell_icon_style(
            &set,
            "zsh",
            Some(style(Some("unicorn"), None))
        ))
        .unwrap(),
        shell_styles["invalid"]
    );
    assert_eq!(
        serde_json::to_value(with_shell_icon_style(
            &TabIconConfig::default(),
            "  ZSH ",
            Some(style(Some("star"), None))
        ))
        .unwrap(),
        shell_styles["spaced"]
    );
    assert_eq!(
        serde_json::to_value(with_shell_icon_style(
            &TabIconConfig::default(),
            "",
            Some(style(Some("star"), None))
        ))
        .unwrap(),
        shell_styles["emptyKey"]
    );
    assert_eq!(
        serde_json::to_string(
            &normalize_tab_icon_style(&serde_json::json!({ "icon": "rocket", "color": "#ABCDEF" }))
                .expect("a normalized style")
        )
        .unwrap(),
        icons["serializedStyle"].as_str().expect("style bytes")
    );
    assert_eq!(
        serde_json::to_string(&*DEFAULT_TAB_ICON_CONFIG).unwrap(),
        icons["serializedDefaultConfig"]
            .as_str()
            .expect("default config bytes")
    );
    for case in icons["serializedStyles"]
        .as_array()
        .expect("serialized styles")
    {
        let style = normalize_tab_icon_style(&case["input"]).expect("a normalized style");
        assert_eq!(
            serde_json::to_value(&style).unwrap(),
            case["style"],
            "{case:?}"
        );
        assert_eq!(
            serde_json::to_string(&style).unwrap(),
            case["bytes"].as_str().expect("style bytes"),
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_renumbered_shell_maps_and_local_distro() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];

    let renumbered = &icons["serializedRenumberedShells"];
    let normalized = normalize_tab_icon_config(&renumbered["input"]);
    assert_eq!(
        serde_json::to_value(&normalized).unwrap(),
        renumbered["config"]
    );
    assert_eq!(
        serde_json::to_string(&normalized).unwrap(),
        renumbered["bytes"].as_str().expect("config bytes")
    );

    let shell_styles = &icons["withShellStyles"];
    let with_numeric_key = with_shell_icon_style(
        &config(&[("a", style(Some("box"), None))]),
        "1",
        Some(style(Some("star"), None)),
    );
    assert_eq!(
        serde_json::to_value(&with_numeric_key).unwrap(),
        shell_styles["renumbered"]
    );
    assert_eq!(
        serde_json::to_string(&with_numeric_key).unwrap(),
        shell_styles["renumberedBytes"]
            .as_str()
            .expect("renumbered bytes")
    );

    let local = &icons["localDistro"];
    let mut candidate = Tab::new("a", "WSL", TabType::Terminal);
    candidate.title = "WSL".to_string();
    candidate.is_active = true;
    assert_eq!(serde_json::to_value(&candidate).unwrap(), local["tab"]);
    let next = with_local_distro(&candidate, DistroId::Debian);
    assert_eq!(serde_json::to_value(&next).unwrap(), local["next"]);
    assert_eq!(
        serde_json::to_value(with_local_distro(&next, DistroId::Debian)).unwrap(),
        local["again"]
    );
    assert_eq!(
        with_local_distro(&next, DistroId::Debian) == next,
        local["sameAfter"].as_bool().expect("sameAfter")
    );
    let mut ssh = candidate.clone();
    ssh.tab_type = TabType::Ssh;
    assert_eq!(
        serde_json::to_value(with_local_distro(&ssh, DistroId::Debian)).unwrap(),
        local["ssh"]
    );
}

#[test]
fn oracle_matches_icon_constants_and_prefix_helpers() {
    let oracle = oracle_11d2();
    let icons = &oracle["tabIcons"];
    let constants = &icons["constants"];
    assert_eq!(
        serde_json::to_value(TAB_ICON_IDS).unwrap(),
        constants["ids"]
    );
    assert_eq!(
        serde_json::to_value(AGENT_LOGO_IDS).unwrap(),
        constants["agents"]
    );
    assert_eq!(
        serde_json::to_value(TAB_ICON_COLORS).unwrap(),
        constants["colors"]
    );
    let agent_colors: serde_json::Map<String, serde_json::Value> = AGENT_LOGO_COLORS
        .iter()
        .map(|(agent, color)| ((*agent).to_string(), serde_json::json!(color)))
        .collect();
    assert_eq!(
        serde_json::Value::Object(agent_colors),
        constants["agentColors"]
    );
    assert_eq!(
        serde_json::to_value(&*DEFAULT_TAB_ICON_CONFIG).unwrap(),
        constants["defaultConfig"]
    );
    for case in icons["displayNames"].as_array().expect("display names") {
        assert_eq!(
            shell_display_name(case["key"].as_str().expect("key")),
            case["name"].as_str().expect("name"),
            "{case:?}"
        );
    }
    for case in icons["builtInIcons"].as_array().expect("built-ins") {
        assert_eq!(
            serde_json::to_value(built_in_shell_icon(case["key"].as_str().expect("key"))).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    for case in icons["prefixes"].as_array().expect("prefixes") {
        let value = case["icon"].as_str().expect("icon");
        assert_eq!(is_brand_tab_icon(value), case["isBrand"].as_bool().unwrap());
        assert_eq!(is_agent_tab_icon(value), case["isAgent"].as_bool().unwrap());
        assert_eq!(
            is_distro_tab_icon(value),
            case["isDistro"].as_bool().unwrap()
        );
        assert_eq!(
            is_brand_tab_icon(value).then(|| tab_icon_brand(value)),
            case["brand"].as_str(),
            "{case:?}"
        );
        assert_eq!(
            is_agent_tab_icon(value).then(|| tab_icon_agent(value)),
            case["agent"].as_str(),
            "{case:?}"
        );
        assert_eq!(
            is_distro_tab_icon(value).then(|| tab_icon_distro(value)),
            case["distro"].as_str().map(str::to_string),
            "{case:?}"
        );
    }
    for case in icons["agentIcons"].as_array().expect("agent icons") {
        assert_eq!(
            serde_json::to_value(agent_tab_icon(case["agent"].as_str().expect("agent"))).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    for case in icons["sshIcons"].as_array().expect("ssh icons") {
        assert_eq!(
            serde_json::to_value(ssh_tab_icon(case["distro"].as_str().map(distro_id))).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    for case in icons["inherited"].as_array().expect("inherited") {
        let tab = tab_from_partial(&case["tab"]);
        assert_eq!(
            serde_json::to_value(inherited_tab_icon(&tab, &TabIconConfig::default())).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
    for case in icons["hasIcons"].as_array().expect("has icons") {
        assert_eq!(
            has_tab_icon(&tab_from_partial(&case["tab"])),
            case["has"].as_bool().unwrap(),
            "{case:?}"
        );
    }
    for case in icons["appliedStyles"].as_array().expect("applied styles") {
        let base: ResolvedTabIcon = serde_json::from_value(case["base"].clone()).unwrap();
        let style: Option<TabIconStyle> = serde_json::from_value(case["style"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(apply_tab_icon_style(&base, style.as_ref())).unwrap(),
            case["icon"],
            "{case:?}"
        );
    }
}

fn tab_type(value: &serde_json::Value) -> TabType {
    serde_json::from_value(value.clone()).expect("a tab type")
}

fn icon_config_from_json(value: &serde_json::Value) -> TabIconConfig {
    let mut config = TabIconConfig {
        enabled: value["enabled"].as_bool().unwrap_or(true),
        agents: value["agents"].as_bool().unwrap_or(true),
        shells: IndexMap::new(),
    };
    if let Some(shells) = value["shells"].as_object() {
        for (key, style) in shells {
            config.shells.insert(
                key.clone(),
                serde_json::from_value(style.clone()).expect("a style"),
            );
        }
    }
    config
}

fn distro_id(value: &str) -> DistroId {
    DISTRO_IDS
        .into_iter()
        .find(|distro| distro.as_str() == value)
        .expect("a known distribution")
}

fn tab_from_partial(value: &serde_json::Value) -> Tab {
    let mut tab = Tab::new("a", "a", tab_type(&value["type"]));
    tab.shell_name = value["shellName"].as_str().map(str::to_string);
    tab.distro_id = value["distroId"].as_str().map(str::to_string);
    tab
}
