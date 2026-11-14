//! Proxy summaries and form validation, from `src/lib/proxy.ts`.

use super::{ProxyKind, ProxyServer};
use crate::js::trim_js;
use serde::Serialize;

/// The first field of a proxy being edited that cannot be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProxyDraftError {
    Name,
    Host,
    Port,
    Command,
    CommandToken,
}

/// The editable fields of a proxy server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyDraft {
    pub name: String,
    pub kind: ProxyKind,
    pub host: String,
    pub port: i64,
    pub command: Option<String>,
}

impl From<&ProxyServer> for ProxyDraft {
    fn from(server: &ProxyServer) -> Self {
        Self {
            name: server.name.clone(),
            kind: server.kind,
            host: server.host.clone(),
            port: i64::from(server.port),
            command: server.command.clone(),
        }
    }
}

/// How a list names a server's protocol.
pub fn proxy_kind_label(kind: ProxyKind) -> &'static str {
    match kind {
        ProxyKind::Http => "HTTP",
        ProxyKind::Socks5 => "SOCKS5",
        ProxyKind::Command => "command",
    }
}

/// What a list shows under a proxy's name.
pub fn proxy_summary(server: &ProxyServer) -> String {
    if server.kind == ProxyKind::Command {
        return format!("$ {}", server.command.as_deref().unwrap_or(""));
    }
    format!(
        "{} {}",
        proxy_kind_label(server.kind),
        super::choices::proxy_address(server)
    )
}

/// Whether every `%` in a proxy command starts a token the app replaces.
pub fn has_only_known_proxy_tokens(command: &str) -> bool {
    let mut characters = command.chars();
    while let Some(character) = characters.next() {
        if character == '%' && !characters.next().is_some_and(|next| "%hnpr".contains(next)) {
            return false;
        }
    }
    true
}

/// The first field of a proxy being edited that cannot be saved.
pub fn validate_proxy_draft(draft: &ProxyDraft) -> Option<ProxyDraftError> {
    if trim_js(&draft.name).is_empty() {
        return Some(ProxyDraftError::Name);
    }
    if draft.kind == ProxyKind::Command {
        let command = draft.command.as_deref().map(trim_js).unwrap_or_default();
        if command.is_empty() {
            return Some(ProxyDraftError::Command);
        }
        return (!has_only_known_proxy_tokens(command)).then_some(ProxyDraftError::CommandToken);
    }
    let host = trim_js(&draft.host);
    if host.is_empty() || host.chars().any(crate::js::is_js_whitespace) {
        return Some(ProxyDraftError::Host);
    }
    if !(1..=65535).contains(&draft.port) {
        return Some(ProxyDraftError::Port);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        ProxyDraft, ProxyDraftError, has_only_known_proxy_tokens, proxy_summary,
        validate_proxy_draft,
    };
    use crate::proxy::choices::proxy_address;
    use crate::proxy::{ProxyKind, ProxyServer};

    #[test]
    fn brackets_ipv6_addresses() {
        let mut server = server(ProxyKind::Socks5);
        server.host = "127.0.0.1".to_owned();
        server.port = 7890;
        assert_eq!(proxy_address(&server), "127.0.0.1:7890");
        server.host = "::1".to_owned();
        server.port = 1080;
        assert_eq!(proxy_address(&server), "[::1]:1080");
    }

    fn server(kind: ProxyKind) -> ProxyServer {
        ProxyServer {
            id: "p".to_owned(),
            name: "P".to_owned(),
            kind,
            host: "::1".to_owned(),
            port: 1080,
            username: None,
            command: None,
        }
    }

    #[test]
    fn reports_the_first_field_that_cannot_be_saved() {
        let valid = ProxyDraft {
            name: "Office".to_owned(),
            kind: ProxyKind::Http,
            host: "proxy.corp".to_owned(),
            port: 8080,
            command: None,
        };
        assert_eq!(validate_proxy_draft(&valid), None);
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                name: " ".to_owned(),
                ..valid.clone()
            }),
            Some(ProxyDraftError::Name)
        );
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                host: "a b".to_owned(),
                ..valid.clone()
            }),
            Some(ProxyDraftError::Host)
        );
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                port: 0,
                ..valid.clone()
            }),
            Some(ProxyDraftError::Port)
        );
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                port: 70000,
                ..valid
            }),
            Some(ProxyDraftError::Port)
        );
    }

    #[test]
    fn checks_a_command_proxys_command_instead_of_its_address() {
        let command = ProxyDraft {
            name: "Cloudflare".to_owned(),
            kind: ProxyKind::Command,
            host: String::new(),
            port: 0,
            command: Some("cloudflared access ssh --hostname %h".to_owned()),
        };
        assert_eq!(validate_proxy_draft(&command), None);
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                command: Some(" ".to_owned()),
                ..command.clone()
            }),
            Some(ProxyDraftError::Command)
        );
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                command: Some("nc %h %p 100%%".to_owned()),
                ..command.clone()
            }),
            None
        );
        assert_eq!(
            validate_proxy_draft(&ProxyDraft {
                command: Some("nc %d".to_owned()),
                ..command.clone()
            }),
            Some(ProxyDraftError::CommandToken)
        );
        assert!(!has_only_known_proxy_tokens("nc 100%"));
    }

    #[test]
    fn summarizes_a_proxy_by_address_or_command() {
        let mut socks = server(ProxyKind::Socks5);
        assert_eq!(proxy_summary(&socks), "SOCKS5 [::1]:1080");
        socks.kind = ProxyKind::Command;
        socks.command = Some("nc %h %p".to_owned());
        assert_eq!(proxy_summary(&socks), "$ nc %h %p");
    }
}
