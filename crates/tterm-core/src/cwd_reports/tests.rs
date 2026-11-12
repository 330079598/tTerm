use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{CwdReportOptions, CwdReports};

struct Harness {
    reports: CwdReports,
    cwds: Rc<RefCell<Vec<String>>>,
    remote_cwds: Rc<RefCell<Vec<Option<String>>>>,
    away_hosts: Rc<RefCell<Vec<Option<String>>>>,
    remote_reports: Rc<Cell<usize>>,
}

fn setup(remote: bool, hostname: Option<&str>) -> Harness {
    let is_remote = Rc::new(Cell::new(remote));
    let hostname = Rc::new(RefCell::new(hostname.map(str::to_string)));
    let cwds = Rc::new(RefCell::new(Vec::new()));
    let remote_cwds = Rc::new(RefCell::new(Vec::new()));
    let away_hosts = Rc::new(RefCell::new(Vec::new()));
    let remote_reports = Rc::new(Cell::new(0));

    let remote_flag = Rc::clone(&is_remote);
    let hostname_read = Rc::clone(&hostname);
    let cwd_sink = Rc::clone(&cwds);
    let remote_cwd_sink = Rc::clone(&remote_cwds);
    let away_sink = Rc::clone(&away_hosts);
    let report_count = Rc::clone(&remote_reports);
    let reports = CwdReports::new(CwdReportOptions {
        tab_id: "tab".to_string(),
        session_nonce: 1,
        is_remote: Box::new(move || remote_flag.get()),
        on_cwd: Box::new(move |cwd| cwd_sink.borrow_mut().push(cwd.to_string())),
        on_remote_cwd: Box::new(move |cwd| {
            remote_cwd_sink.borrow_mut().push(cwd.map(str::to_string));
        }),
        remote_hostname: Box::new(move || hostname_read.borrow().clone()),
        on_remote_away_host: Box::new(move |host| {
            away_sink.borrow_mut().push(host.map(str::to_string));
        }),
        on_remote_report: Box::new(move || report_count.set(report_count.get() + 1)),
    });
    Harness {
        reports,
        cwds,
        remote_cwds,
        away_hosts,
        remote_reports,
    }
}

#[test]
fn sends_a_local_shells_directory_to_on_cwd_only() {
    let mut harness = setup(false, Some("web01"));
    harness.reports.handle_osc(7, "file:///home/me");
    assert_eq!(*harness.cwds.borrow(), vec!["/home/me".to_string()]);
    assert!(harness.remote_cwds.borrow().is_empty());
}

#[test]
fn sends_a_remote_shells_directory_to_on_remote_cwd_once_per_change() {
    let mut harness = setup(true, Some("web01"));
    harness.reports.handle_osc(7, "file:///var/log");
    harness.reports.handle_osc(7, "file:///var/log");
    harness.reports.handle_osc(7, "file:///srv/my%25app");
    assert!(harness.cwds.borrow().is_empty());
    assert_eq!(
        *harness.remote_cwds.borrow(),
        vec![
            Some("/var/log".to_string()),
            Some("/srv/my%app".to_string()),
        ]
    );
}

#[test]
fn ignores_osc_9_9_over_ssh() {
    let mut harness = setup(true, Some("web01"));
    harness.reports.handle_osc(9, "9;\"C:\\Users\"");
    assert!(harness.remote_cwds.borrow().is_empty());
    assert_eq!(harness.remote_reports.get(), 0);
}

#[test]
fn follows_reports_naming_the_connected_host_however_fully() {
    let mut harness = setup(true, Some("web01"));
    harness
        .reports
        .handle_osc(7, "file://WEB01.example.com/srv");
    harness.reports.handle_osc(7, "file://localhost/tmp");
    assert_eq!(
        *harness.remote_cwds.borrow(),
        vec![Some("/srv".to_string()), Some("/tmp".to_string()),]
    );
    assert!(harness.away_hosts.borrow().is_empty());
}

#[test]
fn pauses_while_another_host_reports_and_resumes_when_the_connected_one_does() {
    let mut harness = setup(true, Some("web01"));
    harness.reports.handle_osc(7, "file:///srv");
    harness.reports.handle_osc(7, "file://db01/etc");
    harness.reports.handle_osc(7, "file://db01/var");
    assert_eq!(
        *harness.remote_cwds.borrow(),
        vec![Some("/srv".to_string())]
    );
    assert_eq!(*harness.away_hosts.borrow(), vec![Some("db01".to_string())]);

    harness.reports.handle_osc(7, "file:///srv");
    assert_eq!(harness.away_hosts.borrow().last(), Some(&None));
    assert_eq!(
        *harness.remote_cwds.borrow(),
        vec![Some("/srv".to_string())]
    );
    assert_eq!(harness.remote_reports.get(), 4);
}

#[test]
fn takes_the_first_host_a_report_names_when_the_hosts_name_is_not_known() {
    let mut harness = setup(true, None);
    harness.reports.handle_osc(7, "file://web01/srv");
    harness.reports.handle_osc(7, "file://db01/etc");
    assert_eq!(
        *harness.remote_cwds.borrow(),
        vec![Some("/srv".to_string())]
    );
    assert_eq!(*harness.away_hosts.borrow(), vec![Some("db01".to_string())]);
}

#[test]
fn clears_the_remote_directory_and_host_when_the_session_goes() {
    let mut harness = setup(true, Some("web01"));
    harness.reports.handle_osc(7, "file:///var/log");
    harness.reports.handle_osc(7, "file://db01/etc");
    harness.reports.dispose();
    assert_eq!(harness.remote_cwds.borrow().last(), Some(&None));
    assert_eq!(harness.away_hosts.borrow().last(), Some(&None));
}

#[test]
fn oracle_pty_events_require_the_matching_session_nonce() {
    let mut harness = setup(false, Some("web01"));
    harness.reports.handle_pty_cwd(2, "/ignored");
    harness.reports.handle_pty_cwd(1, "/accepted");
    assert_eq!(*harness.cwds.borrow(), vec!["/accepted".to_string()]);
}
