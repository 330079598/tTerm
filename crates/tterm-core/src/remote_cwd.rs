//! Per-tab directories and hosts reported by remote shells.
//!
//! The frontend exposes React hooks on top of a module-level store. The native
//! client has no React, so the store itself and its subscriptions are ported;
//! the two hooks become view state in the workspace card.

use std::collections::HashMap;

/// The directory each SSH tab's remote shell last reported, with its away
/// host and connected hostname.
#[derive(Default)]
pub struct RemoteCwdStore {
    cwds: HashMap<String, String>,
    away_hosts: HashMap<String, String>,
    hostnames: HashMap<String, String>,
    listeners: Vec<(u64, Box<dyn Fn()>)>,
    next_listener_id: u64,
}

impl RemoteCwdStore {
    /// A store with no tabs.
    pub fn new() -> Self {
        Self::default()
    }

    /// The tab's shell reported `cwd`, or `None` once its session is gone.
    ///
    /// Returns whether subscribers were notified: an unchanged directory and
    /// deleting a directory that was not present do not notify.
    pub fn set_tab_remote_cwd(&mut self, tab_id: &str, cwd: Option<&str>) -> bool {
        let changed = match cwd {
            None => self.cwds.remove(tab_id).is_some(),
            Some(cwd) => {
                if self.cwds.get(tab_id).map(String::as_str) == Some(cwd) {
                    false
                } else {
                    self.cwds.insert(tab_id.to_owned(), cwd.to_owned());
                    true
                }
            }
        };
        if changed {
            self.notify();
        }
        changed
    }

    /// The tab's last reported directory.
    pub fn get_tab_remote_cwd(&self, tab_id: &str) -> Option<&str> {
        self.cwds.get(tab_id).map(String::as_str)
    }

    /// The tab's terminal reports from `host`, not the host it connected to,
    /// or `None` once it is back there or its session is gone.
    pub fn set_tab_remote_away_host(&mut self, tab_id: &str, host: Option<&str>) -> bool {
        let changed = match host {
            None => self.away_hosts.remove(tab_id).is_some(),
            Some(host) => {
                if self.away_hosts.get(tab_id).map(String::as_str) == Some(host) {
                    false
                } else {
                    self.away_hosts.insert(tab_id.to_owned(), host.to_owned());
                    true
                }
            }
        };
        if changed {
            self.notify();
        }
        changed
    }

    /// The other host the tab's terminal is on.
    pub fn get_tab_remote_away_host(&self, tab_id: &str) -> Option<&str> {
        self.away_hosts.get(tab_id).map(String::as_str)
    }

    /// Sets the connected host's name; this is not rendered, so it does not
    /// notify subscribers.
    pub fn set_tab_remote_hostname(&mut self, tab_id: &str, hostname: Option<&str>) {
        match hostname {
            None => {
                self.hostnames.remove(tab_id);
            }
            Some(hostname) => {
                self.hostnames
                    .insert(tab_id.to_owned(), hostname.to_owned());
            }
        }
    }

    /// The connected host's name, when the shell integration read it.
    pub fn get_tab_remote_hostname(&self, tab_id: &str) -> Option<&str> {
        self.hostnames.get(tab_id).map(String::as_str)
    }

    /// Registers a listener for directory and away-host changes.
    pub fn subscribe(&mut self, listener: impl Fn() + 'static) -> u64 {
        let id = self.next_listener_id;
        self.next_listener_id += 1;
        self.listeners.push((id, Box::new(listener)));
        id
    }

    /// Removes a listener registered by [`RemoteCwdStore::subscribe`].
    pub fn unsubscribe(&mut self, id: u64) {
        self.listeners.retain(|(listener_id, _)| *listener_id != id);
    }

    fn notify(&self) {
        for (_, listener) in &self.listeners {
            listener();
        }
    }
}

#[cfg(test)]
mod tests;
