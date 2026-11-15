//! The languages the shared catalogue ships in, and their plural rules.

use std::fmt;

use sys_locale::get_locale;

/// A language both clients' translations are available in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {
    /// English.
    En,
    /// Chinese.
    Zh,
}

/// A CLDR plural category, named the way i18next suffixes a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

const EN_PLURAL_CATEGORIES: [PluralCategory; 2] = [PluralCategory::One, PluralCategory::Other];
const ZH_PLURAL_CATEGORIES: [PluralCategory; 1] = [PluralCategory::Other];

impl Language {
    /// Every language the catalogue ships.
    pub const ALL: [Self; 2] = [Self::En, Self::Zh];

    /// The language code stored in the configuration and in i18next.
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    /// Maps a locale such as `zh-CN`, `zh_Hans` or `en-US` to a supported
    /// language. This keeps the first-launch rule the configuration defaults
    /// used before the catalogue moved into core.
    pub fn from_locale(locale: &str) -> Self {
        let normalized_locale = locale.replace('_', "-").to_ascii_lowercase();

        if normalized_locale.starts_with("zh") {
            Self::Zh
        } else {
            Self::En
        }
    }

    /// The language the operating system reports, or English when it cannot
    /// be read.
    pub fn system() -> Self {
        get_locale()
            .map(|locale| Self::from_locale(&locale))
            .unwrap_or(Self::En)
    }

    /// The plural category i18next selects for `count` in this language.
    pub(crate) fn plural_category(self, count: f64) -> PluralCategory {
        match self {
            Self::En => {
                // CLDR treats both 1 and -1 as singular, while decimals such
                // as 1.5 use the other form.
                if count.abs() == 1.0 {
                    PluralCategory::One
                } else {
                    PluralCategory::Other
                }
            }
            Self::Zh => PluralCategory::Other,
        }
    }

    /// The plural forms a complete catalogue needs for this language. Extra
    /// forms, such as Chinese `_one` entries kept for review, are allowed.
    pub(crate) const fn required_plural_categories(self) -> &'static [PluralCategory] {
        match self {
            Self::En => &EN_PLURAL_CATEGORIES,
            Self::Zh => &ZH_PLURAL_CATEGORIES,
        }
    }
}

impl PluralCategory {
    /// The i18next key suffix for this category.
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Zero => "_zero",
            Self::One => "_one",
            Self::Two => "_two",
            Self::Few => "_few",
            Self::Many => "_many",
            Self::Other => "_other",
        }
    }

    /// Reads an i18next plural suffix without its leading underscore.
    pub fn from_suffix(suffix: &str) -> Option<Self> {
        match suffix {
            "zero" => Some(Self::Zero),
            "one" => Some(Self::One),
            "two" => Some(Self::Two),
            "few" => Some(Self::Few),
            "many" => Some(Self::Many),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl fmt::Display for PluralCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.suffix())
    }
}
