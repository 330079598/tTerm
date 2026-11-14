//! Where an SSH tab's SFTP drawer first opens, from
//! `src/lib/sftpStartDirectory.ts`.

/// The choices for the drawer's first directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpStartDirectory {
    /// The user's home directory.
    Home,
    /// The last directory listed on that host, which the backend remembers.
    Last,
    /// The directory the terminal's shell reported.
    Terminal,
}

/// The stored names of the choices, in display order.
pub const SFTP_START_DIRECTORIES: [SftpStartDirectory; 3] = [
    SftpStartDirectory::Home,
    SftpStartDirectory::Last,
    SftpStartDirectory::Terminal,
];

impl SftpStartDirectory {
    /// The name the setting stores.
    pub fn as_str(self) -> &'static str {
        match self {
            SftpStartDirectory::Home => "home",
            SftpStartDirectory::Last => "last",
            SftpStartDirectory::Terminal => "terminal",
        }
    }

    /// The choice a stored name names, or `None` for anything unknown.
    pub fn parse(value: &str) -> Option<SftpStartDirectory> {
        SFTP_START_DIRECTORIES
            .into_iter()
            .find(|choice| choice.as_str() == value)
    }
}

/// A stored choice, or `None` for anything unknown.
pub fn normalize_sftp_start_directory(value: Option<&str>) -> Option<SftpStartDirectory> {
    value.and_then(SftpStartDirectory::parse)
}

/// The path `sftp_list_directory` gets for the first listing: the terminal's
/// directory when the drawer goes there and one is known, `.` (home) or
/// `None` (the last directory, then home, then `/`).
pub fn sftp_start_path(
    choice: SftpStartDirectory,
    follows_shell_cwd: bool,
    shell_cwd: Option<&str>,
) -> Option<String> {
    if (choice == SftpStartDirectory::Terminal || follows_shell_cwd)
        && let Some(cwd) = shell_cwd.filter(|cwd| !cwd.is_empty())
    {
        return Some(cwd.to_owned());
    }
    sftp_fallback_path(choice)
}

/// Where the first listing goes when the start path cannot be listed.
pub fn sftp_fallback_path(choice: SftpStartDirectory) -> Option<String> {
    match choice {
        SftpStartDirectory::Last => None,
        _ => Some(".".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SftpStartDirectory, normalize_sftp_start_directory, sftp_fallback_path, sftp_start_path,
    };

    #[test]
    fn keeps_the_known_choices_and_drops_anything_else() {
        assert_eq!(
            normalize_sftp_start_directory(Some("home")),
            Some(SftpStartDirectory::Home)
        );
        assert_eq!(
            normalize_sftp_start_directory(Some("last")),
            Some(SftpStartDirectory::Last)
        );
        assert_eq!(
            normalize_sftp_start_directory(Some("terminal")),
            Some(SftpStartDirectory::Terminal)
        );
        assert_eq!(normalize_sftp_start_directory(Some("cwd")), None);
        assert_eq!(normalize_sftp_start_directory(None), None);
    }

    #[test]
    fn opens_at_the_terminals_directory_when_the_drawer_goes_there() {
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Terminal, true, Some("/var/log")).as_deref(),
            Some("/var/log")
        );
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Terminal, false, Some("/var/log")).as_deref(),
            Some("/var/log")
        );
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Home, true, Some("/var/log")).as_deref(),
            Some("/var/log")
        );
    }

    #[test]
    fn opens_at_home_or_the_last_directory_otherwise() {
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Home, false, Some("/var/log")).as_deref(),
            Some(".")
        );
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Last, false, Some("/var/log")),
            None
        );
    }

    #[test]
    fn falls_back_to_home_when_the_terminal_reported_no_directory() {
        assert_eq!(
            sftp_start_path(SftpStartDirectory::Terminal, true, None).as_deref(),
            Some(".")
        );
        assert_eq!(sftp_start_path(SftpStartDirectory::Last, true, None), None);
    }

    #[test]
    fn leaves_the_last_directory_to_the_backends_own_fallbacks() {
        assert_eq!(sftp_fallback_path(SftpStartDirectory::Last), None);
        assert_eq!(
            sftp_fallback_path(SftpStartDirectory::Home).as_deref(),
            Some(".")
        );
        assert_eq!(
            sftp_fallback_path(SftpStartDirectory::Terminal).as_deref(),
            Some(".")
        );
    }
}
