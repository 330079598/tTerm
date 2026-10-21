//! Matches file names the way the drawer's filter does
//! (`src/components/SftpDrawer/sftpSearch.ts`), so a search of subfolders
//! finds what the filter would show in each folder: plain text matches
//! anywhere in the name, `*`, `?` and `[...]` make a glob over the whole
//! name, and the regex option takes a regular expression, falling back to a
//! glob when it is not one. Every kind ignores case.

use regex::{Regex, RegexBuilder};

pub enum NameMatcher {
    /// Lowercased.
    Substring(String),
    Glob {
        glob: String,
        pattern: Regex,
    },
    Regex(Regex),
}

fn has_glob_syntax(query: &str) -> bool {
    query.contains(['*', '?', '['])
}

fn escape_class_content(content: &str) -> String {
    // A Rust class also gives meaning to `[`, `&&` and `~~`, which a glob
    // class means literally.
    content
        .chars()
        .map(|char| match char {
            '\\' | ']' | '[' | '&' | '~' => format!("\\{char}"),
            _ => char.to_string(),
        })
        .collect()
}

fn glob_to_regex_source(glob: &str) -> String {
    let chars = glob.chars().collect::<Vec<_>>();
    let mut source = String::from("^");
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '*' => source.push_str(".*"),
            '?' => source.push('.'),
            '[' => match chars[index + 1..].iter().position(|&char| char == ']') {
                None => source.push_str("\\["),
                Some(0) => {
                    source.push_str("\\[\\]");
                    index += 1;
                }
                Some(length) => {
                    let content = chars[index + 1..index + 1 + length]
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
            char => source.push_str(&regex::escape(&char.to_string())),
        }
        index += 1;
    }
    source.push('$');
    source
}

fn case_insensitive(source: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(source).case_insensitive(true).build()
}

fn glob_matcher(glob: &str) -> Result<NameMatcher, String> {
    case_insensitive(&glob_to_regex_source(glob))
        .map(|pattern| NameMatcher::Glob {
            glob: glob.to_string(),
            pattern,
        })
        .map_err(|err| format!("Invalid pattern: {err}"))
}

impl NameMatcher {
    pub fn new(query: &str, regex: bool) -> Result<Self, String> {
        let query = query.trim();
        if query.is_empty() {
            return Err("Enter a name to search for".to_string());
        }
        if !regex {
            return if has_glob_syntax(query) {
                glob_matcher(query)
            } else {
                Ok(Self::Substring(query.to_lowercase()))
            };
        }
        match case_insensitive(query) {
            Ok(pattern) => Ok(Self::Regex(pattern)),
            Err(_) if has_glob_syntax(query) => glob_matcher(query),
            Err(err) => Err(format!("Invalid regular expression: {err}")),
        }
    }

    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::Substring(query) => name.to_lowercase().contains(query.as_str()),
            Self::Glob { pattern, .. } | Self::Regex(pattern) => pattern.is_match(name),
        }
    }

    /// A pattern for `find -iname` that keeps every name this matcher
    /// matches, so fewer names cross the network; the matcher still checks
    /// each one. None when `find`'s idea of the pattern could differ: its
    /// case folding covers ASCII only, and it reads `\` and class edge cases
    /// its own way.
    pub fn find_name_pattern(&self) -> Option<String> {
        match self {
            Self::Substring(query) if query.is_ascii() && !query.contains('\\') => {
                Some(format!("*{query}*"))
            }
            Self::Glob { glob, .. }
                if glob.is_ascii() && !glob.contains('\\') && !glob.contains('[') =>
            {
                Some(glob.clone())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matcher(query: &str, regex: bool) -> NameMatcher {
        NameMatcher::new(query, regex).unwrap()
    }

    #[test]
    fn plain_text_matches_anywhere_ignoring_case() {
        let text = matcher(" Log ", false);
        assert!(text.matches("app.LOG"));
        assert!(text.matches("catalog"));
        assert!(!text.matches("app.txt"));
        assert_eq!(text.find_name_pattern().as_deref(), Some("*log*"));
    }

    #[test]
    fn globs_match_the_whole_name() {
        let glob = matcher("*.rs", false);
        assert!(glob.matches("main.RS"));
        assert!(!glob.matches("main.rs.bak"));
        assert_eq!(glob.find_name_pattern().as_deref(), Some("*.rs"));

        let one = matcher("a?c", false);
        assert!(one.matches("abc"));
        assert!(!one.matches("abbc"));
    }

    #[test]
    fn glob_classes_follow_the_drawer_filter() {
        let class = matcher("file[0-9].txt", false);
        assert!(class.matches("file7.txt"));
        assert!(!class.matches("filex.txt"));
        assert!(class.find_name_pattern().is_none());

        let negated = matcher("[!a]*", false);
        assert!(negated.matches("beta"));
        assert!(!negated.matches("alpha"));

        // An unclosed or empty class is literal text.
        assert!(matcher("a[b", false).matches("a[b"));
        assert!(matcher("x[]*", false).matches("x[]y"));
        // Characters a Rust class treats specially stay literal.
        assert!(matcher("[&~]*", false).matches("&x"));
    }

    #[test]
    fn regex_option_takes_a_regular_expression() {
        let regex = matcher("^error_\\d+$", true);
        assert!(regex.matches("ERROR_42"));
        assert!(!regex.matches("error_x"));
        assert!(regex.find_name_pattern().is_none());
    }

    #[test]
    fn invalid_regex_falls_back_to_glob_or_fails() {
        assert!(matcher("*.log", true).matches("a.log"));
        assert!(NameMatcher::new("(", true).is_err());
        assert!(NameMatcher::new("  ", false).is_err());
    }

    #[test]
    fn leaves_non_ascii_and_backslashes_to_the_matcher() {
        assert!(matcher("Ä", false).find_name_pattern().is_none());
        assert!(matcher("a\\b", false).find_name_pattern().is_none());
        assert!(matcher("Ä", false).matches("ä.txt"));
    }
}
