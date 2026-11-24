//! P2-12: each terminal query has one responder over a real SSH channel.
//!
//! The loopback shell waits for ordinary client input before it emits its next
//! query. Each reply is therefore fenced by the same client's own input, so
//! the test does not depend on a quiet period or on two independent reply
//! paths racing each other.

use std::time::{Duration, Instant};

use gpui_kit::TestAppContext;
use gpui_kit::test::TestWindowExt as _;
use tterm_terminal::grid::WindowSize;

use super::server::{ShellScript, ShellServer};
use super::{direct_launch, harness, output, pump_until, trust_server};

const DA1_REPLY: &[u8] = b"\x1b[?64;1;2;4;6;9;15;21;22c";
const STATUS_REPLY: &[u8] = b"\x1b[0n";
const CPR_REPLY: &[u8] = b"\x1b[4;7R";
const START_GATE: &[u8] = b"P2-12-START";
const STATUS_GATE: &[u8] = b"P2-12-STATUS";
const WINDOW_GATE: &[u8] = b"P2-12-WINDOW";
const CPR_GATE: &[u8] = b"P2-12-CPR";
const MIXED_CPR_OSR_GATE: &[u8] = b"P2-12-MIXED-CPR-OSR";
const MIXED_OSR_CPR_GATE: &[u8] = b"P2-12-MIXED-OSR-CPR";
const DONE_GATE: &[u8] = b"P2-12-DONE";
const END_MARKER: &str = "P2-12-END";
const DONE_ACK: &str = "P2-12-DONE-ACK";
const CPR_OSR_REPLY: &[u8] = b"\x1b[4;7R\x1b[0n";
const OSR_CPR_REPLY: &[u8] = b"\x1b[0n\x1b[4;7R";
const REPLY_DEADLINE: Duration = Duration::from_secs(20);
const STABLE: Duration = Duration::from_millis(100);

/// Waits for the exact transcript and fails as soon as it stops being a prefix.
fn expect_transcript(
    visual: &mut gpui_kit::VisualTestContext,
    server: &ShellServer,
    expected: &[u8],
    step: &str,
) {
    let deadline = Instant::now() + REPLY_DEADLINE;
    loop {
        visual.run_until_parked();
        let actual = server.inputs();
        assert!(
            expected.starts_with(&actual),
            "{step}: transcript diverged after {} bytes; expected {:?}, observed {:?}",
            expected.len(),
            String::from_utf8_lossy(expected),
            String::from_utf8_lossy(&actual)
        );
        if actual == expected {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "{step}: expected {:?}, observed {:?}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(&actual)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Builds the reply Alacritty encodes for `CSI 14 t` from the live model size.
fn text_area_reply(size: WindowSize) -> Vec<u8> {
    let height = size.grid.screen_lines * usize::from(size.cell.height);
    let width = size.grid.columns * usize::from(size.cell.width);
    format!("\x1b[4;{height};{width}t").into_bytes()
}

#[gpui_kit::test]
fn core_and_model_queries_are_answered_once_and_in_order(cx: &mut TestAppContext) {
    let harness = harness(cx);
    let script = ShellScript::new(vec![b"SAVED_SSH_READY\r\n".to_vec()])
        .after_input(START_GATE, vec![b"\x1b[".to_vec(), b"c".to_vec()])
        .after_input(STATUS_GATE, vec![b"\x1b[5n".to_vec()])
        .after_input(WINDOW_GATE, vec![b"\x1b[14t".to_vec()])
        .after_input(
            CPR_GATE,
            vec![
                b"\x1b[4;7H\x1b[6n".to_vec(),
                format!("\r\n{END_MARKER}\r\n").into_bytes(),
            ],
        )
        .after_input(
            MIXED_CPR_OSR_GATE,
            vec![b"\x1b[4;7H\x1b[6n\x1b[5n".to_vec()],
        )
        .after_input(
            MIXED_OSR_CPR_GATE,
            vec![b"\x1b[4;7H\x1b[5n\x1b[6n".to_vec()],
        )
        .after_input(DONE_GATE, vec![format!("\r\n{DONE_ACK}\r\n").into_bytes()]);
    let server = ShellServer::start_scripted(harness._runtime._runtime.handle(), script);
    let name = "Query Responder";
    trust_server("query-responder", name, &server);
    let root = harness.root.clone();
    let launch = direct_launch("query-responder", name, &server);
    let tab_id = harness.visual.update(|window, cx| {
        root.update(cx, |workspace, cx| {
            workspace
                .open_saved_profile_launch(launch, window, cx)
                .expect("the saved launch should open one SSH tab")
        })
    });
    let (session, model) = harness.visual.update(|window, cx| {
        window.render_frame(cx);
        let workspace = root.read(cx);
        let session = workspace
            .tabs
            .get(tab_id)
            .and_then(|tab| tab.binding.as_ref())
            .and_then(|binding| binding.session())
            .expect("the saved launch runs a live session");
        (session.clone(), session.model())
    });
    pump_until(harness.visual, "the SSH shell to become ready", |_| {
        output(&model).contains("SAVED_SSH_READY")
    });
    assert_eq!(server.connections(), 1, "one loopback connection");
    assert_eq!(server.shells(), 1, "one interactive shell");

    let input = session.input();
    let mut expected = Vec::new();

    input(START_GATE);
    expected.extend_from_slice(START_GATE);
    expected.extend_from_slice(DA1_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "split DA1 has exactly one core reply",
    );

    input(STATUS_GATE);
    expected.extend_from_slice(STATUS_GATE);
    expected.extend_from_slice(STATUS_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "status has exactly one core reply",
    );

    let window_reply = text_area_reply(model.snapshot().window_size());
    input(WINDOW_GATE);
    expected.extend_from_slice(WINDOW_GATE);
    expected.extend_from_slice(&window_reply);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "window size has exactly one model reply",
    );

    input(CPR_GATE);
    expected.extend_from_slice(CPR_GATE);
    expected.extend_from_slice(CPR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "a non-home CPR has exactly one model reply",
    );
    pump_until(harness.visual, "the parser to apply the end marker", |_| {
        output(&model).contains(END_MARKER)
    });

    // Packet order is the contract, not responder priority. The reverse case
    // fails if core replies are merely delayed behind every model reply.
    input(MIXED_CPR_OSR_GATE);
    expected.extend_from_slice(MIXED_CPR_OSR_GATE);
    expected.extend_from_slice(CPR_OSR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "CPR then OSR in one packet keeps request order",
    );

    input(MIXED_OSR_CPR_GATE);
    expected.extend_from_slice(MIXED_OSR_CPR_GATE);
    expected.extend_from_slice(OSR_CPR_REPLY);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "OSR then CPR in one packet keeps request order",
    );

    input(DONE_GATE);
    expected.extend_from_slice(DONE_GATE);
    expect_transcript(
        harness.visual,
        &server,
        &expected,
        "the final ordinary-input fence closes the transcript",
    );
    pump_until(harness.visual, "the final fence acknowledgement", |_| {
        output(&model).contains(DONE_ACK)
    });
    std::thread::sleep(STABLE);
    harness.visual.run_until_parked();
    assert_eq!(
        server.inputs(),
        expected,
        "a reply may not be duplicated after the acknowledged fence"
    );
    assert_eq!(
        server.script_remaining(),
        0,
        "every scripted query should have been requested"
    );
    harness.visual.update(|window, _| window.remove_window());
}
