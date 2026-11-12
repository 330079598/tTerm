//! Safe commands for resuming an AI agent after a terminal restart.
//!
//! The values in a saved workspace are untrusted. This module is the only
//! place that turns them into text that may be sent to a shell, so callers
//! cannot accidentally replay saved screen text as a command.

use serde::Deserialize;

/// The agents whose sessions tTerm can resume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResumeAgent {
    ClaudeCode,
    Codex,
}

/// A shell recognized from the shell integration report or local shell name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeShell {
    Posix,
    Fish,
    PowerShell,
    Cmd,
}

/// Untrusted information needed to construct one resume command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRequest<'a> {
    pub agent: ResumeAgent,
    pub session_id: &'a str,
    pub cwd: &'a str,
    pub shell: &'a str,
}

pub struct BuildAgentResumeCommandInput {
    pub agent: ResumeAgent,
    pub session_id: String,
    pub cwd: String,
    pub shell: String,
}

pub fn build_agent_resume_command_service(
    arguments: BuildAgentResumeCommandInput,
) -> Option<String> {
    let BuildAgentResumeCommandInput {
        agent,
        session_id,
        cwd,
        shell,
    } = arguments;

    build_resume_command(&ResumeRequest {
        agent,
        session_id: &session_id,
        cwd: &cwd,
        shell: &shell,
    })
}

/// Builds one line that changes directory and invokes the agent's official
/// resume command. `None` means the saved values are not safe to use.
pub fn build_resume_command(request: &ResumeRequest<'_>) -> Option<String> {
    validate_session_id(request.session_id)?;
    validate_cwd(request.cwd)?;
    let shell = ResumeShell::parse(request.shell)?;
    let resume = match request.agent {
        ResumeAgent::ClaudeCode => "claude --resume",
        ResumeAgent::Codex => "codex resume",
    };
    let command = match shell {
        ResumeShell::Posix => format!(
            "cd -- {} && {resume} {}",
            quote_posix(request.cwd),
            quote_posix(request.session_id)
        ),
        ResumeShell::Fish => format!(
            "cd -- {} && {resume} {}",
            quote_fish(request.cwd),
            quote_fish(request.session_id)
        ),
        ResumeShell::PowerShell => format!(
            "Set-Location -LiteralPath {}; if ($?) {{ {resume} {} }}",
            quote_powershell(request.cwd),
            quote_powershell(request.session_id)
        ),
        ResumeShell::Cmd => {
            let cwd = quote_cmd(request.cwd)?;
            format!("cd /d {cwd} && {resume} {}", request.session_id)
        }
    };
    (!command.chars().any(char::is_control)).then_some(command)
}

impl ResumeShell {
    fn parse(value: &str) -> Option<Self> {
        let basename = value.rsplit(['/', '\\']).next()?;
        let name = basename
            .trim_start_matches('-')
            .strip_suffix(".exe")
            .unwrap_or_else(|| basename.trim_start_matches('-'))
            .to_ascii_lowercase();
        match name.as_str() {
            "bash" | "zsh" | "sh" => Some(Self::Posix),
            "fish" => Some(Self::Fish),
            "powershell" | "pwsh" => Some(Self::PowerShell),
            "cmd" => Some(Self::Cmd),
            _ => None,
        }
    }
}

fn validate_session_id(value: &str) -> Option<()> {
    (!value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    .then_some(())
}

fn validate_cwd(value: &str) -> Option<()> {
    (!value.is_empty() && !value.chars().any(char::is_control)).then_some(())
}

fn quote_posix(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn quote_powershell(value: &str) -> String {
    let mut literal = String::from("'");
    for character in value.chars() {
        // PowerShell recognizes typographic apostrophes as string delimiters too.
        if matches!(
            character,
            '\'' | '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}'
        ) {
            literal.push(character);
        }
        literal.push(character);
    }
    literal.push('\'');
    literal
}

fn quote_fish(value: &str) -> String {
    // Fish interprets escaped backslashes even inside single quotes.
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// Cmd has no literal single-quoted string and expands `%NAME%` before it
/// runs a command. Rejecting those metacharacters is safer than guessing at
/// cmd's context-dependent percent escaping.
fn quote_cmd(value: &str) -> Option<String> {
    if value
        .chars()
        .any(|character| matches!(character, '"' | '%' | '&' | '|' | '<' | '>' | '^' | '!'))
    {
        return None;
    }
    Some(format!("\"{value}\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(shell: &'a str, cwd: &'a str, session_id: &'a str) -> ResumeRequest<'a> {
        ResumeRequest {
            agent: ResumeAgent::ClaudeCode,
            session_id,
            cwd,
            shell,
        }
    }

    #[test]
    fn builds_posix_commands_for_each_shell() {
        for shell in ["bash", "zsh", "sh", "/bin/bash", "-zsh"] {
            let command = build_resume_command(&request(shell, "/work dir/o'neil", "abc-123"));
            assert_eq!(
                command,
                Some("cd -- '/work dir/o'\\''neil' && claude --resume 'abc-123'".to_string())
            );
        }
        let fish = build_resume_command(&request("fish", "/工作/$tick`", "abc-123"));
        assert_eq!(
            fish,
            Some("cd -- '/工作/$tick`' && claude --resume 'abc-123'".to_string())
        );
    }

    #[test]
    fn builds_power_shell_commands_with_literal_arguments() {
        let command = build_resume_command(&ResumeRequest {
            agent: ResumeAgent::Codex,
            session_id: "abc-123",
            cwd: r"C:\Users\A B\O'Neil$`%",
            shell: "pwsh.exe",
        });
        assert_eq!(
            command,
            Some(
                "Set-Location -LiteralPath 'C:\\Users\\A B\\O''Neil$`%'; if ($?) { codex resume 'abc-123' }"
                    .to_string()
            )
        );
    }

    #[test]
    fn fish_quotes_backslashes_and_single_quotes_as_one_literal_argument() {
        let command = build_resume_command(&request("fish", "/工作/a\\' $`% dir", "abc-123"));
        assert_eq!(
            command,
            Some("cd -- '/工作/a\\\\\\' $`% dir' && claude --resume 'abc-123'".to_string())
        );
    }

    #[test]
    fn quotes_special_directories_for_every_unix_shell() {
        for shell in ["bash", "zsh", "sh"] {
            assert_eq!(
                build_resume_command(&request(shell, "/工作/space ' $`%\\dir", "abc-123")),
                Some("cd -- '/工作/space '\\'' $`%\\dir' && claude --resume 'abc-123'".to_string())
            );
        }
    }

    #[test]
    fn builds_cmd_commands_and_rejects_cmd_metacharacters() {
        let command = build_resume_command(&request("cmd.exe", r"C:\Work Folder\中文", "abc-123"));
        assert_eq!(
            command,
            Some(r#"cd /d "C:\Work Folder\中文" && claude --resume abc-123"#.to_string())
        );
        for cwd in [r"C:\%PATH%", r"C:\a&b", "C:\\a\nb"] {
            assert!(build_resume_command(&request("cmd", cwd, "abc-123")).is_none());
        }
    }

    #[test]
    fn rejects_tampered_ids_directories_and_shells() {
        assert!(build_resume_command(&request("bash", "/safe", "; rm -rf ~")).is_none());
        assert!(build_resume_command(&request("bash", "/safe\nrm -rf ~", "abc-123")).is_none());
        assert!(build_resume_command(&request("unknown", "/safe", "abc-123")).is_none());
        assert!(build_resume_command(&request("bash", "/safe", "")).is_none());
        assert!(build_resume_command(&request("bash", "/safe", "abc_def")).is_none());
    }

    #[test]
    fn powershell_doubles_every_apostrophe_delimiter() {
        for quote in ['\'', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'] {
            let cwd = format!("C:\\x{quote}; calc; {quote}");
            assert_eq!(
                quote_powershell(&cwd),
                format!("'C:\\x{quote}{quote}; calc; {quote}{quote}'")
            );
            assert_eq!(
                build_resume_command(&request("pwsh", &cwd, "abc-123")),
                Some(format!(
                    "Set-Location -LiteralPath 'C:\\x{quote}{quote}; calc; {quote}{quote}'; if ($?) {{ claude --resume 'abc-123' }}"
                ))
            );
        }
        assert_eq!(quote_powershell("C:\\John’s files"), "'C:\\John’’s files'");
    }

    #[test]
    fn other_shells_keep_unicode_quote_lookalikes_literal() {
        let cwd = "John‘s ’ ‚ ‛ “ ” ＇ ＂ files";
        assert_eq!(quote_posix(cwd), format!("'{cwd}'"));
        assert_eq!(quote_fish(cwd), format!("'{cwd}'"));
        assert_eq!(quote_cmd(cwd), Some(format!("\"{cwd}\"")));
    }

    #[cfg(windows)]
    #[test]
    fn powershell_parser_keeps_curly_quote_injections_inside_the_path_argument() {
        for cwd in [
            "C:\\x‘; calc; ‘",
            "C:\\x’; calc; ’",
            "C:\\x‚; calc; ‚",
            "C:\\x‛; calc; ‛",
            "C:\\John’s files",
        ] {
            let command = build_resume_command(&request("powershell", cwd, "abc-123")).unwrap();
            // Parse only. Never evaluate the generated command or its malicious directory.
            let output = std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", r#"
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseInput($env:TTERM_TEST_COMMAND, [ref]$tokens, [ref]$errors)
if ($errors.Count -ne 0 -or $ast.EndBlock.Statements.Count -ne 2) { exit 1 }
$elements = $ast.EndBlock.Statements[0].PipelineElements[0].CommandElements
if ($elements.Count -ne 3 -or $elements[2].Value -cne $env:TTERM_TEST_CWD) { exit 2 }
"#])
                .env("TTERM_TEST_COMMAND", command)
                .env("TTERM_TEST_CWD", cwd)
                .output()
                .unwrap();
            assert!(output.status.success(), "{cwd}: {output:?}");
        }
    }
}
