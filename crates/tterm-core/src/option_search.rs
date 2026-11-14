//! Filtering for the theme and connection lists, from
//! `src/lib/optionSearch.ts`.

use crate::text_search::matches_query;

/// One choice of a searchable list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchableOption {
    /// The value the choice selects.
    pub value: String,
    /// The text shown for the choice.
    pub label: String,
    /// Secondary text shown beside the label, such as an address.
    pub detail: Option<String>,
    /// Further text a search finds the option by.
    pub keywords: Vec<String>,
}

/// Choices under one heading, or above every heading when `label` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchableOptionGroup {
    /// Heading of the group; `None` for options listed above every group.
    pub label: Option<String>,
    /// The choices in the group.
    pub options: Vec<SearchableOption>,
}

/// The options whose label, detail or keywords hold every word of `query`,
/// ignoring case; groups left empty are dropped.
pub fn filter_option_groups(
    groups: &[SearchableOptionGroup],
    query: &str,
) -> Vec<SearchableOptionGroup> {
    groups
        .iter()
        .filter_map(|group| {
            let options: Vec<SearchableOption> = group
                .options
                .iter()
                .filter(|option| {
                    let mut fields: Vec<Option<&str>> =
                        vec![Some(option.label.as_str()), option.detail.as_deref()];
                    fields.extend(option.keywords.iter().map(|keyword| Some(keyword.as_str())));
                    matches_query(query, &fields)
                })
                .cloned()
                .collect();

            if options.is_empty() {
                None
            } else {
                Some(SearchableOptionGroup {
                    label: group.label.clone(),
                    options,
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SearchableOption, SearchableOptionGroup, filter_option_groups};

    fn option(value: &str, label: &str) -> SearchableOption {
        SearchableOption {
            value: value.to_owned(),
            label: label.to_owned(),
            detail: None,
            keywords: Vec::new(),
        }
    }

    fn groups() -> Vec<SearchableOptionGroup> {
        vec![
            SearchableOptionGroup {
                label: None,
                options: vec![option("", "Same as the app theme")],
            },
            SearchableOptionGroup {
                label: Some("Presets".to_owned()),
                options: vec![option("default", "Default"), option("ocean", "Ocean")],
            },
            SearchableOptionGroup {
                label: Some("Library".to_owned()),
                options: vec![
                    option("catalog:Red Planet", "Red Planet"),
                    option("catalog:Tomorrow Night", "Tomorrow Night"),
                ],
            },
        ]
    }

    #[test]
    fn keeps_everything_for_an_empty_query() {
        assert_eq!(filter_option_groups(&groups(), "  "), groups());
    }

    #[test]
    fn matches_every_word_in_any_order_ignoring_case() {
        assert_eq!(
            filter_option_groups(&groups(), "night TOMORROW"),
            vec![SearchableOptionGroup {
                label: Some("Library".to_owned()),
                options: vec![option("catalog:Tomorrow Night", "Tomorrow Night")],
            }]
        );
    }

    #[test]
    fn drops_groups_left_empty() {
        let labels: Vec<Option<String>> = filter_option_groups(&groups(), "red")
            .into_iter()
            .map(|group| group.label)
            .collect();
        assert_eq!(labels, vec![Some("Library".to_owned())]);
        assert!(filter_option_groups(&groups(), "nothing like this").is_empty());
    }

    #[test]
    fn finds_an_option_by_its_detail_and_keywords() {
        let hosts = vec![SearchableOptionGroup {
            label: Some("prod".to_owned()),
            options: vec![
                SearchableOption {
                    value: "a".to_owned(),
                    label: "bastion".to_owned(),
                    detail: Some("ops@10.0.0.1:22".to_owned()),
                    keywords: vec!["prod".to_owned()],
                },
                SearchableOption {
                    value: "b".to_owned(),
                    label: "web".to_owned(),
                    detail: Some("deploy@10.0.0.2:22".to_owned()),
                    keywords: vec!["prod".to_owned()],
                },
            ],
        }];

        let values = |query: &str| -> Vec<String> {
            filter_option_groups(&hosts, query)[0]
                .options
                .iter()
                .map(|option| option.value.clone())
                .collect()
        };
        assert_eq!(values("10.0.0.2"), vec!["b"]);
        assert_eq!(values("prod ops"), vec!["a"]);
    }
}
