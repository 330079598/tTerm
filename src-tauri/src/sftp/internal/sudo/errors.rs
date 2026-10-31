//! What to tell the user when sudo will not let a connection work as root.
//!
//! sudo 1.9 and sudo-rs word their refusals differently ("Sorry, user dave
//! may not run sudo" against "I'm sorry dave. I'm afraid I can't do that"),
//! so a refusal is told apart by the step it happened at, and sudo's own
//! words go along in the message for the cases no pattern here knows.

use super::command::MISSING_SFTP_SERVER;

/// The exit status a shell gives a command it cannot find.
const NOT_FOUND_STATUS: u32 = 127;

/// sudo asks again after a wrong password: "Sorry, try again." in sudo 1.9,
/// "Authentication failed, try again." in sudo-rs.
pub fn is_wrong_password(stderr: &str) -> bool {
    stderr.contains("try again") || stderr.contains("incorrect password")
}

/// sudoers says `requiretty`, which an SSH command channel cannot meet.
fn needs_tty(stderr: &str) -> bool {
    stderr.contains("must have a tty")
}

/// sudo's message, or its exit status when it said nothing.
fn detail(exit_status: Option<u32>, stderr: &str) -> String {
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    exit_status.map_or_else(
        || "it gave no reason".to_string(),
        |code| format!("exit status {code}"),
    )
}

fn tty_required() -> String {
    "sudo on this host only runs from a terminal (requiretty), so the file manager cannot \
     work as root here. Ask the administrator to remove requiretty for this account."
        .to_string()
}

/// Why the passwordless probe means the connection cannot go on, or None
/// when sudo merely wants a password.
pub fn probe_failed(exit_status: Option<u32>, stderr: &str) -> Option<String> {
    if needs_tty(stderr) {
        return Some(tty_required());
    }
    if exit_status == Some(NOT_FOUND_STATUS) {
        return Some(
            "sudo is not installed on this host, so the file manager cannot work as root."
                .to_string(),
        );
    }
    None
}

/// Why sudo turned down the password it was given.
pub fn validation_failed(exit_status: Option<u32>, stderr: &str) -> String {
    if needs_tty(stderr) {
        return tty_required();
    }
    if is_wrong_password(stderr) {
        return wrong_password();
    }
    format!(
        "sudo would not let this account work as root: {}",
        detail(exit_status, stderr)
    )
}

pub fn wrong_password() -> String {
    "sudo did not accept the saved password. Correct the connection's sudo password and try \
     again."
        .to_string()
}

/// The script's own line saying no `sftp-server` was found. sudo 1.9 quotes
/// a command it refuses in full, script included, so a mention of the line
/// inside another one is sudo's refusal, not the script's.
fn says_sftp_server_missing(stderr: &str) -> bool {
    stderr
        .lines()
        .any(|line| line.trim() == MISSING_SFTP_SERVER)
}

/// Why a channel ended before the command run as root said it was ready.
pub fn elevation_failed(exit_status: Option<u32>, stderr: &str) -> String {
    if says_sftp_server_missing(stderr) {
        return "This host has no sftp-server program, which the file manager needs to work as \
                root. Its SSH server may only offer the built-in internal-sftp."
            .to_string();
    }
    if needs_tty(stderr) {
        return tty_required();
    }
    if is_wrong_password(stderr) {
        return wrong_password();
    }
    format!(
        "sudo would not start the file manager as root, which happens when sudoers lets this \
         account run only some commands: {}",
        detail(exit_status, stderr)
    )
}

/// The account needs a password for sudo and none is saved.
pub fn no_saved_password() -> String {
    "sudo needs a password for this account. Save it as the connection's sudo password, or \
     as its login password, and try again."
        .to_string()
}
