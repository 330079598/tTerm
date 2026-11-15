//! The embedded translation catalogue: flattening, semantic parity checks and
//! lookups.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use serde_json::Value;

use super::language::{Language, PluralCategory};
use super::translate::{
    Segment, TranslationError, TranslationKey, TranslationRequest, format_template, parse_segments,
};

pub(crate) const EN_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/i18n/locales/en.json"
));
pub(crate) const ZH_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../src/i18n/locales/zh.json"
));

static EMBEDDED: OnceLock<Result<Catalogue, CatalogueError>> = OnceLock::new();

/// Both language catalogues, keyed by their flattened dotted paths.
#[derive(Debug)]
pub struct Catalogue {
    messages: BTreeMap<Language, BTreeMap<String, String>>,
    plural_bases: BTreeSet<String>,
}

/// Why the locale files could not form a usable catalogue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogueError {
    /// A locale file is not valid JSON, or its root is not an object.
    InvalidJson { language: Language, message: String },
    /// A locale value is neither a message nor a nested object.
    NonStringValue { language: Language, key: String },
    /// A message has malformed `{{name}}` interpolation syntax.
    InvalidTemplate {
        language: Language,
        key: String,
        reason: &'static str,
    },
    /// The two languages do not describe the same semantic keys. Plural
    /// suffixes are compared by their base key, so Chinese may omit `_one`
    /// where English needs it.
    SemanticKeysDiffer { missing_from: Language, key: String },
    /// A language is missing a plural form its CLDR rules require.
    MissingPluralForm {
        language: Language,
        base: String,
        suffix: &'static str,
    },
    /// A named placeholder used by one language is absent from the other.
    /// `count` is exempt because a translation may describe the plural
    /// without printing the number.
    PlaceholderMismatch {
        base: String,
        language: Language,
        expected: Vec<String>,
        found: Vec<String>,
    },
}

impl Catalogue {
    /// Loads and validates the locale files embedded at build time. The JSON
    /// stays under `src/i18n/locales/`; it is not copied into this crate.
    pub fn embedded() -> Result<&'static Catalogue, CatalogueError> {
        match EMBEDDED.get_or_init(|| Self::from_sources(EN_JSON, ZH_JSON)) {
            Ok(catalogue) => Ok(catalogue),
            Err(error) => Err(error.clone()),
        }
    }

    /// Builds a catalogue from the two locale documents.
    pub fn from_sources(english: &str, chinese: &str) -> Result<Self, CatalogueError> {
        let messages = BTreeMap::from([
            (Language::En, flatten(Language::En, english)?),
            (Language::Zh, flatten(Language::Zh, chinese)?),
        ]);
        let plural_bases = collect_plural_bases(&messages);
        let catalogue = Self {
            messages,
            plural_bases,
        };
        catalogue.validate()?;
        Ok(catalogue)
    }

    /// Translates one request, falling back to English only when the whole
    /// message is absent from the requested language.
    pub fn translate(
        &self,
        language: Language,
        request: TranslationRequest<'_>,
    ) -> Result<String, TranslationError> {
        let template = match self.lookup_template(language, request.key, request.count)? {
            Some(template) => template,
            None if language != Language::En => {
                match self.lookup_template(Language::En, request.key, request.count)? {
                    Some(template) => template,
                    None => {
                        return Err(TranslationError::MissingKey {
                            language,
                            key: request.key.as_str(),
                        });
                    }
                }
            }
            None => {
                return Err(TranslationError::MissingKey {
                    language,
                    key: request.key.as_str(),
                });
            }
        };
        format_template(
            template,
            language,
            request.key,
            request.count,
            request.arguments,
        )
    }

    /// Whether a language has a message or plural group for `key`.
    pub fn contains(&self, language: Language, key: TranslationKey) -> bool {
        let Some(messages) = self.messages.get(&language) else {
            return false;
        };
        let base = key.as_str();
        if self.plural_bases.contains(base) {
            messages
                .keys()
                .any(|candidate| semantic_key(candidate, &self.plural_bases) == base)
        } else {
            messages.contains_key(base)
        }
    }

    fn lookup_template(
        &self,
        language: Language,
        key: TranslationKey,
        count: Option<f64>,
    ) -> Result<Option<&str>, TranslationError> {
        let Some(messages) = self.messages.get(&language) else {
            return Ok(None);
        };
        let base = key.as_str();

        if self.plural_bases.contains(base) {
            let Some(count) = count else {
                if let Some(template) = messages.get(base) {
                    return Ok(Some(template));
                }
                return Err(TranslationError::MissingCount {
                    language,
                    key: base,
                });
            };
            let category = if count == 0.0 && messages.contains_key(&format!("{base}_zero")) {
                PluralCategory::Zero
            } else {
                language.plural_category(count)
            };
            for candidate in [category, PluralCategory::Other] {
                let plural_key = format!("{base}{}", candidate.suffix());
                if let Some(template) = messages.get(&plural_key) {
                    return Ok(Some(template));
                }
            }
            if let Some(template) = messages.get(base) {
                return Ok(Some(template));
            }
            return Ok(messages.get(base).map(String::as_str));
        }

        Ok(messages.get(base).map(String::as_str))
    }

    fn validate(&self) -> Result<(), CatalogueError> {
        for language in Language::ALL {
            let Some(messages) = self.messages.get(&language) else {
                continue;
            };
            for (key, template) in messages {
                parse_segments(template).map_err(|reason| CatalogueError::InvalidTemplate {
                    language,
                    key: key.clone(),
                    reason,
                })?;
            }
        }

        let english = self.semantic_keys(Language::En);
        let chinese = self.semantic_keys(Language::Zh);
        for key in &english {
            if !chinese.contains(key) {
                return Err(CatalogueError::SemanticKeysDiffer {
                    missing_from: Language::Zh,
                    key: key.clone(),
                });
            }
        }
        for key in &chinese {
            if !english.contains(key) {
                return Err(CatalogueError::SemanticKeysDiffer {
                    missing_from: Language::En,
                    key: key.clone(),
                });
            }
        }

        for base in &self.plural_bases {
            for language in Language::ALL {
                let Some(messages) = self.messages.get(&language) else {
                    continue;
                };
                for &category in language.required_plural_categories() {
                    let key = format!("{base}{}", category.suffix());
                    if !messages.contains_key(&key) {
                        return Err(CatalogueError::MissingPluralForm {
                            language,
                            base: base.clone(),
                            suffix: category.suffix(),
                        });
                    }
                }
            }
        }

        self.validate_placeholders()
    }

    fn semantic_keys(&self, language: Language) -> BTreeSet<String> {
        self.messages
            .get(&language)
            .into_iter()
            .flat_map(|messages| messages.keys())
            .map(|key| semantic_key(key, &self.plural_bases).to_string())
            .collect()
    }

    fn validate_placeholders(&self) -> Result<(), CatalogueError> {
        let mut unions: BTreeMap<Language, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

        for language in Language::ALL {
            let Some(messages) = self.messages.get(&language) else {
                continue;
            };
            let mut by_base: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (key, template) in messages {
                let base = semantic_key(key, &self.plural_bases).to_string();
                let names = by_base.entry(base).or_default();
                for segment in
                    parse_segments(template).map_err(|reason| CatalogueError::InvalidTemplate {
                        language,
                        key: key.clone(),
                        reason,
                    })?
                {
                    if let Segment::Placeholder(name) = segment
                        && name != "count"
                    {
                        names.insert(name.to_string());
                    }
                }
            }
            unions.insert(language, by_base);
        }

        let english = unions.get(&Language::En);
        let chinese = unions.get(&Language::Zh);
        let (Some(english), Some(chinese)) = (english, chinese) else {
            return Ok(());
        };

        for (base, expected) in english {
            let found = chinese.get(base).cloned().unwrap_or_default();
            if &found != expected {
                return Err(CatalogueError::PlaceholderMismatch {
                    base: base.clone(),
                    language: Language::Zh,
                    expected: expected.iter().cloned().collect(),
                    found: found.into_iter().collect(),
                });
            }
        }

        Ok(())
    }
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { language, message } => {
                write!(
                    formatter,
                    "the {language} locale is not valid JSON: {message}"
                )
            }
            Self::NonStringValue { language, key } => {
                write!(formatter, "{key} in the {language} locale is not a string")
            }
            Self::InvalidTemplate {
                language,
                key,
                reason,
            } => write!(
                formatter,
                "{key} in the {language} locale has an invalid template: {reason}"
            ),
            Self::SemanticKeysDiffer { missing_from, key } => {
                write!(formatter, "{key} is missing from the {missing_from} locale")
            }
            Self::MissingPluralForm {
                language,
                base,
                suffix,
            } => write!(formatter, "the {language} locale is missing {base}{suffix}"),
            Self::PlaceholderMismatch {
                base,
                language,
                expected,
                found,
            } => write!(
                formatter,
                "the {language} placeholders for {base} are {found:?}, expected {expected:?}"
            ),
        }
    }
}

impl std::error::Error for CatalogueError {}

fn flatten(language: Language, source: &str) -> Result<BTreeMap<String, String>, CatalogueError> {
    let value: Value =
        serde_json::from_str(source).map_err(|error| CatalogueError::InvalidJson {
            language,
            message: error.to_string(),
        })?;
    let Value::Object(root) = value else {
        return Err(CatalogueError::InvalidJson {
            language,
            message: "the document root is not an object".to_string(),
        });
    };

    let mut messages = BTreeMap::new();
    flatten_object(language, &root, "", &mut messages)?;
    Ok(messages)
}

fn flatten_object(
    language: Language,
    object: &serde_json::Map<String, Value>,
    prefix: &str,
    messages: &mut BTreeMap<String, String>,
) -> Result<(), CatalogueError> {
    for (key, value) in object {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        if full_key.is_empty() {
            return Err(CatalogueError::NonStringValue {
                language,
                key: full_key,
            });
        }
        match value {
            Value::String(text) => {
                messages.insert(full_key, text.clone());
            }
            Value::Object(child) => flatten_object(language, child, &full_key, messages)?,
            _ => {
                return Err(CatalogueError::NonStringValue {
                    language,
                    key: full_key,
                });
            }
        }
    }
    Ok(())
}

fn collect_plural_bases(
    messages: &BTreeMap<Language, BTreeMap<String, String>>,
) -> BTreeSet<String> {
    let mut bases = BTreeSet::new();
    for messages in messages.values() {
        for key in messages.keys() {
            if let Some((base, _)) = split_plural_key(key)
                && !base.is_empty()
            {
                bases.insert(base.to_string());
            }
        }
    }
    bases
}

fn split_plural_key(key: &str) -> Option<(&str, PluralCategory)> {
    let (base, suffix) = key.rsplit_once('_')?;
    let category = PluralCategory::from_suffix(suffix)?;
    Some((base, category))
}

fn semantic_key<'a>(key: &'a str, plural_bases: &BTreeSet<String>) -> &'a str {
    if let Some((base, _)) = split_plural_key(key)
        && plural_bases.contains(base)
    {
        return base;
    }
    key
}
