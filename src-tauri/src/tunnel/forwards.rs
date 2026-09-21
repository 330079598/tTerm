//! OpenSSH `LocalForward` / `RemoteForward` / `DynamicForward` support for
//! importing an ssh_config: parsing their values and turning them into rules.

use super::storage::{load_tunnels_from_disk, write_tunnels_to_disk};
use super::types::{TunnelKind, TunnelRule};
use serde::Serialize;

/// One forward found in an ssh_config, before it is bound to a saved host.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForwardSpec {
    pub kind: TunnelKind,
    pub bind_host: String,
    pub bind_port: u16,
    pub dest_host: String,
    pub dest_port: u16,
}

/// Where OpenSSH listens when no bind address is given (loopback only).
const DEFAULT_BIND_HOST: &str = "localhost";

fn parse_port(text: &str) -> Result<u16, String> {
    match text.trim().parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(format!("'{text}' is not a port from 1 to 65535")),
    }
}

/// Splits `port`, `host:port`, `[v6]:port` or `host/port` into its parts.
fn split_endpoint(text: &str) -> Result<(Option<String>, u16), String> {
    if let Some(rest) = text.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| format!("'{text}' has an unclosed '['"))?;
        let port = after
            .strip_prefix(':')
            .ok_or_else(|| format!("'{text}' is missing a port"))?;
        return Ok((Some(host.to_string()), parse_port(port)?));
    }
    if let Some((host, port)) = text.rsplit_once('/') {
        return Ok((Some(host.to_string()), parse_port(port)?));
    }
    match text.rsplit_once(':') {
        Some((host, port)) => Ok((Some(host.to_string()), parse_port(port)?)),
        None => Ok((None, parse_port(text)?)),
    }
}

fn bind_host(host: Option<String>) -> String {
    match host.as_deref().map(str::trim) {
        None => DEFAULT_BIND_HOST.to_string(),
        Some("") => "*".to_string(),
        Some(host) => host.to_string(),
    }
}

/// Parses the value of one forward directive.
///
/// - `LocalForward` / `RemoteForward`: `[bind_address:]port host:hostport`
/// - `DynamicForward`: `[bind_address:]port`
pub fn parse_forward(kind: TunnelKind, value: &str) -> Result<ForwardSpec, String> {
    let words: Vec<&str> = value.split_whitespace().collect();
    match (kind, words.as_slice()) {
        (TunnelKind::Dynamic, [listen]) => {
            let (host, port) = split_endpoint(listen)?;
            Ok(ForwardSpec {
                kind,
                bind_host: bind_host(host),
                bind_port: port,
                dest_host: String::new(),
                dest_port: 0,
            })
        }
        (TunnelKind::Dynamic, _) => Err("expected [bind_address:]port".to_string()),
        (_, [listen, target]) => {
            let (host, port) = split_endpoint(listen)?;
            let (dest_host, dest_port) = split_endpoint(target)?;
            let dest_host = dest_host
                .filter(|host| !host.trim().is_empty())
                .ok_or_else(|| format!("'{target}' needs a host and a port"))?;
            Ok(ForwardSpec {
                kind,
                bind_host: bind_host(host),
                bind_port: port,
                dest_host,
                dest_port,
            })
        }
        _ => Err("expected [bind_address:]port host:hostport".to_string()),
    }
}

fn rule_name(profile_name: &str, spec: &ForwardSpec) -> String {
    match spec.kind {
        TunnelKind::Dynamic => format!("{profile_name} · SOCKS {}", spec.bind_port),
        _ => format!("{profile_name} · {}:{}", spec.dest_host, spec.dest_port),
    }
}

pub fn rule_from_spec(profile_id: &str, profile_name: &str, spec: &ForwardSpec) -> TunnelRule {
    TunnelRule {
        id: format!("tunnel-{}", uuid::Uuid::new_v4()),
        name: rule_name(profile_name, spec),
        profile_id: profile_id.to_string(),
        kind: spec.kind,
        bind_host: spec.bind_host.clone(),
        bind_port: spec.bind_port,
        dest_host: spec.dest_host.clone(),
        dest_port: spec.dest_port,
        auto_start: false,
    }
}

fn is_same_forward(a: &TunnelRule, b: &TunnelRule) -> bool {
    a.profile_id == b.profile_id
        && a.kind == b.kind
        && a.bind_host == b.bind_host
        && a.bind_port == b.bind_port
        && a.dest_host == b.dest_host
        && a.dest_port == b.dest_port
}

/// Adds the rules that are not already present, so importing the same
/// ssh_config twice does not duplicate anything. Returns `(added, skipped)`.
fn merge_new_rules(existing: &mut Vec<TunnelRule>, candidates: Vec<TunnelRule>) -> (usize, usize) {
    let (mut added, mut skipped) = (0, 0);
    for mut candidate in candidates {
        let duplicate = existing
            .iter()
            .any(|rule| is_same_forward(rule, &candidate));
        if duplicate || candidate.validate().is_err() {
            skipped += 1;
        } else {
            existing.push(candidate);
            added += 1;
        }
    }
    (added, skipped)
}

/// Saves imported rules next to the existing ones. Returns `(added, skipped)`.
pub(crate) fn add_rules(candidates: Vec<TunnelRule>) -> Result<(usize, usize), String> {
    if candidates.is_empty() {
        return Ok((0, 0));
    }
    let mut rules = load_tunnels_from_disk()?;
    let counts = merge_new_rules(&mut rules, candidates);
    if counts.0 > 0 {
        write_tunnels_to_disk(&rules)?;
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: TunnelKind, value: &str) -> ForwardSpec {
        parse_forward(kind, value).unwrap_or_else(|e| panic!("{value}: {e}"))
    }

    #[test]
    fn local_forward_defaults_to_loopback() {
        let s = spec(TunnelKind::Local, "5432 db.internal:5432");
        assert_eq!(s.bind_host, "localhost");
        assert_eq!(
            (s.bind_port, s.dest_host.as_str(), s.dest_port),
            (5432, "db.internal", 5432)
        );
    }

    #[test]
    fn bind_addresses_are_parsed_in_all_spellings() {
        let s = spec(TunnelKind::Local, "0.0.0.0:8080 web:80");
        assert_eq!((s.bind_host.as_str(), s.bind_port), ("0.0.0.0", 8080));

        let s = spec(TunnelKind::Local, "[::1]:8080 [fd00::5]:80");
        assert_eq!(
            (s.bind_host.as_str(), s.dest_host.as_str()),
            ("::1", "fd00::5")
        );

        let s = spec(TunnelKind::Local, "*:8080 web/80");
        assert_eq!((s.bind_host.as_str(), s.dest_host.as_str()), ("*", "web"));
    }

    #[test]
    fn dynamic_forward_takes_only_a_listen_address() {
        let s = spec(TunnelKind::Dynamic, "1080");
        assert_eq!(
            (s.bind_host.as_str(), s.bind_port, s.dest_port),
            ("localhost", 1080, 0)
        );
        let s = spec(TunnelKind::Dynamic, "0.0.0.0:1080");
        assert_eq!(s.bind_host, "0.0.0.0");
        assert!(parse_forward(TunnelKind::Dynamic, "1080 host:80").is_err());
    }

    #[test]
    fn remote_forward_parses_like_local() {
        let s = spec(TunnelKind::Remote, "9000 localhost:3000");
        assert_eq!(s.kind, TunnelKind::Remote);
        assert_eq!(
            (s.bind_port, s.dest_host.as_str(), s.dest_port),
            (9000, "localhost", 3000)
        );
    }

    #[test]
    fn malformed_forwards_are_rejected() {
        for bad in [
            "",
            "5432",
            "abc db:5432",
            "5432 db",
            "5432 db:0",
            "0 db:80",
            "5432 db:70000",
        ] {
            assert!(parse_forward(TunnelKind::Local, bad).is_err(), "{bad:?}");
        }
        assert!(parse_forward(TunnelKind::Remote, "[::1 host:80").is_err());
    }

    #[test]
    fn rules_are_named_after_their_destination() {
        let local = rule_from_spec("p1", "prod", &spec(TunnelKind::Local, "5432 db:5432"));
        assert_eq!(local.name, "prod · db:5432");
        assert!(!local.auto_start);
        let socks = rule_from_spec("p1", "prod", &spec(TunnelKind::Dynamic, "1080"));
        assert_eq!(socks.name, "prod · SOCKS 1080");
    }

    #[test]
    fn merging_twice_adds_nothing_new() {
        let make = || {
            vec![
                rule_from_spec("p1", "prod", &spec(TunnelKind::Local, "5432 db:5432")),
                rule_from_spec("p1", "prod", &spec(TunnelKind::Dynamic, "1080")),
            ]
        };
        let mut existing = Vec::new();
        assert_eq!(merge_new_rules(&mut existing, make()), (2, 0));
        assert_eq!(merge_new_rules(&mut existing, make()), (0, 2));
        assert_eq!(existing.len(), 2);

        // The same forward on another host is a different rule.
        let other = vec![rule_from_spec(
            "p2",
            "stage",
            &spec(TunnelKind::Dynamic, "1080"),
        )];
        assert_eq!(merge_new_rules(&mut existing, other), (1, 0));
    }
}
