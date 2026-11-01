//! Fills in the `%` tokens and `${VAR}` references that `ssh` allows in
//! `IdentityFile` and `CertificateFile` paths, such as `~/.ssh/%h.key`.

use super::paths::{expand_home_path, home_dir};
use sha1::{Digest, Sha1};
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
    /// The name of this machine, `%l`, whose first label is `%L`. Empty when
    /// it cannot be found.
    pub local_host: &'a str,
    /// The host of the last jump host as `ProxyJump` writes it, `%j`, or
    /// empty without one.
    pub jump_host: &'a str,
}

/// The name of this machine as `gethostname` gives it, which `ssh` uses for
/// `%l`, or empty when it cannot be found.
#[cfg(unix)]
pub(super) fn local_host_name() -> String {
    let mut buffer = [0u8; 256];
    // SAFETY: the buffer is valid for writes of its whole length, which is
    // the length passed.
    let result = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    if result != 0 {
        return String::new();
    }
    let end = buffer
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

/// The DNS name of this machine, which `gethostname` gives the Windows port
/// of OpenSSH for `%l`, or empty when it cannot be found.
#[cfg(windows)]
pub(super) fn local_host_name() -> String {
    use windows::core::PWSTR;
    use windows::Win32::System::SystemInformation::{ComputerNameDnsHostname, GetComputerNameExW};

    let mut buffer = [0u16; 256];
    let mut size = buffer.len() as u32;
    // SAFETY: the buffer is valid for writes of `size` UTF-16 units, and
    // `size` lives across the call.
    let result = unsafe {
        GetComputerNameExW(
            ComputerNameDnsHostname,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut size,
        )
    };
    if result.is_err() {
        return String::new();
    }
    let end = (size as usize).min(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// The local user id, `%i`.
#[cfg(unix)]
fn local_user_id() -> Option<String> {
    // SAFETY: getuid has no preconditions and cannot fail.
    Some(unsafe { libc::getuid() }.to_string())
}

/// Windows has no user id for `%i`.
#[cfg(not(unix))]
fn local_user_id() -> Option<String> {
    None
}

/// `%C`: the SHA-1 of `%l%h%p%r%j` in hex, as `ssh_connection_hash` makes it.
fn connection_hash(tokens: &PathTokens) -> String {
    let mut hasher = Sha1::new();
    for part in [
        tokens.local_host,
        tokens.hostname,
        &tokens.port.to_string(),
        tokens.user,
        tokens.jump_host,
    ] {
        hasher.update(part.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The path with its tokens filled in, or the token tTerm cannot fill.
pub(super) fn expand_tokens(value: &str, tokens: &PathTokens) -> Result<String, String> {
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
                Some('j') => expanded.push_str(tokens.jump_host),
                // tTerm reads no `HostKeyAlias`, which would come first.
                Some('k') => expanded.push_str(tokens.alias),
                Some('l') if !tokens.local_host.is_empty() => expanded.push_str(tokens.local_host),
                Some('L') if !tokens.local_host.is_empty() => {
                    expanded.push_str(tokens.local_host.split('.').next().unwrap_or_default());
                }
                Some('C') if !tokens.local_host.is_empty() => {
                    expanded.push_str(&connection_hash(tokens));
                }
                Some('i') => match local_user_id() {
                    Some(id) => expanded.push_str(&id),
                    None => return Err("%i".to_string()),
                },
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
        local_host: "laptop.example.com",
        jump_host: "bastion",
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
    fn fills_in_the_local_host_the_jump_host_and_the_connection_hash() {
        assert_eq!(
            expand_tokens("%l %L %j %k", &TOKENS).as_deref(),
            Ok("laptop.example.com laptop bastion web")
        );
        // The SHA-1 of "laptop.example.com", "web.example.com", "2222",
        // "deploy" and "bastion" run together.
        assert_eq!(
            expand_tokens("~/.ssh/%C", &TOKENS).as_deref(),
            Ok("~/.ssh/404bf006ff81540c1716033779084742c4d7eb2b")
        );
        let unknown_host = PathTokens {
            local_host: "",
            ..TOKENS
        };
        assert_eq!(expand_tokens("%C", &unknown_host), Err("%C".to_string()));
        #[cfg(unix)]
        assert!(expand_tokens("%i", &TOKENS).is_ok_and(|id| id.parse::<u32>().is_ok()));
        #[cfg(windows)]
        assert_eq!(expand_tokens("%i", &TOKENS), Err("%i".to_string()));
    }

    #[test]
    fn finds_the_name_of_this_machine() {
        let name = local_host_name();
        assert!(!name.is_empty());
        assert!(!name.contains('\0'), "{name:?}");
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
            vec!["~/.ssh/%Z".to_string(), "/keys/%h".to_string()],
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
            ["IdentityFile '~/.ssh/%Z' was not imported because tTerm cannot fill in %Z."]
        );
    }
}
