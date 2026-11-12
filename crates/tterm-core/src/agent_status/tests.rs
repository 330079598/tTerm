use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::json;

use super::{
    AGENT_CLAIM_WINDOW_MS, AGENT_WAIT_ANNOUNCE_DELAY_MS, AgentActivity, AgentAnnouncement,
    AgentAnnouncementState, AgentReport, AgentReportState, AgentResumeSession, AgentScope,
    AgentStatus, AgentTracker, AgentTrackerCallbacks, AgentTrackerRegistry, agent_display_name,
    agent_resume_cwd_matches, parse_agent_report,
};

#[derive(Clone, Default)]
struct Harness {
    statuses: Rc<RefCell<Vec<Option<AgentStatus>>>>,
    agents: Rc<RefCell<Vec<Option<String>>>>,
    announcements: Rc<RefCell<Vec<AgentAnnouncement>>>,
    shell_cwd: Rc<RefCell<Option<String>>>,
}

impl Harness {
    fn callbacks(&self, initial_session: Option<AgentResumeSession>) -> AgentTrackerCallbacks {
        AgentTrackerCallbacks {
            initial_session,
            shell_cwd: Some({
                let shell_cwd = self.shell_cwd.clone();
                Box::new(move || shell_cwd.borrow().clone())
            }),
            on_status: {
                let statuses = self.statuses.clone();
                Box::new(move |status| statuses.borrow_mut().push(status))
            },
            on_agent: {
                let agents = self.agents.clone();
                Box::new(move |agent| agents.borrow_mut().push(agent))
            },
            on_announce: {
                let announcements = self.announcements.clone();
                Box::new(move |announcement| announcements.borrow_mut().push(announcement))
            },
        }
    }
}

fn tracker_on(registry: Arc<AgentTrackerRegistry>, harness: Harness) -> AgentTracker {
    AgentTracker::with_registry(registry, harness.callbacks(None), 3000)
}

#[test]
fn keeps_the_frontend_delays() {
    assert_eq!(AGENT_WAIT_ANNOUNCE_DELAY_MS, 3000);
    assert_eq!(AGENT_CLAIM_WINDOW_MS, 10_000);
}

fn tracker() -> (AgentTracker, Harness) {
    let harness = Harness::default();
    let tracker = tracker_on(Arc::new(AgentTrackerRegistry::new()), harness.clone());
    (tracker, harness)
}

fn tracker_with_shell(shell: Option<&str>) -> (AgentTracker, Harness) {
    let harness = Harness::default();
    *harness.shell_cwd.borrow_mut() = shell.map(str::to_owned);
    let tracker = tracker_on(Arc::new(AgentTrackerRegistry::new()), harness.clone());
    (tracker, harness)
}

fn two_trackers() -> ((AgentTracker, Harness), (AgentTracker, Harness)) {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let first = Harness::default();
    let second = Harness::default();
    (
        (tracker_on(registry.clone(), first.clone()), first),
        (tracker_on(registry, second.clone()), second),
    )
}

fn report(agent: &str, state: AgentReportState) -> AgentReport {
    AgentReport {
        agent: agent.to_owned(),
        state,
        session: None,
        scope: None,
        cwd: None,
    }
}

fn session_report(
    agent: &str,
    state: AgentReportState,
    session: &str,
    scope: Option<AgentScope>,
    cwd: Option<&str>,
) -> AgentReport {
    AgentReport {
        agent: agent.to_owned(),
        state,
        session: Some(session.to_owned()),
        scope,
        cwd: cwd.map(str::to_owned),
    }
}

fn processing(agent: &str) -> AgentStatus {
    AgentStatus {
        agent: agent.to_owned(),
        activity: AgentActivity::Processing,
    }
}

fn waiting(agent: &str) -> AgentStatus {
    AgentStatus {
        agent: agent.to_owned(),
        activity: AgentActivity::Waiting,
    }
}

#[test]
fn reads_an_agent_and_its_state() {
    assert_eq!(
        parse_agent_report("tterm-agent;claude;processing"),
        Some(report("claude", AgentReportState::Processing))
    );
    assert_eq!(
        parse_agent_report("tterm-agent;my-agent2;ended"),
        Some(report("my-agent2", AgentReportState::Ended))
    );
}

#[test]
fn reads_the_session_a_report_names() {
    let mut expected = report("codex", AgentReportState::Done);
    expected.session = Some("01a12490-4b68-76d0".to_owned());
    assert_eq!(
        parse_agent_report("tterm-agent;codex;done;01a12490-4b68-76d0"),
        Some(expected)
    );
}

#[test]
fn distinguishes_direct_sessions_from_broadcasts_and_rejects_unknown_scope() {
    for scope in [AgentScope::Direct, AgentScope::Broadcast] {
        let name = match scope {
            AgentScope::Direct => "direct",
            AgentScope::Broadcast => "broadcast",
        };
        assert_eq!(
            parse_agent_report(&format!("tterm-agent;codex;done;abc-123;{name}")),
            Some(session_report(
                "codex",
                AgentReportState::Done,
                "abc-123",
                Some(scope),
                None
            ))
        );
    }
    assert_eq!(
        parse_agent_report("tterm-agent;codex;done;abc-123;other"),
        None
    );
}

#[test]
fn leaves_other_osc777_commands_alone() {
    assert_eq!(parse_agent_report("notify;tterm-agent;done"), None);
    assert_eq!(parse_agent_report("precmd"), None);
}

#[test]
fn decodes_hook_cwd_without_allowing_protocol_delimiters_or_control_sequences() {
    assert_eq!(
        parse_agent_report(
            "tterm-agent;claude;processing;abc-123;direct;C%3A%5CJohn%E2%80%99s%20files%3Bx"
        )
        .and_then(|report| report.cwd),
        Some("C:\\John’s files;x".to_owned())
    );
    for cwd in ["%0A", "%0D", "%09", "%00", "%1B", "%C2%85", "%FF", "%"] {
        assert_eq!(
            parse_agent_report(&format!("tterm-agent;codex;done;abc-123;direct;{cwd}")),
            None
        );
    }
    let longest = "a".repeat(16_384);
    assert!(
        parse_agent_report(&format!("tterm-agent;codex;done;abc-123;direct;{longest}")).is_some()
    );
    let too_long = "a".repeat(16_385);
    assert_eq!(
        parse_agent_report(&format!("tterm-agent;codex;done;abc-123;direct;{too_long}")),
        None
    );
}

#[test]
fn limits_the_hook_cwd_by_utf16_units() {
    for (count, accepted) in [(8192, true), (8193, false)] {
        let cwd = "😀".repeat(count);
        assert_eq!(
            parse_agent_report(&format!("tterm-agent;codex;done;abc-123;direct;{cwd}")).is_some(),
            accepted,
            "{count} emoji"
        );
    }
}

#[test]
fn rejects_unknown_states_odd_names_and_extra_fields() {
    assert_eq!(parse_agent_report("tterm-agent;claude;sleeping"), None);
    assert_eq!(parse_agent_report("tterm-agent;Claude;done"), None);
    assert_eq!(parse_agent_report("tterm-agent;;done"), None);
    assert_eq!(parse_agent_report("tterm-agent;claude;done;x;y"), None);
    assert_eq!(parse_agent_report("tterm-agent;claude;done;"), None);
    assert_eq!(parse_agent_report("tterm-agent;claude;done;a b"), None);
    assert_eq!(parse_agent_report("tterm-agent;claude"), None);
}

#[test]
fn names_known_agents_and_keeps_others_as_they_are() {
    assert_eq!(agent_display_name("claude"), "Claude Code");
    assert_eq!(agent_display_name("codex"), "Codex");
    assert_eq!(agent_display_name("opencode"), "OpenCode");
    assert_eq!(agent_display_name("pi"), "pi");
    assert_eq!(agent_display_name("aider"), "aider");
}

#[test]
fn serializes_the_wire_shapes_the_frontend_uses() {
    let parsed =
        parse_agent_report("tterm-agent;codex;done;abc-123;direct;C%3A%5CJohn%E2%80%99s%20files")
            .unwrap_or_else(|| report("codex", AgentReportState::Done));
    assert_eq!(
        serde_json::to_value(&parsed).unwrap_or_default(),
        json!({
            "agent": "codex",
            "state": "done",
            "session": "abc-123",
            "scope": "direct",
            "cwd": "C:\\John’s files"
        })
    );
    assert_eq!(
        serde_json::to_value(processing("claude")).unwrap_or_default(),
        json!({"agent": "claude", "activity": "processing"})
    );
    assert_eq!(
        serde_json::to_value(AgentAnnouncement {
            agent: "claude".to_owned(),
            state: AgentAnnouncementState::Waiting,
        })
        .unwrap_or_default(),
        json!({"agent": "claude", "state": "waiting"})
    );
    assert_eq!(
        serde_json::to_value(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "abc-123".to_owned(),
            cwd: None,
        })
        .unwrap_or_default(),
        json!({"agent": "codex", "sessionId": "abc-123"})
    );
    let saved = json!({"agent": "codex", "sessionId": "abc-123", "cwd": "/remote work"});
    let resume: Option<AgentResumeSession> = serde_json::from_value(saved.clone()).ok();
    assert_eq!(
        resume,
        Some(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "abc-123".to_owned(),
            cwd: Some("/remote work".to_owned()),
        })
    );
    assert_eq!(
        serde_json::to_value(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "abc-123".to_owned(),
            cwd: Some("/remote work".to_owned()),
        })
        .unwrap_or_default(),
        saved
    );
}

#[test]
fn shows_work_and_announces_the_end_of_a_turn() {
    let (mut tracker, harness) = tracker();
    assert!(tracker.report(&report("claude", AgentReportState::Processing), 0));
    assert!(tracker.report(&report("claude", AgentReportState::Processing), 0));
    assert_eq!(*harness.statuses.borrow(), vec![Some(processing("claude"))]);
    assert!(tracker.speaks_for_agent(0));

    assert!(tracker.report(&report("claude", AgentReportState::Done), 0));
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
    assert_eq!(
        *harness.announcements.borrow(),
        vec![AgentAnnouncement {
            agent: "claude".to_owned(),
            state: AgentAnnouncementState::Done,
        }]
    );
    assert!(tracker.speaks_for_agent(0));
    assert!(!tracker.speaks_for_agent(10_000));
}

#[test]
fn announces_a_wait_only_once_it_lasts() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("claude", AgentReportState::Waiting), 0);
    assert_eq!(*harness.statuses.borrow(), vec![Some(waiting("claude"))]);
    tracker.poll(2999);
    assert!(harness.announcements.borrow().is_empty());
    tracker.poll(3000);
    assert_eq!(
        *harness.announcements.borrow(),
        vec![AgentAnnouncement {
            agent: "claude".to_owned(),
            state: AgentAnnouncementState::Waiting,
        }]
    );
}

#[test]
fn forgets_a_wait_answered_in_time() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("claude", AgentReportState::Waiting), 0);
    tracker.poll(1000);
    tracker.report(&report("claude", AgentReportState::Processing), 1000);
    tracker.poll(6000);
    assert!(harness.announcements.borrow().is_empty());
    assert_eq!(
        harness.statuses.borrow().last(),
        Some(&Some(processing("claude")))
    );
}

#[test]
fn speaks_for_an_agent_at_work_however_long_it_takes() {
    let (mut tracker, _) = tracker();
    tracker.report(&report("claude", AgentReportState::Processing), 0);
    assert!(tracker.speaks_for_agent(600_000));
}

#[test]
fn clears_the_status_on_an_interrupt() {
    let (mut tracker, harness) = tracker();
    tracker.interrupt();
    assert!(harness.statuses.borrow().is_empty());
    tracker.report(&report("claude", AgentReportState::Waiting), 0);
    tracker.interrupt();
    tracker.poll(5000);
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
    assert!(harness.announcements.borrow().is_empty());
}

#[test]
fn leaves_stops_to_an_agent_that_reports_them() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("codex", AgentReportState::Processing), 0);
    tracker.interrupt();
    assert_eq!(*harness.statuses.borrow(), vec![Some(processing("codex"))]);
}

#[test]
fn works_on_once_a_wait_is_answered() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("codex", AgentReportState::Waiting), 0);
    tracker.answer();
    tracker.poll(5000);
    assert_eq!(
        harness.statuses.borrow().last(),
        Some(&Some(processing("codex")))
    );
    assert!(harness.announcements.borrow().is_empty());
    tracker.report(&report("codex", AgentReportState::Idle), 5000);
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
}

#[test]
fn keeps_waiting_where_an_answer_may_be_a_refusal_nobody_reports() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("claude", AgentReportState::Waiting), 0);
    tracker.answer();
    assert_eq!(*harness.statuses.borrow(), vec![Some(waiting("claude"))]);
    tracker.report(&report("codex", AgentReportState::Processing), 0);
    tracker.answer();
    assert_eq!(
        harness.statuses.borrow().last(),
        Some(&Some(processing("codex")))
    );
}

#[test]
fn goes_quiet_when_the_agent_reports_it_was_stopped() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("codex", AgentReportState::Waiting), 0);
    tracker.report(&report("codex", AgentReportState::Idle), 0);
    tracker.poll(5000);
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
    assert!(harness.announcements.borrow().is_empty());
    assert!(tracker.speaks_for_agent(5000));
}

#[test]
fn stops_on_the_end_of_the_session_or_a_reset() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("claude", AgentReportState::Waiting), 0);
    tracker.report(&report("claude", AgentReportState::Ended), 0);
    tracker.poll(5000);
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
    assert!(harness.announcements.borrow().is_empty());
    assert!(!tracker.speaks_for_agent(5000));

    tracker.report(&report("claude", AgentReportState::Processing), 5000);
    tracker.reset();
    assert_eq!(harness.statuses.borrow().last(), Some(&None));
    assert!(!tracker.speaks_for_agent(5000));
}

#[test]
fn names_the_agent_running_from_its_first_report_until_it_ends() {
    let (mut tracker, harness) = tracker();
    tracker.report(&report("claude", AgentReportState::Processing), 0);
    tracker.report(&report("claude", AgentReportState::Done), 0);
    tracker.report(&report("claude", AgentReportState::Idle), 0);
    assert_eq!(*harness.agents.borrow(), vec![Some("claude".to_owned())]);

    tracker.report(&report("claude", AgentReportState::Ended), 0);
    assert_eq!(
        *harness.agents.borrow(),
        vec![Some("claude".to_owned()), None]
    );

    tracker.report(&report("pi", AgentReportState::Waiting), 0);
    tracker.reset();
    assert_eq!(
        *harness.agents.borrow(),
        vec![Some("claude".to_owned()), None, Some("pi".to_owned()), None]
    );
}

#[test]
fn records_a_direct_session_without_enter_and_never_adopts_a_broadcast() {
    let ((mut a, _), (mut b, _)) = two_trackers();
    let direct = session_report(
        "codex",
        AgentReportState::Processing,
        "own-1",
        Some(AgentScope::Direct),
        None,
    );
    assert!(a.report(&direct, 0));
    assert_eq!(
        a.resume_session(&direct),
        Some(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "own-1".to_owned(),
            cwd: None,
        })
    );
    let broadcast = AgentReport {
        scope: Some(AgentScope::Broadcast),
        ..direct
    };
    assert!(a.report(&broadcast, 0));
    assert!(!b.report(&broadcast, 0));
    assert_eq!(b.resume_session(&broadcast), None);
}

#[test]
fn only_updates_the_exact_session_already_restored_in_this_tab() {
    let harness = Harness::default();
    let mut tracker = AgentTracker::with_registry(
        Arc::new(AgentTrackerRegistry::new()),
        harness.callbacks(Some(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "restored".to_owned(),
            cwd: None,
        })),
        3000,
    );
    assert!(tracker.report(
        &session_report(
            "codex",
            AgentReportState::Processing,
            "restored",
            Some(AgentScope::Broadcast),
            None,
        ),
        0
    ));
    assert!(!tracker.report(
        &session_report(
            "codex",
            AgentReportState::Processing,
            "other",
            Some(AgentScope::Broadcast),
            None,
        ),
        0
    ));
    assert!(!tracker.report(
        &session_report(
            "claude",
            AgentReportState::Processing,
            "restored",
            Some(AgentScope::Broadcast),
            None,
        ),
        0
    ));
}

#[test]
fn uses_shell_cwd_at_tool_start_and_freezes_it_for_that_session() {
    for shell in [None, Some("/shell start")] {
        let (mut tracker, harness) = tracker_with_shell(shell);
        let first = session_report(
            "claude",
            AgentReportState::Processing,
            "start-1",
            Some(AgentScope::Direct),
            Some("/hook start"),
        );
        tracker.report(&first, 0);
        assert_eq!(
            tracker.resume_session(&first).and_then(|resume| resume.cwd),
            shell
                .map(str::to_owned)
                .or_else(|| Some("/hook start".to_owned()))
        );
        *harness.shell_cwd.borrow_mut() = Some("/changed after start".to_owned());
        let later = AgentReport {
            cwd: Some("/hook later".to_owned()),
            ..first.clone()
        };
        tracker.report(&later, 0);
        assert_eq!(
            tracker.resume_session(&first).and_then(|resume| resume.cwd),
            shell
                .map(str::to_owned)
                .or_else(|| Some("/hook start".to_owned()))
        );
    }
}

#[test]
fn two_tabs_in_the_same_directory_ignore_each_others_broadcast_even_after_typing() {
    let ((mut a, _), (mut b, _)) = two_trackers();
    a.report(
        &session_report(
            "codex",
            AgentReportState::Processing,
            "session-a",
            Some(AgentScope::Direct),
            None,
        ),
        0,
    );
    a.answer();
    b.report(
        &session_report(
            "codex",
            AgentReportState::Processing,
            "session-b",
            Some(AgentScope::Direct),
            None,
        ),
        0,
    );
    b.answer();
    for scope in [Some(AgentScope::Broadcast), None] {
        let report = session_report("codex", AgentReportState::Done, "session-a", scope, None);
        assert!(a.report(&report, 0));
        assert!(!b.report(&report, 0));
        assert_eq!(b.resume_session(&report), None);
        let own = session_report("codex", AgentReportState::Done, "session-b", scope, None);
        assert_eq!(
            b.resume_session(&own).map(|resume| resume.session_id),
            Some("session-b".to_owned())
        );
    }
}

fn direct_session_is_cleared_by_state(agent: &str) {
    let (mut tracker, _) = tracker();
    let direct = session_report(
        agent,
        AgentReportState::Processing,
        "first",
        Some(AgentScope::Direct),
        None,
    );
    tracker.report(&direct, 0);
    tracker.report(
        &session_report(
            agent,
            AgentReportState::Processing,
            "second",
            Some(AgentScope::Direct),
            None,
        ),
        0,
    );
    assert!(!tracker.report(
        &session_report(
            agent,
            AgentReportState::Processing,
            "first",
            Some(AgentScope::Broadcast),
            None,
        ),
        0
    ));
    for state in [AgentReportState::Ended, AgentReportState::Idle] {
        tracker.report(&direct, 0);
        tracker.report(
            &session_report(agent, state, "first", Some(AgentScope::Broadcast), None),
            0,
        );
        assert_eq!(tracker.resume_session(&direct), None);
        assert_eq!(
            tracker.report(
                &session_report(
                    agent,
                    AgentReportState::Processing,
                    "first",
                    Some(AgentScope::Broadcast),
                    None,
                ),
                0
            ),
            state == AgentReportState::Idle
        );
        assert_eq!(tracker.resume_session(&direct), None);
    }
    tracker.report(&direct, 0);
    tracker.reset();
    assert!(!tracker.report(
        &session_report(
            agent,
            AgentReportState::Processing,
            "first",
            Some(AgentScope::Broadcast),
            None,
        ),
        0
    ));
}

#[test]
fn direct_claude_session_replaces_the_old_record_and_ended_idle_and_reset_clear_it() {
    direct_session_is_cleared_by_state("claude");
}

#[test]
fn direct_codex_session_replaces_the_old_record_and_ended_idle_and_reset_clear_it() {
    direct_session_is_cleared_by_state("codex");
}

fn claim_tracker(shell: Option<&str>) -> (AgentTracker, Harness) {
    let harness = Harness::default();
    *harness.shell_cwd.borrow_mut() = shell.map(str::to_owned);
    (
        tracker_on(Arc::new(AgentTrackerRegistry::new()), harness.clone()),
        harness,
    )
}

fn claim_pair(shell: Option<&str>) -> ((AgentTracker, Harness), (AgentTracker, Harness)) {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let first = Harness::default();
    let second = Harness::default();
    *first.shell_cwd.borrow_mut() = shell.map(str::to_owned);
    *second.shell_cwd.borrow_mut() = shell.map(str::to_owned);
    (
        (tracker_on(registry.clone(), first.clone()), first),
        (tracker_on(registry, second.clone()), second),
    )
}

#[test]
fn records_a_single_recent_claimant_and_keeps_later_broadcasts_and_its_start_cwd() {
    let ((mut a, ha), (mut b, hb)) = claim_pair(Some("/same directory"));
    let report = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    a.submit(0);
    assert!(!b.report(&report, 0));
    assert!(a.report(&report, 0));
    assert_eq!(*ha.statuses.borrow(), vec![Some(processing("codex"))]);
    assert!(hb.statuses.borrow().is_empty());
    assert_eq!(
        a.resume_session(&report),
        Some(AgentResumeSession {
            agent: "codex".to_owned(),
            session_id: "shared-session".to_owned(),
            cwd: Some("/same directory".to_owned()),
        })
    );
    let later = AgentReport {
        cwd: Some("/later".to_owned()),
        ..report
    };
    assert!(a.report(&later, AGENT_CLAIM_WINDOW_MS + 1));
    assert_eq!(
        a.resume_session(&later).and_then(|resume| resume.cwd),
        Some("/same directory".to_owned())
    );
}

#[test]
fn keeps_dev_first_receiver_status_routing_but_never_records_overlapping_claimants() {
    let ((mut a, ha), (mut b, hb)) = claim_pair(Some("/same directory"));
    let report = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    a.submit(0);
    b.submit(0);
    assert!(a.report(&report, 0));
    assert!(!b.report(&report, 0));
    assert_eq!(*ha.statuses.borrow(), vec![Some(processing("codex"))]);
    assert!(hb.statuses.borrow().is_empty());
    assert_eq!(a.resume_session(&report), None);
    assert_eq!(b.resume_session(&report), None);
    let now = AGENT_CLAIM_WINDOW_MS + 1;
    a.submit(now);
    a.report(&report, now);
    assert_eq!(a.resume_session(&report), None);
}

#[test]
fn never_records_or_displays_another_tabs_claimed_session_even_after_enter() {
    let ((mut a, ha), (mut b, hb)) = claim_pair(Some("/same directory"));
    let report = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    a.submit(0);
    a.report(&report, 0);
    b.submit(0);
    assert!(!b.report(&report, 0));
    assert!(!b.report(
        &AgentReport {
            scope: Some(AgentScope::Direct),
            ..report.clone()
        },
        0
    ));
    assert_eq!(b.resume_session(&report), None);
    assert!(hb.statuses.borrow().is_empty());
    let waiting_report = AgentReport {
        state: AgentReportState::Waiting,
        ..report
    };
    assert!(a.report(&waiting_report, 0));
    assert_eq!(ha.statuses.borrow().last(), Some(&Some(waiting("codex"))));
}

#[test]
fn does_not_forget_a_competing_enter_consumed_by_another_sessions_status_claim() {
    let ((mut a, _), (mut b, hb)) = claim_pair(Some("/same directory"));
    let report = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    a.submit(0);
    b.submit(0);
    let first = AgentReport {
        session: Some("first-session".to_owned()),
        ..report.clone()
    };
    a.report(&first, 0);
    b.report(&report, 0);
    assert_eq!(a.resume_session(&first), None);
    assert_eq!(b.resume_session(&report), None);
    assert_eq!(*hb.statuses.borrow(), vec![Some(processing("codex"))]);
}

#[test]
fn does_not_replace_the_current_resume_with_a_later_report_from_an_older_status_claim() {
    let (mut a, ha) = claim_tracker(Some("/same directory"));
    let old = session_report(
        "codex",
        AgentReportState::Processing,
        "old-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    let current = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        Some("/same directory"),
    );
    a.submit(0);
    a.report(&old, 0);
    a.submit(0);
    a.report(&current, 0);
    let done = AgentReport {
        state: AgentReportState::Done,
        ..old.clone()
    };
    a.report(&done, 0);
    assert_eq!(
        a.resume_session(&current).map(|resume| resume.session_id),
        Some("shared-session".to_owned())
    );
    assert_eq!(a.resume_session(&old), None);
    assert_eq!(ha.statuses.borrow().last(), Some(&None));
}

fn hook_cwd_gate(hook: Option<&str>, shell: &str, records: bool) {
    let ((mut a, ha), (mut b, hb)) = claim_pair(Some(shell));
    let report = session_report(
        "codex",
        AgentReportState::Processing,
        "shared-session",
        Some(AgentScope::Broadcast),
        hook,
    );
    a.submit(0);
    assert!(a.report(&report, 0));
    assert!(!b.report(&report, 0));
    assert_eq!(*ha.statuses.borrow(), vec![Some(processing("codex"))]);
    assert!(hb.statuses.borrow().is_empty());
    assert_eq!(a.resume_session(&report).is_some(), records);
    assert_eq!(b.resume_session(&report), None);
    let waiting_report = AgentReport {
        state: AgentReportState::Waiting,
        ..report
    };
    assert!(a.report(&waiting_report, 0));
    assert_eq!(ha.statuses.borrow().last(), Some(&Some(waiting("codex"))));
    assert_eq!(a.resume_session(&waiting_report).is_some(), records);
}

#[test]
fn hook_cwd_external_and_same_directory_gate_only_resume_not_dev_status() {
    hook_cwd_gate(Some("/external"), "/same directory", false);
}

#[test]
fn hook_cwd_missing_and_same_directory_gate_only_resume_not_dev_status() {
    hook_cwd_gate(None, "/same directory", false);
}

#[test]
fn hook_cwd_same_directory_and_empty_shell_gate_only_resume_not_dev_status() {
    hook_cwd_gate(Some("/same directory"), "", false);
}

#[test]
fn hook_cwd_same_directory_and_same_directory_records_resume() {
    hook_cwd_gate(Some("/same directory"), "/same directory", true);
}

#[test]
fn hook_cwd_windows_path_with_chinese_and_shell_cwd_records_resume() {
    hook_cwd_gate(Some("C:/WORK/中文 dir"), "c:\\work\\中文 dir", true);
}

#[test]
fn hook_cwd_other_windows_path_and_shell_cwd_gates_resume() {
    hook_cwd_gate(Some("C:/other"), "c:\\work", false);
}

#[test]
fn hook_and_shell_directories_match_like_the_frontend() {
    for (hook, shell) in [
        (
            Some("C:/Users/Stone/中文 dir"),
            Some("c:\\users\\stone\\中文 dir"),
        ),
        (Some("\\\\?\\C:\\work"), Some("c:/work")),
        (
            Some("\\\\?\\UNC\\HOST\\Share\\work"),
            Some("\\\\host\\share\\work"),
        ),
        (Some("/work/中文 dir"), Some("/work/中文 dir")),
    ] {
        assert!(agent_resume_cwd_matches(hook, shell), "{hook:?} {shell:?}");
    }
    for (hook, shell) in [
        (None, Some("/work")),
        (Some("/work"), None),
        (Some(""), Some("")),
        (Some("C:/work"), Some("C:/external")),
        (Some("/Work"), Some("/work")),
        (Some("/a\\b"), Some("/a/b")),
        (Some("/work\n"), Some("/work\n")),
    ] {
        assert!(!agent_resume_cwd_matches(hook, shell), "{hook:?} {shell:?}");
    }
}

fn dev_trackers() -> ((AgentTracker, Harness), (AgentTracker, Harness)) {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let first = Harness::default();
    let second = Harness::default();
    (
        (tracker_on(registry.clone(), first.clone()), first),
        (tracker_on(registry, second.clone()), second),
    )
}

fn broadcast(trackers: &mut [&mut AgentTracker], state: AgentReportState, session: &str, now: u64) {
    for tracker in trackers {
        tracker.report(
            &AgentReport {
                agent: "codex".to_owned(),
                state,
                session: Some(session.to_owned()),
                scope: None,
                cwd: None,
            },
            now,
        );
    }
}

#[test]
fn belongs_to_the_terminal_it_was_submitted_from() {
    let ((mut first, hf), (mut second, hs)) = dev_trackers();
    second.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s1",
        1500,
    );
    assert!(hf.statuses.borrow().is_empty());
    assert_eq!(*hs.statuses.borrow(), vec![Some(processing("codex"))]);

    first.submit(1500);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Done,
        "s1",
        1500,
    );
    assert!(hf.announcements.borrow().is_empty());
    assert_eq!(
        *hs.announcements.borrow(),
        vec![AgentAnnouncement {
            agent: "codex".to_owned(),
            state: AgentAnnouncementState::Done,
        }]
    );
}

#[test]
fn is_nobodys_without_a_recent_enter() {
    let ((mut first, hf), (mut second, hs)) = dev_trackers();
    first.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s1",
        AGENT_CLAIM_WINDOW_MS + 1,
    );
    assert!(hf.statuses.borrow().is_empty());
    assert!(hs.statuses.borrow().is_empty());
}

#[test]
fn takes_one_session_per_enter() {
    let ((mut first, hf), (mut second, hs)) = dev_trackers();
    first.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s1",
        0,
    );
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s2",
        0,
    );
    assert_eq!(*hf.statuses.borrow(), vec![Some(processing("codex"))]);
    assert!(hs.statuses.borrow().is_empty());
}

#[test]
fn is_free_again_once_it_ends_or_its_terminal_resets() {
    let ((mut first, hf), (mut second, hs)) = dev_trackers();
    first.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s1",
        0,
    );
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Ended,
        "s1",
        0,
    );
    second.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Processing,
        "s1",
        0,
    );
    assert_eq!(*hs.statuses.borrow(), vec![Some(processing("codex"))]);

    second.reset();
    first.submit(0);
    broadcast(
        &mut [&mut first, &mut second],
        AgentReportState::Done,
        "s1",
        0,
    );
    assert_eq!(
        *hf.announcements.borrow(),
        vec![AgentAnnouncement {
            agent: "codex".to_owned(),
            state: AgentAnnouncementState::Done,
        }]
    );
}
