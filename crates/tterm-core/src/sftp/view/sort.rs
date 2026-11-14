//! Sorting and filtering the SFTP file table, from
//! `src/components/SftpDrawer/sftpView.ts`.

use std::cmp::Ordering;

use serde_json::Value;

use crate::js::trim_js;
use crate::sftp::entry::SftpDirectoryEntry;

/// Columns of the file table, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpSortColumn {
    /// The file name.
    Name,
    /// The modification time.
    Modified,
    /// The size.
    Size,
    /// Whether the entry is a link, then its extension.
    Kind,
    /// The permission string.
    Permissions,
    /// The owner, then the group.
    Owner,
}

/// The columns in the order the table shows them.
pub const SFTP_SORT_COLUMNS: [SftpSortColumn; 6] = [
    SftpSortColumn::Name,
    SftpSortColumn::Modified,
    SftpSortColumn::Size,
    SftpSortColumn::Kind,
    SftpSortColumn::Permissions,
    SftpSortColumn::Owner,
];

impl SftpSortColumn {
    /// The name the preferences store.
    pub fn as_str(self) -> &'static str {
        match self {
            SftpSortColumn::Name => "name",
            SftpSortColumn::Modified => "modified",
            SftpSortColumn::Size => "size",
            SftpSortColumn::Kind => "kind",
            SftpSortColumn::Permissions => "permissions",
            SftpSortColumn::Owner => "owner",
        }
    }

    /// The column a stored name names, or `None` for anything unknown.
    pub fn parse(value: &str) -> Option<SftpSortColumn> {
        SFTP_SORT_COLUMNS
            .into_iter()
            .find(|column| column.as_str() == value)
    }
}

/// Whether a column sorts from its smallest value or its largest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpSortDirection {
    /// From the smallest value.
    Asc,
    /// From the largest value.
    Desc,
}

/// One column and its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SftpSort {
    /// The column to sort by.
    pub column: SftpSortColumn,
    /// The direction to sort in.
    pub direction: SftpSortDirection,
}

/// How the listing is presented; remembered across sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SftpViewPreferences {
    /// The sort column and direction.
    pub sort: SftpSort,
    /// Whether dotfiles are listed.
    pub show_hidden: bool,
}

/// The view a drawer starts with when nothing valid was saved.
pub const DEFAULT_SFTP_VIEW_PREFERENCES: SftpViewPreferences = SftpViewPreferences {
    sort: SftpSort {
        column: SftpSortColumn::Name,
        direction: SftpSortDirection::Asc,
    },
    show_hidden: true,
};

/// Whether the entry is a dotfile.
pub fn is_hidden_entry(entry: &SftpDirectoryEntry) -> bool {
    entry.name.starts_with('.')
}

/// Clicking the sorted column flips its direction; another column starts
/// ascending.
pub fn next_sftp_sort(current: SftpSort, column: SftpSortColumn) -> SftpSort {
    if current.column == column {
        SftpSort {
            column,
            direction: match current.direction {
                SftpSortDirection::Asc => SftpSortDirection::Desc,
                SftpSortDirection::Desc => SftpSortDirection::Asc,
            },
        }
    } else {
        SftpSort {
            column,
            direction: SftpSortDirection::Asc,
        }
    }
}

/// File extension, lowercased; empty for folders, dotfiles and names without
/// one.
fn extension_of(entry: &SftpDirectoryEntry) -> String {
    if entry.is_dir {
        return String::new();
    }
    match entry.name.rfind('.') {
        Some(dot) if dot > 0 => entry.name[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// Compares text the way the table's `localeCompare` calls do for ASCII:
/// case-insensitively first, then by the exact text.
fn locale_compare(left: &str, right: &str) -> Ordering {
    let primary = left
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .cmp(
            right
                .chars()
                .map(|character| character.to_ascii_lowercase()),
        );
    primary.then_with(|| left.cmp(right))
}

/// The numeric-aware, case-insensitive name order the table uses.
///
/// `Intl.Collator` compares whole numbers in a name by value, so `file9`
/// sorts before `file10`. Non-ASCII letters fall back to their code point
/// order instead of the locale's, which keeps the port free of an ICU
/// dependency; names that differ only there may sort differently than in the
/// WebView.
fn compare_names(left: &str, right: &str) -> Ordering {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let (mut at_left, mut at_right) = (0, 0);

    while at_left < left.len() && at_right < right.len() {
        if left[at_left].is_ascii_digit() && right[at_right].is_ascii_digit() {
            let start_left = at_left;
            while at_left < left.len() && left[at_left].is_ascii_digit() {
                at_left += 1;
            }
            let start_right = at_right;
            while at_right < right.len() && right[at_right].is_ascii_digit() {
                at_right += 1;
            }
            let digits_left: String = left[start_left..at_left].iter().collect();
            let digits_right: String = right[start_right..at_right].iter().collect();
            let value_left = digits_left.trim_start_matches('0');
            let value_right = digits_right.trim_start_matches('0');
            let order = value_left
                .len()
                .cmp(&value_right.len())
                .then_with(|| value_left.cmp(value_right));
            if order != Ordering::Equal {
                return order;
            }
            continue;
        }

        let left_character = left[at_left].to_ascii_lowercase();
        let right_character = right[at_right].to_ascii_lowercase();
        if left_character != right_character {
            return left_character.cmp(&right_character);
        }
        at_left += 1;
        at_right += 1;
    }
    left.len().cmp(&right.len())
}

fn compare_by_column(
    left: &SftpDirectoryEntry,
    right: &SftpDirectoryEntry,
    column: SftpSortColumn,
) -> Ordering {
    match column {
        SftpSortColumn::Name => Ordering::Equal,
        SftpSortColumn::Modified => left
            .modified_at
            .unwrap_or(0)
            .cmp(&right.modified_at.unwrap_or(0)),
        SftpSortColumn::Size => left.size.unwrap_or(0).cmp(&right.size.unwrap_or(0)),
        SftpSortColumn::Kind => left
            .is_symlink
            .cmp(&right.is_symlink)
            .then_with(|| locale_compare(&extension_of(left), &extension_of(right))),
        SftpSortColumn::Permissions => locale_compare(
            left.permissions.as_deref().unwrap_or(""),
            right.permissions.as_deref().unwrap_or(""),
        ),
        SftpSortColumn::Owner => locale_compare(
            left.owner.as_deref().unwrap_or(""),
            right.owner.as_deref().unwrap_or(""),
        )
        .then_with(|| {
            locale_compare(
                left.group.as_deref().unwrap_or(""),
                right.group.as_deref().unwrap_or(""),
            )
        }),
    }
}

/// A sorted copy of `entries`.
///
/// Folders stay ahead of files whatever the direction, and equal values fall
/// back to the name so the order is stable.
pub fn sort_sftp_entries(
    entries: &[SftpDirectoryEntry],
    sort: SftpSort,
) -> Vec<SftpDirectoryEntry> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|left, right| {
        if left.is_dir != right.is_dir {
            return if left.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let column = compare_by_column(left, right, sort.column);
        let column = match sort.direction {
            SftpSortDirection::Asc => column,
            SftpSortDirection::Desc => column.reverse(),
        };
        let names = compare_names(&left.name, &right.name);
        let names = match sort.direction {
            SftpSortDirection::Asc => names,
            SftpSortDirection::Desc => names.reverse(),
        };
        column.then(names)
    });
    sorted
}

/// The entries to show for `preferences`, in display order.
pub fn apply_sftp_view(
    entries: &[SftpDirectoryEntry],
    preferences: SftpViewPreferences,
) -> Vec<SftpDirectoryEntry> {
    let visible: Vec<SftpDirectoryEntry> = if preferences.show_hidden {
        entries.to_vec()
    } else {
        entries
            .iter()
            .filter(|entry| !is_hidden_entry(entry))
            .cloned()
            .collect()
    };
    sort_sftp_entries(&visible, preferences.sort)
}

/// Validates the stored preferences text, falling back to the defaults for
/// anything malformed.
pub fn normalize_sftp_view_preferences(raw: Option<&str>) -> SftpViewPreferences {
    let Some(raw) = raw else {
        return DEFAULT_SFTP_VIEW_PREFERENCES;
    };
    let Ok(Value::Object(stored)) = serde_json::from_str::<Value>(raw) else {
        return DEFAULT_SFTP_VIEW_PREFERENCES;
    };

    let sort = stored
        .get("sort")
        .and_then(Value::as_object)
        .and_then(|sort| {
            let column = sort
                .get("column")
                .and_then(Value::as_str)
                .and_then(SftpSortColumn::parse)?;
            let direction = match sort.get("direction").and_then(Value::as_str) {
                Some("asc") => SftpSortDirection::Asc,
                Some("desc") => SftpSortDirection::Desc,
                _ => return None,
            };
            Some(SftpSort { column, direction })
        })
        .unwrap_or(DEFAULT_SFTP_VIEW_PREFERENCES.sort);
    let show_hidden = stored
        .get("showHidden")
        .and_then(Value::as_bool)
        .unwrap_or(DEFAULT_SFTP_VIEW_PREFERENCES.show_hidden);

    SftpViewPreferences { sort, show_hidden }
}

/// One scope of a mode's nine rwx bits, as the permissions dialog shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionBits {
    /// The scope's name, `owner`, `group` or `others`.
    pub scope: &'static str,
    /// The read bit.
    pub read: u32,
    /// The write bit.
    pub write: u32,
    /// The execute bit.
    pub execute: u32,
}

/// The nine rwx bits of a mode, in the order the permissions dialog shows
/// them.
pub const PERMISSION_BITS: [PermissionBits; 3] = [
    PermissionBits {
        scope: "owner",
        read: 0o400,
        write: 0o200,
        execute: 0o100,
    },
    PermissionBits {
        scope: "group",
        read: 0o040,
        write: 0o020,
        execute: 0o010,
    },
    PermissionBits {
        scope: "others",
        read: 0o004,
        write: 0o002,
        execute: 0o001,
    },
];

/// Three or four octal digits, e.g. `644` or `2755`.
pub fn parse_octal_mode(value: &str) -> Option<u32> {
    let trimmed = trim_js(value);
    if trimmed.len() < 3 || trimmed.len() > 4 || !trimmed.chars().all(|c| ('0'..='7').contains(&c))
    {
        return None;
    }
    u32::from_str_radix(trimmed, 8).ok()
}

/// The mode as the table's three or four octal digits.
pub fn format_octal_mode(mode: u32) -> String {
    format!("{:03o}", mode & 0o7777)
}
