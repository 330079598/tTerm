//! OpenSSH wildcard patterns, where `*` stands for any run of characters and
//! `?` for exactly one, as `Include` paths, `Host` lines and `known_hosts`
//! entries use them.

pub fn has_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?')
}

pub fn wildcard_matches(pattern: &str, value: &str) -> bool {
    fn matches(pattern: &[char], value: &[char]) -> bool {
        if pattern.is_empty() {
            return value.is_empty();
        }

        match pattern[0] {
            '*' => {
                matches(&pattern[1..], value)
                    || (!value.is_empty() && matches(pattern, &value[1..]))
            }
            '?' => !value.is_empty() && matches(&pattern[1..], &value[1..]),
            ch => !value.is_empty() && ch == value[0] && matches(&pattern[1..], &value[1..]),
        }
    }

    matches(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
    )
}

/// Whether a `Host` pattern stands for more than one host, or negates one.
pub fn is_wildcard_host(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('!')
}
