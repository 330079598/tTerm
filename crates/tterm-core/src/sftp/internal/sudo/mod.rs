//! The file manager's sudo mode: an SFTP connection whose channels work on
//! the host as root.
//!
//! sudo's credentials do not carry from one SSH channel to the next, so every
//! channel goes through sudo on its own: its command is `sudo -k`, which
//! never relies on credentials sudo cached, with the password written as the
//! first line of its standard input. The command run as root then prints
//! [`command::READY_MARKER`] before it hands over to `sftp-server` or `sh`,
//! and nothing is sent past sudo until that line arrives, so a password can
//! never be read as SFTP data and sudo's own messages never reach the
//! protocol. Both sudo 1.9 and sudo-rs read exactly the password line and
//! leave the rest of standard input to the command, without a tty.

pub(crate) mod command;
pub(crate) mod errors;
pub(crate) mod handshake;
#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod tests;

use zeroize::Zeroizing;

/// How sudo lets the account in, found once per connection.
pub enum SudoAuth {
    /// sudoers lets the account run commands without a password.
    NoPassword,
    /// sudo accepted this password when the connection was opened.
    Password(Zeroizing<String>),
}
