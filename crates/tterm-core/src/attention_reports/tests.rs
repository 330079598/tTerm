use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use super::AttentionReports;
use crate::agent_status::{
    AgentResumeSession, AgentTracker, AgentTrackerCallbacks, AgentTrackerRegistry,
};
use crate::terminal_notifications::TerminalAttentionEvent;

struct Tab {
    tracker: AgentTracker,
    reports: AttentionReports,
    sessions: Rc<RefCell<Vec<Option<AgentResumeSession>>>>,
}

fn tab(registry: &Arc<AgentTrackerRegistry>, cwd: Option<&str>) -> Tab {
    let shell_cwd = cwd.map(str::to_string);
    let tracker = AgentTracker::with_registry(
        Arc::clone(registry),
        AgentTrackerCallbacks {
            initial_session: None,
            shell_cwd: Some(Box::new(move || shell_cwd.clone())),
            on_status: Box::new(|_| {}),
            on_agent: Box::new(|_| {}),
            on_announce: Box::new(|_| {}),
        },
        0,
    );
    let sessions = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&sessions);
    let reports = AttentionReports::new(|_| {}, move |session| sink.borrow_mut().push(session));
    Tab {
        tracker,
        reports,
        sessions,
    }
}

#[test]
fn direct_claude_reports_persist_in_the_receiving_tab_and_broadcasts_cannot_adopt_another() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let mut a = tab(&registry, None);
    let mut b = tab(&registry, None);
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;claude;processing;session-a;direct;%2Fsame",
        1,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;claude;processing;session-b;direct;%2Fsame",
        1,
    );
    assert_eq!(
        a.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "claudeCode".to_string(),
            session_id: "session-a".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    assert_eq!(
        b.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "claudeCode".to_string(),
            session_id: "session-b".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    b.sessions.borrow_mut().clear();
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;claude;done;session-a;broadcast;%2Fother",
        2,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;claude;done;session-a;broadcast;%2Fother",
        2,
    );
    assert_eq!(
        a.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "claudeCode".to_string(),
            session_id: "session-a".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    assert!(b.sessions.borrow().is_empty());
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;claude;ended;session-a;broadcast;",
        3,
    );
    assert!(b.sessions.borrow().is_empty());
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;claude;ended;session-a;broadcast;",
        3,
    );
    assert_eq!(a.sessions.borrow().last().cloned(), Some(None));
}

#[test]
fn direct_codex_reports_persist_in_the_receiving_tab_and_broadcasts_cannot_adopt_another() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let mut a = tab(&registry, None);
    let mut b = tab(&registry, None);
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;processing;session-a;direct;%2Fsame",
        1,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;codex;processing;session-b;direct;%2Fsame",
        1,
    );
    assert_eq!(
        a.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "codex".to_string(),
            session_id: "session-a".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    assert_eq!(
        b.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "codex".to_string(),
            session_id: "session-b".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    b.sessions.borrow_mut().clear();
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;done;session-a;broadcast;%2Fother",
        2,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;codex;done;session-a;broadcast;%2Fother",
        2,
    );
    assert_eq!(
        a.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "codex".to_string(),
            session_id: "session-a".to_string(),
            cwd: Some("/same".to_string()),
        })
    );
    assert!(b.sessions.borrow().is_empty());
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;codex;ended;session-a;broadcast;",
        3,
    );
    assert!(b.sessions.borrow().is_empty());
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;ended;session-a;broadcast;",
        3,
    );
    assert_eq!(a.sessions.borrow().last().cloned(), Some(None));
}

#[test]
fn unowned_broadcasts_and_malformed_cwd_never_reach_persistence() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let mut a = tab(&registry, None);
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;processing;unknown;broadcast;%2Ftmp",
        1,
    );
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;claude;processing;abc-123;direct;%0A",
        1,
    );
    assert!(a.sessions.borrow().is_empty());
}

#[test]
fn persists_a_sole_claimant_but_not_competing_claims() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let mut a = tab(&registry, Some("/tmp"));
    let mut b = tab(&registry, Some("/tmp"));
    a.reports.submit(&mut a.tracker, 10);
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;processing;sole;broadcast;%2Ftmp",
        10,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;codex;processing;sole;broadcast;%2Ftmp",
        10,
    );
    assert_eq!(
        a.sessions.borrow().last().cloned().flatten(),
        Some(AgentResumeSession {
            agent: "codex".to_string(),
            session_id: "sole".to_string(),
            cwd: Some("/tmp".to_string()),
        })
    );
    assert!(b.sessions.borrow().is_empty());
    a.sessions.borrow_mut().clear();
    a.reports.submit(&mut a.tracker, 20);
    b.reports.submit(&mut b.tracker, 20);
    a.reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;processing;contested;broadcast;%2Ftmp",
        20,
    );
    b.reports.handle_osc(
        &mut b.tracker,
        777,
        "tterm-agent;codex;processing;contested;broadcast;%2Ftmp",
        20,
    );
    assert!(a.sessions.borrow().is_empty());
    assert!(b.sessions.borrow().is_empty());
}

#[test]
fn forwards_notification_requests_and_bells() {
    let mut tracker = AgentTracker::new(
        AgentTrackerCallbacks {
            initial_session: None,
            shell_cwd: None,
            on_status: Box::new(|_| {}),
            on_agent: Box::new(|_| {}),
            on_announce: Box::new(|_| {}),
        },
        0,
    );
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    let mut reports = AttentionReports::new(move |event| sink.borrow_mut().push(event), |_| {});
    reports.handle_osc(&mut tracker, 777, "notify;title;body", 1);
    reports.handle_bell();
    assert_eq!(
        *events.borrow(),
        vec![
            TerminalAttentionEvent::Request {
                title: Some("title".to_string()),
                body: "body".to_string(),
            },
            TerminalAttentionEvent::Bell,
        ]
    );
}

#[test]
fn ignores_notifications_an_agent_repeats_after_its_report() {
    let registry = Arc::new(AgentTrackerRegistry::new());
    let mut a = tab(&registry, None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&events);
    let mut reports = AttentionReports::new(move |event| sink.borrow_mut().push(event), |_| {});
    reports.handle_osc(
        &mut a.tracker,
        777,
        "tterm-agent;codex;processing;agent;direct;%2Ftmp",
        1,
    );
    reports.handle_osc(&mut a.tracker, 777, "notify;title;body", 2);
    assert!(events.borrow().is_empty());
}
