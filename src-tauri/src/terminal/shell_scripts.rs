//! The scripts behind tTerm's shell integration for bash, zsh and fish,
//! shared by WSL (`shell_integration`) and SSH sessions, which install them
//! on the remote host (`crate::ssh::shell_integration`), and by local shells
//! on macOS and Linux when turned on (`local_integration`).

use std::fs;
use std::path::Path;

/// Exported as a bash function (`BASH_FUNC_<name>%%`) for Git Bash and WSL bash.
pub(crate) const BASH_REPORT_CWD: &str = include_str!("shell_integration/bash/report-cwd.bash");
/// The directory report plus command marks (OSC 133 D and A).
pub(crate) const BASH_PROMPT: &str = include_str!("shell_integration/bash/prompt.bash");

/// `unix/start.sh`, which starts the login shell, and the scripts it loads,
/// by path relative to the directory they are installed in.
pub(crate) const UNIX_SCRIPTS: &[(&str, &str)] = &[
    (
        "unix/start.sh",
        include_str!("shell_integration/unix/start.sh"),
    ),
    ("bash/report-cwd.bash", BASH_REPORT_CWD),
    ("bash/prompt.bash", BASH_PROMPT),
    ("zsh/.zshenv", include_str!("shell_integration/zsh/zshenv")),
    (
        "zsh/.zprofile",
        include_str!("shell_integration/zsh/zprofile"),
    ),
    ("zsh/.zshrc", include_str!("shell_integration/zsh/zshrc")),
    ("zsh/.zlogin", include_str!("shell_integration/zsh/zlogin")),
    (
        "fish/tterm.fish",
        include_str!("shell_integration/fish/tterm.fish"),
    ),
];

/// A CRLF checkout must not reach the shells as CRLF.
pub(crate) fn lf(content: &str) -> String {
    content.replace("\r\n", "\n")
}

/// Writes `scripts` under `dir` by relative path, skipping those already up
/// to date.
pub(crate) fn write_scripts<'a>(
    dir: &Path,
    scripts: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> std::io::Result<()> {
    for (relative, content) in scripts {
        let content = lf(content);
        let path = dir.join(relative);
        if fs::read_to_string(&path).is_ok_and(|existing| existing == content) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
    }
    Ok(())
}
