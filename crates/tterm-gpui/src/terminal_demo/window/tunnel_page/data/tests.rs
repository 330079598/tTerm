//! The tunnel data entity's ordering, stale-result and polling behaviour.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::{AppContext, TestAppContext};
use tterm_core::events::tunnel::{TunnelState, TunnelStatus, TunnelTraffic};
use tterm_core::profiles::launch::SavedSshProfileSummary;
use tterm_core::tunnel::manager::TunnelManager;
use tterm_core::tunnel::types::{TunnelKind, TunnelRule};

use super::load::{TunnelPageSnapshot, build_model};
use super::{Loader, TunnelPageData, TunnelPageState};

fn rule(id: &str, profile_id: &str, name: &str) -> TunnelRule {
    TunnelRule {
        id: id.to_string(),
        name: name.to_string(),
        profile_id: profile_id.to_string(),
        kind: TunnelKind::Local,
        bind_host: "127.0.0.1".to_string(),
        bind_port: 8080,
        dest_host: "db.internal".to_string(),
        dest_port: 5432,
        auto_start: false,
    }
}

fn profile(id: &str, name: &str, username: &str, host: &str) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some(username.to_string()),
        host: Some(host.to_string()),
        port: Some(22),
    }
}

fn status(id: &str, state: TunnelState) -> TunnelStatus {
    let mut status = TunnelStatus::stopped(id, TunnelTraffic::default());
    status.state = state;
    status
}

fn snapshot(
    rules: Vec<TunnelRule>,
    statuses: Vec<TunnelStatus>,
    profiles: Vec<SavedSshProfileSummary>,
) -> TunnelPageSnapshot {
    TunnelPageSnapshot {
        rules,
        statuses,
        profiles,
    }
}

fn stopped_snapshot() -> TunnelPageSnapshot {
    snapshot(
        vec![rule("alpha", "prod", "Database")],
        vec![status("alpha", TunnelState::Stopped)],
        vec![profile("prod", "Production", "deploy", "example.com")],
    )
}

fn running_snapshot() -> TunnelPageSnapshot {
    snapshot(
        vec![rule("alpha", "prod", "Database")],
        vec![status("alpha", TunnelState::Running)],
        vec![profile("prod", "Production", "deploy", "example.com")],
    )
}

fn ready_loader(snapshot: TunnelPageSnapshot) -> Loader {
    Arc::new(move || {
        let snapshot = snapshot.clone();
        Box::pin(async move { Ok(snapshot) })
    })
}

fn row_state(model: &TunnelPageData) -> TunnelState {
    model.model().rules[0].status.state
}

#[test]
fn real_manager_statuses_and_profiles_fill_rows_without_starting_anything() {
    let manager = TunnelManager::default();
    let rules = vec![
        rule("alpha", "prod", "Database"),
        rule("beta", "missing", "Removed host"),
        rule("gamma", "unnamed", "Unnamed host"),
    ];
    let saved = HashMap::from([(
        "alpha".to_string(),
        TunnelTraffic {
            bytes_up: 12,
            bytes_down: 34,
            connections: 5,
            since: Some(1_000),
        },
    )]);
    let statuses = manager.statuses_for(&rules, &saved);

    assert_eq!(statuses.len(), rules.len());
    assert!(
        statuses
            .iter()
            .all(|status| status.state == TunnelState::Stopped),
        "a manager that has run nothing reports every rule as stopped"
    );
    assert_eq!(statuses[0].lifetime, saved["alpha"]);
    let model = build_model(snapshot(
        rules.clone(),
        statuses.clone(),
        vec![
            profile("prod", "Production", "deploy", "example.com"),
            profile("unnamed", "", "root", "unnamed.example"),
        ],
    ));

    assert_eq!(model.state, TunnelPageState::Ready);
    assert_eq!(model.running_count, 0);
    assert_eq!(model.rules[0].host_label, "Production");
    assert!(!model.rules[0].host_missing);
    assert!(model.rules[1].host_missing);
    assert!(model.rules[1].host_label.is_empty());
    assert_eq!(model.rules[2].host_label, "root@unnamed.example");
    assert_eq!(model.rules[2].status.lifetime, TunnelTraffic::default());
    assert_eq!(
        manager.statuses_for(&rules, &saved),
        statuses,
        "reading the real manager snapshot must not change it"
    );
}

#[gpui_kit::test]
fn late_results_and_status_events_respect_visibility_and_generation(cx: &mut TestAppContext) {
    let data = cx.update(|cx| {
        cx.new(|cx| {
            let mut data = TunnelPageData::with_loader(
                ready_loader(stopped_snapshot()),
                Duration::from_secs(1),
            );
            data.model = build_model(stopped_snapshot());
            data.visible = true;
            data.generation = 7;
            cx.notify();
            data
        })
    });

    let stale = cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.finish_load(6, Ok(running_snapshot()), cx)
        })
    });
    assert_eq!(stale, (false, false));
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Stopped
    );

    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.apply_status(status("alpha", TunnelState::Running), cx);
        });
    });
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running
    );
    assert_eq!(cx.update(|cx| data.read(cx).model().running_count), 1);

    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.close(cx);
            data.apply_status(status("alpha", TunnelState::Stopped), cx);
        });
    });
    assert!(!cx.update(|cx| data.read(cx).is_visible()));
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running,
        "a closed page keeps its last snapshot and ignores forwarded events"
    );
    let late = cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.finish_load(7, Ok(stopped_snapshot()), cx)
        })
    });
    assert_eq!(late, (false, false));
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running
    );
}

#[gpui_kit::test]
fn a_status_forwarded_while_loading_wins_over_the_older_snapshot(cx: &mut TestAppContext) {
    let data = cx.update(|cx| {
        cx.new(|_| {
            let mut data = TunnelPageData::with_loader(
                ready_loader(stopped_snapshot()),
                Duration::from_secs(1),
            );
            data.visible = true;
            data.generation = 3;
            data.in_flight = true;
            data
        })
    });
    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.apply_status(status("alpha", TunnelState::Running), cx);
        });
    });
    assert_eq!(
        cx.update(|cx| data.read(cx).model().state.clone()),
        TunnelPageState::Loading,
        "the forwarded event is held until a snapshot exists"
    );

    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.finish_load(3, Ok(stopped_snapshot()), cx);
        });
    });
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running
    );
    assert_eq!(cx.update(|cx| data.read(cx).model().running_count), 1);
}

#[gpui_kit::test]
fn a_status_event_during_a_ready_poll_is_not_overwritten_by_its_stale_result(
    cx: &mut TestAppContext,
) {
    let data = cx.update(|cx| {
        cx.new(|_| {
            let mut data = TunnelPageData::with_loader(
                ready_loader(stopped_snapshot()),
                Duration::from_secs(1),
            );
            data.model = build_model(stopped_snapshot());
            data.visible = true;
            data.generation = 4;
            data.in_flight = true;
            data
        })
    });
    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.apply_status(status("alpha", TunnelState::Running), cx);
        });
    });
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running,
        "the event updates the visible row immediately"
    );

    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.finish_load(4, Ok(stopped_snapshot()), cx);
        });
    });
    assert_eq!(
        cx.update(|cx| row_state(data.read(cx))),
        TunnelState::Running,
        "the poll that began before the event must not restore the old state"
    );
    assert_eq!(cx.update(|cx| data.read(cx).model().running_count), 1);
}

#[derive(Default)]
struct Gate {
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    release: tokio::sync::Notify,
}

struct ActiveGuard(Arc<Gate>);

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}

fn gated_loader(gate: Arc<Gate>, snapshot: TunnelPageSnapshot) -> Loader {
    Arc::new(move || {
        let gate = Arc::clone(&gate);
        let call = gate.calls.fetch_add(1, Ordering::SeqCst);
        let snapshot = snapshot.clone();
        Box::pin(async move {
            let active = gate.active.fetch_add(1, Ordering::SeqCst) + 1;
            gate.max_active.fetch_max(active, Ordering::SeqCst);
            let _active = ActiveGuard(Arc::clone(&gate));
            if call == 0 {
                gate.release.notified().await;
            }
            Ok(snapshot)
        })
    })
}

fn pump_until(cx: &mut TestAppContext, message: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "{message}");
        // Timers in the test app run on a simulated clock, so real sleeping
        // never lets the poll interval elapse.
        cx.executor().advance_clock(Duration::from_millis(10));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn a_visible_reload_is_coalesced_into_one_follow_up_load(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let gate = Arc::new(Gate::default());
    let loader = gated_loader(Arc::clone(&gate), stopped_snapshot());
    let data =
        cx.update(|cx| cx.new(|_| TunnelPageData::with_loader(loader, Duration::from_secs(30))));
    cx.update(|cx| data.update(cx, |data, cx| data.open(cx)));
    pump_until(cx, "the first load should start", || {
        gate.calls.load(Ordering::SeqCst) == 1
    });

    cx.update(|cx| {
        data.update(cx, |data, cx| {
            data.refresh(cx);
            data.refresh(cx);
        });
    });
    std::thread::sleep(Duration::from_millis(10));
    cx.run_until_parked();
    assert_eq!(
        gate.calls.load(Ordering::SeqCst),
        1,
        "a reload must not overlap the request already in flight"
    );

    gate.release.notify_waiters();
    pump_until(cx, "the coalesced reload should start", || {
        gate.calls.load(Ordering::SeqCst) == 2
    });
    std::thread::sleep(Duration::from_millis(20));
    cx.run_until_parked();
    assert_eq!(
        gate.calls.load(Ordering::SeqCst),
        2,
        "repeated refreshes collapse to one follow-up"
    );
    assert_eq!(gate.max_active.load(Ordering::SeqCst), 1);
}

#[gpui_kit::test]
fn closing_stops_polling_and_reopening_does_not_accumulate_tasks(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let gate = Arc::new(Gate::default());
    let loader = gated_loader(Arc::clone(&gate), stopped_snapshot());
    let data =
        cx.update(|cx| cx.new(|_| TunnelPageData::with_loader(loader, Duration::from_millis(5))));
    cx.update(|cx| data.update(cx, |data, cx| data.open(cx)));
    pump_until(cx, "the first load should start", || {
        gate.calls.load(Ordering::SeqCst) == 1
    });
    gate.release.notify_waiters();
    pump_until(cx, "the visible page should keep polling", || {
        gate.calls.load(Ordering::SeqCst) >= 3
    });
    assert_eq!(gate.max_active.load(Ordering::SeqCst), 1);

    cx.update(|cx| data.update(cx, |data, cx| data.close(cx)));
    let closed_at = gate.calls.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(30));
    cx.run_until_parked();
    assert_eq!(
        gate.calls.load(Ordering::SeqCst),
        closed_at,
        "closing the page must stop its poll"
    );

    cx.update(|cx| data.update(cx, |data, cx| data.open(cx)));
    pump_until(cx, "reopening should load again", || {
        gate.calls.load(Ordering::SeqCst) > closed_at
    });
    cx.update(|cx| data.update(cx, |data, cx| data.close(cx)));
    let reopened_closed_at = gate.calls.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(30));
    cx.run_until_parked();
    assert_eq!(
        gate.calls.load(Ordering::SeqCst),
        reopened_closed_at,
        "a closed reopened page leaves no second poll running"
    );
}
