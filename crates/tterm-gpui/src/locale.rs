//! The GPUI client's shared language state.
//!
//! One [`Locale`] entity lives in GPUI's global state and every view observes
//! it. A language change notifies those observers, so labels read from the
//! catalogue update without rebuilding the window.

use std::fmt;

use gpui_kit::{App, Context, Entity, Global};
use tterm_core::i18n::catalogue::{Catalogue, CatalogueError};
use tterm_core::i18n::language::Language;
use tterm_core::i18n::translate::{TranslationError, TranslationKey, TranslationRequest};

/// The language and catalogue shared by every GPUI view.
pub struct Locale {
    language: Language,
    catalogue: Result<&'static Catalogue, CatalogueError>,
}

/// The single [`Locale`] entity stored in GPUI's global state.
pub struct LocaleGlobal(Entity<Locale>);

impl Global for LocaleGlobal {}

/// A catalogue or interpolation failure from a locale lookup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocaleError {
    /// The embedded locale files did not form a valid catalogue.
    Catalogue(CatalogueError),
    /// The requested key or its interpolation arguments are missing.
    Translation(TranslationError),
}

impl Locale {
    /// Loads the embedded catalogue and starts in `language`.
    pub fn new(language: Language) -> Self {
        Self {
            language,
            catalogue: Catalogue::embedded(),
        }
    }

    /// The language this entity currently renders.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Switches language and notifies every observing view when it changed.
    pub fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.language == language {
            return;
        }
        self.language = language;
        cx.notify();
    }

    /// Translates one request in the current language.
    pub fn translate(&self, request: TranslationRequest<'_>) -> Result<String, LocaleError> {
        let catalogue = self
            .catalogue
            .as_ref()
            .map_err(|error| LocaleError::Catalogue(error.clone()))?;
        catalogue
            .translate(self.language, request)
            .map_err(LocaleError::Translation)
    }

    /// Translates a key without plural or interpolation arguments.
    pub fn text(&self, key: TranslationKey) -> Result<String, LocaleError> {
        self.translate(TranslationRequest::new(key))
    }
}

impl LocaleGlobal {
    /// Replaces the global locale entity. Startup installs exactly one.
    pub fn set(cx: &mut App, locale: Entity<Locale>) {
        cx.set_global(Self(locale));
    }

    /// Returns the global locale entity when startup installed one.
    pub fn get(cx: &App) -> Option<Entity<Locale>> {
        cx.try_global::<Self>().map(|global| global.0.clone())
    }
}

impl fmt::Display for LocaleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalogue(error) => write!(formatter, "{error}"),
            Self::Translation(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LocaleError {}

#[cfg(test)]
mod tests;
