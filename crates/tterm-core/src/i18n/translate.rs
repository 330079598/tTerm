//! Translation keys, typed interpolation values and template formatting.

use std::fmt;

use super::language::Language;

/// A catalogue key. Production callers use the constants in
/// [`super::keys`], so a misspelt key is a compile error rather than an empty
/// label at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TranslationKey(&'static str);

impl TranslationKey {
    /// Wraps a dotted catalogue key. GPUI callers use the checked registry in
    /// [`super::keys`]; the constructor stays crate-internal so client crates
    /// cannot invent unchecked keys.
    pub(crate) const fn new(key: &'static str) -> Self {
        Self(key)
    }

    /// The dotted key as written in the locale JSON.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for TranslationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// A value substituted for a `{{name}}` in a translated template.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgumentValue<'a> {
    /// Text supplied by the caller.
    Text(&'a str),
    /// A finite number supplied by the caller.
    Number(f64),
}

impl fmt::Display for ArgumentValue<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => formatter.write_str(text),
            Self::Number(number) => write!(formatter, "{number}"),
        }
    }
}

/// One translation lookup: a key, the optional plural count and interpolation
/// arguments.
#[derive(Clone, Copy, Debug)]
pub struct TranslationRequest<'a> {
    /// The catalogue key, without a plural suffix.
    pub key: TranslationKey,
    /// The count that selects a plural suffix. It also fills `{{count}}`.
    pub count: Option<f64>,
    /// Additional named values for `{{name}}` placeholders.
    pub arguments: &'a [(&'a str, ArgumentValue<'a>)],
}

impl<'a> TranslationRequest<'a> {
    /// A request without a plural count or interpolation arguments.
    pub const fn new(key: TranslationKey) -> Self {
        Self {
            key,
            count: None,
            arguments: &[],
        }
    }

    /// Adds the plural count and makes it available as `{{count}}`.
    pub const fn count(mut self, count: f64) -> Self {
        self.count = Some(count);
        self
    }

    /// Adds interpolation arguments.
    pub const fn arguments(mut self, arguments: &'a [(&'a str, ArgumentValue<'a>)]) -> Self {
        self.arguments = arguments;
        self
    }
}

/// Why a catalogue lookup or interpolation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TranslationError {
    /// Neither the requested language nor the English fallback has the key.
    MissingKey {
        language: Language,
        key: &'static str,
    },
    /// A plural key needs a count to choose `_one`, `_other` and so on.
    MissingCount {
        language: Language,
        key: &'static str,
    },
    /// The template references a placeholder the caller did not supply.
    MissingArgument {
        language: Language,
        key: &'static str,
        name: String,
    },
    /// The same argument name was supplied twice, or `count` was both a typed
    /// count and an ordinary argument.
    DuplicateArgument { name: String },
    /// A locale JSON template has malformed interpolation syntax.
    InvalidTemplate {
        key: &'static str,
        reason: &'static str,
    },
    /// A numeric count or argument is NaN or infinite.
    InvalidNumber { name: String },
}

impl fmt::Display for TranslationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKey { language, key } => {
                write!(formatter, "{key} is missing from the {language} catalogue")
            }
            Self::MissingCount { language, key } => {
                write!(formatter, "{key} needs a plural count in {language}")
            }
            Self::MissingArgument {
                language,
                key,
                name,
            } => write!(
                formatter,
                "{key} in {language} needs the interpolation argument {{{{{name}}}}}"
            ),
            Self::DuplicateArgument { name } => {
                write!(
                    formatter,
                    "the interpolation argument {name} was supplied twice"
                )
            }
            Self::InvalidTemplate { key, reason } => {
                write!(
                    formatter,
                    "{key} has an invalid interpolation template: {reason}"
                )
            }
            Self::InvalidNumber { name } => {
                write!(formatter, "the interpolation argument {name} is not finite")
            }
        }
    }
}

impl std::error::Error for TranslationError {}

/// One piece of a translation template.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    Literal(&'a str),
    Placeholder(&'a str),
}

/// Splits a template on i18next's `{{name}}` placeholders. Single braces stay
/// literal, which matters for shell syntax and JSON examples in the text.
pub(crate) fn parse_segments(template: &str) -> Result<Vec<Segment<'_>>, &'static str> {
    let mut segments = Vec::new();
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        if start > 0 {
            segments.push(Segment::Literal(&rest[..start]));
        }
        let after_open = &rest[start + 2..];
        let Some(end) = after_open.find("}}") else {
            return Err("an interpolation is not closed");
        };
        let name = after_open[..end].trim();
        if name.is_empty() {
            return Err("an interpolation name is empty");
        }
        if !is_interpolation_name(name) {
            return Err("an interpolation name is not an identifier");
        }
        segments.push(Segment::Placeholder(name));
        rest = &after_open[end + 2..];
    }

    if !rest.is_empty() {
        segments.push(Segment::Literal(rest));
    }
    Ok(segments)
}

fn is_interpolation_name(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    characters
        .all(|character| character.is_ascii_alphanumeric() || character == '_' || character == '.')
}

/// Replaces every placeholder with its typed argument.
pub(crate) fn format_template(
    template: &str,
    language: Language,
    key: TranslationKey,
    count: Option<f64>,
    arguments: &[(&str, ArgumentValue<'_>)],
) -> Result<String, TranslationError> {
    if let Some(count) = count
        && !count.is_finite()
    {
        return Err(TranslationError::InvalidNumber {
            name: "count".to_string(),
        });
    }
    for (name, value) in arguments {
        if let ArgumentValue::Number(number) = *value
            && !number.is_finite()
        {
            return Err(TranslationError::InvalidNumber {
                name: (*name).to_string(),
            });
        }
    }
    for (index, (name, _)) in arguments.iter().enumerate() {
        if arguments[..index].iter().any(|(other, _)| other == name) {
            return Err(TranslationError::DuplicateArgument {
                name: (*name).to_string(),
            });
        }
    }
    if count.is_some() && arguments.iter().any(|(name, _)| *name == "count") {
        return Err(TranslationError::DuplicateArgument {
            name: "count".to_string(),
        });
    }

    let segments =
        parse_segments(template).map_err(|reason| TranslationError::InvalidTemplate {
            key: key.as_str(),
            reason,
        })?;
    let mut rendered = String::with_capacity(template.len());

    for segment in segments {
        match segment {
            Segment::Literal(text) => rendered.push_str(text),
            Segment::Placeholder(name) => {
                let value = if name == "count" {
                    count
                        .map(ArgumentValue::Number)
                        .or_else(|| argument_value(arguments, name))
                } else {
                    argument_value(arguments, name)
                };
                let Some(value) = value else {
                    return Err(TranslationError::MissingArgument {
                        language,
                        key: key.as_str(),
                        name: name.to_string(),
                    });
                };
                use std::fmt::Write as _;
                let _ = write!(rendered, "{value}");
            }
        }
    }

    Ok(rendered)
}

fn argument_value<'a>(
    arguments: &'a [(&'a str, ArgumentValue<'a>)],
    name: &str,
) -> Option<ArgumentValue<'a>> {
    arguments
        .iter()
        .find(|(argument_name, _)| *argument_name == name)
        .map(|(_, value)| *value)
}
