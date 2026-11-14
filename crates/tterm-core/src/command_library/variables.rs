//! Saved-command placeholders, their values and the preview text, from
//! `src/lib/commandVariables.ts`.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use crate::command_library::models::CommandVariable;
use crate::js::{is_js_whitespace, trim_js, utf16_len};

/// What a placeholder looks like, for hints. It is built here rather than
/// translated because i18next interpolates `{{…}}`.
pub const PLACEHOLDER_EXAMPLE: &str = "{{host}}";

/// The placeholder for `name`, as the commands write it.
pub fn format_placeholder(name: &str) -> String {
    format!("{{{{{name}}}}}")
}

fn skip_whitespace(text: &str, mut cursor: usize) -> usize {
    while let Some(character) = text[cursor..].chars().next() {
        if !is_js_whitespace(character) {
            break;
        }
        cursor += character.len_utf8();
    }
    cursor
}

/// One `{{ name }}` in a command and the byte range it covers.
struct Placeholder {
    start: usize,
    end: usize,
    name: String,
}

fn placeholder_at(text: &str, start: usize) -> Option<(String, usize)> {
    let mut cursor = skip_whitespace(text, start + 2);
    let name_start = cursor;
    let first = text[cursor..].chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    cursor += first.len_utf8();
    while let Some(character) = text[cursor..].chars().next() {
        if character.is_ascii_alphanumeric() || character == '_' {
            cursor += character.len_utf8();
        } else {
            break;
        }
    }
    let name = text[name_start..cursor].to_owned();
    cursor = skip_whitespace(text, cursor);
    if !text[cursor..].starts_with("}}") {
        return None;
    }
    Some((name, cursor + 2))
}

fn placeholders(text: &str) -> Vec<Placeholder> {
    let mut found = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let Some(offset) = text[index..].find("{{") else {
            break;
        };
        let start = index + offset;
        if let Some((name, end)) = placeholder_at(text, start) {
            found.push(Placeholder { start, end, name });
            index = end;
        } else {
            index = start + 1;
        }
    }
    found
}

/// Placeholder names in `command_text`, each once, in order of first appearance.
pub fn extract_placeholder_names(command_text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for placeholder in placeholders(command_text) {
        if !names.contains(&placeholder.name) {
            names.push(placeholder.name);
        }
    }
    names
}

/// The options of a `choice` variable; empty when none are stored or the value
/// is malformed.
pub fn parse_choice_options(options_json: Option<&str>) -> Vec<String> {
    let Some(options_json) = options_json.filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    let Ok(Value::Array(options)) = serde_json::from_str::<Value>(options_json) else {
        return Vec::new();
    };
    options
        .into_iter()
        .filter_map(|option| option.as_str().map(str::to_owned))
        .collect()
}

/// Splits the editor's one-per-line (or comma separated) options field.
pub fn parse_choice_options_text(value: &str) -> Vec<String> {
    let mut options: Vec<String> = Vec::new();
    for option in value
        .split(['\n', ',', '，'])
        .map(trim_js)
        .filter(|option| !option.is_empty())
    {
        if !options.iter().any(|existing| existing == option) {
            options.push(option.to_owned());
        }
    }
    options
}

/// The variables to ask for when inserting: those defined on the command whose
/// placeholder still appears in its text.
///
/// A `{{…}}` without a definition is literal text (Go templates, Jinja), so it
/// is left alone.
pub fn active_variables(command_text: &str, variables: &[CommandVariable]) -> Vec<CommandVariable> {
    let names = extract_placeholder_names(command_text);
    let mut active: Vec<CommandVariable> = variables
        .iter()
        .filter(|variable| names.contains(&variable.name))
        .cloned()
        .collect();
    active.sort_by_key(|variable| variable.position);
    active
}

/// The value each variable starts with: its default, never a secret, and a
/// valid choice.
pub fn initial_variable_values(variables: &[CommandVariable]) -> IndexMap<String, String> {
    let mut values = IndexMap::new();
    for variable in variables {
        let value = match variable.value_type.as_str() {
            "secret" => String::new(),
            "choice" => {
                let options = parse_choice_options(variable.options_json.as_deref());
                let fallback = if variable.is_required {
                    options.first().cloned().unwrap_or_default()
                } else {
                    String::new()
                };
                match variable.default_value.as_deref() {
                    Some(default)
                        if !default.is_empty()
                            && options.iter().any(|option| option == default) =>
                    {
                        default.to_owned()
                    }
                    _ => fallback,
                }
            }
            _ => variable.default_value.clone().unwrap_or_default(),
        };
        values.insert(variable.name.clone(), value);
    }
    values
}

/// A problem with a value entered for a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableValueError {
    /// A required variable has no value.
    Required,
    /// A number variable holds something that is not a finite number.
    Number,
}

/// Problems with the values entered for `variables`, keyed by variable name.
pub fn validate_variable_values(
    variables: &[CommandVariable],
    values: &IndexMap<String, String>,
) -> IndexMap<String, VariableValueError> {
    let mut errors = IndexMap::new();
    for variable in variables {
        let value = values.get(&variable.name).map_or("", String::as_str);
        if trim_js(value).is_empty() {
            if variable.is_required {
                errors.insert(variable.name.clone(), VariableValueError::Required);
            }
        } else if variable.value_type == "number" && !is_finite_number(value) {
            errors.insert(variable.name.clone(), VariableValueError::Number);
        }
    }
    errors
}

fn is_finite_number(value: &str) -> bool {
    tterm_terminal::theme::js::js_number(value).is_finite()
}

/// Replaces the placeholders of `variables` with `values`. With `mask_secrets`
/// a filled-in secret shows as bullets, for previews.
pub fn render_command_text(
    command_text: &str,
    variables: &[CommandVariable],
    values: &IndexMap<String, String>,
    mask_secrets: bool,
) -> String {
    let mut output = String::new();
    let mut last = 0;
    for placeholder in placeholders(command_text) {
        output.push_str(&command_text[last..placeholder.start]);
        let variable = variables
            .iter()
            .find(|variable| variable.name == placeholder.name);
        match variable {
            None => output.push_str(&command_text[placeholder.start..placeholder.end]),
            Some(variable) => {
                let value = values.get(&placeholder.name).map_or("", String::as_str);
                if mask_secrets && variable.value_type == "secret" && !value.is_empty() {
                    output.push_str(&"•".repeat(utf16_len(value).min(8)));
                } else {
                    output.push_str(value);
                }
            }
        }
        last = placeholder.end;
    }
    output.push_str(&command_text[last..]);
    output
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;

    use super::{
        VariableValueError, active_variables, extract_placeholder_names, initial_variable_values,
        parse_choice_options, parse_choice_options_text, render_command_text,
        validate_variable_values,
    };
    use crate::command_library::models::CommandVariable;

    fn variable(name: &str) -> CommandVariable {
        CommandVariable {
            name: name.to_owned(),
            label: name.to_owned(),
            value_type: "text".to_owned(),
            default_value: None,
            options_json: None,
            is_required: true,
            position: 0,
        }
    }

    fn with_type(mut variable: CommandVariable, value_type: &str) -> CommandVariable {
        variable.value_type = value_type.to_owned();
        variable
    }

    fn value_map(entries: &[(&str, &str)]) -> IndexMap<String, String> {
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn lists_each_placeholder_once_in_order_of_first_appearance() {
        assert_eq!(
            extract_placeholder_names("ssh {{user}}@{{ host }} -p {{port}} # {{user}}"),
            vec!["user", "host", "port"]
        );
    }

    #[test]
    fn ignores_template_syntax_that_is_not_a_plain_name() {
        assert!(
            extract_placeholder_names(
                "docker inspect -f '{{.State.Status}} {{range .Mounts}}' {{1st}} {}"
            )
            .is_empty()
        );
    }

    #[test]
    fn parses_stored_options_and_tolerates_malformed_values() {
        assert_eq!(
            parse_choice_options(Some(r#"["dev","prod"]"#)),
            vec!["dev", "prod"]
        );
        assert!(parse_choice_options(Some(r#"{"a":1}"#)).is_empty());
        assert!(parse_choice_options(Some("not json")).is_empty());
        assert!(parse_choice_options(None).is_empty());
    }

    #[test]
    fn splits_the_editor_field_on_lines_and_commas_without_duplicates() {
        assert_eq!(
            parse_choice_options_text("dev, prod\nstaging，dev\n\n"),
            vec!["dev", "prod", "staging"]
        );
    }

    #[test]
    fn keeps_defined_variables_still_present_in_the_text_by_position() {
        let mut pod = variable("pod");
        pod.position = 1;
        let mut removed = variable("removed");
        removed.position = 2;
        let namespace = variable("namespace");

        let active = active_variables(
            "kubectl -n {{namespace}} logs {{pod}} '{{end}}'",
            &[pod, removed, namespace],
        );
        assert_eq!(
            active
                .iter()
                .map(|variable| variable.name.as_str())
                .collect::<Vec<_>>(),
            vec!["namespace", "pod"]
        );
    }

    #[test]
    fn prefills_defaults_never_secrets_and_picks_a_valid_choice() {
        let mut host = variable("host");
        host.default_value = Some("example.com".to_owned());
        let mut env = with_type(variable("env"), "choice");
        env.options_json = Some(r#"["dev","prod"]"#.to_owned());
        env.default_value = Some("qa".to_owned());
        let mut tier = with_type(variable("tier"), "choice");
        tier.options_json = Some(r#"["a","b"]"#.to_owned());
        tier.default_value = Some("b".to_owned());
        let mut zone = with_type(variable("zone"), "choice");
        zone.options_json = Some(r#"["x"]"#.to_owned());
        zone.is_required = false;

        let values = initial_variable_values(&[
            host,
            with_type(variable("token"), "secret"),
            env,
            tier,
            zone,
        ]);
        assert_eq!(
            values,
            value_map(&[
                ("host", "example.com"),
                ("token", ""),
                ("env", "dev"),
                ("tier", "b"),
                ("zone", ""),
            ])
        );
    }

    #[test]
    fn reports_missing_required_values_and_non_numeric_numbers() {
        let mut note = variable("note");
        note.is_required = false;
        let port = with_type(variable("port"), "number");
        let mut count = with_type(variable("count"), "number");
        count.is_required = false;
        let variables = vec![variable("host"), note, port, count];

        let errors = validate_variable_values(
            &variables,
            &value_map(&[("host", " "), ("port", "22x"), ("count", "")]),
        );
        assert_eq!(
            errors,
            values_errors(&[
                ("host", VariableValueError::Required),
                ("port", VariableValueError::Number)
            ])
        );
        assert!(
            validate_variable_values(&variables, &value_map(&[("host", "a"), ("port", "2222")]))
                .is_empty()
        );
    }

    fn values_errors(
        entries: &[(&str, VariableValueError)],
    ) -> IndexMap<String, VariableValueError> {
        entries
            .iter()
            .map(|(name, error)| ((*name).to_owned(), *error))
            .collect()
    }

    #[test]
    fn replaces_defined_placeholders_and_leaves_the_rest_untouched() {
        let variables = vec![variable("host"), with_type(variable("token"), "secret")];
        assert_eq!(
            render_command_text(
                "curl -H 'X: {{token}}' {{ host }}/{{host}} '{{end}}'",
                &variables,
                &value_map(&[("host", "a.io"), ("token", "s3cret")]),
                false,
            ),
            "curl -H 'X: s3cret' a.io/a.io '{{end}}'"
        );
    }

    #[test]
    fn masks_secrets_for_previews() {
        let variables = vec![variable("host"), with_type(variable("token"), "secret")];
        assert_eq!(
            render_command_text(
                "login {{token}} {{host}}",
                &variables,
                &value_map(&[("host", "a.io"), ("token", "s3cret")]),
                true,
            ),
            "login •••••• a.io"
        );
    }

    #[test]
    fn does_not_re_expand_placeholders_that_appear_in_a_value() {
        let variables = vec![variable("host"), with_type(variable("token"), "secret")];
        assert_eq!(
            render_command_text(
                "echo {{host}}",
                &variables,
                &value_map(&[("host", "{{token}}"), ("token", "x")]),
                false,
            ),
            "echo {{token}}"
        );
    }
}
