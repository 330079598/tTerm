use tterm_core::events::background::{BackgroundEvent, QuitBlockers};
use tterm_core::events::launch_dir::LaunchDirEvent;
use tterm_core::events::notification::{ActivatedPayload, NotificationEvent};
use tterm_core::events::serial::SerialEvent;
use tterm_core::events::session_log::{
    LogErrorPayload, LogErrorPhase, SessionLogEvent, TerminalLogStatus,
};
use tterm_core::events::ssh::{
    AuthUsedPayload, LinuxDistributionInfo, RemoteOsPayload, ShellIntegrationStatus,
    SshConnectionProgressPayload, SshEvent, UnavailableReason,
};
use tterm_core::events::terminal::{CwdPayload, TerminalEvent};
use tterm_core::events::zmodem::{
    TransferCompletePayload, TransferEvent, TransferProgressPayload, TransferStartPayload,
    ZmodemEvent,
};
use tterm_core::events::{AppEvent, EventTarget};

use crate::event_sink::TauriEventSink;
use serde_json::Value;

fn check(event: AppEvent, name: &str, target: EventTarget, key: &str) {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/p1_03.json")).unwrap();
    let (actual_name, actual_target, payload) = TauriEventSink::wire(&event).unwrap();
    assert_eq!(actual_name.as_string(), name);
    assert_eq!(actual_target, target);
    assert_eq!(payload, fixtures[key], "{name}: {key}");
}

mod background;
mod other;
mod session_log;
mod ssh;
mod transfer;
