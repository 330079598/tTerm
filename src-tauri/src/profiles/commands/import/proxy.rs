//! The proxies an SSH config import connects hosts through. A server or
//! command already in the saved list is reused; a new one is added. Added
//! commands are not allowed on this device, so each asks before it first runs.

use crate::profiles::types::SshConfigImportProxy;
use crate::proxy::storage::{list_proxies, upsert_proxy};
use crate::proxy::{ProxyChoice, ProxyKind, ProxyServer};
use rusqlite::Connection;

pub(super) struct ImportedProxies {
    saved: Vec<ProxyServer>,
    added: Vec<ProxyServer>,
}

impl ImportedProxies {
    pub(super) fn load() -> Result<Self, String> {
        Ok(Self {
            saved: crate::db::read(list_proxies)?,
            added: Vec::new(),
        })
    }

    /// The choice of a host whose config sets `proxy`, adding the proxy when needed.
    pub(super) fn choice(&mut self, proxy: &SshConfigImportProxy) -> ProxyChoice {
        let id = match proxy {
            SshConfigImportProxy::Direct => return ProxyChoice::Direct,
            SshConfigImportProxy::Server { kind, host, port } => self.find_or_add(
                |server| {
                    server.kind == *kind
                        && server.host.eq_ignore_ascii_case(host)
                        && server.port == *port
                        && server.username.is_none()
                },
                || ProxyServer {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: format!("{} {}", kind.label(), address(host, *port)),
                    kind: *kind,
                    host: host.clone(),
                    port: *port,
                    username: None,
                    command: None,
                },
            ),
            SshConfigImportProxy::Command { command } => {
                let name = self.unique_name(program_name(command));
                self.find_or_add(
                    |server| server.command.as_deref() == Some(command.as_str()),
                    || ProxyServer {
                        id: uuid::Uuid::new_v4().to_string(),
                        name,
                        kind: ProxyKind::Command,
                        host: String::new(),
                        port: 0,
                        username: None,
                        command: Some(command.clone()),
                    },
                )
            }
        };
        ProxyChoice::Server { id }
    }

    /// Saves the proxies the import added.
    pub(super) fn save(&self, connection: &Connection) -> Result<(), String> {
        self.added
            .iter()
            .try_for_each(|server| upsert_proxy(connection, server))
    }

    fn find_or_add(
        &mut self,
        matches: impl Fn(&ProxyServer) -> bool,
        create: impl FnOnce() -> ProxyServer,
    ) -> String {
        if let Some(server) = self
            .saved
            .iter()
            .chain(&self.added)
            .find(|server| matches(server))
        {
            return server.id.clone();
        }
        let server = create();
        let id = server.id.clone();
        self.added.push(server);
        id
    }

    /// `base`, or `base 2`, `base 3`… when a saved proxy has that name.
    fn unique_name(&self, base: &str) -> String {
        let taken = |name: &str| {
            self.saved
                .iter()
                .chain(&self.added)
                .any(|server| server.name == name)
        };
        (1..)
            .map(|index| match index {
                1 => base.to_string(),
                _ => format!("{base} {index}"),
            })
            .find(|name| !taken(name))
            .unwrap_or_else(|| base.to_string())
    }
}

/// `host:port`, with an IPv6 address in brackets.
fn address(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// What a command proxy is named after: its program, without path or `.exe`.
fn program_name(command: &str) -> &str {
    let program = command.split_whitespace().next().unwrap_or("command");
    let program = program.trim_matches(['"', '\'']);
    let program = program.rsplit(['/', '\\']).next().unwrap_or(program);
    program.strip_suffix(".exe").unwrap_or(program)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxies() -> ImportedProxies {
        ImportedProxies {
            saved: vec![ProxyServer {
                id: "clash".to_string(),
                name: "cloudflared".to_string(),
                kind: ProxyKind::Socks5,
                host: "127.0.0.1".to_string(),
                port: 7890,
                username: None,
                command: None,
            }],
            added: Vec::new(),
        }
    }

    #[test]
    fn a_saved_server_is_reused_and_a_new_one_added_once() {
        let mut proxies = proxies();
        let clash = SshConfigImportProxy::Server {
            kind: ProxyKind::Socks5,
            host: "127.0.0.1".to_string(),
            port: 7890,
        };
        assert_eq!(
            proxies.choice(&clash),
            ProxyChoice::Server {
                id: "clash".to_string()
            }
        );
        let office = SshConfigImportProxy::Server {
            kind: ProxyKind::Http,
            host: "::1".to_string(),
            port: 8080,
        };
        let first = proxies.choice(&office);
        assert_eq!(proxies.choice(&office), first);
        assert_eq!(proxies.added.len(), 1);
        assert_eq!(proxies.added[0].name, "HTTP [::1]:8080");
        assert_eq!(
            proxies.choice(&SshConfigImportProxy::Direct),
            ProxyChoice::Direct
        );
    }

    #[test]
    fn a_command_is_named_after_its_program() {
        let mut proxies = proxies();
        let command = |text: &str| SshConfigImportProxy::Command {
            command: text.to_string(),
        };
        proxies.choice(&command(
            "/usr/local/bin/cloudflared access ssh --hostname %h",
        ));
        proxies.choice(&command("cloudflared access ssh --hostname %h.corp"));
        proxies.choice(&command(r"C:\Tools\nc.exe %h %p"));
        let names: Vec<_> = proxies
            .added
            .iter()
            .map(|server| server.name.as_str())
            .collect();
        assert_eq!(names, ["cloudflared 2", "cloudflared 3", "nc"]);
    }
}
