//! The SFTP drawer's file-name filter, from
//! `src/components/SftpDrawer/sftpSearch.ts`.
//!
//! User regular expressions use `regress`, whose pattern language follows
//! JavaScript. Its syntax-error wording differs from the WebView engine's.

use regress::Regex;

use crate::sftp::entry::SftpDirectoryEntry;

#[cfg(test)]
mod tests;

/// What Enter does in the drawer's search field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SftpSearchOptions {
    /// Treat the query as a regular expression.
    pub regex: bool,
    /// Search the folders under the current one on the server.
    pub recursive: bool,
}

/// The query compiled into the drawer's filter.
pub struct SftpSearchMatcher {
    /// Why the regular expression did not compile.
    pub error: Option<String>,
    /// Whether there is a query to filter by.
    pub has_query: bool,
    matcher: QueryMatcher,
}

/// Lets every entry through: what a search of subfolders found is shown as is.
pub const MATCH_ALL: SftpSearchMatcher = SftpSearchMatcher {
    error: None,
    has_query: false,
    matcher: QueryMatcher::All,
};

enum QueryMatcher {
    All,
    None,
    Substring(String),
    Glob(Regex),
    Regex(Regex),
}

fn has_glob_syntax(query: &str) -> bool {
    query.contains(['*', '?', '['])
}

fn escape_literal(character: char, source: &mut String) {
    if "\\^$+?.()|{}".contains(character) {
        source.push('\\');
    }
    source.push(character);
}

fn escape_class_content(content: &str) -> String {
    content.replace('\\', "\\\\").replace(']', "\\]")
}

fn glob_to_regex_source(glob: &str) -> String {
    let characters = glob.chars().collect::<Vec<_>>();
    let mut source = String::from("^");
    let mut index = 0;
    while index < characters.len() {
        match characters[index] {
            '*' => source.push_str(".*"),
            '?' => source.push('.'),
            '[' => match characters[index + 1..]
                .iter()
                .position(|character| *character == ']')
            {
                None => source.push_str("\\["),
                Some(0) => {
                    source.push_str("\\[\\]");
                    index += 1;
                }
                Some(length) => {
                    let content = characters[index + 1..index + 1 + length]
                        .iter()
                        .collect::<String>();
                    match content.strip_prefix('!') {
                        Some(negated) => {
                            source.push_str("[^");
                            source.push_str(&escape_class_content(negated));
                        }
                        None => {
                            source.push('[');
                            source.push_str(&escape_class_content(&content));
                        }
                    }
                    source.push(']');
                    index += length + 1;
                }
            },
            character => escape_literal(character, &mut source),
        }
        index += 1;
    }
    source.push('$');
    source
}

fn compile(source: &str) -> Result<Regex, String> {
    Regex::with_flags(source, "i").map_err(|error| error.to_string())
}

fn glob_matcher(glob: &str) -> Result<QueryMatcher, String> {
    compile(&glob_to_regex_source(glob)).map(QueryMatcher::Glob)
}

fn failed(query: &str, error: String) -> SftpSearchMatcher {
    SftpSearchMatcher {
        error: Some(format!("Invalid regular expression: /{query}/i: {error}")),
        has_query: true,
        matcher: QueryMatcher::None,
    }
}

/// Compiles the drawer's query with its current options.
pub fn create_sftp_search_matcher(
    search_query: &str,
    options: SftpSearchOptions,
) -> SftpSearchMatcher {
    let query = crate::js::trim_js(search_query);
    if query.is_empty() {
        return SftpSearchMatcher {
            matcher: QueryMatcher::All,
            ..MATCH_ALL
        };
    }

    if !options.regex {
        if has_glob_syntax(query) {
            return match glob_matcher(query) {
                Ok(matcher) => SftpSearchMatcher {
                    error: None,
                    has_query: true,
                    matcher,
                },
                Err(error) => failed(query, error),
            };
        }
        return SftpSearchMatcher {
            error: None,
            has_query: true,
            matcher: QueryMatcher::Substring(query.to_lowercase()),
        };
    }

    match compile(query) {
        Ok(pattern) => SftpSearchMatcher {
            error: None,
            has_query: true,
            matcher: QueryMatcher::Regex(pattern),
        },
        Err(_) if has_glob_syntax(query) => match glob_matcher(query) {
            Ok(matcher) => SftpSearchMatcher {
                error: None,
                has_query: true,
                matcher,
            },
            Err(error) => failed(query, error),
        },
        Err(error) => failed(query, error),
    }
}

impl SftpSearchMatcher {
    /// Whether `name` passes the filter.
    pub fn matches_name(&self, name: &str) -> bool {
        match &self.matcher {
            QueryMatcher::All => true,
            QueryMatcher::None => false,
            QueryMatcher::Substring(query) => name.to_lowercase().contains(query),
            QueryMatcher::Glob(pattern) | QueryMatcher::Regex(pattern) => {
                pattern.find(name).is_some()
            }
        }
    }

    /// Whether `entry` passes the filter.
    pub fn matches(&self, entry: &SftpDirectoryEntry) -> bool {
        self.matches_name(&entry.name)
    }
}
