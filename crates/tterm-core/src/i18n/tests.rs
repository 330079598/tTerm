//! Behaviour tests for the shared catalogue, language rules and formatting.

use std::collections::BTreeMap;

use serde_json::Value;

use super::catalogue::{Catalogue, CatalogueError, EN_JSON, ZH_JSON};
use super::keys;
use super::language::{Language, PluralCategory};
use super::translate::{
    ArgumentValue, TranslationError, TranslationKey, TranslationRequest, format_template,
};

fn flatten(source: &str) -> BTreeMap<String, String> {
    let value: Value = serde_json::from_str(source).expect("valid locale JSON");
    let mut messages = BTreeMap::new();
    walk(&value, "", &mut messages);
    messages
}

fn walk(value: &Value, prefix: &str, messages: &mut BTreeMap<String, String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let full_key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                walk(value, &full_key, messages);
            }
        }
        Value::String(text) => {
            messages.insert(prefix.to_string(), text.clone());
        }
        other => panic!("unexpected locale value at {prefix}: {other}"),
    }
}

#[test]
fn language_normalization_keeps_the_first_launch_behaviour() {
    assert_eq!(Language::from_locale("zh"), Language::Zh);
    assert_eq!(Language::from_locale("zh_CN"), Language::Zh);
    assert_eq!(Language::from_locale("zh-Hant-TW"), Language::Zh);
    assert_eq!(Language::from_locale("en-US"), Language::En);
    assert_eq!(Language::from_locale("fr-FR"), Language::En);
}

#[test]
fn plural_categories_follow_the_two_languages_rules() {
    assert_eq!(Language::En.plural_category(1.0), PluralCategory::One);
    assert_eq!(Language::En.plural_category(-1.0), PluralCategory::One);
    assert_eq!(Language::En.plural_category(0.0), PluralCategory::Other);
    assert_eq!(Language::En.plural_category(2.0), PluralCategory::Other);
    assert_eq!(Language::En.plural_category(1.5), PluralCategory::Other);
    assert_eq!(Language::Zh.plural_category(1.0), PluralCategory::Other);
    assert_eq!(Language::Zh.plural_category(2.0), PluralCategory::Other);
    assert_eq!(PluralCategory::One.suffix(), "_one");
    assert_eq!(PluralCategory::Other.suffix(), "_other");
}

#[test]
fn embedded_sources_keep_their_intentional_plural_difference() {
    let english = flatten(EN_JSON);
    let chinese = flatten(ZH_JSON);

    let english_only: Vec<_> = english
        .keys()
        .filter(|key| !chinese.contains_key(*key))
        .collect();
    for key in english_only {
        let base = ["_zero", "_one", "_two", "_few", "_many", "_other"]
            .iter()
            .find_map(|suffix| key.strip_suffix(suffix))
            .unwrap_or_else(|| panic!("{key} is not a plural form"));
        assert!(
            chinese.contains_key(&format!("{base}_other")),
            "{key} has no Chinese plural counterpart"
        );
    }

    Catalogue::embedded().expect("embedded catalogue is valid");
}

#[test]
fn every_gpui_key_exists_in_both_languages() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    for key in keys::ALL {
        assert!(
            catalogue.contains(Language::En, *key),
            "{} is missing from English",
            key.as_str()
        );
        assert!(
            catalogue.contains(Language::Zh, *key),
            "{} is missing from Chinese",
            key.as_str()
        );
    }
}

#[test]
fn plural_translation_selects_the_language_specific_form() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    let key = TranslationKey::new("workspace.paneCount");

    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(1.0))
            .expect("one English pane"),
        "1 pane"
    );
    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(-1.0))
            .expect("negative singular English pane"),
        "-1 pane"
    );
    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(0.0))
            .expect("zero English panes"),
        "0 panes"
    );
    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(2.0))
            .expect("two English panes"),
        "2 panes"
    );
    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(1.5))
            .expect("fractional English panes"),
        "1.5 panes"
    );
    assert_eq!(
        catalogue
            .translate(Language::Zh, TranslationRequest::new(key).count(1.0))
            .expect("one Chinese pane"),
        "1 个分屏"
    );
    assert_eq!(
        catalogue
            .translate(Language::Zh, TranslationRequest::new(key).count(2.0))
            .expect("two Chinese panes"),
        "2 个分屏"
    );
}

#[test]
fn a_missing_count_is_an_error_for_a_plural_key() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    let key = TranslationKey::new("workspace.paneCount");

    assert_eq!(
        catalogue.translate(Language::En, TranslationRequest::new(key)),
        Err(TranslationError::MissingCount {
            language: Language::En,
            key: "workspace.paneCount",
        })
    );
}

#[test]
fn a_translation_may_describe_a_plural_without_printing_the_count() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    let key = TranslationKey::new("broadcast.autoReconnectConfirmDescription");

    let translated = catalogue
        .translate(Language::Zh, TranslationRequest::new(key).count(2.0))
        .expect("Chinese plural without the count placeholder");
    assert!(translated.contains("断开的连接"));
}

#[test]
fn interpolation_uses_typed_text_and_numbers() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    let key = TranslationKey::new("app.version");
    let arguments = [("version", ArgumentValue::Text("1.2.3"))];

    assert_eq!(
        catalogue
            .translate(
                Language::En,
                TranslationRequest::new(key).arguments(&arguments)
            )
            .expect("English version"),
        "Version 1.2.3"
    );
    assert_eq!(
        catalogue
            .translate(
                Language::Zh,
                TranslationRequest::new(key).arguments(&arguments)
            )
            .expect("Chinese version"),
        "版本 1.2.3"
    );
}

#[test]
fn missing_and_duplicate_arguments_are_reported() {
    let key = TranslationKey::new("app.version");
    let language = Language::En;

    assert_eq!(
        format_template("Version {{version}}", language, key, None, &[]),
        Err(TranslationError::MissingArgument {
            language,
            key: "app.version",
            name: "version".to_string(),
        })
    );

    let duplicates = [
        ("version", ArgumentValue::Text("1")),
        ("version", ArgumentValue::Text("2")),
    ];
    assert_eq!(
        format_template("Version {{version}}", language, key, None, &duplicates),
        Err(TranslationError::DuplicateArgument {
            name: "version".to_string(),
        })
    );

    let count = TranslationKey::new("workspace.paneCount");
    let count_argument = [("count", ArgumentValue::Number(2.0))];
    assert_eq!(
        format_template(
            "{{count}} panes",
            language,
            count,
            Some(2.0),
            &count_argument
        ),
        Err(TranslationError::DuplicateArgument {
            name: "count".to_string(),
        })
    );
}

#[test]
fn malformed_templates_are_rejected() {
    let key = TranslationKey::new("test.broken");

    assert_eq!(
        format_template("Hello {{name", Language::En, key, None, &[]),
        Err(TranslationError::InvalidTemplate {
            key: "test.broken",
            reason: "an interpolation is not closed",
        })
    );
}

#[test]
fn a_catalogue_accepts_the_intentional_chinese_plural_omission() {
    let catalogue = Catalogue::from_sources(
        r#"{"x_one":"one","x_other":"{{count}} others"}"#,
        r#"{"x_other":"{{count}} 个"}"#,
    )
    .expect("semantic plural bases match");
    let key = TranslationKey::new("x");

    assert_eq!(
        catalogue
            .translate(Language::Zh, TranslationRequest::new(key).count(1.0))
            .expect("Chinese plural"),
        "1 个"
    );
}

#[test]
fn a_catalogue_rejects_different_semantic_keys() {
    let error = Catalogue::from_sources(r#"{"x":"X"}"#, r#"{"x":"X","y":"Y"}"#)
        .expect_err("different bases are rejected");
    assert_eq!(
        error,
        CatalogueError::SemanticKeysDiffer {
            missing_from: Language::En,
            key: "y".to_string(),
        }
    );
}

#[test]
fn a_catalogue_rejects_a_missing_plural_form() {
    let error =
        Catalogue::from_sources(r#"{"x_one":"one","x_other":"other"}"#, r#"{"x_one":"一"}"#)
            .expect_err("Chinese needs _other");
    assert_eq!(
        error,
        CatalogueError::MissingPluralForm {
            language: Language::Zh,
            base: "x".to_string(),
            suffix: "_other",
        }
    );
}

#[test]
fn a_catalogue_rejects_different_named_placeholders() {
    let error = Catalogue::from_sources(r#"{"x":"Hello {{name}}"}"#, r#"{"x":"你好"}"#)
        .expect_err("the named placeholder differs");
    assert_eq!(
        error,
        CatalogueError::PlaceholderMismatch {
            base: "x".to_string(),
            language: Language::Zh,
            expected: vec!["name".to_string()],
            found: Vec::new(),
        }
    );
}

#[test]
fn a_catalogue_allows_a_missing_count_placeholder_in_one_language() {
    Catalogue::from_sources(
        r#"{"x_one":"one item","x_other":"{{count}} items"}"#,
        r#"{"x_other":"项目"}"#,
    )
    .expect("count may be omitted from a translation");
}

#[test]
fn the_zero_override_applies_only_when_the_zero_key_exists() {
    let catalogue = Catalogue::from_sources(
        r#"{"x_zero":"none","x_one":"one","x_other":"{{count}} others"}"#,
        r#"{"x_other":"{{count}} 个"}"#,
    )
    .expect("zero is an optional extra form");
    let key = TranslationKey::new("x");

    assert_eq!(
        catalogue
            .translate(Language::En, TranslationRequest::new(key).count(0.0))
            .expect("zero override"),
        "none"
    );

    let without_zero = Catalogue::embedded().expect("embedded catalogue");
    let pane = TranslationKey::new("workspace.paneCount");
    assert_eq!(
        without_zero
            .translate(Language::En, TranslationRequest::new(pane).count(0.0))
            .expect("zero without a zero form"),
        "0 panes"
    );
}

#[test]
fn non_finite_numbers_are_rejected() {
    let catalogue = Catalogue::embedded().expect("embedded catalogue");
    let key = TranslationKey::new("workspace.paneCount");

    assert_eq!(
        catalogue.translate(Language::En, TranslationRequest::new(key).count(f64::NAN)),
        Err(TranslationError::InvalidNumber {
            name: "count".to_string(),
        })
    );
}
