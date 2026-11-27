//! The transient theme-library browser behind the Appearance page.
//!
//! The browser owns only what the user is looking at: the catalog, the search
//! text, the light/dark filter and the highlighted entry. Nothing in this
//! module writes the configuration; applying a highlighted entry goes through
//! the shared `ConfigState` save path in [`super::ThemeSetting`].

use gpui_kit::component::input::InputState;
use gpui_kit::{Entity, FocusHandle, ScrollHandle, Subscription};
use tterm_core::i18n::keys;
use tterm_core::i18n::translate::TranslationKey;
use tterm_core::theme::catalog::load_theme_catalog;
use tterm_core::theme::types::CatalogTheme;

/// The library panel and its controls.
pub(crate) const LIBRARY_ID: &str = "settings-theme-library";
/// The button that opens the library from the preset list.
pub(crate) const LIBRARY_OPEN_ID: &str = "settings-theme-library-open";
/// The name search field.
pub(crate) const LIBRARY_SEARCH_ID: &str = "settings-theme-library-search";
/// The clear button inside the search field.
pub(crate) const LIBRARY_SEARCH_CLEAR_ID: &str = "settings-theme-library-search-clear";
/// The all/dark/light filter buttons.
pub(crate) const LIBRARY_ALL_ID: &str = "settings-theme-library-all";
pub(crate) const LIBRARY_DARK_ID: &str = "settings-theme-library-dark";
pub(crate) const LIBRARY_LIGHT_ID: &str = "settings-theme-library-light";
/// The scrollable result list.
pub(crate) const LIBRARY_LIST_ID: &str = "settings-theme-library-list";
/// The selected entry's detail panel.
pub(crate) const LIBRARY_DETAIL_ID: &str = "settings-theme-library-detail";
/// The result count above the list.
pub(crate) const LIBRARY_COUNT_ID: &str = "settings-theme-library-count";
/// The visible load failure, empty result and save failure lines.
pub(crate) const LIBRARY_LOAD_ERROR_ID: &str = "settings-theme-library-load-error";
pub(crate) const LIBRARY_EMPTY_ID: &str = "settings-theme-library-empty";
pub(crate) const LIBRARY_ERROR_ID: &str = "settings-theme-library-error";
/// The busy state and footer actions.
pub(crate) const LIBRARY_APPLY_ID: &str = "settings-theme-library-apply";
pub(crate) const LIBRARY_CANCEL_ID: &str = "settings-theme-library-cancel";
pub(crate) const LIBRARY_RETRY_ID: &str = "settings-theme-library-retry";
/// Each catalog row, followed by the catalog id.
pub(crate) const LIBRARY_ROW_PREFIX: &str = "settings-theme-library-row-";

/// The debug selector of one catalog row.
pub(crate) fn row_id(theme_id: &str) -> String {
    format!("{LIBRARY_ROW_PREFIX}{theme_id}")
}

/// Which entries the browser shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ThemeFilter {
    /// Every bundled library theme.
    #[default]
    All,
    /// Themes whose background is darker than their foreground.
    Dark,
    /// Themes whose background is lighter than their foreground.
    Light,
}

impl ThemeFilter {
    /// The translateable label of the filter button.
    pub(crate) fn label_key(self) -> TranslationKey {
        match self {
            Self::All => keys::THEME_GALLERY_ALL,
            Self::Dark => keys::THEME_GALLERY_DARK,
            Self::Light => keys::THEME_GALLERY_LIGHT,
        }
    }

    /// The button selector that selects this filter.
    pub(crate) fn selector(self) -> &'static str {
        match self {
            Self::All => LIBRARY_ALL_ID,
            Self::Dark => LIBRARY_DARK_ID,
            Self::Light => LIBRARY_LIGHT_ID,
        }
    }

    /// Whether `theme` belongs in this filter.
    pub(crate) fn matches(self, theme: &CatalogTheme) -> bool {
        match self {
            Self::All => true,
            Self::Dark => theme.is_dark(),
            Self::Light => !theme.is_dark(),
        }
    }
}

/// The catalog entries that match a query and filter.
///
/// Every word of the query must occur in the name, case-insensitively, like
/// the reference theme gallery.
pub(crate) fn visible_themes<'a>(
    catalog: &'a [CatalogTheme],
    query: &str,
    filter: ThemeFilter,
) -> Vec<&'a CatalogTheme> {
    let words: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect();
    catalog
        .iter()
        .filter(|theme| filter.matches(theme))
        .filter(|theme| {
            let name = theme.name.to_lowercase();
            words.iter().all(|word| name.contains(word))
        })
        .collect()
}

/// The browser state. It is rebuilt for every opening and survives only while
/// the Appearance page keeps the library open.
pub(crate) struct ThemeLibrary {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) filter: ThemeFilter,
    /// The highlighted catalog id, always one of [`Self::catalog`] when set.
    pub(crate) selected: Option<String>,
    pub(crate) catalog: Option<&'static [CatalogTheme]>,
    pub(crate) load_failed: bool,
    pub(crate) search: Option<Entity<InputState>>,
    pub(crate) focus: FocusHandle,
    pub(crate) scroll: ScrollHandle,
    pub(crate) _search_events: Option<Subscription>,
    pub(crate) _search_observer: Option<Subscription>,
}

impl ThemeLibrary {
    /// Creates the closed browser with its focus and scroll handles.
    pub(crate) fn new(focus: FocusHandle) -> Self {
        Self {
            open: false,
            query: String::new(),
            filter: ThemeFilter::All,
            selected: None,
            catalog: None,
            load_failed: false,
            search: None,
            focus,
            scroll: ScrollHandle::new(),
            _search_events: None,
            _search_observer: None,
        }
    }

    /// Starts a browsing session, highlighting the effective theme when it is
    /// a library entry.
    pub(crate) fn open(&mut self, effective_id: Option<&str>) {
        self.open = true;
        self.query.clear();
        self.filter = ThemeFilter::All;
        self.load_failed = false;
        self.catalog = None;
        self.selected = None;
        match load_theme_catalog() {
            Ok(catalog) => {
                self.catalog = Some(catalog);
                self.selected = effective_id
                    .filter(|id| catalog.iter().any(|theme| theme.id == *id))
                    .map(str::to_string);
            }
            Err(error) => {
                self.load_failed = true;
                eprintln!("tterm-gpui: failed to load the theme library: {error}");
            }
        }
        self.scroll.set_offset(Default::default());
        self.reveal_selected();
    }

    /// Ends a browsing session and drops the transient state.
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.filter = ThemeFilter::All;
        self.selected = None;
        self.load_failed = false;
        self.search = None;
        self._search_events = None;
        self._search_observer = None;
        self.scroll.set_offset(Default::default());
    }

    /// The entries the current query and filter show.
    pub(crate) fn visible(&self) -> Vec<&'static CatalogTheme> {
        self.catalog.map_or_else(Vec::new, |catalog| {
            visible_themes(catalog, &self.query, self.filter)
        })
    }

    /// The highlighted entry, if any.
    pub(crate) fn selected_theme(&self) -> Option<&'static CatalogTheme> {
        let id = self.selected.as_deref()?;
        self.catalog?.iter().find(|theme| theme.id == id)
    }

    /// Selects a catalog id and scrolls its row into view.
    pub(crate) fn select(&mut self, theme_id: &str) {
        if !self
            .catalog
            .is_some_and(|catalog| catalog.iter().any(|theme| theme.id == theme_id))
        {
            return;
        }
        self.selected = Some(theme_id.to_string());
        self.reveal_selected();
    }

    /// Clears the highlight when it no longer matches the query or filter.
    pub(crate) fn settle_selection(&mut self) -> bool {
        let Some(selected) = self.selected.clone() else {
            return false;
        };
        if self.visible().iter().any(|theme| theme.id == selected) {
            return false;
        }
        self.selected = None;
        true
    }

    /// Moves the highlight through the visible entries. `delta` is in rows;
    /// the first move from no selection enters at the matching end.
    pub(crate) fn step(&mut self, delta: isize) -> bool {
        let visible = self.visible();
        if visible.is_empty() {
            return false;
        }
        let last = visible.len() - 1;
        let index = self
            .selected
            .as_deref()
            .and_then(|id| visible.iter().position(|theme| theme.id == id));
        let target = match index {
            Some(index) => index.saturating_add_signed(delta).min(last),
            None if delta < 0 => last,
            None => 0,
        };
        let id = visible[target].id.clone();
        self.select(&id);
        true
    }

    /// Moves to the first or last visible entry.
    pub(crate) fn jump(&mut self, to_last: bool) -> bool {
        let visible = self.visible();
        let theme = if to_last {
            visible.last()
        } else {
            visible.first()
        };
        let Some(theme) = theme else {
            return false;
        };
        let id = theme.id.clone();
        self.select(&id);
        true
    }

    /// Scrolls the highlighted row into view when the list is already laid
    /// out. The first frame lays every row out, so this is safe before that.
    pub(crate) fn reveal_selected(&self) {
        let Some(id) = self.selected.as_deref() else {
            return;
        };
        if let Some(index) = self.visible().iter().position(|theme| theme.id == id) {
            self.scroll.scroll_to_item(index);
        }
    }

    /// The translated result count.
    pub(crate) fn count_text(&self, locale: &crate::locale::Locale) -> String {
        count_text(locale, self.visible().len())
    }
}

/// Builds the pluralized result count with the shared catalogue.
pub(crate) fn count_text(locale: &crate::locale::Locale, count: usize) -> String {
    let request = tterm_core::i18n::translate::TranslationRequest::new(keys::THEME_GALLERY_COUNT)
        .count(count as f64);
    locale.translate(request).unwrap_or_else(|error| {
        eprintln!("tterm-gpui: failed to translate the theme library count: {error}");
        count.to_string()
    })
}
