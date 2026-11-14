//! The session the prototype opens.
//!
//! Local shells and `--exec` workloads become local connection options.
//! `--ssh` selects a saved connection from the isolated GPUI database by id
//! or name. The core's `AppServices` builds the actual PTY or SSH session.

use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::profiles::connection::connection_options_for_profile;
use tterm_core::profiles::types::SavedProfile;

const USAGE: &str =
    "usage: tterm-gpui [--ssh <saved-profile-id-or-name> | --exec <program> [args...]]";

/// What the prototype opens through `AppServices`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CommandSpec {
    /// The platform's configured local shell.
    Shell,
    /// A local program and its arguments.
    Program { program: String, args: Vec<String> },
    /// A saved SSH profile selected by id or name.
    SshProfile { selector: String },
}

impl CommandSpec {
    /// Reads the command line after the program name.
    pub fn from_args(arguments: &[String]) -> Result<Self, String> {
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
            _ => Err(USAGE.to_string()),
        }
    }

    /// The connection options passed to `AppServices`.
    pub fn connection_options(&self) -> Result<PtyConnectionOptions, String> {
        match self {
            Self::Shell => local_options(None, &[]),
            Self::Program { program, args } => local_options(Some(program), args),
            Self::SshProfile { selector } => saved_ssh_profile(selector),
        }
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

fn saved_ssh_profile(selector: &str) -> Result<PtyConnectionOptions, String> {
    let profiles = tterm_core::profiles::storage::load_profiles()?;
    let mut profile = select_saved_profile(selector, &profiles)?;
    tterm_core::profiles::storage::normalize_profile(&mut profile);
    connection_options_for_profile(&profile)
}

fn select_saved_profile(selector: &str, profiles: &[SavedProfile]) -> Result<SavedProfile, String> {
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
mod tests {
    use super::{CommandSpec, join_arguments, select_saved_profile};
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
            select_saved_profile("prod", &profiles).expect("id").name,
            "Production"
        );
        assert_eq!(
            select_saved_profile("production", &profiles)
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
        assert!(select_saved_profile("production", &profiles).is_err());
    }
}
