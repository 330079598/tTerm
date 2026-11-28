//! The session the prototype opens.
//!
//! Local shells, command-line folders and `--exec` workloads become local
//! connection options. `--ssh` selects a saved connection from the isolated
//! GPUI database by id or name. The core's `AppServices` builds the actual PTY
//! or SSH session.

use std::path::Path;

use tterm_core::connections::quick_connect::{QuickConnection, build_quick_connect_tab};
use tterm_core::connections::quick_launch::connection_options_for_quick;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::launch_dir::{LaunchDir, resolve_dir};
use tterm_core::profiles::connection::connection_options_for_profile;
use tterm_core::profiles::launch::SavedSshLaunch;
use tterm_core::profiles::types::SavedProfile;
use tterm_core::workspace::tab::{ConnectionType, Tab, TabConnection, TabType, TerminalShellType};

use super::session::RestoredHistory;

const USAGE: &str = "usage: tterm-gpui [--ssh <saved-profile-id-or-name> | --exec <program> [args...] | [--] <folder>...]";

/// What the prototype opens through `AppServices`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CommandSpec {
    /// The platform's configured local shell.
    Shell,
    /// A local program and its arguments.
    Program { program: String, args: Vec<String> },
    /// A saved SSH profile selected by id or name.
    SshProfile { selector: String },
    /// Folders to open in the configured local shell, in command-line order.
    /// A path that does not exist is kept so the GUI can report it.
    Folders(Vec<LaunchDir>),
}

impl CommandSpec {
    /// Reads the command line after the program name.
    pub fn from_args(arguments: &[String]) -> Result<Self, String> {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self::from_args_in(arguments, &cwd)
    }

    /// Reads the command line after the program name, resolving positional
    /// folders against the directory it was run in. Options end at `--`, so a
    /// folder whose name starts with a dash can be named after it. A path is
    /// resolved as data; it is never turned into a shell command.
    fn from_args_in(arguments: &[String], cwd: &Path) -> Result<Self, String> {
        let Some((first, rest)) = arguments.split_first() else {
            return Ok(Self::Shell);
        };
        match first.as_str() {
            "--ssh" => {
                let [selector] = rest else {
                    return Err(format!("--ssh needs one saved profile id or name; {USAGE}"));
                };
                if selector.trim().is_empty() {
                    return Err(format!("--ssh needs a saved profile; {USAGE}"));
                }
                Ok(Self::SshProfile {
                    selector: selector.trim().to_string(),
                })
            }
            "--exec" => match rest.split_first() {
                Some((program, args)) => Ok(Self::Program {
                    program: program.clone(),
                    args: args.to_vec(),
                }),
                None => Err(format!("--exec needs a program; {USAGE}")),
            },
            "--" => Ok(Self::folders(
                rest.iter().map(|arg| resolve_dir(arg, cwd)).collect(),
            )),
            option if option.starts_with('-') => {
                Err(format!("unrecognized option '{option}'; {USAGE}"))
            }
            _ => {
                let mut folders = vec![resolve_dir(first, cwd)];
                let mut options_ended = false;
                for argument in rest {
                    if !options_ended {
                        if argument == "--" {
                            options_ended = true;
                            continue;
                        }
                        if argument.starts_with('-') {
                            return Err(format!("unrecognized option '{argument}'; {USAGE}"));
                        }
                    }
                    folders.push(resolve_dir(argument, cwd));
                }
                Ok(Self::folders(folders))
            }
        }
    }

    /// A launch with no folder names is the ordinary shell tab.
    fn folders(folders: Vec<LaunchDir>) -> Self {
        if folders.is_empty() {
            Self::Shell
        } else {
            Self::Folders(folders)
        }
    }

    /// The connection options passed to `AppServices`. A folder launch opens
    /// one tab per folder instead of sharing one connection.
    pub fn connection_options(&self) -> Result<PtyConnectionOptions, String> {
        match self {
            Self::Shell => local_options(None, &[]),
            Self::Program { program, args } => local_options(Some(program), args),
            Self::SshProfile { selector } => saved_ssh_profile_from_cli(selector),
            Self::Folders(_) => Err("Command-line folders open one tab each.".to_string()),
        }
    }

    /// A bare launch restores and saves the workspace, and so does a folder
    /// launch: the folders are appended to the restored tabs. An explicit
    /// `--ssh` or `--exec` run is disposable and must not overwrite the saved
    /// workspace.
    pub fn uses_saved_workspace(&self) -> bool {
        matches!(self, Self::Shell | Self::Folders(_))
    }
}

/// What a tab should open, including a tab restored from a saved workspace.
///
/// Restored tabs carry only a safe connection descriptor: a known local
/// shell, a saved SSH profile selected by id or name, or an inert reason when
/// the tab cannot be opened safely.
pub(super) struct TabLaunch {
    saved_id: String,
    saved: Tab,
    source: LaunchSource,
    may_resume_agent: bool,
    restored: bool,
    restored_history: Option<RestoredHistory>,
}

/// How a saved SSH profile is selected when a tab opens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SshProfileSelector {
    /// A tab that knows its saved profile must reopen exactly that row.
    Id(String),
    /// A workspace saved before tabs carried ids can reopen by a unique name.
    Name(String),
}

pub(super) enum LaunchSource {
    Command(CommandSpec),
    Local(Box<PtyConnectionOptions>),
    SshProfile { selector: SshProfileSelector },
    SavedSshProfile(Box<SavedSshLaunch>),
    QuickConnection(Box<QuickConnection>),
    Unsupported { reason: String },
}

impl TabLaunch {
    /// A tab explicitly opened by the user or a command-line argument.
    pub(super) fn command(saved_id: impl Into<String>, command: CommandSpec) -> Self {
        let saved_id = saved_id.into();
        let options = command.connection_options().ok();
        let saved = saved_tab_for_command(&saved_id, &command, options.as_ref());
        let may_resume_agent = !matches!(command, CommandSpec::Program { .. });
        Self {
            saved_id,
            saved,
            source: LaunchSource::Command(command),
            may_resume_agent,
            restored: false,
            restored_history: None,
        }
    }

    /// A tab restored with a connection that is safe to open again.
    pub(super) fn restored_tab(saved: Tab, source: LaunchSource) -> Self {
        let saved_id = saved.id.clone();
        let may_resume_agent = matches!(
            source,
            LaunchSource::Local(_)
                | LaunchSource::SshProfile { .. }
                | LaunchSource::SavedSshProfile(_)
        );
        Self {
            saved_id,
            saved,
            source,
            may_resume_agent,
            restored: true,
            restored_history: None,
        }
    }

    /// Starts a newly selected local shell without treating it as a restored session.
    pub(super) fn selected_shell(saved: Tab, options: PtyConnectionOptions) -> Self {
        let mut launch = Self::restored_tab(saved, LaunchSource::Local(Box::new(options)));
        launch.restored = false;
        launch
    }

    /// A profile confirmed in the picker, with its connection already built
    /// from the isolated profile database.
    pub(super) fn saved_profile(saved_id: impl Into<String>, launch: SavedSshLaunch) -> Self {
        let saved_id = saved_id.into();
        let mut saved = launch.saved_tab.clone();
        saved.set_id(saved_id.clone());
        Self {
            saved_id,
            saved,
            source: LaunchSource::SavedSshProfile(Box::new(launch)),
            may_resume_agent: true,
            restored: false,
            restored_history: None,
        }
    }

    /// A quick connection typed in the launcher, with no saved profile.
    pub(super) fn quick_connection(
        saved_id: impl Into<String>,
        connection: QuickConnection,
    ) -> Self {
        let saved_id = saved_id.into();
        let mut saved = build_quick_connect_tab(&connection);
        saved.set_id(saved_id.clone());
        Self {
            saved_id,
            saved,
            source: LaunchSource::QuickConnection(Box::new(connection)),
            may_resume_agent: true,
            restored: false,
            restored_history: None,
        }
    }

    /// A saved tab kept in the workspace without opening its contents.
    pub(super) fn unsupported(saved: Tab, reason: String) -> Self {
        Self {
            saved_id: saved.id.clone(),
            saved,
            source: LaunchSource::Unsupported { reason },
            may_resume_agent: false,
            restored: true,
            restored_history: None,
        }
    }

    /// Attaches the snapshot that belongs to a restored tab.
    pub(super) fn with_restored_history(mut self, history: Option<RestoredHistory>) -> Self {
        self.restored_history = history;
        self
    }

    /// Takes the snapshot once the session is about to be created.
    pub(super) fn take_restored_history(&mut self) -> Option<RestoredHistory> {
        self.restored_history.take()
    }

    pub(super) fn saved_id(&self) -> &str {
        &self.saved_id
    }

    pub(super) fn saved(&self) -> &Tab {
        &self.saved
    }

    pub(super) fn is_ssh(&self) -> bool {
        matches!(
            self.source,
            LaunchSource::Command(CommandSpec::SshProfile { .. })
                | LaunchSource::SshProfile { .. }
                | LaunchSource::SavedSshProfile(_)
                | LaunchSource::QuickConnection(_)
        )
    }

    /// The stable saved profile a successful connection should credit.
    ///
    /// `saved_id` is the workspace tab id, not a profile id, so it must not be
    /// used for recents. A restored tab that only knew its profile by name
    /// still gets the real id from the options resolved for this launch.
    pub(super) fn saved_profile_id(&self, options: &PtyConnectionOptions) -> Option<String> {
        if !self.is_ssh() || options.connection_type.as_deref() != Some("ssh") {
            return None;
        }
        let from_source = match &self.source {
            LaunchSource::SavedSshProfile(launch) => Some(launch.summary.id.clone()),
            LaunchSource::SshProfile {
                selector: SshProfileSelector::Id(id),
            } => Some(id.clone()),
            _ => None,
        };
        from_source.filter(|id| !id.trim().is_empty()).or_else(|| {
            options
                .profile_id
                .clone()
                .filter(|id| !id.trim().is_empty())
        })
    }

    pub(super) fn search_kind(&self) -> &'static str {
        if self.is_ssh() { "ssh" } else { "terminal" }
    }

    pub(super) fn may_resume_agent(&self) -> bool {
        self.may_resume_agent
    }

    pub(super) fn local_shell(&self) -> Option<&str> {
        match &self.source {
            LaunchSource::Command(CommandSpec::Shell) => Some("auto"),
            LaunchSource::Local(options) => options.terminal_shell.as_deref(),
            _ => None,
        }
    }

    pub(super) fn local_custom_path(&self) -> Option<&str> {
        match &self.source {
            LaunchSource::Local(options) if options.terminal_shell.as_deref() == Some("custom") => {
                options.terminal_shell_custom_path.as_deref()
            }
            _ => None,
        }
    }

    pub(super) fn is_restored(&self) -> bool {
        self.restored
    }

    /// A restored local tab and the bare shell tab can gain a split pane by
    /// starting another local default shell.
    pub(super) fn is_default_shell(&self) -> bool {
        matches!(
            &self.source,
            LaunchSource::Command(CommandSpec::Shell) | LaunchSource::Local(_)
        )
    }

    pub(super) fn connection_options(&self) -> Result<PtyConnectionOptions, String> {
        match &self.source {
            LaunchSource::Command(command) => command.connection_options(),
            LaunchSource::Local(options) => Ok(options.as_ref().clone()),
            LaunchSource::SshProfile { selector } => saved_ssh_profile(selector),
            LaunchSource::SavedSshProfile(launch) => Ok(launch.connection.clone()),
            LaunchSource::QuickConnection(connection) => {
                Ok(connection_options_for_quick(connection))
            }
            LaunchSource::Unsupported { reason } => Err(reason.clone()),
        }
    }
}

fn saved_tab_for_command(
    saved_id: &str,
    command: &CommandSpec,
    options: Option<&PtyConnectionOptions>,
) -> Tab {
    let tab_type = if matches!(command, CommandSpec::SshProfile { .. }) {
        TabType::Ssh
    } else {
        TabType::Terminal
    };
    let mut tab = Tab::new(saved_id, "", tab_type);
    if let Some(options) = options {
        tab.set_connection(Some(connection_from_options(options)));
    }
    tab
}

fn connection_from_options(options: &PtyConnectionOptions) -> TabConnection {
    let mut connection = TabConnection::default();
    let connection_type = match options.connection_type.as_deref() {
        Some("ssh") => ConnectionType::Ssh,
        Some("serial") => ConnectionType::Serial,
        _ => ConnectionType::Terminal,
    };
    connection.set_connection_type(Some(connection_type));
    connection.set_profile_id(options.profile_id.clone());
    connection.set_profile_name(options.profile_name.clone());
    connection.set_host(options.host.clone());
    connection.set_port(options.port.map(u32::from));
    connection.set_username(options.username.clone());
    connection.set_cwd(options.cwd.clone());
    connection.set_terminal_shell(
        options
            .terminal_shell
            .as_deref()
            .and_then(shell_type_from_saved),
    );
    connection.set_terminal_shell_custom_path(options.terminal_shell_custom_path.clone());
    connection.set_terminal_shell_custom_args(options.terminal_shell_custom_args.clone());
    #[cfg(windows)]
    connection.set_terminal_shell_wsl_distro(options.terminal_shell_wsl_distro.clone());
    connection
}

fn shell_type_from_saved(value: &str) -> Option<TerminalShellType> {
    match value {
        "auto" => Some(TerminalShellType::Auto),
        "cmd" => Some(TerminalShellType::Cmd),
        "powershell" => Some(TerminalShellType::Powershell),
        "pwsh" => Some(TerminalShellType::Pwsh),
        "wsl" => Some(TerminalShellType::Wsl),
        "git-bash" => Some(TerminalShellType::GitBash),
        "custom" => Some(TerminalShellType::Custom),
        _ => None,
    }
}

fn local_options(
    program: Option<&String>,
    args: &[String],
) -> Result<PtyConnectionOptions, String> {
    let (shell, custom_path, custom_args) = match program {
        Some(program) => (
            "custom".to_string(),
            Some(program.clone()),
            Some(join_arguments(args)?),
        ),
        None => ("auto".to_string(), None, None),
    };
    Ok(PtyConnectionOptions {
        connection_type: Some("terminal".to_string()),
        terminal_shell: Some(shell),
        terminal_shell_custom_path: custom_path,
        terminal_shell_custom_args: custom_args,
        session_log: Some("never".to_string()),
        ..PtyConnectionOptions::default()
    })
}

fn saved_ssh_profile(selector: &SshProfileSelector) -> Result<PtyConnectionOptions, String> {
    let profiles = tterm_core::profiles::storage::load_profiles()?;
    let mut profile = match selector {
        SshProfileSelector::Id(id) => select_saved_profile_by_id(id, &profiles)?,
        SshProfileSelector::Name(name) => select_saved_profile_by_name(name, &profiles)?,
    };
    tterm_core::profiles::storage::normalize_profile(&mut profile);
    connection_options_for_profile(&profile)
}

fn saved_ssh_profile_from_cli(selector: &str) -> Result<PtyConnectionOptions, String> {
    let profiles = tterm_core::profiles::storage::load_profiles()?;
    let mut profile = select_saved_profile_from_cli(selector, &profiles)?;
    tterm_core::profiles::storage::normalize_profile(&mut profile);
    let options = connection_options_for_profile(&profile)?;
    #[cfg(all(debug_assertions, feature = "automation"))]
    let options = {
        let mut options = options;
        super::field::inject_password(&mut options)?;
        options
    };
    Ok(options)
}

fn select_saved_profile_by_id(id: &str, profiles: &[SavedProfile]) -> Result<SavedProfile, String> {
    profiles
        .iter()
        .find(|profile| profile.id == id)
        .cloned()
        .ok_or_else(|| {
            format!(
                "The saved connection with id '{id}' no longer exists. Choose another saved \
                 connection."
            )
        })
}

fn select_saved_profile_by_name(
    name: &str,
    profiles: &[SavedProfile],
) -> Result<SavedProfile, String> {
    let mut matches = profiles
        .iter()
        .filter(|profile| profile.name.eq_ignore_ascii_case(name));
    let Some(profile) = matches.next() else {
        return Err(format!(
            "No saved connection matches '{name}'. Save it in tTerm first."
        ));
    };
    if matches.next().is_some() {
        return Err(format!(
            "More than one saved connection is named '{name}'; reopen the tab after choosing a \
             unique name."
        ));
    }
    Ok(profile.clone())
}

fn select_saved_profile_from_cli(
    selector: &str,
    profiles: &[SavedProfile],
) -> Result<SavedProfile, String> {
    if let Some(profile) = profiles
        .iter()
        .find(|profile| profile.id == selector)
        .cloned()
    {
        return Ok(profile);
    }
    let mut matches = profiles
        .iter()
        .filter(|profile| profile.name.eq_ignore_ascii_case(selector));
    let Some(profile) = matches.next() else {
        return Err(format!(
            "No saved connection matches '{selector}'. Save it in tTerm first, or pass its id."
        ));
    };
    if matches.next().is_some() {
        return Err(format!(
            "More than one saved connection is named '{selector}'; pass its id instead."
        ));
    }
    Ok(profile.clone())
}

#[cfg(windows)]
fn join_arguments(args: &[String]) -> Result<String, String> {
    let mut words = Vec::with_capacity(args.len());
    for argument in args {
        if argument.contains('"') {
            return Err(format!(
                "The GPUI prototype cannot represent a double quote in '--exec' argument {argument:?}"
            ));
        }
        if argument.is_empty() || argument.chars().any(char::is_whitespace) {
            words.push(format!("\"{argument}\""));
        } else {
            words.push(argument.clone());
        }
    }
    Ok(words.join(" "))
}

#[cfg(not(windows))]
fn join_arguments(args: &[String]) -> Result<String, String> {
    Ok(args
        .iter()
        .map(|argument| format!("'{}'", argument.replace('\'', r"'\''")))
        .collect::<Vec<_>>()
        .join(" "))
}

#[cfg(test)]
mod quick;

#[cfg(test)]
mod tests {
    use super::{
        CommandSpec, join_arguments, select_saved_profile_by_id, select_saved_profile_by_name,
        select_saved_profile_from_cli,
    };
    use tterm_core::profiles::types::SavedProfile;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn the_command_line_picks_a_local_shell_a_program_or_a_saved_profile() {
        assert_eq!(CommandSpec::from_args(&[]), Ok(CommandSpec::Shell));
        assert_eq!(
            CommandSpec::from_args(&args(&["--ssh", "Production"])),
            Ok(CommandSpec::SshProfile {
                selector: "Production".to_string()
            })
        );
        assert_eq!(
            CommandSpec::from_args(&args(&["--exec", "vim", "notes.txt"])),
            Ok(CommandSpec::Program {
                program: "vim".to_string(),
                args: args(&["notes.txt"]),
            })
        );
        assert!(CommandSpec::from_args(&args(&["--ssh"])).is_err());
        assert!(CommandSpec::from_args(&args(&["--ssh", "one", "extra"])).is_err());
        assert!(CommandSpec::from_args(&args(&["--exec"])).is_err());
        assert!(CommandSpec::from_args(&args(&["--unknown"])).is_err());
    }

    #[test]
    fn positional_folders_keep_their_order_and_spaces() {
        let cwd = std::env::temp_dir();
        let command = CommandSpec::from_args_in(
            &args(&["first folder", "two words", ".", "--", "-dash", "--"]),
            &cwd,
        )
        .expect("folder arguments");
        assert!(command.uses_saved_workspace());
        let CommandSpec::Folders(dirs) = command else {
            panic!("folder arguments open folders");
        };
        let paths: Vec<_> = dirs
            .iter()
            .map(|dir| std::path::PathBuf::from(dir.path()))
            .collect();
        let base = std::path::absolute(&cwd).expect("absolute directory");
        assert_eq!(
            paths,
            [
                base.join("first folder"),
                base.join("two words"),
                base.clone(),
                base.join("-dash"),
                base.join("--"),
            ]
        );
        assert!(dirs[2].exists());
    }

    #[test]
    fn positional_launches_reject_unknown_options() {
        let cwd = std::env::temp_dir();
        assert!(CommandSpec::from_args_in(&args(&["--unknown"]), &cwd).is_err());
        assert!(CommandSpec::from_args_in(&args(&["-x"]), &cwd).is_err());
        assert!(CommandSpec::from_args_in(&args(&["folder", "-x"]), &cwd).is_err());
        assert!(CommandSpec::from_args_in(&args(&["-x", "--", "folder"]), &cwd).is_err());
        assert_eq!(
            CommandSpec::from_args_in(&args(&["--"]), &cwd),
            Ok(CommandSpec::Shell)
        );
    }

    #[test]
    fn explicit_modes_stay_strict_and_do_not_take_folders() {
        let cwd = std::env::temp_dir();
        assert!(CommandSpec::from_args_in(&args(&["--ssh", "one", "folder"]), &cwd).is_err());
        assert!(CommandSpec::from_args_in(&args(&["--ssh", ""]), &cwd).is_err());
        assert_eq!(
            CommandSpec::from_args_in(&args(&["--exec", "prog", "--flag", "folder"]), &cwd),
            Ok(CommandSpec::Program {
                program: "prog".to_string(),
                args: args(&["--flag", "folder"]),
            })
        );
        assert!(
            !CommandSpec::Program {
                program: "prog".to_string(),
                args: Vec::new(),
            }
            .uses_saved_workspace()
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn a_quoted_drive_root_resolves_as_a_folder() {
        let cwd = std::env::temp_dir();
        let drive = cwd.to_string_lossy()[..2].to_string();
        let quoted = format!("{drive}\"");
        let command =
            CommandSpec::from_args_in(&args(&[&quoted]), &cwd).expect("quoted drive root");
        let CommandSpec::Folders(dirs) = command else {
            panic!("the drive root is a folder");
        };
        assert_eq!(dirs[0].path(), format!("{drive}\\"));
        assert!(dirs[0].exists());
    }

    #[test]
    fn program_arguments_are_quoted_for_the_core_custom_shell_parser() {
        let joined = join_arguments(&args(&["one", "two words"])).expect("join arguments");
        if cfg!(windows) {
            assert_eq!(joined, r#"one "two words""#);
        } else {
            assert_eq!(joined, "'one' 'two words'");
        }
    }

    #[test]
    fn local_options_use_the_core_auto_or_custom_shell_path() {
        let shell = CommandSpec::Shell
            .connection_options()
            .expect("local shell options");
        assert_eq!(shell.connection_type.as_deref(), Some("terminal"));
        assert_eq!(shell.terminal_shell.as_deref(), Some("auto"));

        let program = CommandSpec::Program {
            program: "vim".to_string(),
            args: args(&["two words"]),
        }
        .connection_options()
        .expect("program options");
        assert_eq!(program.terminal_shell_custom_path.as_deref(), Some("vim"));
        assert!(
            program
                .terminal_shell_custom_args
                .as_deref()
                .is_some_and(|value| value.contains("two words"))
        );
    }

    #[test]
    fn saved_profiles_are_selected_by_id_before_name() {
        let profile = |id: &str, name: &str| -> SavedProfile {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": name, "connection_type": "ssh",
                "host": "example.com", "username": "user"
            }))
            .expect("profile")
        };
        let profiles = vec![profile("prod", "Production"), profile("other", "prod")];

        assert_eq!(
            select_saved_profile_from_cli("prod", &profiles)
                .expect("id")
                .name,
            "Production"
        );
        assert_eq!(
            select_saved_profile_from_cli("production", &profiles)
                .expect("case-insensitive name")
                .id,
            "prod"
        );
    }

    #[test]
    fn ambiguous_saved_profile_names_are_rejected() {
        let profile = |id: &str| -> SavedProfile {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": "Production", "connection_type": "ssh",
                "host": "example.com", "username": "user"
            }))
            .expect("profile")
        };
        let profiles = vec![profile("one"), profile("two")];
        assert!(select_saved_profile_from_cli("production", &profiles).is_err());
        assert!(select_saved_profile_by_name("Production", &profiles).is_err());
    }

    #[test]
    fn exact_ids_and_legacy_names_never_borrow_each_others_lookup() {
        let profile = |id: &str, name: &str| -> SavedProfile {
            serde_json::from_value(serde_json::json!({
                "id": id, "name": name, "connection_type": "ssh",
                "host": "example.com", "username": "user"
            }))
            .expect("profile")
        };
        let profiles = vec![profile("prod", "Production"), profile("other", "prod")];

        assert_eq!(
            select_saved_profile_by_id("prod", &profiles)
                .expect("exact id")
                .name,
            "Production"
        );
        assert_eq!(
            select_saved_profile_by_name("prod", &profiles)
                .expect("name-only legacy tab")
                .id,
            "other"
        );
        assert!(select_saved_profile_by_id("Production", &profiles).is_err());
    }
}
