//! Tab ordering for next, previous and nth-tab shortcuts, from
//! `src/lib/tabNavigation.ts`.

use std::cmp::Ordering;

/// Returns the tab id `offset` positions away from `active_tab_id` in display
/// order, wrapping around both ends.
pub fn get_sibling_tab_id(
    tab_ids: &[String],
    active_tab_id: Option<&str>,
    offset: i64,
) -> Option<String> {
    let active_tab_id = active_tab_id.filter(|id| !id.is_empty())?;
    if tab_ids.is_empty() {
        return None;
    }
    let index = tab_ids.iter().position(|id| id == active_tab_id)?;
    let next = (index as i128 + offset as i128 + tab_ids.len() as i128) % tab_ids.len() as i128;
    if next < 0 {
        return None;
    }
    tab_ids.get(next as usize).cloned()
}

const LAST_TAB_POSITION: i64 = 9;

/// Returns the tab id at a 1-based position, or `None` when out of range.
/// Position 9 is the last tab, however many tabs there are.
pub fn get_tab_id_at_position(tab_ids: &[String], position: i64) -> Option<String> {
    if position == LAST_TAB_POSITION {
        return tab_ids.last().cloned();
    }
    if position < 1 || position > tab_ids.len() as i64 {
        return None;
    }
    tab_ids.get((position - 1) as usize).cloned()
}

/// What a tab shortcut leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabShortcutTarget {
    /// Switch to the tab.
    Switch(String),
    /// The shortcut names the tab already in front.
    Current(String),
    /// The shortcut found no tab.
    None,
}

/// Classifies the tab a shortcut resolved to.
pub fn resolve_tab_shortcut_target(
    tab_id: Option<&str>,
    active_tab_id: Option<&str>,
) -> TabShortcutTarget {
    let Some(tab_id) = tab_id.filter(|id| !id.is_empty()) else {
        return TabShortcutTarget::None;
    };
    if active_tab_id == Some(tab_id) {
        TabShortcutTarget::Current(tab_id.to_owned())
    } else {
        TabShortcutTarget::Switch(tab_id.to_owned())
    }
}

fn tab_number(id: &str) -> Option<f64> {
    let digits = id.strip_prefix("tab-")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<f64>().ok()
}

/// Orders tab ids as the tabs were opened. Numbered `tab-<n>` ids sort by
/// number; other ids sort by text.
pub fn compare_tab_ids(first: &str, second: &str) -> f64 {
    let first_number = tab_number(first);
    let second_number = tab_number(second);
    let difference = match (first_number, second_number) {
        (Some(first), Some(second)) => first - second,
        _ => f64::NAN,
    };
    if difference.is_nan() {
        match first.cmp(second) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        }
    } else {
        difference
    }
}
