use std::cell::RefCell;
use std::rc::Rc;

use crate::workspace::tab_attention::{
    AgentActivity, AgentStatus, RUNNING_COMMAND_DELAY_MS, TabAttentionLevel, TabAttentionStore,
    stronger_attention,
};

use super::ids;

#[test]
fn counts_notifications_across_tabs_until_each_tab_is_seen() {
    let mut store = TabAttentionStore::new();
    let totals = Rc::new(RefCell::new(Vec::new()));
    let recorder = totals.clone();
    let stop = store.watch_unseen_notification_total(move |total| {
        recorder.borrow_mut().push(total);
    });

    store.count_tab_notification("tab-1");
    store.count_tab_notification("tab-1");
    store.count_tab_notification("tab-2");
    store.clear_tab_attention("tab-1");
    store.clear_tab_attention("tab-2");
    store.stop_watching_unseen_notification_total(stop);

    assert_eq!(*totals.borrow(), vec![0, 1, 2, 3, 1, 0]);
}

#[test]
fn stays_quiet_when_only_a_tabs_mark_changes() {
    let mut store = TabAttentionStore::new();
    let calls = Rc::new(RefCell::new(0));
    let counter = calls.clone();
    let stop = store.watch_unseen_notification_total(move |_| {
        *counter.borrow_mut() += 1;
    });

    store.mark_tab_attention("tab-1", TabAttentionLevel::Bell);
    store.clear_tab_attention("tab-1");
    store.stop_watching_unseen_notification_total(stop);

    assert_eq!(*calls.borrow(), 1);
}

#[test]
fn stops_hearing_once_unsubscribed() {
    let mut store = TabAttentionStore::new();
    let calls = Rc::new(RefCell::new(0));
    let counter = calls.clone();
    let stop = store.watch_unseen_notification_total(move |_| {
        *counter.borrow_mut() += 1;
    });
    store.stop_watching_unseen_notification_total(stop);

    store.count_tab_notification("tab-1");

    assert_eq!(*calls.borrow(), 1);
}

#[test]
fn shows_a_command_once_it_has_run_a_moment_and_hides_it_when_it_ends() {
    let mut store = TabAttentionStore::new();
    store.set_tab_running_command("tab-1", Some("npm run dev"), 0);
    assert_eq!(store.running_commands().get("tab-1"), None);
    store.advance_to(RUNNING_COMMAND_DELAY_MS);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        Some("npm run dev")
    );
    store.set_tab_running_command("tab-1", None, RUNNING_COMMAND_DELAY_MS);
    assert_eq!(store.running_commands().get("tab-1"), None);
}

#[test]
fn never_shows_a_command_that_ends_at_once() {
    let mut store = TabAttentionStore::new();
    store.set_tab_running_command("tab-1", Some("cd src"), 0);
    store.set_tab_running_command("tab-1", None, 0);
    store.advance_to(RUNNING_COMMAND_DELAY_MS * 2);
    assert_eq!(store.running_commands().get("tab-1"), None);
}

#[test]
fn marks_a_tab_of_the_bar_while_any_of_its_panes_waits_for_a_password() {
    let mut store = TabAttentionStore::new();
    let panes = ids(&["pane-1", "pane-2"]);
    assert!(!store.tab_password_prompt(&panes));
    store.set_tab_password_prompt("pane-2", true);
    assert!(store.tab_password_prompt(&panes));
    store.set_tab_password_prompt("pane-2", false);
    assert!(!store.tab_password_prompt(&panes));
}

#[test]
fn oracle_waits_the_full_delay_before_showing_a_command() {
    let oracle = super::oracle_json();
    let expected = &oracle["tabAttention"];
    let delay = expected["commandScheduledDelay"]
        .as_u64()
        .expect("the command delay");
    let mut store = TabAttentionStore::new();
    store.set_tab_running_command("tab-1", Some("npm run dev"), 0);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["commandImmediately"].as_str()
    );
    store.advance_to(delay - 1);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["commandImmediately"].as_str()
    );
    store.advance_to(delay);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["commandAfterDelay"].as_str()
    );
}

#[test]
fn oracle_replaces_a_pending_command_with_the_newer_one() {
    let oracle = super::oracle_json();
    let expected = &oracle["tabAttention"];
    let mut store = TabAttentionStore::new();
    store.set_tab_running_command("tab-1", Some("cd src"), 0);
    store.set_tab_running_command("tab-1", Some("cd /tmp"), 100);
    assert_eq!(expected["replacementPendingTimers"], 1);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["replacementBeforeDelay"].as_str()
    );
    store.advance_to(999);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["replacementBeforeDelay"].as_str()
    );
    store.advance_to(1100);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["replacementAfterDelay"].as_str()
    );
    store.set_tab_running_command("tab-1", None, 1100);
    assert_eq!(
        store.running_commands().get("tab-1").map(String::as_str),
        expected["replacementAfterClear"].as_str()
    );
}

#[test]
fn oracle_ranks_attention_and_aggregates_tabs() {
    let mut store = TabAttentionStore::new();
    let panes = ids(&["pane-1", "pane-2"]);
    store.set_tab_agent_status(
        "pane-1",
        Some(AgentStatus {
            agent: "claude".to_owned(),
            activity: AgentActivity::Waiting,
        }),
    );
    store.set_tab_running_agent("pane-2", Some("codex"));
    store.mark_tab_attention("pane-1", TabAttentionLevel::Bell);
    store.mark_tab_attention("pane-2", TabAttentionLevel::Activity);
    assert_eq!(store.tab_attention(&panes), Some(TabAttentionLevel::Bell));
    assert_eq!(store.tab_agent_status(&panes).unwrap().agent, "claude");
    assert_eq!(store.tab_running_agent(&panes), Some("codex"));
    assert_eq!(store.tab_agent_status(&ids(&["missing"])), None);
    assert_eq!(store.tab_running_agent(&ids(&["missing"])), None);
    assert_eq!(store.tab_attention(&ids(&["missing"])), None);
    assert_eq!(
        stronger_attention(Some(TabAttentionLevel::Activity), TabAttentionLevel::Bell),
        TabAttentionLevel::Bell
    );
    assert_eq!(
        stronger_attention(
            Some(TabAttentionLevel::Notification),
            TabAttentionLevel::Bell
        ),
        TabAttentionLevel::Notification
    );
    assert_eq!(
        stronger_attention(None, TabAttentionLevel::Activity),
        TabAttentionLevel::Activity
    );
    store.set_tab_running_agent("pane-2", None);
    store.set_tab_agent_status("pane-1", None);
    assert_eq!(store.tab_running_agent(&panes), None);
    assert_eq!(store.tab_agent_status(&panes), None);
}

#[test]
fn oracle_clears_a_tabs_mark_and_notifications_together() {
    let oracle = super::oracle_json();
    let expected = &oracle["tabAttention"];
    let mut store = TabAttentionStore::new();
    store.mark_tab_attention("tab-1", TabAttentionLevel::Notification);
    store.count_tab_notification("tab-1");
    assert_eq!(
        store.get_tab_attention("tab-1").map(|level| match level {
            TabAttentionLevel::Activity => "activity",
            TabAttentionLevel::Bell => "bell",
            TabAttentionLevel::Notification => "notification",
        }),
        expected["clearBefore"]["mark"].as_str()
    );
    assert_eq!(
        store.unseen_notification_total(),
        expected["clearBefore"]["total"]
            .as_u64()
            .expect("the notification total")
    );
    store.clear_tab_attention("tab-1");
    assert_eq!(
        store.get_tab_attention("tab-1").map(|level| match level {
            TabAttentionLevel::Activity => "activity",
            TabAttentionLevel::Bell => "bell",
            TabAttentionLevel::Notification => "notification",
        }),
        expected["clearAfter"]["mark"].as_str()
    );
    assert_eq!(
        store.unseen_notification_total(),
        expected["clearAfter"]["total"]
            .as_u64()
            .expect("the notification total")
    );
}
