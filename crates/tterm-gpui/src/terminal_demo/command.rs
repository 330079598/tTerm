use portable_pty::CommandBuilder;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandSpec {
    Shell,
    Ssh { target: String, args: Vec<String> },
}

pub fn command_from_environment() -> CommandSpec {
    match std::env::var("TTERM_GPUI_SSH_TARGET") {
        Ok(target) if !target.trim().is_empty() => CommandSpec::Ssh {
            target,
            args: std::env::var("TTERM_GPUI_SSH_ARGS")
                .map(|value| value.split_whitespace().map(str::to_owned).collect())
                .unwrap_or_default(),
        },
        _ => CommandSpec::Shell,
    }
}

impl CommandSpec {
    pub fn builder(&self) -> CommandBuilder {
        let mut command = match self {
            Self::Shell => CommandBuilder::new_default_prog(),
            Self::Ssh { target, args } => {
                let mut command = CommandBuilder::new("ssh");
                command.arg(target);
                for argument in args {
                    command.arg(argument);
                }
                command
            }
        };
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if std::env::var_os("LANG").is_none() {
            command.env("LANG", "en_US.UTF-8");
        }
        command
    }
}
