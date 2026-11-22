//! Unit tests for the pure capture and restore decisions.

use std::sync::Arc;
use std::time::Duration;

use tterm_core::agent_status::{
    AgentReport, AgentReportState, AgentResumeSession, AgentScope, AgentTrackerRegistry,
};

use crate::terminal_demo::binding::scoped::ConnectionState;

use super::controller::{is_terminal_reply, ready_to_resume, resume_agent, should_resume};
use super::state::{Capture, NoticeReason, PROMPT_TIMEOUT, Restore};

fn session(agent: &str, id: &str, cwd: Option<&str>) -> AgentResumeSession {
    AgentResumeSession {
        agent: agent.to_string(),
        session_id: id.to_string(),
        cwd: cwd.map(str::to_owned),
    }
}

fn report(
    agent: &str,
    state: AgentReportState,
    id: &str,
    scope: AgentScope,
    cwd: Option<&str>,
) -> AgentReport {
    AgentReport {
        agent: agent.to_string(),
        state,
        session: Some(id.to_string()),
        scope: Some(scope),
        cwd: cwd.map(str::to_owned),
    }
}

#[test]
fn only_a_restored_tab_that_may_resume_with_the_setting_on_waits() {
    assert!(should_resume(true, true, true));
    assert!(
        !should_resume(true, false, true),
        "--exec tabs never resume"
    );
    assert!(
        !should_resume(false, true, true),
        "a fresh tab has nothing to resume"
    );
    assert!(
        !should_resume(true, true, false),
        "the setting turns it off"
    );
}

#[test]
fn only_an_authenticated_ssh_shell_or_a_starting_local_shell_is_ready() {
    assert!(
        !ready_to_resume(true, ConnectionState::Connecting),
        "SSH authentication has not finished"
    );
    assert!(
        !ready_to_resume(true, ConnectionState::Reconnecting),
        "a retry has not authenticated yet"
    );
    assert!(!ready_to_resume(true, ConnectionState::Error));
    assert!(!ready_to_resume(true, ConnectionState::Disconnected));
    assert!(ready_to_resume(true, ConnectionState::Connected));

    assert!(ready_to_resume(false, ConnectionState::Connecting));
    assert!(ready_to_resume(false, ConnectionState::Connected));
    assert!(
        !ready_to_resume(false, ConnectionState::Disconnected),
        "an ended local shell has no prompt"
    );
    assert!(!ready_to_resume(false, ConnectionState::Error));
}

#[test]
fn maps_the_supported_agents_and_rejects_others() {
    assert_eq!(
        resume_agent("claudeCode"),
        Some(tterm_core::core::agent_resume::ResumeAgent::ClaudeCode)
    );
    assert_eq!(
        resume_agent("claude"),
        Some(tterm_core::core::agent_resume::ResumeAgent::ClaudeCode)
    );
    assert_eq!(
        resume_agent("codex"),
        Some(tterm_core::core::agent_resume::ResumeAgent::Codex)
    );
    assert_eq!(resume_agent("opencode"), None);
}

#[test]
fn terminal_replies_do_not_look_like_the_user_typing() {
    assert!(is_terminal_reply(b"\x1b[I"));
    assert!(is_terminal_reply(b"\x1b[O"));
    assert!(is_terminal_reply(b"\x1b[<0;1;2M"));
    assert!(is_terminal_reply(b"\x1b[M !!"));
    assert!(!is_terminal_reply(b"a"));
    assert!(!is_terminal_reply(b"\x1b[A"));
    assert!(!is_terminal_reply(b"\x1b[200~pasted"));
    assert!(!is_terminal_reply(b"\r"));
}

#[test]
fn captures_the_session_a_direct_agent_report_names() {
    let mut capture = Capture::new(None, Some(Arc::new(AgentTrackerRegistry::new())));
    let change = capture.report(
        &report(
            "codex",
            AgentReportState::Processing,
            "abc-123",
            AgentScope::Direct,
            Some("/work"),
        ),
        1_000,
    );
    assert_eq!(
        change,
        Some(Some(session("codex", "abc-123", Some("/work"))))
    );
    assert_eq!(
        capture.saved().cloned(),
        Some(session("codex", "abc-123", Some("/work")))
    );
}

#[test]
fn a_shell_prompt_clears_the_saved_session_once() {
    let initial = session("claudeCode", "abc-123", Some("/work"));
    let mut capture = Capture::new(Some(initial), Some(Arc::new(AgentTrackerRegistry::new())));
    assert_eq!(capture.prompt(), Some(None));
    assert_eq!(capture.prompt(), None, "the second prompt changes nothing");
    assert_eq!(capture.saved(), None);
}

#[test]
fn ended_and_idle_reports_clear_the_saved_session() {
    for state in [AgentReportState::Ended, AgentReportState::Idle] {
        let initial = session("codex", "abc-123", Some("/work"));
        let mut capture = Capture::new(Some(initial), Some(Arc::new(AgentTrackerRegistry::new())));
        let change = capture.report(
            &report("codex", state, "abc-123", AgentScope::Direct, None),
            1_000,
        );
        assert_eq!(change, Some(None), "{state:?}");
        assert_eq!(capture.saved(), None, "{state:?}");
    }
}

#[test]
fn a_broadcast_for_another_tabs_session_is_ignored() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let _owner = Capture::new(
        Some(session("codex", "abc-123", Some("/work"))),
        Some(Arc::clone(&registry)),
    );
    let mut other = Capture::new(None, Some(registry));
    let change = other.report(
        &report(
            "codex",
            AgentReportState::Processing,
            "abc-123",
            AgentScope::Broadcast,
            None,
        ),
        1_000,
    );
    assert_eq!(change, None);
    assert_eq!(other.saved(), None);
}

#[test]
fn enter_claims_a_codex_broadcast_with_the_matching_directory() {
    let mut capture = Capture::new(None, Some(Arc::new(AgentTrackerRegistry::new())));
    capture.set_cwd("/work".to_string());
    capture.submit(1_000);
    let change = capture.report(
        &report(
            "codex",
            AgentReportState::Processing,
            "abc-123",
            AgentScope::Broadcast,
            Some("/work"),
        ),
        1_000,
    );
    assert_eq!(
        change,
        Some(Some(session("codex", "abc-123", Some("/work"))))
    );
}

#[test]
fn a_restore_starts_disabled_unless_the_workspace_has_a_record() {
    let mut disabled = Restore::disabled();
    assert!(!disabled.is_enabled());
    assert!(!disabled.cancel());
    assert_eq!(disabled.ready(Some("bash"), true), None);
    assert!(disabled.notice().is_none());
}

#[test]
fn a_missing_shell_offers_no_manual_resume() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    let notice = restore.ready(None, true).expect("unknown shell notice");
    assert_eq!(notice.reason, NoticeReason::UnknownShell);
    assert!(!notice.enabled);
    assert!(!restore.wants_send());
    assert!(!restore.is_waiting());
}

#[test]
fn a_disabled_restore_never_shows_a_notice() {
    let mut restore = Restore::disabled();
    assert!(!restore.failed(NoticeReason::Failed, true));
    assert!(restore.notice().is_none());
}

#[test]
fn cmd_and_missing_command_marks_need_a_manual_resume() {
    for (shell, marks) in [(Some("cmd"), true), (Some("bash"), false)] {
        let mut restore = Restore::waiting(session("codex", "abc-123", None));
        let notice = restore.ready(shell, marks).expect("unavailable notice");
        assert_eq!(notice.reason, NoticeReason::Unavailable);
        assert!(notice.enabled);
        assert!(!restore.wants_send());
        assert!(!restore.is_waiting());
    }
}

#[test]
fn a_prompt_makes_a_ready_restore_send() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    assert_eq!(restore.ready(Some("/bin/zsh"), true), None);
    assert!(restore.is_waiting());
    assert!(!restore.wants_send());
    restore.prompt();
    assert!(restore.wants_send());
    assert_eq!(restore.timeout(), None, "a prompt arrived before the timer");
}

#[test]
fn a_prompt_before_a_known_shell_does_not_want_to_send() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    restore.prompt();
    assert!(
        !restore.wants_send(),
        "a prompt without an authenticated shell is not enough"
    );
    assert_eq!(restore.ready(Some("bash"), true), None);
    assert!(restore.wants_send(), "readiness completes the same prompt");
}

#[test]
fn the_first_prompt_timeout_offers_the_manual_button() {
    assert_eq!(PROMPT_TIMEOUT, Duration::from_secs(15));
    let mut restore = Restore::waiting(session("claudeCode", "abc-123", None));
    assert_eq!(restore.ready(Some("bash"), true), None);
    let notice = restore.timeout().expect("timeout notice");
    assert_eq!(notice.reason, NoticeReason::Timeout);
    assert!(notice.enabled);
    assert_eq!(restore.notice(), Some(&notice));
    assert!(!restore.wants_send());
    assert!(!restore.is_waiting());
}

#[test]
fn user_input_cancels_a_waiting_restore_or_its_notice() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    assert_eq!(restore.ready(Some("bash"), true), None);
    assert!(restore.cancel());
    assert!(!restore.cancel(), "cancelling twice changes nothing");
    assert!(restore.notice().is_none());

    let mut timed_out = Restore::waiting(session("codex", "abc-123", None));
    timed_out.ready(Some("bash"), true);
    timed_out.timeout().expect("timeout notice");
    assert!(timed_out.cancel());
    assert!(timed_out.notice().is_none());
}

#[test]
fn a_sent_restore_ignores_later_prompts_and_cancellation() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    restore.ready(Some("bash"), true);
    restore.prompt();
    restore.sent();
    assert!(!restore.wants_send());
    assert!(!restore.cancel());
    restore.prompt();
    assert!(!restore.wants_send(), "the command is only sent once");
}

#[test]
fn command_started_keeps_a_waiting_restore_from_sending() {
    let mut restore = Restore::waiting(session("codex", "abc-123", None));
    restore.ready(Some("bash"), true);
    restore.prompt();
    restore.command_started();
    assert!(!restore.wants_send());
}
