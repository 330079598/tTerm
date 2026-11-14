//! Values the proxy picker exchanges with storage, from `src/lib/proxy.ts`.

use super::{ProxyChoice, ProxyKind, ProxyServer};
use serde_json::Value;

/// The ports proxies of each kind usually listen on.
pub fn default_proxy_port(kind: ProxyKind) -> Option<u16> {
    match kind {
        ProxyKind::Http => Some(8080),
        ProxyKind::Socks5 => Some(1080),
        ProxyKind::Command => None,
    }
}

/// A stored proxy choice; anything unknown follows the default proxy.
pub fn normalize_proxy_choice(value: &Value) -> ProxyChoice {
    let Ok(ProxyChoice::Server { id }) = serde_json::from_value::<ProxyChoice>(value.clone())
    else {
        return serde_json::from_value::<ProxyChoice>(value.clone())
            .unwrap_or(ProxyChoice::Default);
    };
    if id.is_empty() {
        ProxyChoice::Default
    } else {
        ProxyChoice::Server { id }
    }
}

/// The choice to store or send, omitted when it follows the default proxy.
pub fn proxy_choice_payload(choice: &ProxyChoice) -> Option<&ProxyChoice> {
    (!choice.is_default()).then_some(choice)
}

/// A choice as the value of a `<select>` option.
pub fn proxy_choice_key(choice: &ProxyChoice) -> String {
    match choice {
        ProxyChoice::Default => "default".to_owned(),
        ProxyChoice::Direct => "direct".to_owned(),
        ProxyChoice::Server { id } => format!("server:{id}"),
    }
}

/// The choice a `<select>` option's value stands for.
pub fn parse_proxy_choice_key(key: &str) -> ProxyChoice {
    if key == "direct" {
        return ProxyChoice::Direct;
    }
    key.strip_prefix("server:")
        .filter(|id| !id.is_empty())
        .map(|id| ProxyChoice::Server { id: id.to_owned() })
        .unwrap_or(ProxyChoice::Default)
}

/// How many profiles connect through the server with `id`.
pub fn count_profiles_using_proxy(profiles: &[Value], id: &str) -> usize {
    profiles
        .iter()
        .filter(|profile| {
            let choice = profile
                .get("proxy")
                .map(normalize_proxy_choice)
                .unwrap_or(ProxyChoice::Default);
            matches!(choice, ProxyChoice::Server { id: chosen } if chosen == id)
        })
        .count()
}

/// `host:port`, with an IPv6 address in brackets.
pub fn proxy_address(server: &ProxyServer) -> String {
    if server.host.contains(':') {
        format!("[{}]:{}", server.host, server.port)
    } else {
        format!("{}:{}", server.host, server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        count_profiles_using_proxy, normalize_proxy_choice, parse_proxy_choice_key,
        proxy_choice_key, proxy_choice_payload,
    };
    use crate::proxy::ProxyChoice;
    use serde_json::json;

    #[test]
    fn treats_anything_unknown_as_the_default_proxy() {
        assert_eq!(normalize_proxy_choice(&json!(null)), ProxyChoice::Default);
        assert_eq!(
            normalize_proxy_choice(&json!({ "mode": "server" })),
            ProxyChoice::Default
        );
        assert_eq!(
            normalize_proxy_choice(&json!({ "mode": "direct" })),
            ProxyChoice::Direct
        );
        assert_eq!(
            normalize_proxy_choice(&json!({ "mode": "server", "id": "p1" })),
            ProxyChoice::Server {
                id: "p1".to_owned()
            }
        );
    }

    #[test]
    fn round_trips_through_select_values() {
        for choice in [
            ProxyChoice::Default,
            ProxyChoice::Direct,
            ProxyChoice::Server {
                id: "a:b".to_owned(),
            },
        ] {
            assert_eq!(parse_proxy_choice_key(&proxy_choice_key(&choice)), choice);
        }
        assert_eq!(parse_proxy_choice_key("server:"), ProxyChoice::Default);
    }

    #[test]
    fn leaves_the_default_out_of_what_is_stored() {
        assert_eq!(proxy_choice_payload(&ProxyChoice::Default), None);
        assert_eq!(
            proxy_choice_payload(&ProxyChoice::Direct),
            Some(&ProxyChoice::Direct)
        );
    }

    #[test]
    fn counts_profiles_that_chose_a_server() {
        let profiles = [
            json!({ "proxy": { "mode": "server", "id": "p1" } }),
            json!({ "proxy": { "mode": "server", "id": "p2" } }),
            json!({ "proxy": { "mode": "direct" } }),
            json!({}),
        ];
        assert_eq!(count_profiles_using_proxy(&profiles, "p1"), 1);
        assert_eq!(count_profiles_using_proxy(&profiles, "p3"), 0);
    }
}
