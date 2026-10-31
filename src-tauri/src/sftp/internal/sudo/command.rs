//! The commands a sudo-mode connection runs on its host.
//!
//! They go through the account's login shell, which may be bash, zsh, fish
//! or csh, so each is one line whose only quoting is single quotes around
//! plain text: no single quotes, `!` or doubled backslashes inside, which
//! fish and csh would read differently from a POSIX shell. The script inside
//! then runs under `sh`.

use super::SudoAuth;

/// The line the command run as root prints once sudo let it in, before it
/// becomes the program the channel is for.
pub const READY_MARKER: &str = "tterm-sudo-ready";

/// What the script says on standard error when no `sftp-server` program is
/// found, so that the failure can be told from sudo's own.
pub const MISSING_SFTP_SERVER: &str = "tterm: no sftp-server program found";

/// Succeeds only when sudoers lets the account in without a password: `-k`
/// ignores credentials sudo cached, and `-n` fails rather than ask.
pub const PROBE: &str = "sudo -k -n true";

/// Checks the password on standard input, without running anything.
pub const VALIDATE: &str = "sudo -k -S -p '' -v";

/// Where distributions install OpenSSH's `sftp-server`, tried after the path
/// the host's `sshd_config` names: Debian and Ubuntu, Fedora and RHEL,
/// Arch and Alpine, openSUSE, macOS and the BSDs.
const SFTP_SERVER_PATHS: &[&str] = &[
    "/usr/lib/openssh/sftp-server",
    "/usr/libexec/openssh/sftp-server",
    "/usr/lib/ssh/sftp-server",
    "/usr/libexec/ssh/sftp-server",
    "/usr/libexec/sftp-server",
    "/usr/lib/sftp-server",
];

/// sudo's options for the account: never ask, or read the password from
/// standard input without printing a prompt.
fn sudo(auth: &SudoAuth) -> &'static str {
    match auth {
        SudoAuth::NoPassword => "sudo -k -n --",
        SudoAuth::Password(_) => "sudo -k -S -p '' --",
    }
}

/// The command that runs `sftp-server` as root on the channel. The path in
/// `sshd_config` comes first, since it is the one the host's own SFTP uses;
/// `internal-sftp` there is no program and is passed over.
pub fn sftp_server(auth: &SudoAuth) -> String {
    let config_path = "$(sed -n \"s/^[[:space:]]*[Ss]ubsystem[[:space:]]*sftp[[:space:]]*\\([^[:space:]]*\\).*/\\1/p\" /etc/ssh/sshd_config 2>/dev/null)";
    let script = format!(
        "for p in {config_path} {paths}; do if [ -x \"$p\" ]; then printf \"%s\\n\" {READY_MARKER}; exec \"$p\"; fi; done; echo \"{MISSING_SFTP_SERVER}\" >&2; exit 127",
        paths = SFTP_SERVER_PATHS.join(" "),
    );
    format!("{} sh -c '{script}'", sudo(auth))
}

/// The command that runs `sh` as root on the channel, reading its script
/// from standard input once the marker is out.
pub fn script_shell(auth: &SudoAuth) -> String {
    format!(
        "{} sh -c 'printf \"%s\\n\" {READY_MARKER}; exec sh -s'",
        sudo(auth)
    )
}
