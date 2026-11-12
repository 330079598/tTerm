//! Matches the host names of a `known_hosts` entry the way `ssh` does in
//! `hostfile.c`.

use crate::profiles::parser::pattern::wildcard_matches;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::ssh_key::known_hosts::HostPatterns;
use sha1::Sha1;

/// How `ssh` names a host in `known_hosts`: on its own on port 22 and as
/// `[host]:port` on any other, in lower case.
pub fn host_key_name(host: &str, port: u16) -> String {
    let host = host.to_ascii_lowercase();
    if port == 22 {
        host
    } else {
        format!("[{host}]:{port}")
    }
}

/// Whether an entry names the host [`host_key_name`] names. A hashed entry
/// holds an HMAC-SHA1 of the name, and a matching `!pattern` rules the host
/// out.
pub fn patterns_match(patterns: &HostPatterns, name: &str) -> bool {
    match patterns {
        HostPatterns::HashedName { salt, hash } => {
            let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(salt) else {
                return false;
            };
            mac.update(name.as_bytes());
            mac.verify_slice(hash).is_ok()
        }
        HostPatterns::Patterns(patterns) => {
            let mut matched = false;
            for pattern in patterns {
                let pattern = pattern.to_ascii_lowercase();
                if let Some(negated) = pattern.strip_prefix('!') {
                    if wildcard_matches(negated, name) {
                        return false;
                    }
                } else if wildcard_matches(&pattern, name) {
                    matched = true;
                }
            }
            matched
        }
    }
}
