//! Tab icons and icon settings, from `src/lib/tabIcons.ts`.

use std::cmp::Ordering;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regress::{Flags, Regex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::distro::{DistroId, is_distro_id};
use crate::workspace::pane_templates::js_trim;
use crate::workspace::tab::{Tab, TabType, is_session_tab};

/// The icons a local tab can show, in picker order.
pub const TAB_ICON_IDS: &[&str] = &[
    "terminal",
    "square-terminal",
    "square-chevron-right",
    "chevron-right",
    "code",
    "braces",
    "brackets",
    "hash",
    "binary",
    "file-code",
    "bot",
    "sparkles",
    "git-branch",
    "git-merge",
    "git-commit-horizontal",
    "package",
    "layers",
    "puzzle",
    "wrench",
    "hammer",
    "server",
    "database",
    "cloud",
    "container",
    "box",
    "cpu",
    "hard-drive",
    "network",
    "globe",
    "wifi",
    "monitor",
    "laptop",
    "usb",
    "zap",
    "flame",
    "rocket",
    "bug",
    "shield",
    "lock",
    "key",
    "star",
    "heart",
    "flag",
    "bookmark",
    "bell",
    "eye",
    "target",
    "crown",
    "gem",
    "anchor",
    "compass",
    "fish",
    "leaf",
    "tree-pine",
    "sun",
    "moon",
    "snowflake",
    "coffee",
    "cat",
    "dog",
    "bird",
    "rabbit",
    "turtle",
    "ghost",
    "skull",
    "gamepad-2",
    "music",
    "palette",
];

/// The AI agents with a logo of their own.
pub const AGENT_LOGO_IDS: &[&str] = &["claude", "codex", "opencode", "pi"];

/// The color each agent logo brings; an empty color follows the text color.
pub const AGENT_LOGO_COLORS: &[(&str, &str)] = &[
    ("claude", "#d97757"),
    ("codex", ""),
    ("opencode", ""),
    ("pi", ""),
];

/// Colors offered beside a custom one.
pub const TAB_ICON_COLORS: &[&str] = &[
    "#3b9ae1", "#2bb3a3", "#4caf7a", "#d4a72c", "#e9853a", "#f05133", "#e5534b", "#db61a2",
    "#a371f7", "#8b949e",
];

const BRAND_LOGOS: &[(&str, &str)] = &[
    ("bash", "#4eaa25"),
    ("zsh", "#f15a24"),
    ("nushell", "#4e9a06"),
    ("git", "#f03c2e"),
    ("github", ""),
    ("gitlab", "#fc6d26"),
    ("linux", ""),
    ("apple", ""),
    ("raspberry-pi", "#a22846"),
    ("docker", "#2496ed"),
    ("kubernetes", "#326ce5"),
    ("python", "#3776ab"),
    ("nodejs", "#5fa04e"),
    ("go", "#00add8"),
    ("vim", "#019733"),
    ("tmux", "#1bb91f"),
    ("redis", "#ff4438"),
    ("postgresql", "#4169e1"),
    ("mysql", "#4479a1"),
    ("mongodb", "#47a248"),
    ("nginx", "#009639"),
    ("ansible", "#ee0000"),
    ("terraform", "#844fba"),
    ("google-cloud", "#4285f4"),
    ("cloudflare", "#f38020"),
];

const WINDOWS_SHELLS: &[&str] = &["pwsh", "powershell", "cmd", "git bash", "wsl"];
const UNIX_SHELLS: &[&str] = &["zsh", "bash", "fish", "sh"];

/// An icon and color chosen for a shell or a tab.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabIconStyle {
    /// The chosen icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The chosen color; an empty string means the text color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// An icon as shown, with the color it draws in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedTabIcon {
    /// The icon or logo identifier.
    pub icon: String,
    /// The color; empty means the tab's text color.
    pub color: String,
}

/// The tab icon settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabIconConfig {
    /// Show an icon in front of each terminal tab's title.
    pub enabled: bool,
    /// Show an AI agent's logo while it runs in a tab.
    pub agents: bool,
    /// Choices by shell, keyed by [`shell_key`].
    pub shells: IndexMap<String, TabIconStyle>,
}

impl Default for TabIconConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            agents: true,
            shells: IndexMap::new(),
        }
    }
}

/// The icon settings before the user changes anything.
pub static DEFAULT_TAB_ICON_CONFIG: LazyLock<TabIconConfig> = LazyLock::new(TabIconConfig::default);

fn resolved(icon: &str, color: &str) -> ResolvedTabIcon {
    ResolvedTabIcon {
        icon: icon.to_string(),
        color: color.to_string(),
    }
}

fn plain_icon() -> ResolvedTabIcon {
    resolved("terminal", "")
}

fn powershell_icon() -> ResolvedTabIcon {
    resolved("square-chevron-right", "#3b9ae1")
}

fn unix_icon() -> ResolvedTabIcon {
    resolved("terminal", "#4caf7a")
}

fn server_icon() -> ResolvedTabIcon {
    resolved("server", "")
}

fn serial_icon() -> ResolvedTabIcon {
    resolved("usb", "")
}

fn brand_icon(id: &str) -> ResolvedTabIcon {
    resolved(
        &format!("logo:{id}"),
        brand_logo_color(id).unwrap_or_default(),
    )
}

fn built_in_icon(key: &str) -> ResolvedTabIcon {
    match key {
        "pwsh" | "powershell" => powershell_icon(),
        "cmd" => resolved("square-terminal", ""),
        "git bash" => brand_icon("git"),
        "wsl" => resolved("terminal", "#e9853a"),
        "bash" => brand_icon("bash"),
        "zsh" => brand_icon("zsh"),
        "sh" => unix_icon(),
        "fish" => resolved("fish", "#4caf7a"),
        "nu" => brand_icon("nushell"),
        _ => plain_icon(),
    }
}

/// The key a shell's choices are kept under: its trimmed, lowercased name.
pub fn shell_key(shell_name: Option<&str>) -> String {
    shell_name.map(js_trim).unwrap_or_default().to_lowercase()
}

/// A shell's name for the settings list.
pub fn shell_display_name(key: &str) -> String {
    match key {
        "pwsh" => "PowerShell 7 (pwsh)".to_string(),
        "powershell" => "Windows PowerShell".to_string(),
        "cmd" => "cmd".to_string(),
        "git bash" => "Git Bash".to_string(),
        "wsl" => "WSL".to_string(),
        "nu" => "Nushell".to_string(),
        _ => key.to_string(),
    }
}

/// How a shell looks before any choice of the user's.
pub fn built_in_shell_icon(key: &str) -> ResolvedTabIcon {
    built_in_icon(key)
}

fn canonical_array_index(key: &str) -> Option<u64> {
    if key.is_empty() || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    if !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let index = key.parse::<u64>().ok()?;
    (index < 4_294_967_295).then_some(index)
}

fn js_object_entries(value: &Value) -> Vec<(String, &Value)> {
    match value {
        Value::Object(map) => {
            let mut indexed = Vec::new();
            let mut named = Vec::new();
            for (key, entry) in map {
                match canonical_array_index(key) {
                    Some(index) => indexed.push((index, key.clone(), entry)),
                    None => named.push((key.clone(), entry)),
                }
            }
            indexed.sort_by_key(|(index, _, _)| *index);
            indexed
                .into_iter()
                .map(|(_, key, entry)| (key, entry))
                .chain(named)
                .collect()
        }
        Value::Array(entries) => entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (index.to_string(), entry))
            .collect(),
        _ => Vec::new(),
    }
}

fn js_ordered_shells(shells: IndexMap<String, TabIconStyle>) -> IndexMap<String, TabIconStyle> {
    let mut indexed = Vec::new();
    let mut named = Vec::new();
    for (key, style) in shells {
        match canonical_array_index(&key) {
            Some(index) => indexed.push((index, key, style)),
            None => named.push((key, style)),
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    indexed
        .into_iter()
        .map(|(_, key, style)| (key, style))
        .chain(named)
        .collect()
}

/// The shells the settings list: those of this platform, then the ones found
/// on this machine, then any other with a choice saved.
pub fn shells_for_settings(
    platform: &str,
    config: &TabIconConfig,
    found: &[Option<&str>],
) -> Vec<String> {
    let mut keys: Vec<String> = if platform == "windows" {
        WINDOWS_SHELLS
            .iter()
            .map(|key| (*key).to_string())
            .collect()
    } else {
        UNIX_SHELLS.iter().map(|key| (*key).to_string()).collect()
    };
    for name in found {
        let key = shell_key(*name);
        if !key.is_empty() && !keys.contains(&key) {
            keys.push(key);
        }
    }
    let mut saved: Vec<String> = config
        .shells
        .keys()
        .filter(|key| !keys.contains(key))
        .cloned()
        .collect();
    saved.sort_by(|first, second| compare_utf16(first, second));
    keys.extend(saved);
    keys
}

/// A style laid over another: each choice the top one makes wins.
pub fn apply_tab_icon_style(
    base: &ResolvedTabIcon,
    style: Option<&TabIconStyle>,
) -> ResolvedTabIcon {
    ResolvedTabIcon {
        icon: style
            .and_then(|style| style.icon.clone())
            .unwrap_or_else(|| base.icon.clone()),
        color: style
            .and_then(|style| style.color.clone())
            .unwrap_or_else(|| base.color.clone()),
    }
}

/// What a shell's tabs show without a choice of their own.
pub fn shell_tab_icon(config: &TabIconConfig, shell_name: Option<&str>) -> ResolvedTabIcon {
    let key = shell_key(shell_name);
    apply_tab_icon_style(&built_in_icon(&key), config.shells.get(&key))
}

/// Whether an icon names a distribution's logo.
pub fn is_distro_tab_icon(icon: &str) -> bool {
    icon.starts_with("distro:")
}

/// Whether an icon names a shell's or tool's logo.
pub fn is_brand_tab_icon(icon: &str) -> bool {
    icon.starts_with("logo:")
}

/// The shell or tool whose logo `icon` is.
pub fn tab_icon_brand(icon: &str) -> &str {
    icon.strip_prefix("logo:").unwrap_or_default()
}

/// Whether an icon names an AI agent's logo.
pub fn is_agent_tab_icon(icon: &str) -> bool {
    icon.starts_with("agent:")
}

/// The agent whose logo `icon` is.
pub fn tab_icon_agent(icon: &str) -> &str {
    icon.strip_prefix("agent:").unwrap_or_default()
}

fn is_agent_logo_id(value: &str) -> bool {
    AGENT_LOGO_IDS.contains(&value)
}

/// An agent's logo as a tab shows it, or `None` for an agent without one.
pub fn agent_tab_icon(agent: &str) -> Option<ResolvedTabIcon> {
    let color = AGENT_LOGO_COLORS
        .iter()
        .find_map(|(id, color)| (*id == agent).then_some(*color))?;
    Some(resolved(&format!("agent:{agent}"), color))
}

/// The color a logo brings when picked.
pub fn picked_logo_color(icon: &str) -> Option<String> {
    if is_brand_tab_icon(icon) {
        return Some(
            brand_logo_color(tab_icon_brand(icon))
                .unwrap_or_default()
                .to_string(),
        );
    }
    if is_agent_tab_icon(icon) {
        return AGENT_LOGO_COLORS
            .iter()
            .find_map(|(id, color)| (*id == tab_icon_agent(icon)).then_some((*color).to_string()));
    }
    None
}

/// The distribution whose logo `icon` is.
pub fn tab_icon_distro(icon: &str) -> String {
    icon.strip_prefix("distro:").unwrap_or_default().to_string()
}

/// What an SSH tab shows without a choice of its own.
pub fn ssh_tab_icon(distro_id: Option<&str>) -> ResolvedTabIcon {
    distro_id
        .filter(|distro| !distro.is_empty())
        .map_or_else(server_icon, |distro| {
            resolved(&format!("distro:{distro}"), "")
        })
}

fn local_tab_icon(
    config: &TabIconConfig,
    shell_name: Option<&str>,
    distro_id: Option<&str>,
) -> ResolvedTabIcon {
    let key = shell_key(shell_name);
    let Some(distro) = distro_id.filter(|distro| !distro.is_empty() && key == "wsl") else {
        return shell_tab_icon(config, shell_name);
    };
    apply_tab_icon_style(
        &resolved(&format!("distro:{distro}"), ""),
        config.shells.get(&key),
    )
}

/// A local tab with the distribution its WSL shell reported.
pub fn with_local_distro(tab: &Tab, distro_id: DistroId) -> Tab {
    if tab.tab_type != TabType::Terminal || tab.distro_id.as_deref() == Some(distro_id.as_str()) {
        return tab.clone();
    }
    let mut next = tab.clone();
    next.distro_id = Some(distro_id.as_str().to_string());
    next
}

/// Whether a tab's icon carries the local badge.
pub fn has_local_badge(
    tab_type: TabType,
    shell_name: Option<&str>,
    icon: &ResolvedTabIcon,
) -> bool {
    tab_type == TabType::Terminal
        && shell_key(shell_name) == "wsl"
        && is_distro_tab_icon(&icon.icon)
}

/// Whether a tab shows an icon.
pub fn has_tab_icon(tab: &Tab) -> bool {
    is_session_tab(tab)
}

/// What a terminal tab shows without a choice of its own.
pub fn inherited_tab_icon(tab: &Tab, config: &TabIconConfig) -> ResolvedTabIcon {
    match tab.tab_type {
        TabType::Ssh => ssh_tab_icon(tab.distro_id.as_deref()),
        TabType::Serial => serial_icon(),
        _ => local_tab_icon(config, tab.shell_name.as_deref(), tab.distro_id.as_deref()),
    }
}

/// The icon a terminal tab shows, or `None` for other tabs and while icons are
/// off.
pub fn resolve_tab_icon(
    tab: &Tab,
    config: &TabIconConfig,
    agent: Option<&str>,
) -> Option<ResolvedTabIcon> {
    if !has_tab_icon(tab) || !config.enabled {
        return None;
    }
    let agent_icon = agent
        .filter(|agent| !agent.is_empty())
        .filter(|_| config.agents)
        .and_then(agent_tab_icon);
    let base = agent_icon.unwrap_or_else(|| inherited_tab_icon(tab, config));
    Some(apply_tab_icon_style(&base, tab.icon_style.as_ref()))
}

fn is_tab_icon_choice(value: &str) -> bool {
    if let Some(distro) = value.strip_prefix("distro:") {
        return is_distro_id(distro);
    }
    if let Some(brand) = value.strip_prefix("logo:") {
        return brand_logo_color(brand).is_some();
    }
    if let Some(agent) = value.strip_prefix("agent:") {
        return is_agent_logo_id(agent);
    }
    TAB_ICON_IDS.contains(&value)
}

static HEX_COLOR: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::with_flags(
        "^#[0-9a-f]{6}$",
        Flags {
            icase: true,
            ..Flags::default()
        },
    )
    .ok()
});

fn is_hex_color(value: &str) -> bool {
    HEX_COLOR
        .as_ref()
        .is_some_and(|regex| regex.find(value).is_some())
}

/// A saved style with unknown icons and malformed colors left out.
pub fn normalize_tab_icon_style(value: &Value) -> Option<TabIconStyle> {
    let Value::Object(map) = value else {
        return None;
    };
    normalize_style_fields(
        map.get("icon").and_then(Value::as_str),
        map.get("color").and_then(Value::as_str),
    )
}

fn normalize_style_fields(icon: Option<&str>, color: Option<&str>) -> Option<TabIconStyle> {
    let mut style = TabIconStyle::default();
    if let Some(icon) = icon.filter(|icon| is_tab_icon_choice(icon)) {
        style.icon = Some(icon.to_string());
    }
    if let Some(color) = color.filter(|color| color.is_empty() || is_hex_color(color)) {
        style.color = Some(color.to_lowercase());
    }
    if style.icon.is_none() && style.color.is_none() {
        None
    } else {
        Some(style)
    }
}

/// A saved SSH tab with its own icon and its host's distribution checked.
pub fn normalize_saved_ssh_tab(tab: &Tab) -> Tab {
    if tab.tab_type != TabType::Ssh {
        return tab.clone();
    }
    normalize_saved_icon_fields(tab)
}

/// Checks only the saved icon and distribution, without changing unrelated fields.
pub(crate) fn normalize_saved_icon_fields(tab: &Tab) -> Tab {
    let mut saved = tab.clone();
    let raw_style = tab.saved_field("iconStyle").cloned().or_else(|| {
        tab.icon_style
            .as_ref()
            .and_then(|style| serde_json::to_value(style).ok())
    });
    saved.icon_style = raw_style.as_ref().and_then(normalize_tab_icon_style);
    if let Some(style) = &saved.icon_style {
        if let Ok(value) = serde_json::to_value(style) {
            saved.raw_fields.insert("iconStyle".to_string(), value);
        }
    } else {
        saved.forget_saved_field("iconStyle");
    }
    saved.distro_id = tab
        .distro_id
        .as_deref()
        .filter(|value| is_distro_id(value))
        .map(str::to_owned);
    if saved.distro_id.is_none() {
        saved.forget_saved_field("distroId");
    }
    saved
}

/// The tab icon settings from a saved config, each choice valid.
pub fn normalize_tab_icon_config(value: &Value) -> TabIconConfig {
    let Value::Object(map) = value else {
        return TabIconConfig::default();
    };
    let mut shells = IndexMap::new();
    if let Some(raw_shells) = map.get("shells") {
        for (key, style) in js_object_entries(raw_shells) {
            let key = shell_key(Some(&key));
            if key.is_empty() {
                continue;
            }
            if let Some(valid) = normalize_tab_icon_style(style) {
                shells.insert(key, valid);
            }
        }
    }
    TabIconConfig {
        enabled: map.get("enabled").and_then(Value::as_bool) != Some(false),
        agents: map.get("agents").and_then(Value::as_bool) != Some(false),
        shells: js_ordered_shells(shells),
    }
}

/// The config with one shell's choice replaced; `None` forgets it.
pub fn with_shell_icon_style(
    config: &TabIconConfig,
    key: &str,
    style: Option<TabIconStyle>,
) -> TabIconConfig {
    let mut shells = config.shells.clone();
    match style
        .as_ref()
        .and_then(|style| normalize_style_fields(style.icon.as_deref(), style.color.as_deref()))
    {
        Some(valid) => {
            shells.insert(key.to_string(), valid);
        }
        None => {
            shells.shift_remove(key);
        }
    }
    TabIconConfig {
        enabled: config.enabled,
        agents: config.agents,
        shells: js_ordered_shells(shells),
    }
}

fn brand_logo_color(id: &str) -> Option<&'static str> {
    BRAND_LOGOS
        .iter()
        .find_map(|(brand, color)| (*brand == id).then_some(*color))
}

fn compare_utf16(first: &str, second: &str) -> Ordering {
    first.encode_utf16().cmp(second.encode_utf16())
}
