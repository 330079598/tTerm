use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;

use super::{
    AttentionDecision, CommandTimer, FinishedCommand, KittyNotificationAssembler,
    LocalShellMarkConfig, NOTIFICATION_OSC_CODES, NotificationSettings, TabAttentionLevel,
    TerminalAttentionEvent, TerminalNotificationRequest, decide_attention, format_command_duration,
    local_shells_mark_commands, parse_osc9_notification, parse_osc777_notification,
    stronger_attention,
};
use crate::agent_status::{AgentAnnouncement, AgentAnnouncementState};
use tterm_terminal::command_mark::CommandMark;

fn encode(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut encoded = String::new();
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (first << 16) | (second << 8) | third;
        encoded.push(ALPHABET[((value >> 18) & 0x3f) as usize] as char);
        encoded.push(ALPHABET[((value >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            encoded.push(ALPHABET[((value >> 6) & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
        if chunk.len() > 2 {
            encoded.push(ALPHABET[(value & 0x3f) as usize] as char);
        } else {
            encoded.push('=');
        }
    }
    encoded
}

fn settings() -> NotificationSettings {
    NotificationSettings {
        notifications_enabled: true,
        notify_command_finished: true,
        notify_command_min_secs: 10.0,
        notify_terminal_requests: true,
        notify_agent_waiting: true,
        notify_agent_done: true,
        bell_notify: false,
    }
}

#[test]
fn names_the_osc_codes_that_carry_notifications() {
    assert_eq!(NOTIFICATION_OSC_CODES, [9, 777, 99]);
}

#[test]
fn reads_a_message() {
    assert_eq!(
        parse_osc9_notification("Claude is waiting for your input"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "Claude is waiting for your input".to_owned(),
        })
    );
}

#[test]
fn leaves_conemus_numbered_commands_alone() {
    assert_eq!(parse_osc9_notification("9;C:\\Users"), None);
    assert_eq!(parse_osc9_notification("4;1;50"), None);
    assert_eq!(parse_osc9_notification("4"), None);
    assert_eq!(parse_osc9_notification("9"), None);
    assert_eq!(
        parse_osc9_notification("9\n"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "9".to_owned(),
        })
    );
    assert_eq!(
        parse_osc9_notification("9\r\n"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "9".to_owned(),
        })
    );
}

#[test]
fn drops_control_characters_and_empty_messages() {
    assert_eq!(
        parse_osc9_notification("a\x07b\x1b"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "a b".to_owned(),
        })
    );
    assert_eq!(parse_osc9_notification("  "), None);
}

#[test]
fn shortens_long_messages() {
    let body = parse_osc9_notification(&"x".repeat(5000))
        .map(|request| request.body)
        .unwrap_or_default();
    assert_eq!(body.chars().count(), 1024);
    assert!(body.ends_with('…'));
}

#[test]
fn truncates_unicode_text_by_utf16_units() {
    let request = parse_osc9_notification(&"中".repeat(1100)).unwrap_or_else(|| {
        TerminalNotificationRequest {
            title: None,
            body: String::new(),
        }
    });
    assert_eq!(tterm_terminal::js_text::utf16_len(&request.body), 1024);
    assert!(request.body.ends_with('…'));
}

#[test]
fn truncates_emoji_and_combining_text_by_utf16_units() {
    let emoji = parse_osc9_notification(&"😀".repeat(600)).expect("an emoji message");
    assert_eq!(tterm_terminal::js_text::utf16_len(&emoji.body), 1024);
    assert_eq!(emoji.body.chars().count(), 513);
    assert!(emoji.body.starts_with(&"😀".repeat(511)));
    assert!(emoji.body.ends_with('…'));

    let combining = parse_osc9_notification(&"e\u{301}".repeat(600)).expect("a combining message");
    assert_eq!(combining.body, format!("{}e…", "e\u{301}".repeat(511)));
}

#[test]
fn reads_title_and_body_keeping_semicolons_in_the_body() {
    assert_eq!(
        parse_osc777_notification("notify;Build;done; 3 warnings"),
        Some(TerminalNotificationRequest {
            title: Some("Build".to_owned()),
            body: "done; 3 warnings".to_owned(),
        })
    );
}

#[test]
fn shows_a_lone_title_as_the_message() {
    assert_eq!(
        parse_osc777_notification("notify;Build;"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "Build".to_owned(),
        })
    );
}

#[test]
fn ignores_other_commands() {
    assert_eq!(parse_osc777_notification("precmd"), None);
}

#[test]
fn shows_a_one_part_notification_as_its_title() {
    assert_eq!(
        KittyNotificationAssembler::new().handle(";Hello"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "Hello".to_owned(),
        })
    );
}

#[test]
fn joins_chunks_with_the_same_id() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(kitty.handle("i=1:d=0;Build"), None);
    assert_eq!(
        kitty.handle("i=1:p=body;finished"),
        Some(TerminalNotificationRequest {
            title: Some("Build".to_owned()),
            body: "finished".to_owned(),
        })
    );
}

#[test]
fn keeps_titles_and_bodies_across_finished_and_pending_chunks() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(
        kitty.handle("i=1:p=title;one"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "one".to_owned(),
        })
    );
    assert_eq!(kitty.handle("i=1:d=0:p=body;two"), None);
    assert_eq!(
        kitty.handle("i=1:p=body;three"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "twothree".to_owned(),
        })
    );
}

#[test]
fn evicts_the_oldest_of_sixteen_pending_ids() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(kitty.handle("i=0:d=0:p=title;zero"), None);
    for index in 1..=16 {
        assert_eq!(
            kitty.handle(&format!("i={index}:d=0:p=title;t{index}")),
            None
        );
    }
    assert_eq!(
        kitty.handle("i=0:p=body;late"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "late".to_owned(),
        })
    );
}

#[test]
fn duplicate_metadata_keys_use_the_last_value() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(
        kitty.handle("i=1:p=title:x:i=1:p=body;ignored"),
        Some(TerminalNotificationRequest {
            title: None,
            body: "ignored".to_owned(),
        })
    );
}

#[test]
fn an_unknown_part_does_not_change_a_pending_partial() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(kitty.handle("i=1:d=0:p=title;one"), None);
    assert_eq!(kitty.handle("i=1:p=icon;two"), None);
    assert_eq!(
        kitty.handle("i=1:p=body;three"),
        Some(TerminalNotificationRequest {
            title: Some("one".to_owned()),
            body: "three".to_owned(),
        })
    );
}

#[test]
fn decodes_base64_payloads() {
    let encoded = encode("完成");
    assert_eq!(
        KittyNotificationAssembler::new().handle(&format!("e=1;{encoded}")),
        Some(TerminalNotificationRequest {
            title: None,
            body: "完成".to_owned(),
        })
    );
}

#[test]
fn ignores_queries_and_other_payload_types() {
    let mut kitty = KittyNotificationAssembler::new();
    assert_eq!(kitty.handle("i=1:p=?;"), None);
    assert_eq!(kitty.handle("p=close;"), None);
    assert_eq!(kitty.handle("no separator"), None);
}

#[test]
fn times_a_command_from_its_output_mark_to_its_done_mark() {
    let mut timer = CommandTimer::new();
    timer.prompt();
    timer.set_command("make");
    timer.start(1000);
    assert_eq!(
        timer.finish(Some(0), 13_000),
        Some(FinishedCommand {
            command: Some("make".to_owned()),
            exit_code: Some(0),
            duration_ms: 12_000,
        })
    );
}

#[test]
fn keeps_the_earliest_start() {
    let mut timer = CommandTimer::new();
    timer.start(1000);
    timer.start(1500);
    assert_eq!(
        timer
            .finish(Some(1), 2000)
            .map(|finished| finished.duration_ms),
        Some(1000)
    );
}

#[test]
fn reports_nothing_for_a_done_mark_without_a_started_command() {
    let mut timer = CommandTimer::new();
    assert_eq!(timer.finish(Some(0), 1000), None);
    timer.start(1000);
    timer.prompt();
    assert_eq!(timer.finish(Some(0), 2000), None);
}

#[test]
fn forgets_the_command_once_it_finishes() {
    let mut timer = CommandTimer::new();
    timer.set_command("make");
    timer.start(0);
    timer.finish(Some(0), 10);
    timer.start(20);
    assert_eq!(
        timer
            .finish(Some(0), 30)
            .and_then(|finished| finished.command),
        None
    );
}

#[test]
fn reports_the_first_command_started_after_the_prompt_until_the_prompt_is_back() {
    let running = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
    let recorder = running.clone();
    let mut timer = CommandTimer::with_on_running(Box::new(move |command| {
        recorder.borrow_mut().push(command.map(str::to_owned));
    }));
    timer.set_typed_command("ls");
    timer.mark(CommandMark::Prompt, 0);
    timer.set_typed_command("python");
    timer.start(10);
    timer.mark(CommandMark::Output, 20);
    assert_eq!(
        timer.mark(CommandMark::Done { exit_code: Some(0) }, 30),
        Some(FinishedCommand {
            command: Some("python".to_owned()),
            exit_code: Some(0),
            duration_ms: 20,
        })
    );
    timer.mark(CommandMark::Prompt, 40);
    assert_eq!(*running.borrow(), vec![Some("python".to_owned()), None]);
}

#[test]
fn is_not_named_by_lines_typed_into_the_command_it_runs() {
    let running = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
    let recorder = running.clone();
    let mut timer = CommandTimer::with_on_running(Box::new(move |command| {
        recorder.borrow_mut().push(command.map(str::to_owned));
    }));
    timer.mark(CommandMark::Prompt, 0);
    timer.start(10);
    timer.set_typed_command("fix the login bug");
    assert!(running.borrow().is_empty());
    assert_eq!(
        timer
            .finish(Some(0), 30)
            .and_then(|finished| finished.command),
        None
    );
}

#[test]
fn takes_the_command_line_the_shell_reports_over_what_was_typed() {
    let running = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
    let recorder = running.clone();
    let mut timer = CommandTimer::with_on_running(Box::new(move |command| {
        recorder.borrow_mut().push(command.map(str::to_owned));
    }));
    timer.mark(CommandMark::Prompt, 0);
    timer.set_typed_command("cl");
    timer.start(10);
    timer.set_command("claude --resume");
    timer.set_command("claude --resume");
    assert_eq!(
        *running.borrow(),
        vec![Some("cl".to_owned()), Some("claude --resume".to_owned())]
    );
    assert_eq!(
        timer
            .finish(Some(0), 30)
            .and_then(|finished| finished.command),
        Some("claude --resume".to_owned())
    );
}

#[test]
fn announces_a_pane_in_sight_while_the_user_works_in_another() {
    let beside_focus = (true, true, false);
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Request {
                title: None,
                body: "hi".to_owned(),
            },
            &settings(),
            beside_focus.0,
            beside_focus.1,
            beside_focus.2,
        ),
        super::AttentionDecision {
            mark: None,
            system: false,
            toast: true,
        }
    );
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Bell,
            &settings(),
            beside_focus.0,
            beside_focus.1,
            beside_focus.2,
        ),
        super::AttentionDecision {
            mark: None,
            system: false,
            toast: false,
        }
    );
}

#[test]
fn leaves_a_user_looking_at_the_tab_alone() {
    let long_command = TerminalAttentionEvent::Command {
        command: None,
        exit_code: Some(0),
        duration_ms: 30_000.0,
    };
    for event in [
        long_command,
        TerminalAttentionEvent::Request {
            title: None,
            body: "hi".to_owned(),
        },
        TerminalAttentionEvent::Bell,
    ] {
        assert_eq!(
            decide_attention(&event, &settings(), true, true, true),
            super::AttentionDecision {
                mark: None,
                system: false,
                toast: false,
            }
        );
    }
}

#[test]
fn notifies_the_system_while_the_window_is_in_the_background() {
    let long_command = TerminalAttentionEvent::Command {
        command: None,
        exit_code: Some(0),
        duration_ms: 30_000.0,
    };
    assert_eq!(
        decide_attention(&long_command, &settings(), false, true, true),
        super::AttentionDecision {
            mark: None,
            system: true,
            toast: false,
        }
    );
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Request {
                title: None,
                body: "hi".to_owned(),
            },
            &settings(),
            false,
            false,
            false,
        ),
        super::AttentionDecision {
            mark: Some(TabAttentionLevel::Notification),
            system: true,
            toast: false,
        }
    );
}

#[test]
fn announces_agents_by_their_own_switches() {
    let hidden = (false, false, false);
    let waiting = TerminalAttentionEvent::Agent(AgentAnnouncement {
        agent: "claude".to_owned(),
        state: AgentAnnouncementState::Waiting,
    });
    let done = TerminalAttentionEvent::Agent(AgentAnnouncement {
        agent: "claude".to_owned(),
        state: AgentAnnouncementState::Done,
    });
    let error = TerminalAttentionEvent::Agent(AgentAnnouncement {
        agent: "claude".to_owned(),
        state: AgentAnnouncementState::Error,
    });
    assert_eq!(
        decide_attention(&waiting, &settings(), hidden.0, hidden.1, hidden.2),
        super::AttentionDecision {
            mark: Some(TabAttentionLevel::Notification),
            system: true,
            toast: false,
        }
    );
    let no_waiting = NotificationSettings {
        notify_agent_waiting: false,
        ..settings()
    };
    assert_eq!(
        decide_attention(&waiting, &no_waiting, hidden.0, hidden.1, hidden.2).mark,
        None
    );
    assert!(decide_attention(&done, &no_waiting, hidden.0, hidden.1, hidden.2).system);
    let no_done = NotificationSettings {
        notify_agent_done: false,
        ..settings()
    };
    assert_eq!(
        decide_attention(&done, &no_done, hidden.0, hidden.1, hidden.2).mark,
        None
    );
    assert_eq!(
        decide_attention(&error, &no_done, hidden.0, hidden.1, hidden.2).mark,
        None
    );
    assert!(decide_attention(&waiting, &no_done, hidden.0, hidden.1, hidden.2).system);
}

#[test]
fn toasts_and_marks_a_hidden_tab_in_a_focused_window() {
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Request {
                title: None,
                body: "hi".to_owned(),
            },
            &settings(),
            true,
            false,
            false,
        ),
        super::AttentionDecision {
            mark: Some(TabAttentionLevel::Notification),
            system: false,
            toast: true,
        }
    );
}

#[test]
fn ignores_short_commands_and_turned_off_events() {
    let hidden = (false, false, false);
    let short = TerminalAttentionEvent::Command {
        command: None,
        exit_code: Some(0),
        duration_ms: 9_999.0,
    };
    assert_eq!(
        decide_attention(&short, &settings(), hidden.0, hidden.1, hidden.2).mark,
        None
    );
    let no_finished = NotificationSettings {
        notify_command_finished: false,
        ..settings()
    };
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Command {
                command: None,
                exit_code: Some(0),
                duration_ms: 30_000.0,
            },
            &no_finished,
            hidden.0,
            hidden.1,
            hidden.2,
        )
        .mark,
        None
    );
    let no_requests = NotificationSettings {
        notify_terminal_requests: false,
        ..settings()
    };
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Request {
                title: None,
                body: "hi".to_owned(),
            },
            &no_requests,
            hidden.0,
            hidden.1,
            hidden.2,
        )
        .mark,
        None
    );
}

#[test]
fn only_marks_tabs_when_announcements_are_off() {
    let no_announcements = NotificationSettings {
        notifications_enabled: false,
        ..settings()
    };
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Request {
                title: None,
                body: "hi".to_owned(),
            },
            &no_announcements,
            false,
            false,
            false,
        ),
        super::AttentionDecision {
            mark: Some(TabAttentionLevel::Notification),
            system: false,
            toast: false,
        }
    );
}

#[test]
fn marks_a_bell_and_announces_it_only_when_asked_to_never_as_a_toast() {
    assert_eq!(
        decide_attention(
            &TerminalAttentionEvent::Bell,
            &settings(),
            false,
            false,
            false
        ),
        super::AttentionDecision {
            mark: Some(TabAttentionLevel::Bell),
            system: false,
            toast: false,
        }
    );
    let with_bell = NotificationSettings {
        bell_notify: true,
        ..settings()
    };
    assert!(
        decide_attention(
            &TerminalAttentionEvent::Bell,
            &with_bell,
            false,
            false,
            false
        )
        .system
    );
    assert!(
        !decide_attention(
            &TerminalAttentionEvent::Bell,
            &with_bell,
            true,
            false,
            false
        )
        .toast
    );
}

#[test]
fn keeps_the_strongest_mark() {
    assert_eq!(
        stronger_attention(None, TabAttentionLevel::Activity),
        TabAttentionLevel::Activity
    );
    assert_eq!(
        stronger_attention(Some(TabAttentionLevel::Activity), TabAttentionLevel::Bell),
        TabAttentionLevel::Bell
    );
    assert_eq!(
        stronger_attention(
            Some(TabAttentionLevel::Notification),
            TabAttentionLevel::Activity
        ),
        TabAttentionLevel::Notification
    );
    assert_eq!(
        stronger_attention(
            Some(TabAttentionLevel::Bell),
            TabAttentionLevel::Notification
        ),
        TabAttentionLevel::Notification
    );
}

#[test]
fn formats_seconds_minutes_and_hours() {
    assert_eq!(format_command_duration(42_400.0), "42s");
    assert_eq!(format_command_duration(42_500.0), "43s");
    assert_eq!(format_command_duration(499.0), "0s");
    assert_eq!(format_command_duration(500.0), "1s");
    assert_eq!(format_command_duration(59_999.0), "1m 0s");
    assert_eq!(format_command_duration(60_500.0), "1m 1s");
    assert_eq!(format_command_duration(185_000.0), "3m 5s");
    assert_eq!(format_command_duration(3_599_500.0), "1h 0m");
    assert_eq!(format_command_duration(3_720_000.0), "1h 2m");
    assert_eq!(format_command_duration(-500.0), "0s");
    assert_eq!(format_command_duration(-1_500.0), "-1s");
    assert_eq!(format_command_duration(f64::NAN), "NaNs");
    assert_eq!(format_command_duration(f64::INFINITY), "Infinityh NaNm");
    assert_eq!(format_command_duration(f64::NEG_INFINITY), "NaNs");
}

#[test]
fn rounds_like_javascript_math_round() {
    assert_eq!(format_command_duration(499.99999999999994), "0s");
}

#[test]
fn needs_command_marks_everywhere_and_shell_integration_outside_windows() {
    let on = LocalShellMarkConfig {
        command_marks: true,
        local_unix_shell_integration: true,
    };
    assert!(local_shells_mark_commands(&on, "macos"));
    assert!(!local_shells_mark_commands(
        &LocalShellMarkConfig {
            local_unix_shell_integration: false,
            ..on
        },
        "linux"
    ));
    assert!(local_shells_mark_commands(
        &LocalShellMarkConfig {
            local_unix_shell_integration: false,
            ..on
        },
        "windows"
    ));
    assert!(!local_shells_mark_commands(
        &LocalShellMarkConfig {
            command_marks: false,
            ..on
        },
        "windows"
    ));
}

#[test]
fn serializes_the_shapes_the_frontend_uses() {
    assert_eq!(
        serde_json::to_value(TerminalNotificationRequest {
            title: None,
            body: "hi".to_owned(),
        })
        .unwrap_or_default(),
        json!({"body": "hi"})
    );
    assert_eq!(
        serde_json::to_value(TerminalAttentionEvent::Command {
            command: None,
            exit_code: Some(0),
            duration_ms: 30_000.0,
        })
        .unwrap_or_default(),
        json!({"kind": "command", "exitCode": 0, "durationMs": 30000.0})
    );
    assert_eq!(
        serde_json::to_value(TerminalAttentionEvent::Agent(AgentAnnouncement {
            agent: "claude".to_owned(),
            state: AgentAnnouncementState::Waiting,
        }))
        .unwrap_or_default(),
        json!({"kind": "agent", "agent": "claude", "state": "waiting"})
    );
    assert_eq!(
        serde_json::to_value(FinishedCommand {
            command: Some("make".to_owned()),
            exit_code: None,
            duration_ms: 12_000,
        })
        .unwrap_or_default(),
        json!({"command": "make", "durationMs": 12000})
    );
    assert_eq!(
        serde_json::to_value(AttentionDecision {
            mark: Some(TabAttentionLevel::Bell),
            system: false,
            toast: false,
        })
        .unwrap_or_default(),
        json!({"mark": "bell", "system": false, "toast": false})
    );
    assert_eq!(
        serde_json::to_value(NotificationSettings {
            notifications_enabled: true,
            notify_command_finished: true,
            notify_command_min_secs: 10.0,
            notify_terminal_requests: true,
            notify_agent_waiting: true,
            notify_agent_done: true,
            bell_notify: false,
        })
        .unwrap_or_default(),
        json!({
            "notifications_enabled": true,
            "notify_command_finished": true,
            "notify_command_min_secs": 10.0,
            "notify_terminal_requests": true,
            "notify_agent_waiting": true,
            "notify_agent_done": true,
            "bell_notify": false
        })
    );
}

#[test]
fn round_trips_attention_events_and_requests_through_json() {
    let events = [
        TerminalAttentionEvent::Command {
            command: Some("make".to_owned()),
            exit_code: Some(0),
            duration_ms: 12_000.0,
        },
        TerminalAttentionEvent::Request {
            title: Some("Build".to_owned()),
            body: "done".to_owned(),
        },
        TerminalAttentionEvent::Agent(AgentAnnouncement {
            agent: "claude".to_owned(),
            state: AgentAnnouncementState::Waiting,
        }),
        TerminalAttentionEvent::Bell,
    ];
    for event in events {
        let json = serde_json::to_string(&event).expect("serialize an event");
        assert_eq!(
            serde_json::from_str::<TerminalAttentionEvent>(&json).expect("deserialize an event"),
            event
        );
    }
    let request = TerminalNotificationRequest {
        title: Some("Build".to_owned()),
        body: "done".to_owned(),
    };
    let json = serde_json::to_string(&request).expect("serialize a request");
    assert_eq!(
        serde_json::from_str::<TerminalNotificationRequest>(&json).expect("deserialize a request"),
        request
    );
}
