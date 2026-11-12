use std::cell::RefCell;
use std::rc::Rc;

use crate::workspace::app_navigation::{AppNavigation, AppPage, LogsTabRequest, SessionType};

fn logs_request_from_oracle(value: &serde_json::Value) -> LogsTabRequest {
    LogsTabRequest {
        tab_id: value["tabId"]
            .as_str()
            .expect("a requested tab id")
            .to_owned(),
        session_type: match value["sessionType"].as_str().expect("a session type") {
            "ssh" => SessionType::Ssh,
            "local" => SessionType::Local,
            other => panic!("unexpected session type {other}"),
        },
        host: value["host"].as_str().map(str::to_owned),
        port: value["port"].as_u64().map(|port| port as u16),
        username: value["username"].as_str().map(str::to_owned),
    }
}

#[test]
fn opens_the_logs_page_and_hands_the_tab_over_once() {
    let mut navigation = AppNavigation::new();
    let opened = Rc::new(RefCell::new(Vec::new()));
    let opened_recorder = opened.clone();
    let notified = Rc::new(RefCell::new(0));
    let notified_recorder = notified.clone();
    let stop_handling = navigation.handle_app_page_requests(move |page| {
        opened_recorder.borrow_mut().push(page);
    });
    let stop_listening = navigation.on_logs_tab_request(move || {
        *notified_recorder.borrow_mut() += 1;
    });

    navigation.show_logs_for_tab(LogsTabRequest {
        tab_id: "tab-1".to_owned(),
        session_type: SessionType::Local,
        host: None,
        port: None,
        username: None,
    });

    assert_eq!(*opened.borrow(), vec![AppPage::TerminalLogs]);
    assert_eq!(*notified.borrow(), 1);
    assert_eq!(
        navigation.take_logs_tab_request(),
        Some(LogsTabRequest {
            tab_id: "tab-1".to_owned(),
            session_type: SessionType::Local,
            host: None,
            port: None,
            username: None,
        })
    );
    assert_eq!(navigation.take_logs_tab_request(), None);
    navigation.stop_listening_for_logs_tab_requests(stop_listening);
    navigation.stop_handling_app_page_requests(stop_handling);
}

#[test]
fn stops_handling_once_replaced() {
    let mut navigation = AppNavigation::new();
    let first = Rc::new(RefCell::new(Vec::new()));
    let first_recorder = first.clone();
    let stop_first = navigation.handle_app_page_requests(move |page| {
        first_recorder.borrow_mut().push(page);
    });
    let second = Rc::new(RefCell::new(Vec::new()));
    let second_recorder = second.clone();
    let stop_second = navigation.handle_app_page_requests(move |page| {
        second_recorder.borrow_mut().push(page);
    });
    navigation.stop_handling_app_page_requests(stop_first);
    navigation.show_logs_for_tab(LogsTabRequest {
        tab_id: "tab-2".to_owned(),
        session_type: SessionType::Local,
        host: None,
        port: None,
        username: None,
    });
    assert!(first.borrow().is_empty());
    assert_eq!(*second.borrow(), vec![AppPage::TerminalLogs]);
    navigation.stop_handling_app_page_requests(stop_second);
    navigation.take_logs_tab_request();
}

#[test]
fn oracle_keeps_the_request_when_no_page_handler_is_registered() {
    let oracle = super::oracle_json();
    let expected = logs_request_from_oracle(&oracle["appNavigation"]["noHandlerRequest"]);
    let mut navigation = AppNavigation::new();
    navigation.show_logs_for_tab(expected.clone());
    assert_eq!(navigation.take_logs_tab_request(), Some(expected));
    assert_eq!(navigation.take_logs_tab_request(), None);
}
