//! Fills in the `%` tokens and `${VAR}` references that `ssh` allows in
//! `IdentityFile` and `CertificateFile` paths, such as `~/.ssh/%h.key`.

use super::paths::{expand_home_path, home_dir};
use std::collections::HashMap;

/// What the tokens of a key file path stand for.
pub(super) struct PathTokens<'a> {
    /// The name as it was typed, `%n`.
    pub alias: &'a str,
    /// The host connected to, `%h`.
    pub hostname: &'a str,
    /// `%p`.
    pub port: u16,
    /// The remote user, `%r`.
    pub user: &'a str,
    /// The local user, `%u`.
    pub local_user: &'a str,
}

/// The path with its tokens filled in, or the token tTerm cannot fill.
fn expand_tokens(value: &str, tokens: &PathTokens) -> Result<String, String> {
    let mut expanded = String::new();
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '%' => match chars.next() {
                Some('%') => expanded.push('%'),
                Some('d') => match home_dir() {
                    Some(home) => expanded.push_str(&home.to_string_lossy()),
                    None => return Err("%d".to_string()),
                },
                Some('h') => expanded.push_str(tokens.hostname),
                Some('n') => expanded.push_str(tokens.alias),
                Some('p') => expanded.push_str(&tokens.port.to_string()),
                Some('r') => expanded.push_str(tokens.user),
                Some('u') => expanded.push_str(tokens.local_user),
                Some(other) => return Err(format!("%{other}")),
                None => return Err("%".to_string()),
            },
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut name = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    name.push(next);
                }
                match std::env::var(&name) {
                    Ok(value) if closed => expanded.push_str(&value),
                    _ => return Err(format!("${{{name}}}")),
                }
            }
            _ => expanded.push(ch),
        }
    }
    Ok(expanded)
}

/// The first path of a key file option whose tokens tTerm can fill in, with
/// `~` expanded. A path it cannot fill is passed over with a warning.
pub(super) fn key_file_path(
    options: &HashMap<String, Vec<String>>,
    key: &str,
    tokens: &PathTokens,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let label = if key == "certificatefile" {
        "CertificateFile"
    } else {
        "IdentityFile"
    };
    for value in options.get(key).into_iter().flatten() {
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match expand_tokens(value, tokens) {
            Ok(path) => return Some(expand_home_path(&path).to_string_lossy().to_string()),
            Err(token) => warnings.push(format!(
                "{label} '{value}' was not imported because tTerm cannot fill in {token}."
            )),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKENS: PathTokens<'static> = PathTokens {
        alias: "web",
        hostname: "web.example.com",
        port: 2222,
        user: "deploy",
        local_user: "alice",
    };

    #[test]
    fn fills_in_the_tokens_ssh_allows() {
        assert_eq!(
            expand_tokens("keys/%n-%h-%p-%r-%u-100%%", &TOKENS).as_deref(),
            Ok("keys/web-web.example.com-2222-deploy-alice-100%")
        );
        let home = home_dir().expect("home directory");
        assert_eq!(
            expand_tokens("%d/key", &TOKENS),
            Ok(format!("{}/key", home.to_string_lossy()))
        );
    }

    #[test]
    fn reads_environment_variables() {
        let path = std::env::var("PATH").expect("PATH is set");
        assert_eq!(expand_tokens("${PATH}/x", &TOKENS), Ok(format!("{path}/x")));
        assert_eq!(
            expand_tokens("${TTERM_SURELY_UNSET_VARIABLE}/x", &TOKENS),
            Err("${TTERM_SURELY_UNSET_VARIABLE}".to_string())
        );
        assert_eq!(expand_tokens("${PATH", &TOKENS), Err("${PATH}".to_string()));
    }

    #[test]
    fn passes_over_a_path_with_a_token_it_cannot_fill() {
        let options = HashMap::from([(
            "identityfile".to_string(),
            vec!["~/.ssh/%C".to_string(), "/keys/%h".to_string()],
        )]);
        let mut warnings = Vec::new();

        let path = key_file_path(&options, "identityfile", &TOKENS, &mut warnings);

        assert_eq!(
            path,
            Some(
                expand_home_path("/keys/web.example.com")
                    .to_string_lossy()
                    .to_string()
            )
        );
        assert_eq!(
            warnings,
            ["IdentityFile '~/.ssh/%C' was not imported because tTerm cannot fill in %C."]
        );
    }
}
