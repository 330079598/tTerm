//! Working-directory reports from tTerm's shell integration.
//!
//! OSC 7 `file://host/path` comes from Git Bash and WSL; OSC 9;9 `"path"`
//! comes from cmd and PowerShell.

use std::sync::LazyLock;

use regress::Regex;

use crate::js_encoding::decode_uri_component;
use crate::js_text::{from_utf16_units_lossy, js_trim, utf16_slice, utf16_units};

/// OSC codes that can carry a working-directory report.
pub const CWD_REPORT_OSC_CODES: [i64; 2] = [7, 9];

static OSC7_REPORT: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^file://([^/]*)(/.*)$").ok());
static WINDOWS_DRIVE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^/[A-Za-z]:([/\\]|$)").ok());

/// The host and directory in an OSC 7 payload, or `None` when it is not one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Osc7Report {
    /// The host, lower case, and empty when the shell left it out.
    pub host: String,
    /// The decoded directory.
    pub path: String,
}

/// The directory in an OSC 7 or OSC 9;9 payload, or `None` when the payload
/// is not one. OSC 7 from another host is ignored: tTerm's own scripts send
/// an empty host.
pub fn parse_cwd_report(osc: i64, data: &str) -> Option<String> {
    if osc == 7 {
        let report = parse_osc7_report(data)?;
        return is_unnamed_host(&report.host).then_some(report.path);
    }
    if osc == 9 {
        return parse_osc9(data);
    }
    None
}

/// The host and directory in an OSC 7 payload, or `None` when it is not one.
pub fn parse_osc7_report(data: &str) -> Option<Osc7Report> {
    let units = utf16_units(data);
    let found = OSC7_REPORT.as_ref()?.find_from_ucs2(&units, 0).next()?;
    let host = from_utf16_units_lossy(&units[found.group(1)?]).to_lowercase();

    let raw_path = from_utf16_units_lossy(&units[found.group(2)?]);
    let mut path = decode_uri_component(&raw_path).unwrap_or(raw_path);
    if WINDOWS_DRIVE
        .as_ref()
        .and_then(|pattern| pattern.find_from_ucs2(&utf16_units(&path), 0).next())
        .is_some()
    {
        path = utf16_slice(&path, 1, usize::MAX);
    }
    Some(Osc7Report { host, path })
}

/// An OSC 7 host that names no machine: tTerm's scripts leave it out, and
/// `localhost` is whichever machine the shell runs on.
pub fn is_unnamed_host(host: &str) -> bool {
    host.is_empty() || host.to_lowercase() == "localhost"
}

/// Two names for the same host, comparing only the first label.
pub fn is_same_host(first: &str, second: &str) -> bool {
    fn short_name(host: &str) -> String {
        host.to_lowercase()
            .split('.')
            .next()
            .unwrap_or_default()
            .to_owned()
    }
    short_name(first) == short_name(second)
}

fn parse_osc9(data: &str) -> Option<String> {
    let path = data.strip_prefix("9;")?;
    let mut path = path;
    if crate::js_text::utf16_len(path) >= 2 && path.starts_with('"') && path.ends_with('"') {
        path = &path[1..path.len() - 1];
    }
    (!js_trim(path).is_empty()).then(|| path.to_owned())
}

#[cfg(test)]
mod tests;
